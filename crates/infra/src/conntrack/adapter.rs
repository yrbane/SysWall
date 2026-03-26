use std::net::IpAddr;
use std::path::PathBuf;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, error, info, warn};

use syswall_domain::entities::Connection;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::{ConnectionEventStream, ConnectionMonitor};

use super::parser::parse_conntrack_line;
use super::transformer::conntrack_to_connection;

/// Configuration for the ConntrackMonitorAdapter.
/// Configuration pour l'adaptateur ConntrackMonitorAdapter.
#[derive(Debug, Clone)]
pub struct ConntrackConfig {
    pub binary_path: PathBuf,
    pub protocols: Vec<String>,
    pub buffer_size: usize,
}

impl Default for ConntrackConfig {
    fn default() -> Self {
        Self {
            binary_path: PathBuf::from("/usr/sbin/conntrack"),
            protocols: vec!["tcp".to_string(), "udp".to_string()],
            buffer_size: 4096,
        }
    }
}

/// Real conntrack-based connection monitor adapter.
/// Adaptateur reel de surveillance des connexions base sur conntrack.
pub struct ConntrackMonitorAdapter {
    config: ConntrackConfig,
    local_ips: Vec<IpAddr>,
}

impl ConntrackMonitorAdapter {
    /// Create a new adapter. Detects local IPs at construction time.
    /// Cree un nouvel adaptateur. Detecte les IPs locales a la construction.
    pub fn new(config: ConntrackConfig) -> Result<Self, DomainError> {
        if !config.binary_path.exists() {
            return Err(DomainError::Infrastructure(format!(
                "conntrack binary not found at: {}. Install conntrack-tools package.",
                config.binary_path.display()
            )));
        }

        let local_ips = detect_local_ips();
        info!(
            "ConntrackMonitorAdapter: detected {} local IPs",
            local_ips.len()
        );

        Ok(Self { config, local_ips })
    }
}

/// Detect local IP addresses by reading network interfaces.
/// Detecte les adresses IP locales en lisant les interfaces reseau.
fn detect_local_ips() -> Vec<IpAddr> {
    let mut ips = vec![
        "127.0.0.1".parse::<IpAddr>().unwrap(),
        "::1".parse::<IpAddr>().unwrap(),
    ];

    // Use nix::ifaddrs to get interface addresses
    if let Ok(addrs) = nix::ifaddrs::getifaddrs() {
        for addr in addrs {
            if let Some(sockaddr) = addr.address {
                if let Some(sin) = sockaddr.as_sockaddr_in() {
                    let ip = IpAddr::V4(std::net::Ipv4Addr::from(sin.ip()));
                    if !ips.contains(&ip) {
                        ips.push(ip);
                    }
                } else if let Some(sin6) = sockaddr.as_sockaddr_in6() {
                    let ip = IpAddr::V6(sin6.ip());
                    if !ips.contains(&ip) {
                        ips.push(ip);
                    }
                }
            }
        }
    }

    ips
}

#[async_trait]
impl ConnectionMonitor for ConntrackMonitorAdapter {
    /// Start streaming conntrack events. Spawns one child process per protocol.
    /// Demarre le streaming des evenements conntrack. Lance un processus enfant par protocole.
    async fn stream_events(&self) -> Result<ConnectionEventStream, DomainError> {
        let (tx, rx) =
            tokio::sync::mpsc::channel::<Result<Connection, DomainError>>(self.config.buffer_size);

        for proto in &self.config.protocols {
            let mut child = tokio::process::Command::new(&self.config.binary_path)
                .args(["-E", "-o", "timestamp", "-p", proto])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| {
                    DomainError::Infrastructure(format!(
                        "Failed to spawn conntrack -E -p {}: {}",
                        proto, e
                    ))
                })?;

            let stdout = child.stdout.take().ok_or_else(|| {
                DomainError::Infrastructure("Failed to capture conntrack stdout".to_string())
            })?;

            let tx = tx.clone();
            let local_ips = self.local_ips.clone();
            let proto_name = proto.clone();

            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                loop {
                    match lines.next_line().await {
                        Ok(Some(line)) => {
                            if let Some(event) = parse_conntrack_line(&line) {
                                if let Some(conn) =
                                    conntrack_to_connection(event, &local_ips)
                                {
                                    if tx.send(Ok(conn)).await.is_err() {
                                        debug!(
                                            "conntrack {} stream: receiver dropped",
                                            proto_name
                                        );
                                        break;
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            warn!("conntrack {} stream ended (EOF)", proto_name);
                            let _ = tx
                                .send(Err(DomainError::Infrastructure(format!(
                                    "conntrack {} process exited",
                                    proto_name
                                ))))
                                .await;
                            break;
                        }
                        Err(e) => {
                            error!("conntrack {} read error: {}", proto_name, e);
                            let _ = tx
                                .send(Err(DomainError::Infrastructure(format!(
                                    "conntrack read error: {}",
                                    e
                                ))))
                                .await;
                            break;
                        }
                    }
                }

                // Cleanup: kill child process
                let _ = child.kill().await;
            });
        }

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    /// Get currently active connections by running `conntrack -L`.
    /// Recupere les connexions actives en executant `conntrack -L`.
    async fn get_active_connections(&self) -> Result<Vec<Connection>, DomainError> {
        let output = tokio::process::Command::new(&self.config.binary_path)
            .args(["-L", "-o", "extended"])
            .output()
            .await
            .map_err(|e| {
                DomainError::Infrastructure(format!("Failed to run conntrack -L: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::Infrastructure(format!(
                "conntrack -L failed: {}",
                &stderr[..stderr.len().min(500)]
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let connections: Vec<Connection> = stdout
            .lines()
            .filter_map(|line| parse_conntrack_line(line))
            .filter_map(|event| conntrack_to_connection(event, &self.local_ips))
            .collect();

        Ok(connections)
    }
}
