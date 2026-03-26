mod bootstrap;
mod config;
mod grpc;
mod signals;
mod supervisor;

use std::path::Path;

use tokio_util::sync::CancellationToken;
use tracing::info;

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
    let _ctx = match bootstrap::bootstrap(&config) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Fatal: bootstrap failed: {}", e);
            std::process::exit(1);
        }
    };

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

    info!("SysWall daemon ready");

    // Run until shutdown
    supervisor.run().await;

    info!("SysWall daemon stopped");
}
