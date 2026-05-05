use std::sync::Arc;

use tokio::sync::Mutex;

use syswall_domain::entities::AuditEvent;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::AuditRepository;

mod command;
mod query;

/// Supported export formats for audit log data.
/// Formats d'export supportés pour les données du journal d'audit.
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    /// JSON format.
    /// Format JSON.
    Json,
}

/// A buffered writer that batches audit events before flushing to the repository.
/// Un écrivain avec tampon qui met en lot les événements d'audit avant de les écrire dans le dépôt.
pub struct BufferedAuditWriter {
    repo: Arc<dyn AuditRepository>,
    buffer: Mutex<Vec<AuditEvent>>,
    batch_size: usize,
}

impl BufferedAuditWriter {
    /// Create a new buffered writer with the given batch size threshold.
    /// Crée un nouvel écrivain avec tampon avec le seuil de taille de lot donné.
    pub fn new(repo: Arc<dyn AuditRepository>, batch_size: usize) -> Self {
        Self {
            repo,
            buffer: Mutex::new(Vec::new()),
            batch_size,
        }
    }

    /// Buffer an audit event. If buffer reaches batch_size, flush automatically.
    /// Met en tampon un événement d'audit. Si le tampon atteint batch_size, vidage automatique.
    pub async fn buffer_event(&self, event: AuditEvent) -> Result<(), DomainError> {
        let should_flush = {
            let mut buf = self.buffer.lock().await;
            buf.push(event);
            buf.len() >= self.batch_size
        };

        if should_flush {
            self.flush().await?;
        }

        Ok(())
    }

    /// Flush all buffered events to the repository.
    /// Vide tous les événements du tampon vers le dépôt.
    pub async fn flush(&self) -> Result<(), DomainError> {
        let events = {
            let mut buf = self.buffer.lock().await;
            std::mem::take(&mut *buf)
        };

        if !events.is_empty() {
            self.repo.append_batch(&events).await?;
        }

        Ok(())
    }

    /// Return current number of buffered events.
    /// Retourne le nombre actuel d'événements en tampon.
    pub async fn buffered_count(&self) -> usize {
        self.buffer.lock().await.len()
    }
}

/// Service for recording and querying audit events.
/// Service d'enregistrement et de consultation des événements d'audit.
pub struct AuditService {
    pub(super) audit_repo: Arc<dyn AuditRepository>,
}

impl AuditService {
    pub fn new(audit_repo: Arc<dyn AuditRepository>) -> Self {
        Self { audit_repo }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::*;
    use chrono::{Duration, Utc};
    use syswall_domain::entities::*;
    use syswall_domain::events::{DomainEvent, FirewallStatus};
    use syswall_domain::ports::AuditFilters;
    use syswall_domain::value_objects::*;

    fn test_rule() -> Rule {
        Rule {
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
        }
    }

    fn test_connection() -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol: Protocol::Tcp,
            source: SocketAddress::new("10.0.0.1".parse().unwrap(), Port::new(5000).unwrap()),
            destination: SocketAddress::new("8.8.8.8".parse().unwrap(), Port::new(443).unwrap()),
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

    fn test_pending_decision() -> PendingDecision {
        PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: ConnectionSnapshot {
                protocol: Protocol::Tcp,
                source: SocketAddress::new("10.0.0.1".parse().unwrap(), Port::new(5000).unwrap()),
                destination: SocketAddress::new(
                    "8.8.8.8".parse().unwrap(),
                    Port::new(443).unwrap(),
                ),
                direction: Direction::Outbound,
                process_name: Some("curl".to_string()),
                process_path: None,
                user: Some("seb".to_string()),
                hostname: None,
            },
            requested_at: Utc::now(),
            expires_at: Utc::now() + Duration::minutes(5),
            deduplication_key: "curl:8.8.8.8:443:tcp".to_string(),
            status: PendingDecisionStatus::Pending,
        }
    }

    #[tokio::test]
    async fn records_rule_created_event() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::RuleCreated(test_rule()))
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
    async fn records_rule_updated_event() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::RuleUpdated(test_rule()))
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].category, EventCategory::Rule);
        assert!(events[0].description.contains("updated"));
    }

    #[tokio::test]
    async fn records_connection_closed_event() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::ConnectionClosed(ConnectionId::new()))
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].category, EventCategory::Connection);
        assert!(events[0].description.contains("closed"));
    }

    #[tokio::test]
    async fn records_rule_matched_event() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::RuleMatched {
                connection_id: ConnectionId::new(),
                rule_id: RuleId::new(),
                verdict: ConnectionVerdict::Allowed,
            })
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, Severity::Debug);
    }

    #[tokio::test]
    async fn records_decision_required_event() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::DecisionRequired(test_pending_decision()))
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].category, EventCategory::Decision);
        assert!(events[0].description.contains("curl"));
    }

    #[tokio::test]
    async fn records_firewall_status_changed_event() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::FirewallStatusChanged(FirewallStatus {
                enabled: true,
                active_rules_count: 5,
                nftables_synced: true,
                uptime_secs: 100,
                version: "0.1.0".to_string(),
            }))
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].category, EventCategory::System);
        assert!(events[0].description.contains("enabled=true"));
    }

    #[tokio::test]
    async fn records_connection_updated_event() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::ConnectionUpdated {
                id: ConnectionId::new(),
                state: ConnectionState::Established,
            })
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].category, EventCategory::Connection);
    }

    #[tokio::test]
    async fn records_all_domain_event_types() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        // All 11 event types should be recorded
        let events_to_record = vec![
            DomainEvent::ConnectionDetected(test_connection()),
            DomainEvent::ConnectionUpdated {
                id: ConnectionId::new(),
                state: ConnectionState::Established,
            },
            DomainEvent::ConnectionClosed(ConnectionId::new()),
            DomainEvent::RuleCreated(test_rule()),
            DomainEvent::RuleUpdated(test_rule()),
            DomainEvent::RuleDeleted(RuleId::new()),
            DomainEvent::RuleMatched {
                connection_id: ConnectionId::new(),
                rule_id: RuleId::new(),
                verdict: ConnectionVerdict::Allowed,
            },
            DomainEvent::DecisionRequired(test_pending_decision()),
            DomainEvent::DecisionResolved(Decision {
                id: DecisionId::new(),
                pending_decision_id: PendingDecisionId::new(),
                connection_snapshot: test_pending_decision().connection_snapshot,
                action: DecisionAction::AllowOnce,
                granularity: DecisionGranularity::AppOnly,
                decided_at: Utc::now(),
                generated_rule: None,
            }),
            DomainEvent::DecisionExpired(PendingDecisionId::new()),
            DomainEvent::FirewallStatusChanged(FirewallStatus {
                enabled: false,
                active_rules_count: 0,
                nftables_synced: false,
                uptime_secs: 0,
                version: "0.1.0".to_string(),
            }),
        ];

        for event in &events_to_record {
            service.record_event(event).await.unwrap();
        }

        let recorded = repo.events.lock().unwrap();
        assert_eq!(recorded.len(), events_to_record.len());
    }

    #[tokio::test]
    async fn get_stats_delegates_to_repo() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::RuleCreated(test_rule()))
            .await
            .unwrap();
        service
            .record_event(&DomainEvent::SystemError {
                message: "oops".to_string(),
                severity: Severity::Error,
            })
            .await
            .unwrap();

        let now = Utc::now();
        let stats = service
            .get_stats(now - Duration::hours(1), now + Duration::hours(1))
            .await
            .unwrap();

        assert_eq!(stats.total, 2);
        assert_eq!(*stats.by_category.get("Rule").unwrap(), 1);
        assert_eq!(*stats.by_category.get("System").unwrap(), 1);
    }

    #[tokio::test]
    async fn delete_before_delegates_to_repo() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::RuleCreated(test_rule()))
            .await
            .unwrap();

        // Delete everything before the future
        let deleted = service
            .delete_before(Utc::now() + Duration::hours(1))
            .await
            .unwrap();
        assert_eq!(deleted, 1);
    }

    #[tokio::test]
    async fn export_events_produces_valid_json() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::RuleCreated(test_rule()))
            .await
            .unwrap();

        let data = service
            .export_events(&AuditFilters::default(), ExportFormat::Json)
            .await
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&data).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn buffered_writer_flushes_at_threshold() {
        let repo = Arc::new(FakeAuditRepository::new());
        let writer = BufferedAuditWriter::new(repo.clone(), 3);

        // Buffer 2 events -- should not flush yet
        writer
            .buffer_event(AuditEvent::new(Severity::Info, EventCategory::Rule, "e1"))
            .await
            .unwrap();
        writer
            .buffer_event(AuditEvent::new(Severity::Info, EventCategory::Rule, "e2"))
            .await
            .unwrap();

        assert_eq!(writer.buffered_count().await, 2);
        assert_eq!(repo.events.lock().unwrap().len(), 0);

        // Buffer 3rd event -- should flush all
        writer
            .buffer_event(AuditEvent::new(Severity::Info, EventCategory::Rule, "e3"))
            .await
            .unwrap();

        assert_eq!(writer.buffered_count().await, 0);
        assert_eq!(repo.events.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn buffered_writer_manual_flush() {
        let repo = Arc::new(FakeAuditRepository::new());
        let writer = BufferedAuditWriter::new(repo.clone(), 100);

        writer
            .buffer_event(AuditEvent::new(Severity::Info, EventCategory::Rule, "e1"))
            .await
            .unwrap();
        writer
            .buffer_event(AuditEvent::new(Severity::Info, EventCategory::Rule, "e2"))
            .await
            .unwrap();

        assert_eq!(repo.events.lock().unwrap().len(), 0);

        writer.flush().await.unwrap();

        assert_eq!(repo.events.lock().unwrap().len(), 2);
        assert_eq!(writer.buffered_count().await, 0);
    }

    #[tokio::test]
    async fn buffered_writer_flush_empty_is_noop() {
        let repo = Arc::new(FakeAuditRepository::new());
        let writer = BufferedAuditWriter::new(repo.clone(), 100);

        writer.flush().await.unwrap();
        assert_eq!(repo.events.lock().unwrap().len(), 0);
    }
}
