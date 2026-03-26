use async_trait::async_trait;
use tokio::sync::broadcast;

use syswall_domain::errors::DomainError;
use syswall_domain::events::DomainEvent;
use syswall_domain::ports::{EventBus, EventReceiver};

/// Event bus backed by tokio broadcast channel.
/// Bus d'événements adossé au canal broadcast de tokio.
pub struct TokioBroadcastEventBus {
    sender: broadcast::Sender<DomainEvent>,
}

impl TokioBroadcastEventBus {
    /// Create a new event bus with the given channel capacity.
    /// Crée un nouveau bus d'événements avec la capacité de canal donnée.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
}

#[async_trait]
impl EventBus for TokioBroadcastEventBus {
    async fn publish(&self, event: DomainEvent) -> Result<(), DomainError> {
        match self.sender.send(event) {
            Ok(_) => Ok(()),
            Err(_) => {
                // No subscribers -- this is fine, events are volatile.
                // Aucun abonné -- pas de problème, les événements sont volatils.
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
}
