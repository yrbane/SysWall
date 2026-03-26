//! gRPC client for connecting to the SysWall daemon over Unix socket.

use std::sync::Arc;
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;
use tracing::info;

use syswall_proto::syswall::sys_wall_control_client::SysWallControlClient;
use syswall_proto::syswall::sys_wall_events_client::SysWallEventsClient;

/// Path to the daemon Unix socket.
const DEFAULT_SOCKET_PATH: &str = "/var/run/syswall/syswall.sock";

/// Holds the gRPC channel and typed clients.
#[derive(Clone)]
pub struct GrpcClient {
    pub control: SysWallControlClient<Channel>,
    pub events: SysWallEventsClient<Channel>,
}

impl GrpcClient {
    /// Connect to the daemon Unix socket.
    pub async fn connect(socket_path: Option<&str>) -> Result<Self, String> {
        let path = socket_path.unwrap_or(DEFAULT_SOCKET_PATH).to_string();

        info!("Connecting to daemon at {}", path);

        let channel = Endpoint::try_from("http://[::]:50051")
            .map_err(|e| format!("Failed to create endpoint: {}", e))?
            .connect_with_connector(service_fn(move |_: Uri| {
                let path = path.clone();
                async move {
                    let stream = UnixStream::connect(path).await?;
                    Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(stream))
                }
            }))
            .await
            .map_err(|e| format!("Failed to connect to daemon socket: {}", e))?;

        info!("Connected to daemon successfully");

        Ok(Self {
            control: SysWallControlClient::new(channel.clone()),
            events: SysWallEventsClient::new(channel),
        })
    }
}

/// Thread-safe wrapper for the gRPC client, stored as Tauri managed state.
pub struct GrpcState {
    pub client: Arc<Mutex<Option<GrpcClient>>>,
}

impl GrpcState {
    /// Create a new empty state (client connects during setup).
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// Get or reconnect the client.
    pub async fn get_client(&self) -> Result<GrpcClient, String> {
        let mut guard = self.client.lock().await;
        if let Some(ref client) = *guard {
            return Ok(client.clone());
        }

        let client = GrpcClient::connect(None).await?;
        *guard = Some(client.clone());
        Ok(client)
    }

    /// Force reconnection (e.g., after transport error).
    pub async fn reconnect(&self) -> Result<GrpcClient, String> {
        let mut guard = self.client.lock().await;
        let client = GrpcClient::connect(None).await?;
        *guard = Some(client.clone());
        Ok(client)
    }
}
