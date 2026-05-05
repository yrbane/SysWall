use async_trait::async_trait;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io;
use tokio::net::TcpStream;

use syswall_domain::ports::connectivity::{ConnectivityProbe, ProbeError, ProbeOutcome};

/// TCP-based connectivity probe.
/// Sonde de connectivité TCP.
#[derive(Debug)]
pub struct TcpProbe {
    endpoints: Vec<SocketAddr>,
    per_endpoint_timeout: Duration,
}

impl TcpProbe {
    pub fn new(endpoints: Vec<SocketAddr>, per_endpoint_timeout: Duration) -> Result<Self, ProbeError> {
        if endpoints.is_empty() {
            return Err(ProbeError::Configuration("empty endpoint list".into()));
        }
        Ok(Self { endpoints, per_endpoint_timeout })
    }
}

#[async_trait]
impl ConnectivityProbe for TcpProbe {
    async fn probe(&self) -> Result<ProbeOutcome, ProbeError> {
        let attempts = self.endpoints.iter().copied().map(|addr| {
            let timeout = self.per_endpoint_timeout;
            async move {
                match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
                    Ok(Ok(_)) => true,                                       // Connected.
                    Ok(Err(e)) if is_reachable_error(&e) => true,            // Refused/Reset = network OK.
                    Ok(Err(_)) | Err(_) => false,                            // Other or timeout.
                }
            }
        });
        let results = futures::future::join_all(attempts).await;
        if results.into_iter().any(|reachable| reachable) {
            Ok(ProbeOutcome::Reachable)
        } else {
            Ok(ProbeOutcome::Unreachable)
        }
    }
}

fn is_reachable_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn empty_endpoints_returns_configuration_error() {
        let err = TcpProbe::new(vec![], Duration::from_secs(1)).unwrap_err();
        assert!(matches!(err, ProbeError::Configuration(_)));
    }

    #[tokio::test]
    async fn local_listener_is_reachable() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let probe = TcpProbe::new(vec![addr], Duration::from_secs(2)).unwrap();
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Reachable);
    }

    #[tokio::test]
    async fn closed_port_is_reachable_via_conn_refused() {
        // Bind then drop to release the port; connect attempts will see ConnectionRefused.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let probe = TcpProbe::new(vec![addr], Duration::from_secs(2)).unwrap();
        // ConnectionRefused on loopback → counts as reachable.
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Reachable);
    }

    #[tokio::test]
    async fn unroutable_address_times_out_to_unreachable() {
        // 192.0.2.0/24 is reserved for documentation (RFC 5737), guaranteed not routable.
        let addr: SocketAddr = "192.0.2.1:65535".parse().unwrap();
        let probe = TcpProbe::new(vec![addr], Duration::from_millis(200)).unwrap();
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Unreachable);
    }

    #[tokio::test]
    async fn one_reachable_one_unreachable_yields_reachable() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr_ok = listener.local_addr().unwrap();
        let addr_ko: SocketAddr = "192.0.2.1:65535".parse().unwrap();
        let probe = TcpProbe::new(vec![addr_ok, addr_ko], Duration::from_millis(200)).unwrap();
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Reachable);
    }
}
