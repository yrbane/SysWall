//! Audit write side: record, domain-event conversion, purge.
//! Cote ecriture audit : enregistrement, conversion d'evenements domaine, purge.

use syswall_domain::entities::{AuditEvent, EventCategory, Severity};
use syswall_domain::errors::DomainError;
use syswall_domain::events::DomainEvent;

use super::AuditService;

impl AuditService {
    /// Convert a domain event into an audit event and persist it.
    /// Convertit un événement du domaine en événement d'audit et le persiste.
    pub async fn record_event(&self, event: &DomainEvent) -> Result<(), DomainError> {
        let audit_event = Self::domain_event_to_audit(event);
        match audit_event {
            Some(ae) => self.audit_repo.append(&ae).await,
            None => Ok(()),
        }
    }

    /// Convert a domain event to an audit event (returns None if the event should not be recorded).
    /// Convertit un événement du domaine en événement d'audit (retourne None si l'événement ne doit pas être enregistré).
    pub fn domain_event_to_audit(event: &DomainEvent) -> Option<AuditEvent> {
        match event {
            DomainEvent::ConnectionDetected(conn) => Some(AuditEvent::new(
                Severity::Debug,
                EventCategory::Connection,
                format!(
                    "Connection detected: {} -> {}",
                    conn.source, conn.destination
                ),
            )),
            DomainEvent::ConnectionUpdated { id, state } => Some(AuditEvent::new(
                Severity::Debug,
                EventCategory::Connection,
                format!("Connection updated: {:?} state={:?}", id, state),
            )),
            DomainEvent::ConnectionClosed(id) => Some(AuditEvent::new(
                Severity::Debug,
                EventCategory::Connection,
                format!("Connection closed: {:?}", id),
            )),
            DomainEvent::RuleCreated(rule) => Some(
                AuditEvent::new(
                    Severity::Info,
                    EventCategory::Rule,
                    format!("Rule created: {}", rule.name),
                )
                .with_metadata("rule_id", rule.id.as_uuid().to_string()),
            ),
            DomainEvent::RuleUpdated(rule) => Some(
                AuditEvent::new(
                    Severity::Info,
                    EventCategory::Rule,
                    format!("Rule updated: {}", rule.name),
                )
                .with_metadata("rule_id", rule.id.as_uuid().to_string()),
            ),
            DomainEvent::RuleDeleted(id) => Some(AuditEvent::new(
                Severity::Info,
                EventCategory::Rule,
                format!("Rule deleted: {:?}", id),
            )),
            DomainEvent::RuleMatched {
                connection_id,
                rule_id,
                verdict,
            } => Some(AuditEvent::new(
                Severity::Debug,
                EventCategory::Rule,
                format!(
                    "Rule {:?} matched connection {:?}: {:?}",
                    rule_id, connection_id, verdict
                ),
            )),
            DomainEvent::DecisionRequired(pd) => Some(AuditEvent::new(
                Severity::Info,
                EventCategory::Decision,
                format!(
                    "Decision required for {} -> {}",
                    pd.connection_snapshot
                        .process_name
                        .as_deref()
                        .unwrap_or("unknown"),
                    pd.connection_snapshot.destination
                ),
            )),
            DomainEvent::DecisionResolved(decision) => Some(AuditEvent::new(
                Severity::Info,
                EventCategory::Decision,
                format!("Decision resolved: {:?}", decision.action),
            )),
            DomainEvent::DecisionExpired(id) => Some(AuditEvent::new(
                Severity::Warning,
                EventCategory::Decision,
                format!("Decision expired: {:?}", id),
            )),
            DomainEvent::FirewallStatusChanged(status) => Some(AuditEvent::new(
                Severity::Info,
                EventCategory::System,
                format!("Firewall status changed: enabled={}", status.enabled),
            )),
            DomainEvent::SystemError { message, severity } => Some(AuditEvent::new(
                *severity,
                EventCategory::System,
                message.clone(),
            )),
            // Anti-lockout est déjà audité directement par run_guard_loop — pas de doublon ici.
            // Anti-lockout is already audited directly by run_guard_loop — no duplicate here.
            DomainEvent::AntilockoutTriggered { .. } => None,
        }
    }

    /// Delete events older than the given timestamp. Returns count of deleted events.
    /// Supprime les événements antérieurs à l'horodatage donné. Retourne le nombre d'événements supprimés.
    pub async fn delete_before(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, DomainError> {
        self.audit_repo.delete_before(before).await
    }
}
