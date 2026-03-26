mod bootstrap;
mod config;
mod grpc;
mod signals;
mod supervisor;

use std::path::Path;
use std::time::Duration;

use futures::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use syswall_app::services::audit_service::{AuditService, BufferedAuditWriter};
use syswall_domain::entities::ConnectionVerdict;
use syswall_domain::ports::{EventBus, RuleRepository};

use crate::config::SysWallConfig;
use crate::grpc::{SysWallControlService, SysWallEventService, start_grpc_server};
use crate::supervisor::Supervisor;

#[tokio::main]
async fn main() {
    // Init tracing with env filter
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "syswall=info".into()),
        )
        .init();

    info!("SysWall daemon starting...");

    // Load config from SYSWALL_CONFIG env var or default path
    let config_path = std::env::var("SYSWALL_CONFIG")
        .unwrap_or_else(|_| "config/default.toml".to_string());

    let config = match SysWallConfig::load(Path::new(&config_path)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Fatal: failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    // Bootstrap application context
    let ctx = match bootstrap::bootstrap(&config) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Fatal: bootstrap failed: {}", e);
            std::process::exit(1);
        }
    };

    // Create system whitelist if first start
    if let Err(e) = syswall_app::services::whitelist::ensure_system_whitelist(
        &ctx.rule_service,
        ctx.rule_repo.as_ref(),
    )
    .await
    {
        error!("Failed to create system whitelist: {}", e);
        // Non-fatal: continue without whitelist
    }

    // Sync nftables rules with database
    match ctx.rule_repo.list_enabled_ordered().await {
        Ok(rules) => {
            if let Err(e) = ctx.firewall.sync_all_rules(&rules).await {
                error!("Failed to sync nftables rules: {}", e);
            } else {
                info!("nftables rules synced ({} rules)", rules.len());
            }
        }
        Err(e) => error!("Failed to load rules for sync: {}", e),
    }

    // Supervisor
    let cancel = CancellationToken::new();
    let mut supervisor = Supervisor::new(cancel.clone());

    // Signal handler
    supervisor.spawn("signal-handler", {
        let cancel = cancel.clone();
        async move {
            signals::wait_for_shutdown(cancel).await;
            Ok(())
        }
    });

    // Connection monitoring pipeline
    supervisor.spawn("connection-monitor", {
        let monitor = ctx.connection_monitor.clone();
        let connection_service = ctx.connection_service.clone();
        let learning_service = ctx.learning_service.clone();
        let cancel = cancel.clone();

        async move {
            let stream = monitor
                .stream_events()
                .await
                .map_err(|e| format!("Failed to start connection monitor: {}", e))?;

            tokio::pin!(stream);

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    event = stream.next() => {
                        match event {
                            Some(Ok(connection)) => {
                                match connection_service.process_connection(connection).await {
                                    Ok(processed) => {
                                        if processed.verdict == ConnectionVerdict::PendingDecision {
                                            let _ = learning_service
                                                .handle_unknown_connection(processed.snapshot())
                                                .await;
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Connection processing error: {}", e);
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                error!("Connection monitor error: {}", e);
                                return Err(format!("Monitor stream failed: {}", e));
                            }
                            None => {
                                warn!("Connection monitor stream ended");
                                return Err("Monitor stream ended unexpectedly".to_string());
                            }
                        }
                    }
                }
            }

            Ok(())
        }
    });

    // gRPC server task
    supervisor.spawn("grpc-server", {
        let control_service = SysWallControlService::new(
            ctx.rule_service.clone(),
            ctx.learning_service.clone(),
            ctx.firewall.clone(),
            ctx.audit_service.clone(),
        );
        let event_service = SysWallEventService::new(ctx.event_bus.clone());
        let socket_path = config.daemon.socket_path.clone();
        let cancel = cancel.clone();

        async move {
            start_grpc_server(socket_path, control_service, event_service, cancel).await
        }
    });

    // Periodic decision expiration task
    supervisor.spawn("decision-expiry", {
        let learning_service = ctx.learning_service.clone();
        let cancel = cancel.clone();

        async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {
                        match learning_service.expire_overdue().await {
                            Ok(expired) if !expired.is_empty() => {
                                info!("Expired {} overdue pending decisions", expired.len());
                            }
                            Err(e) => warn!("Decision expiry error: {}", e),
                            _ => {}
                        }
                    }
                }
            }
            Ok(())
        }
    });

    // Audit event listener -- subscribes to EventBus, buffers events, batch-writes
    supervisor.spawn("audit-listener", {
        let event_bus = ctx.event_bus.clone();
        let audit_service = ctx.audit_service.clone();
        let batch_size = config.database.audit_batch_size;
        let flush_interval_secs = config.database.audit_flush_interval_secs;
        let cancel = cancel.clone();

        async move {
            let mut receiver = event_bus.subscribe();
            let writer =
                BufferedAuditWriter::new(audit_service.repo().clone(), batch_size);
            let mut flush_interval =
                tokio::time::interval(Duration::from_secs(flush_interval_secs));

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        if let Err(e) = writer.flush().await {
                            warn!("Audit listener: failed to flush on shutdown: {}", e);
                        }
                        break;
                    }
                    _ = flush_interval.tick() => {
                        if let Err(e) = writer.flush().await {
                            warn!("Audit listener: periodic flush failed: {}", e);
                        }
                    }
                    result = receiver.recv() => {
                        match result {
                            Ok(event) => {
                                if let Some(audit_event) = AuditService::domain_event_to_audit(&event) {
                                    if let Err(e) = writer.buffer_event(audit_event).await {
                                        warn!("Audit listener: failed to buffer event: {}", e);
                                    }
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                warn!("Audit listener lagged, missed {} events", n);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                info!("Audit listener: event bus closed");
                                break;
                            }
                        }
                    }
                }
            }

            Ok(())
        }
    });

    // Audit cleanup -- periodically purges old events based on retention_days
    supervisor.spawn("audit-cleanup", {
        let audit_service = ctx.audit_service.clone();
        let retention_days = config.database.journal_retention_days;
        let cancel = cancel.clone();

        async move {
            let cleanup_interval = Duration::from_secs(3600); // Run hourly
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(cleanup_interval) => {
                        let cutoff = chrono::Utc::now()
                            - chrono::Duration::days(retention_days as i64);
                        match audit_service.delete_before(cutoff).await {
                            Ok(deleted) if deleted > 0 => {
                                info!(
                                    "Audit cleanup: purged {} events older than {} days",
                                    deleted, retention_days
                                );
                            }
                            Err(e) => warn!("Audit cleanup error: {}", e),
                            _ => {}
                        }
                    }
                }
            }
            Ok(())
        }
    });

    info!("SysWall daemon ready");

    // Run until shutdown
    supervisor.run().await;

    info!("SysWall daemon stopped");
}
