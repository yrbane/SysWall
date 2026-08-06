use std::path::Path;
use std::time::Duration;

use futures::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use syswall_app::services::audit_service::{AuditService, BufferedAuditWriter};
use syswall_domain::entities::ConnectionVerdict;
use syswall_domain::ports::{EventBus, RuleRepository};

use syswall_daemon::config::SysWallConfig;
use syswall_daemon::grpc::{SysWallControlService, SysWallEventService, start_grpc_server};
use syswall_daemon::startup_error::StartupError;
use syswall_daemon::supervisor::Supervisor;
use syswall_daemon::{bootstrap, signals, watchdog};

use syswall_domain::entities::AuditEvent as DomainAuditEvent;

/// Initialise le filtre de tracing avant tout autre code.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "syswall=info".into()),
        )
        .init();
}

#[tokio::main]
async fn main() {
    init_tracing();

    if let Err(e) = run().await {
        error!("{e}");
        std::process::exit(e.exit_code());
    }
}

/// Point d'entrée principal du daemon — retourne une erreur typée en cas d'échec au démarrage.
async fn run() -> Result<(), StartupError> {
    info!("SysWall daemon starting...");

    // Charge la config depuis SYSWALL_CONFIG ou le chemin par défaut
    let config_path =
        std::env::var("SYSWALL_CONFIG").unwrap_or_else(|_| "config/default.toml".to_string());

    let config = SysWallConfig::load(Path::new(&config_path))
        .map_err(|e| StartupError::ConfigInvalid(e.to_string()))?;

    // Assemble le contexte applicatif (DB, repos, services, moniteurs)
    let ctx = bootstrap::bootstrap(&config)?;

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

    // Résolution du GID du groupe 'syswall' — erreur fatale si absent
    // Resolve the 'syswall' group GID — fatal error if missing
    let syswall_gid: u32 = nix::unistd::Group::from_name("syswall")
        .map_err(|e| StartupError::ConfigInvalid(format!("getgrnam: {e}")))?
        .ok_or(StartupError::SyswallGroupMissing)?
        .gid
        .as_raw();

    // Canal d'audit pour l'interceptor gRPC — les événements sont drainés par le listener d'audit
    // Audit channel for the gRPC interceptor — events are drained by the audit listener
    let (grpc_audit_tx, mut grpc_audit_rx) = tokio::sync::mpsc::channel::<DomainAuditEvent>(64);

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
            ctx.connection_service.clone(),
        );
        let event_service = SysWallEventService::new(ctx.event_bus.clone());
        let socket_path = config.daemon.socket_path.clone();
        let cancel = cancel.clone();
        let audit_tx = grpc_audit_tx.clone();

        async move {
            start_grpc_server(
                socket_path,
                control_service,
                event_service,
                syswall_gid,
                audit_tx,
                cancel,
            )
            .await
            .map_err(|e| e.to_string())
        }
    });

    // Tâche de drain du canal d'audit gRPC — persiste les événements de refus d'accès
    // gRPC audit drain task — persists access-denied audit events
    supervisor.spawn("grpc-audit-drain", {
        let audit_repo = ctx.audit_service.repo().clone();
        let cancel = cancel.clone();

        async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    event = grpc_audit_rx.recv() => {
                        match event {
                            Some(e) => {
                                if let Err(err) = audit_repo.append(&e).await {
                                    warn!("grpc-audit-drain: échec d'enregistrement: {}", err);
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
            Ok(())
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
                                if let Some(audit_event) = AuditService::domain_event_to_audit(&event)
                                    && let Err(e) = writer.buffer_event(audit_event).await
                                {
                                    warn!("Audit listener: failed to buffer event: {}", e);
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

    // Rotation quotidienne du journal d'audit — purge les events anterieurs a retention_days
    // Daily audit journal rotation — purges events older than retention_days
    syswall_app::services::journal_rotation::spawn_journal_rotation(
        ctx.audit_service.repo().clone(),
        config.database.journal_retention_days,
        cancel.clone(),
    );

    // Watchdog systemd — envoie WATCHDOG=1 toutes les watchdog_interval_secs/2 secondes
    // Systemd watchdog — sends WATCHDOG=1 every watchdog_interval_secs/2 seconds
    watchdog::spawn_watchdog(config.daemon.watchdog_interval_secs, cancel.clone());

    // Lance l'intercepteur NFQUEUE (mode dégradé si CAP_NET_ADMIN absent)
    // Launch the NFQUEUE interceptor (degraded mode if CAP_NET_ADMIN is missing)
    bootstrap::wire_nfqueue(&ctx, &config, cancel.clone());

    // Lance l'observateur DNS (snooping IP→domaine pour l'identité des connexions)
    // Launch the DNS observer (IP→domain snooping for connection identity)
    bootstrap::wire_dns_observer(&ctx, cancel.clone());

    info!("SysWall daemon ready");

    // Notify systemd that we're ready
    let _ = sd_notify::notify(false, &[sd_notify::NotifyState::Ready]);

    // Run until shutdown
    supervisor.run().await;

    // Notify systemd we're stopping
    let _ = sd_notify::notify(false, &[sd_notify::NotifyState::Stopping]);

    info!("SysWall daemon stopped");

    Ok(())
}
