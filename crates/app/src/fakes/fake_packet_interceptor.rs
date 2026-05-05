use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

use syswall_domain::entities::Connection;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::interception::{
    PacketDecisionHandler, PacketInterceptor, PacketVerdict,
};

/// Programmable fake `PacketInterceptor` for tests.
/// Fake programmable du `PacketInterceptor` pour les tests.
#[derive(Debug, Default)]
pub struct FakePacketInterceptor {
    /// Connections to inject into the handler (FIFO via pop_front semantics on Vec end).
    /// Connexions à injecter dans le handler.
    pub injectable: Arc<Mutex<Vec<Connection>>>,
    /// Verdicts captured from the handler, in arrival order.
    /// Verdicts capturés depuis le handler, dans l'ordre d'arrivée.
    pub captured: Arc<Mutex<Vec<PacketVerdict>>>,
}

impl FakePacketInterceptor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inject(&self, conn: Connection) {
        self.injectable
            .lock()
            .expect("Mutex jamais empoisonne dans un test")
            .insert(0, conn);
    }

    pub fn captured_verdicts(&self) -> Vec<PacketVerdict> {
        self.captured
            .lock()
            .expect("Mutex jamais empoisonne dans un test")
            .clone()
    }
}

#[async_trait]
impl PacketInterceptor for FakePacketInterceptor {
    async fn run(
        &self,
        handler: Arc<dyn PacketDecisionHandler>,
        cancel: CancellationToken,
    ) -> Result<(), DomainError> {
        loop {
            if cancel.is_cancelled() {
                return Ok(());
            }
            let next = {
                let mut q = self
                    .injectable
                    .lock()
                    .expect("Mutex jamais empoisonne dans un test");
                q.pop()
            };
            match next {
                Some(conn) => {
                    let verdict = handler.decide(&conn).await?;
                    self.captured
                        .lock()
                        .expect("Mutex jamais empoisonne dans un test")
                        .push(verdict);
                }
                None => return Ok(()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::entities::{
        ConnectionId, ConnectionState, ConnectionVerdict, ProcessInfo, SystemUser,
    };
    use syswall_domain::value_objects::{Direction, Port, Protocol, SocketAddress};

    struct AlwaysAccept;
    #[async_trait]
    impl PacketDecisionHandler for AlwaysAccept {
        async fn decide(&self, _conn: &Connection) -> Result<PacketVerdict, DomainError> {
            Ok(PacketVerdict::Accept)
        }
    }

    fn dummy_connection() -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol: Protocol::Tcp,
            source: SocketAddress::new(
                "127.0.0.1".parse().unwrap(),
                Port::new(12345).unwrap(),
            ),
            destination: SocketAddress::new(
                "93.184.216.34".parse().unwrap(),
                Port::new(443).unwrap(),
            ),
            direction: Direction::Outbound,
            state: ConnectionState::New,
            process: Some(ProcessInfo {
                pid: 1,
                name: "test".to_string(),
                path: None,
                cmdline: None,
                icon: None,
            }),
            user: Some(SystemUser {
                uid: 1000,
                name: "test".to_string(),
            }),
            bytes_sent: 0,
            bytes_received: 0,
            started_at: Utc::now(),
            verdict: ConnectionVerdict::Unknown,
            matched_rule: None,
            remote_hostname: None,
        }
    }

    #[tokio::test]
    async fn fake_runs_handler_on_each_injected_connection() {
        let fake = FakePacketInterceptor::new();
        fake.inject(dummy_connection());
        fake.inject(dummy_connection());
        let cancel = CancellationToken::new();
        fake.run(Arc::new(AlwaysAccept), cancel).await.unwrap();
        assert_eq!(fake.captured_verdicts().len(), 2);
        assert!(fake
            .captured_verdicts()
            .iter()
            .all(|v| *v == PacketVerdict::Accept));
    }
}
