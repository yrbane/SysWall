use std::sync::Arc;

use syswall_domain::entities::{AuditEvent, EventCategory, Severity};
use syswall_domain::errors::DomainError;
use syswall_domain::events::{DomainEvent, Pagination};
use syswall_domain::ports::{AuditFilters, AuditRepository};

/// Service for recording and querying audit events.
/// Service d'enregistrement et de consultation des événements d'audit.
pub struct AuditService {
    audit_repo: Arc<dyn AuditRepository>,
}

impl AuditService {
    pub fn new(audit_repo: Arc<dyn AuditRepository>) -> Self {
        Self { audit_repo }
    }

    /// Convert a domain event into an audit event and persist it.
    /// Convertit un événement du domaine en événement d'audit et le persiste.
    pub async fn record_event(&self, event: &DomainEvent) -> Result<(), DomainError> {
        let audit_event = match event {
            DomainEvent::ConnectionDetected(conn) => AuditEvent::new(
                Severity::Debug,
                EventCategory::Connection,
                format!(
                    "Connection detected: {} -> {}",
                    conn.source, conn.destination
                ),
            ),
            DomainEvent::RuleCreated(rule) => AuditEvent::new(
                Severity::Info,
                EventCategory::Rule,
                format!("Rule created: {}", rule.name),
            )
            .with_metadata("rule_id", rule.id.as_uuid().to_string()),
            DomainEvent::RuleDeleted(id) => AuditEvent::new(
                Severity::Info,
                EventCategory::Rule,
                format!("Rule deleted: {:?}", id),
            ),
            DomainEvent::DecisionResolved(decision) => AuditEvent::new(
                Severity::Info,
                EventCategory::Decision,
                format!("Decision resolved: {:?}", decision.action),
            ),
            DomainEvent::DecisionExpired(id) => AuditEvent::new(
                Severity::Warning,
                EventCategory::Decision,
                format!("Decision expired: {:?}", id),
            ),
            DomainEvent::SystemError { message, severity } => {
                AuditEvent::new(*severity, EventCategory::System, message.clone())
            }
            _ => return Ok(()), // Not all events need audit records
        };

        self.audit_repo.append(&audit_event).await
    }

    /// Query audit events with optional filters and pagination.
    /// Interroge les événements d'audit avec filtres et pagination optionnels.
    pub async fn query_events(
        &self,
        filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError> {
        self.audit_repo.query(filters, pagination).await
    }

    /// Count audit events matching the given filters.
    /// Compte les événements d'audit correspondant aux filtres donnés.
    pub async fn count_events(&self, filters: &AuditFilters) -> Result<u64, DomainError> {
        self.audit_repo.count(filters).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::*;
    use chrono::Utc;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    #[tokio::test]
    async fn records_rule_created_event() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        let rule = Rule {
            id: RuleId::new(),
            name: "Test Rule".to_string(),
            priority: RulePriority::new(10),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        };

        service
            .record_event(&DomainEvent::RuleCreated(rule))
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].category, EventCategory::Rule);
        assert_eq!(events[0].severity, Severity::Info);
    }

    #[tokio::test]
    async fn records_system_error_with_severity() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::SystemError {
                message: "nftables sync failed".to_string(),
                severity: Severity::Error,
            })
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, Severity::Error);
    }

    #[tokio::test]
    async fn skips_unhandled_event_types() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        // RuleUpdated is not handled and should be silently ignored
        let rule = Rule {
            id: RuleId::new(),
            name: "Test".to_string(),
            priority: RulePriority::new(10),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        };

        service
            .record_event(&DomainEvent::RuleUpdated(rule))
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 0);
    }
}
