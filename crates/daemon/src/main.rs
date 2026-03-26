mod bootstrap;
mod config;
mod grpc;
mod signals;
mod supervisor;

use std::path::Path;

use futures::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use syswall_app::services::whitelist::ensure_system_whitelist;
use syswall_domain::entities::ConnectionVerdict;
use syswall_domain::ports::RuleRepository;

use crate::config::SysWallConfig;
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
    if let Err(e) = ensure_system_whitelist(&ctx.rule_service, ctx.rule_repo.as_ref()).await {
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

    info!("SysWall daemon ready");

    // Run until shutdown
    supervisor.run().await;

    info!("SysWall daemon stopped");
}
