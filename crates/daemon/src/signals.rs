use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Wait for SIGTERM or SIGINT, then trigger cancellation.
/// Attend SIGTERM ou SIGINT, puis déclenche l'annulation.
pub async fn wait_for_shutdown(cancel: CancellationToken) {
    let mut sigterm =
        signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    let mut sigint =
        signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");

    tokio::select! {
        _ = sigterm.recv() => {
            info!("Received SIGTERM, initiating shutdown");
        }
        _ = sigint.recv() => {
            info!("Received SIGINT, initiating shutdown");
        }
        _ = cancel.cancelled() => {
            // Already cancelled externally
            // Déjà annulé de l'extérieur
        }
    }

    cancel.cancel();
}
