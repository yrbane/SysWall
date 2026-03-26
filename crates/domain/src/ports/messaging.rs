use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::entities::PendingDecision;
use crate::errors::DomainError;
use crate::events::{DomainEvent, Notification};

/// Receiver for domain events.
/// Récepteur pour les événements du domaine.
pub type EventReceiver = broadcast::Receiver<DomainEvent>;

/// Publish/subscribe bus for domain events.
/// Bus de publication/abonnement pour les événements du domaine.
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent) -> Result<(), DomainError>;
    fn subscribe(&self) -> EventReceiver;
}

/// Notifies the UI that a decision is required (non-blocking).
/// Notifie l'interface utilisateur qu'une décision est requise (non bloquant).
#[async_trait]
pub trait UserNotifier: Send + Sync {
    async fn notify_decision_required(
        &self,
        request: &PendingDecision,
    ) -> Result<(), DomainError>;
    async fn notify(&self, notification: &Notification) -> Result<(), DomainError>;
}
