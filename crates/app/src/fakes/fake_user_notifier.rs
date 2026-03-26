use async_trait::async_trait;
use std::sync::Mutex;

use syswall_domain::entities::PendingDecision;
use syswall_domain::errors::DomainError;
use syswall_domain::events::Notification;
use syswall_domain::ports::UserNotifier;

/// In-memory fake user notifier for testing.
/// Notificateur utilisateur factice en mémoire pour les tests.
pub struct FakeUserNotifier {
    pub decision_notifications: Mutex<Vec<PendingDecision>>,
    pub notifications: Mutex<Vec<Notification>>,
}

impl FakeUserNotifier {
    pub fn new() -> Self {
        Self {
            decision_notifications: Mutex::new(vec![]),
            notifications: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl UserNotifier for FakeUserNotifier {
    async fn notify_decision_required(
        &self,
        request: &PendingDecision,
    ) -> Result<(), DomainError> {
        self.decision_notifications
            .lock()
            .unwrap()
            .push(request.clone());
        Ok(())
    }

    async fn notify(&self, notification: &Notification) -> Result<(), DomainError> {
        self.notifications
            .lock()
            .unwrap()
            .push(notification.clone());
        Ok(())
    }
}
