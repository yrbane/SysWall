use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use syswall_domain::entities::ConnectionId;
use syswall_domain::errors::DomainError;
use syswall_domain::events::DomainEvent;
use syswall_domain::ports::{EventBus, EventReceiver};

/// Bus d'événements adossé au canal broadcast de tokio.
/// Supporte la fusion optionnelle des événements ConnectionDetected.
///
/// Event bus backed by tokio broadcast channel.
/// Supports optional merging of ConnectionDetected events.
pub struct TokioBroadcastEventBus {
    sender: broadcast::Sender<DomainEvent>,
    merge_buffer: Option<Arc<Mutex<HashMap<ConnectionId, syswall_domain::entities::Connection>>>>,
}

impl TokioBroadcastEventBus {
    /// Crée un bus sans fusion d'événements.
    /// Create a bus without event merging.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            merge_buffer: None,
        }
    }

    /// Crée un bus avec fusion des ConnectionDetected dans une fenêtre temporelle.
    /// Les événements pour le même ConnectionId sont fusionnés : seul le dernier est émis.
    ///
    /// Create a bus that merges ConnectionDetected events within a time window.
    /// Events for the same ConnectionId are merged: only the latest is emitted.
    pub fn with_merge_window(capacity: usize, window: Duration) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        let buffer: Arc<Mutex<HashMap<ConnectionId, syswall_domain::entities::Connection>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let buf_clone = buffer.clone();
        let sender_clone = sender.clone();

        // Tâche de vidage périodique du buffer de fusion
        // Periodic flush task for the merge buffer
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(window).await;
                let events: Vec<syswall_domain::entities::Connection> = {
                    let mut buf = buf_clone
                        .lock()
                        .expect("Mutex jamais empoisonné / never poisoned");
                    buf.drain().map(|(_, conn)| conn).collect()
                };
                for conn in events {
                    let _ = sender_clone.send(DomainEvent::ConnectionDetected(conn));
                }
            }
        });

        Self {
            sender,
            merge_buffer: Some(buffer),
        }
    }
}

#[async_trait]
impl EventBus for TokioBroadcastEventBus {
    async fn publish(&self, event: DomainEvent) -> Result<(), DomainError> {
        // Si le merge buffer est actif et l'événement est ConnectionDetected,
        // on l'insère dans le buffer au lieu de l'envoyer directement
        // If merge buffer is active and event is ConnectionDetected,
        // insert into buffer instead of sending directly
        if let Some(ref buffer) = self.merge_buffer
            && let DomainEvent::ConnectionDetected(ref conn) = event
            && let Ok(mut buf) = buffer.lock()
        {
            buf.insert(conn.id, conn.clone());
            return Ok(());
        }

        match self.sender.send(event) {
            Ok(_) => Ok(()),
            Err(_) => {
                // Aucun abonné — pas de problème, les événements sont volatils.
                // No subscribers — this is fine, events are volatile.
                Ok(())
            }
        }
    }

    fn subscribe(&self) -> EventReceiver {
        self.sender.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn test_connection() -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol: Protocol::Tcp,
            source: SocketAddress::new("192.168.1.100".parse().unwrap(), Port::new(45000).unwrap()),
            destination: SocketAddress::new(
                "93.184.216.34".parse().unwrap(),
                Port::new(443).unwrap(),
            ),
            direction: Direction::Outbound,
            state: ConnectionState::New,
            process: None,
            user: None,
            bytes_sent: 0,
            bytes_received: 0,
            started_at: Utc::now(),
            verdict: ConnectionVerdict::Unknown,
            matched_rule: None,
            remote_hostname: None,
        }
    }

    #[tokio::test]
    async fn publish_and_receive() {
        let bus = TokioBroadcastEventBus::new(128);
        let mut rx = bus.subscribe();

        let rule = Rule {
            id: RuleId::new(),
            name: "Test".to_string(),
            priority: RulePriority::new(1),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        };

        bus.publish(DomainEvent::RuleCreated(rule)).await.unwrap();

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, DomainEvent::RuleCreated(_)));
    }

    #[tokio::test]
    async fn publish_without_subscribers_does_not_error() {
        let bus = TokioBroadcastEventBus::new(128);
        let result = bus.publish(DomainEvent::RuleDeleted(RuleId::new())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn multiple_subscribers_receive_same_event() {
        let bus = TokioBroadcastEventBus::new(128);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(DomainEvent::RuleDeleted(RuleId::new()))
            .await
            .unwrap();

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[tokio::test]
    async fn merge_duplicate_connection_events() {
        let bus = TokioBroadcastEventBus::with_merge_window(128, Duration::from_millis(50));
        let mut rx = bus.subscribe();

        let conn = test_connection();
        let id = conn.id;

        // Publie 5 ConnectionDetected pour la même connexion rapidement
        // Publish 5 ConnectionDetected for the same connection rapidly
        for i in 0..5u64 {
            let mut c = conn.clone();
            c.bytes_sent = i * 100;
            bus.publish(DomainEvent::ConnectionDetected(c))
                .await
                .unwrap();
        }

        // Attendre le flush du buffer
        // Wait for buffer flush
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Devrait recevoir 1 seul événement (le dernier)
        // Should receive only 1 event (the latest)
        let event = rx.try_recv();
        assert!(matches!(event, Ok(DomainEvent::ConnectionDetected(ref c)) if c.id == id));

        // Pas d'autres événements
        // No more events
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn non_connection_events_pass_through_with_merge() {
        let bus = TokioBroadcastEventBus::with_merge_window(128, Duration::from_millis(50));
        let mut rx = bus.subscribe();

        // Les événements non-ConnectionDetected passent directement
        // Non-ConnectionDetected events pass through immediately
        bus.publish(DomainEvent::RuleDeleted(RuleId::new()))
            .await
            .unwrap();

        let event = rx.try_recv();
        assert!(matches!(event, Ok(DomainEvent::RuleDeleted(_))));
    }

    #[tokio::test]
    async fn merge_keeps_different_connections_separate() {
        let bus = TokioBroadcastEventBus::with_merge_window(128, Duration::from_millis(50));
        let mut rx = bus.subscribe();

        let conn1 = test_connection();
        let conn2 = test_connection(); // different ID

        bus.publish(DomainEvent::ConnectionDetected(conn1))
            .await
            .unwrap();
        bus.publish(DomainEvent::ConnectionDetected(conn2))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Devrait recevoir 2 événements distincts
        // Should receive 2 distinct events
        let e1 = rx.try_recv();
        let e2 = rx.try_recv();
        assert!(e1.is_ok());
        assert!(e2.is_ok());
    }
}
