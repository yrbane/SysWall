use async_trait::async_trait;
use std::sync::Mutex;

use syswall_domain::entities::AuditEvent;
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{AuditFilters, AuditRepository};

/// In-memory fake audit repository for testing.
/// Dépôt factice en mémoire des événements d'audit pour les tests.
pub struct FakeAuditRepository {
    pub events: Mutex<Vec<AuditEvent>>,
}

impl FakeAuditRepository {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl AuditRepository for FakeAuditRepository {
    async fn append(&self, event: &AuditEvent) -> Result<(), DomainError> {
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    async fn query(
        &self,
        _filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError> {
        let events = self.events.lock().unwrap();
        let start = pagination.offset as usize;
        let end = (start + pagination.limit as usize).min(events.len());
        if start >= events.len() {
            return Ok(vec![]);
        }
        Ok(events[start..end].to_vec())
    }

    async fn count(&self, _filters: &AuditFilters) -> Result<u64, DomainError> {
        Ok(self.events.lock().unwrap().len() as u64)
    }
}
