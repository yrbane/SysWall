/// gRPC server setup with Unix socket transport.
/// Configuration du serveur gRPC avec transport par socket Unix.

use std::path::PathBuf;

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;
use tracing::{info, warn};

use syswall_proto::syswall::sys_wall_control_server::SysWallControlServer;
use syswall_proto::syswall::sys_wall_events_server::SysWallEventsServer;

use super::control_service::SysWallControlService;
use super::event_service::SysWallEventService;

/// Start the gRPC server on a Unix domain socket.
/// Démarre le serveur gRPC sur un socket Unix.
pub async fn start_grpc_server(
    socket_path: PathBuf,
    control_service: SysWallControlService,
    event_service: SysWallEventService,
    cancel: CancellationToken,
) -> Result<(), String> {
    // Remove stale socket file if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)
            .map_err(|e| format!("Failed to remove stale socket: {}", e))?;
    }

    // Create parent directory if needed
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create socket directory: {}", e))?;
    }

    // Bind Unix socket
    let listener = UnixListener::bind(&socket_path)
        .map_err(|e| format!("Failed to bind Unix socket at {:?}: {}", socket_path, e))?;

    info!("gRPC server listening on {:?}", socket_path);

    // Set socket permissions to 0660 (owner + group read/write)
    set_socket_permissions(&socket_path);

    let uds_stream = UnixListenerStream::new(listener);

    // Build tonic server with both services
    let server = Server::builder()
        .add_service(SysWallControlServer::new(control_service))
        .add_service(SysWallEventsServer::new(event_service));

    // Run until cancellation
    server
        .serve_with_incoming_shutdown(uds_stream, cancel.cancelled())
        .await
        .map_err(|e| format!("gRPC server error: {}", e))?;

    // Clean up socket file on shutdown
    let _ = std::fs::remove_file(&socket_path);
    info!("gRPC server stopped, socket removed");

    Ok(())
}

/// Set socket permissions to 0660 and optionally set group to syswall.
/// Définit les permissions du socket à 0660 et optionnellement le groupe à syswall.
fn set_socket_permissions(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    // Set permissions to 0660
    if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660)) {
        warn!("Failed to set socket permissions: {}", e);
    }

    // Try to set group to syswall (may fail in sandboxed systemd environments)
    // Tente de changer le groupe à syswall (peut échouer dans un environnement systemd sandboxé)
    match nix::unistd::Group::from_name("syswall") {
        Ok(Some(group)) => {
            if let Err(e) = nix::unistd::chown(path, None, Some(group.gid)) {
                tracing::debug!("Could not set socket group to syswall (expected in sandboxed systemd): {}", e);
            }
        }
        Ok(None) => {
            tracing::debug!("Group 'syswall' does not exist, socket uses default group");
        }
        Err(e) => {
            tracing::debug!("Failed to look up syswall group: {}", e);
        }
    }
}
