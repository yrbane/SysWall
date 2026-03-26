use async_trait::async_trait;
use tokio::sync::broadcast;

use syswall_domain::errors::DomainError;
use syswall_domain::events::DomainEvent;
use syswall_domain::ports::{EventBus, EventReceiver};

/// In-memory fake event bus backed by tokio broadcast for testing.
/// Bus d'événements factice en mémoire basé sur tokio broadcast pour les tests.
pub struct FakeEventBus {
    sender: broadcast::Sender<DomainEvent>,
}

impl FakeEventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    /// Access the underlying broadcast sender for test assertions.
    /// Accède à l'émetteur broadcast sous-jacent pour les assertions de test.
    pub fn sender(&self) -> &broadcast::Sender<DomainEvent> {
        &self.sender
    }
}

#[async_trait]
impl EventBus for FakeEventBus {
    async fn publish(&self, event: DomainEvent) -> Result<(), DomainError> {
        let _ = self.sender.send(event);
        Ok(())
    }

    fn subscribe(&self) -> EventReceiver {
        self.sender.subscribe()
    }
}
