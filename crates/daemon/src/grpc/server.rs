/// gRPC server setup with Unix socket transport and SO_PEERCRED auth.
/// Configuration du serveur gRPC avec transport Unix et auth SO_PEERCRED.
use std::collections::HashSet;
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use nix::sys::socket::{getsockopt, sockopt::PeerCredentials as NixPeerCreds};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnixListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::server::Connected;
use tonic::transport::Server;
use tracing::info;

use syswall_domain::entities::AuditEvent;
use syswall_proto::syswall::sys_wall_control_server::SysWallControlServer;
use syswall_proto::syswall::sys_wall_events_server::SysWallEventsServer;

use crate::grpc::interceptors::{PeerAuthInterceptor, PeerAuthPolicy, PeerCredentials};
use crate::startup_error::StartupError;

use super::control_service::SysWallControlService;
use super::event_service::SysWallEventService;

// ---------------------------------------------------------------------------
// PeerStream : wrapper UnixStream qui capture SO_PEERCRED à l'acceptation
// PeerStream: UnixStream wrapper that captures SO_PEERCRED at accept time
// ---------------------------------------------------------------------------

/// Flux Unix enrichi avec les identifiants du pair (SO_PEERCRED).
/// Unix stream enriched with peer credentials (SO_PEERCRED).
pub(crate) struct PeerStream {
    inner: UnixStream,
    creds: PeerCredentials,
}

impl PeerStream {
    /// Extrait SO_PEERCRED depuis le descripteur de fichier du flux accepté.
    /// Extracts SO_PEERCRED from the file descriptor of the accepted stream.
    pub fn from_unix_stream(stream: UnixStream) -> std::io::Result<Self> {
        let raw = getsockopt(&stream.as_fd(), NixPeerCreds)
            .map_err(|e| std::io::Error::other(format!("SO_PEERCRED: {e}")))?;
        let creds = PeerCredentials {
            uid: raw.uid(),
            gid: raw.gid(),
            pid: raw.pid(),
        };
        Ok(PeerStream { inner: stream, creds })
    }
}

// Tonic injecte ConnectInfo dans les extensions de chaque requête entrante.
// Tonic injects ConnectInfo into extensions of each incoming request.
impl Connected for PeerStream {
    type ConnectInfo = PeerCredentials;

    fn connect_info(&self) -> PeerCredentials {
        self.creds
    }
}

impl AsyncRead for PeerStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for PeerStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

// ---------------------------------------------------------------------------
// Démarrage du serveur gRPC
// gRPC server startup
// ---------------------------------------------------------------------------

/// Démarre le serveur gRPC sur un socket Unix avec authentification SO_PEERCRED.
/// Starts the gRPC server on a Unix socket with SO_PEERCRED authentication.
pub async fn start_grpc_server(
    socket_path: PathBuf,
    control_service: SysWallControlService,
    event_service: SysWallEventService,
    syswall_gid: u32,
    audit_tx: mpsc::Sender<AuditEvent>,
    cancel: CancellationToken,
) -> Result<(), StartupError> {
    // Supprime l'ancien fichier socket s'il existe
    if socket_path.exists() {
        std::fs::remove_file(&socket_path).map_err(|e| StartupError::InfrastructureInit(
            format!("impossible de supprimer l'ancien socket: {e}"),
        ))?;
    }

    // Crée le répertoire parent si nécessaire
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| StartupError::InfrastructureInit(
            format!("impossible de créer le répertoire du socket: {e}"),
        ))?;
    }

    // Bind du socket Unix — erreur fatale si impossible
    let listener = UnixListener::bind(&socket_path).map_err(|e| StartupError::SocketBindFailed {
        path: socket_path.display().to_string(),
        source: e,
    })?;

    info!("Serveur gRPC en écoute sur {:?}", socket_path);

    // Permissions 0660 : propriétaire + groupe en lecture/écriture
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o660))
            .map_err(|e| StartupError::SocketChownFailed {
                path: socket_path.display().to_string(),
                source: e,
            })?;
    }

    // Chown vers le groupe syswall — erreur fatale
    nix::unistd::chown(
        &socket_path,
        None,
        Some(nix::unistd::Gid::from_raw(syswall_gid)),
    )
    .map_err(|e| StartupError::SocketChownFailed {
        path: socket_path.display().to_string(),
        source: std::io::Error::from_raw_os_error(e as i32),
    })?;

    // Politique d'autorisation : UID 0 (root) ou GID syswall
    let policy = Arc::new(PeerAuthPolicy::new(
        HashSet::from([0u32]),
        HashSet::from([syswall_gid]),
    ));
    let peer_auth = PeerAuthInterceptor::new(policy, audit_tx);

    // Flux de connexions : chaque UnixStream accepté devient un PeerStream
    let incoming = UnixListenerStream::new(listener).map(|res| {
        res.and_then(PeerStream::from_unix_stream)
    });

    // Limites par service : 1 Mio décodage, 4 Mio encodage
    // Per-service limits: 1 MiB decode, 4 MiB encode
    let control_svc = SysWallControlServer::new(control_service)
        .max_decoding_message_size(1 << 20)
        .max_encoding_message_size(4 << 20);
    let events_svc = SysWallEventsServer::new(event_service)
        .max_decoding_message_size(1 << 20)
        .max_encoding_message_size(4 << 20);

    // Construction du serveur avec limites de transport et double service intercepté
    Server::builder()
        .timeout(Duration::from_secs(30))
        .concurrency_limit_per_connection(64)
        .add_service(InterceptedService::new(control_svc, peer_auth.clone()))
        .add_service(InterceptedService::new(events_svc, peer_auth))
        .serve_with_incoming_shutdown(incoming, cancel.cancelled())
        .await
        .map_err(|e| StartupError::InfrastructureInit(format!("gRPC: {e}")))?;

    // Nettoyage du fichier socket à l'arrêt
    let _ = std::fs::remove_file(&socket_path);
    info!("Serveur gRPC arrêté, socket supprimé");

    Ok(())
}
