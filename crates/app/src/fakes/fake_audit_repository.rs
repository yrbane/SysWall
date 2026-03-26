use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use syswall_domain::entities::{AuditEvent, AuditStats};
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{AuditFilters, AuditRepository};

/// In-memory fake audit repository for testing.
/// Dépôt factice en mémoire des événements d'audit pour les tests.
pub struct FakeAuditRepository {
    pub events: Mutex<Vec<AuditEvent>>,
}

impl Default for FakeAuditRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeAuditRepository {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(vec![]),
        }
    }

    /// Apply filters to a list of events (for realistic fake queries).
    /// Applique les filtres à une liste d'événements (pour des requêtes factices réalistes).
    fn matches(event: &AuditEvent, filters: &AuditFilters) -> bool {
        if let Some(ref severity) = filters.severity {
            if event.severity != *severity {
                return false;
            }
        }
        if let Some(ref category) = filters.category {
            if event.category != *category {
                return false;
            }
        }
        if let Some(ref search) = filters.search {
            if !event.description.contains(search.as_str()) {
                return false;
            }
        }
        if let Some(ref from) = filters.from {
            if event.timestamp < *from {
                return false;
            }
        }
        if let Some(ref to) = filters.to {
            if event.timestamp > *to {
                return false;
            }
        }
        true
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
        filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError> {
        let events = self.events.lock().unwrap();
        let filtered: Vec<AuditEvent> = events
            .iter()
            .filter(|e| Self::matches(e, filters))
            .cloned()
            .collect();
        let start = pagination.offset as usize;
        let end = (start + pagination.limit as usize).min(filtered.len());
        if start >= filtered.len() {
            return Ok(vec![]);
        }
        Ok(filtered[start..end].to_vec())
    }

    async fn count(&self, filters: &AuditFilters) -> Result<u64, DomainError> {
        let events = self.events.lock().unwrap();
        let count = events.iter().filter(|e| Self::matches(e, filters)).count();
        Ok(count as u64)
    }

    async fn append_batch(&self, events: &[AuditEvent]) -> Result<(), DomainError> {
        let mut store = self.events.lock().unwrap();
        for event in events {
            store.push(event.clone());
        }
        Ok(())
    }

    async fn delete_before(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, DomainError> {
        let mut events = self.events.lock().unwrap();
        let original_len = events.len();
        events.retain(|e| e.timestamp >= before);
        Ok((original_len - events.len()) as u64)
    }

    async fn get_stats(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<AuditStats, DomainError> {
        let events = self.events.lock().unwrap();
        let mut by_category: HashMap<String, u64> = HashMap::new();
        let mut by_severity: HashMap<String, u64> = HashMap::new();
        let mut total: u64 = 0;

        for event in events.iter() {
            if event.timestamp >= from && event.timestamp <= to {
                total += 1;
                let cat_str =
                    serde_json::to_string(&event.category).unwrap_or_default();
                let cat_str = cat_str.trim_matches('"').to_string();
                *by_category.entry(cat_str).or_insert(0) += 1;

                let sev_str =
                    serde_json::to_string(&event.severity).unwrap_or_default();
                let sev_str = sev_str.trim_matches('"').to_string();
                *by_severity.entry(sev_str).or_insert(0) += 1;
            }
        }

        Ok(AuditStats {
            total,
            by_category,
            by_severity,
        })
    }
}
