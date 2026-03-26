use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Mutex;

use syswall_domain::entities::{PendingDecision, PendingDecisionId, PendingDecisionStatus};
use syswall_domain::errors::DomainError;
use syswall_domain::ports::PendingDecisionRepository;

/// In-memory fake pending decision repository for testing.
/// Dépôt factice en mémoire des décisions en attente pour les tests.
pub struct FakePendingDecisionRepository {
    decisions: Mutex<HashMap<PendingDecisionId, PendingDecision>>,
}

impl FakePendingDecisionRepository {
    pub fn new() -> Self {
        Self {
            decisions: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl PendingDecisionRepository for FakePendingDecisionRepository {
    async fn create(&self, request: &PendingDecision) -> Result<(), DomainError> {
        self.decisions
            .lock()
            .unwrap()
            .insert(request.id, request.clone());
        Ok(())
    }

    async fn list_pending(&self) -> Result<Vec<PendingDecision>, DomainError> {
        Ok(self
            .decisions
            .lock()
            .unwrap()
            .values()
            .filter(|d| d.status == PendingDecisionStatus::Pending)
            .cloned()
            .collect())
    }

    async fn resolve(&self, id: &PendingDecisionId) -> Result<(), DomainError> {
        if let Some(d) = self.decisions.lock().unwrap().get_mut(id) {
            d.status = PendingDecisionStatus::Resolved;
        }
        Ok(())
    }

    async fn expire_overdue(&self) -> Result<Vec<PendingDecision>, DomainError> {
        let now = Utc::now();
        let mut expired = vec![];
        for d in self.decisions.lock().unwrap().values_mut() {
            if d.status == PendingDecisionStatus::Pending && d.expires_at < now {
                d.status = PendingDecisionStatus::Expired;
                expired.push(d.clone());
            }
        }
        Ok(expired)
    }

    async fn find_by_dedup_key(&self, key: &str) -> Result<Option<PendingDecision>, DomainError> {
        Ok(self
            .decisions
            .lock()
            .unwrap()
            .values()
            .find(|d| d.deduplication_key == key && d.status == PendingDecisionStatus::Pending)
            .cloned())
    }
}
