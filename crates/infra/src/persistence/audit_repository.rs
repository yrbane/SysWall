use async_trait::async_trait;
use std::sync::Arc;

use syswall_domain::entities::{AuditEvent, EventCategory, EventId, Severity};
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{AuditFilters, AuditRepository};

use super::database::Database;

/// SQLite-backed implementation of the audit repository.
/// Implémentation du dépôt d'audit adossée à SQLite.
pub struct SqliteAuditRepository {
    db: Arc<Database>,
}

impl SqliteAuditRepository {
    /// Create a new repository backed by the given database.
    /// Crée un nouveau dépôt adossé à la base de données donnée.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn row_to_audit_event(row: &rusqlite::Row) -> Result<AuditEvent, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let timestamp_str: String = row.get(1)?;
        let severity_str: String = row.get(2)?;
        let category_str: String = row.get(3)?;
        let description: String = row.get(4)?;
        let metadata_json: String = row.get(5)?;

        let severity = serde_json::from_str::<Severity>(&format!("\"{}\"", severity_str))
            .unwrap_or(Severity::Info);
        let category = serde_json::from_str::<EventCategory>(&format!("\"{}\"", category_str))
            .unwrap_or(EventCategory::System);

        Ok(AuditEvent {
            id: EventId::from_uuid(id_str.parse().unwrap()),
            timestamp: timestamp_str.parse().unwrap(),
            severity,
            category,
            description,
            metadata: serde_json::from_str(&metadata_json).unwrap_or_default(),
        })
    }
}

#[async_trait]
impl AuditRepository for SqliteAuditRepository {
    async fn append(&self, event: &AuditEvent) -> Result<(), DomainError> {
        let event = event.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                conn.execute(
                    "INSERT INTO audit_events (id, timestamp, severity, category, description, metadata_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        event.id.as_uuid().to_string(),
                        event.timestamp.to_rfc3339(),
                        serde_json::to_string(&event.severity).unwrap().trim_matches('"'),
                        serde_json::to_string(&event.category).unwrap().trim_matches('"'),
                        event.description,
                        serde_json::to_string(&event.metadata).unwrap(),
                    ],
                )
                .map_err(|e| {
                    DomainError::Infrastructure(format!("Failed to append audit event: {}", e))
                })?;
                Ok(())
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn query(
        &self,
        _filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError> {
        let offset = pagination.offset;
        let limit = pagination.limit;
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let mut stmt = conn
                    .prepare("SELECT id, timestamp, severity, category, description, metadata_json FROM audit_events ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2")
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let events = stmt
                    .query_map(rusqlite::params![limit, offset], Self::row_to_audit_event)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(events)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn count(&self, _filters: &AuditFilters) -> Result<u64, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM audit_events",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                Ok(count as u64)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syswall_domain::events::Pagination;

    async fn setup() -> SqliteAuditRepository {
        let db = Arc::new(Database::open_in_memory().unwrap());
        SqliteAuditRepository::new(db)
    }

    #[tokio::test]
    async fn append_and_query() {
        let repo = setup().await;
        let event = AuditEvent::new(Severity::Info, EventCategory::Rule, "Rule created")
            .with_metadata("rule_id", "abc-123");

        repo.append(&event).await.unwrap();

        let results = repo
            .query(&AuditFilters::default(), &Pagination::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].description, "Rule created");
        assert_eq!(results[0].metadata.get("rule_id").unwrap(), "abc-123");
    }

    #[tokio::test]
    async fn count_returns_correct_number() {
        let repo = setup().await;

        let e1 = AuditEvent::new(Severity::Info, EventCategory::Rule, "Event 1");
        let e2 = AuditEvent::new(Severity::Warning, EventCategory::System, "Event 2");
        let e3 = AuditEvent::new(Severity::Error, EventCategory::Connection, "Event 3");

        repo.append(&e1).await.unwrap();
        repo.append(&e2).await.unwrap();
        repo.append(&e3).await.unwrap();

        let count = repo.count(&AuditFilters::default()).await.unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn query_with_pagination() {
        let repo = setup().await;

        for i in 0..5 {
            let event = AuditEvent::new(
                Severity::Info,
                EventCategory::System,
                format!("Event {}", i),
            );
            repo.append(&event).await.unwrap();
        }

        let page = Pagination {
            offset: 0,
            limit: 2,
        };
        let results = repo.query(&AuditFilters::default(), &page).await.unwrap();
        assert_eq!(results.len(), 2);
    }
}
