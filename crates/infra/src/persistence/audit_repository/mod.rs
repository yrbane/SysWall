//! Dépôt d'audit SQLite : struct, constructeur et implémentation du trait AuditRepository.
//! SQLite audit repository: struct, constructor, and AuditRepository trait implementation.

mod queries;
mod writes;

use async_trait::async_trait;
use std::sync::Arc;

use syswall_domain::entities::{AuditEvent, AuditStats};
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{AuditFilters, AuditRepository};

use super::database::Database;

/// SQLite-backed implementation of the audit repository.
/// Implémentation du dépôt d'audit adossée à SQLite.
pub struct SqliteAuditRepository {
    pub(super) db: Arc<Database>,
}

impl SqliteAuditRepository {
    /// Create a new repository backed by the given database.
    /// Crée un nouveau dépôt adossé à la base de données donnée.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AuditRepository for SqliteAuditRepository {
    async fn append(&self, event: &AuditEvent) -> Result<(), DomainError> {
        self.run_append(event.clone()).await
    }

    async fn query(
        &self,
        filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError> {
        self.run_query(filters.clone(), pagination.offset, pagination.limit).await
    }

    async fn count(&self, filters: &AuditFilters) -> Result<u64, DomainError> {
        self.run_count(filters.clone()).await
    }

    async fn append_batch(&self, events: &[AuditEvent]) -> Result<(), DomainError> {
        self.run_append_batch(events.to_vec()).await
    }

    async fn delete_before(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, DomainError> {
        self.run_delete_before(before).await
    }

    async fn get_stats(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<AuditStats, DomainError> {
        self.run_get_stats(from, to).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use syswall_domain::entities::{EventCategory, Severity};
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

    #[tokio::test]
    async fn query_filter_by_severity() {
        let repo = setup().await;

        repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "info event"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Error, EventCategory::Rule, "error event"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Warning, EventCategory::Rule, "warning event"))
            .await
            .unwrap();

        let filters = AuditFilters {
            severity: Some(Severity::Error),
            ..Default::default()
        };
        let results = repo.query(&filters, &Pagination::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].description, "error event");
    }

    #[tokio::test]
    async fn query_filter_by_category() {
        let repo = setup().await;

        repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "rule event"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Info, EventCategory::System, "system event"))
            .await
            .unwrap();

        let filters = AuditFilters {
            category: Some(EventCategory::System),
            ..Default::default()
        };
        let results = repo.query(&filters, &Pagination::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].description, "system event");
    }

    #[tokio::test]
    async fn query_filter_by_search() {
        let repo = setup().await;

        repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "Rule created: Block SSH"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "Rule deleted: Allow DNS"))
            .await
            .unwrap();

        let filters = AuditFilters {
            search: Some("SSH".to_string()),
            ..Default::default()
        };
        let results = repo.query(&filters, &Pagination::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("SSH"));
    }

    #[tokio::test]
    async fn query_filter_by_date_range() {
        let repo = setup().await;
        let now = Utc::now();

        // Insert events at different times
        let mut old_event = AuditEvent::new(Severity::Info, EventCategory::System, "old");
        old_event.timestamp = now - Duration::hours(5);
        repo.append(&old_event).await.unwrap();

        let mut recent_event = AuditEvent::new(Severity::Info, EventCategory::System, "recent");
        recent_event.timestamp = now - Duration::minutes(10);
        repo.append(&recent_event).await.unwrap();

        let filters = AuditFilters {
            from: Some(now - Duration::hours(1)),
            ..Default::default()
        };
        let results = repo.query(&filters, &Pagination::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].description, "recent");
    }

    #[tokio::test]
    async fn count_with_filters() {
        let repo = setup().await;

        repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "rule 1"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Error, EventCategory::System, "system error"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "rule 2"))
            .await
            .unwrap();

        let filters = AuditFilters {
            severity: Some(Severity::Info),
            ..Default::default()
        };
        let count = repo.count(&filters).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn combined_filters() {
        let repo = setup().await;

        repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "Rule created: SSH"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Error, EventCategory::Rule, "Rule error: SSH"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Info, EventCategory::System, "System SSH"))
            .await
            .unwrap();

        let filters = AuditFilters {
            severity: Some(Severity::Info),
            category: Some(EventCategory::Rule),
            search: Some("SSH".to_string()),
            ..Default::default()
        };
        let results = repo.query(&filters, &Pagination::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].description, "Rule created: SSH");
    }

    #[tokio::test]
    async fn batch_append_empty() {
        let repo = setup().await;
        repo.append_batch(&[]).await.unwrap();
        let count = repo.count(&AuditFilters::default()).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn batch_append_multiple() {
        let repo = setup().await;
        let events: Vec<AuditEvent> = (0..50)
            .map(|i| AuditEvent::new(Severity::Info, EventCategory::System, format!("batch {}", i)))
            .collect();

        repo.append_batch(&events).await.unwrap();

        let count = repo.count(&AuditFilters::default()).await.unwrap();
        assert_eq!(count, 50);
    }

    #[tokio::test]
    async fn delete_before_removes_old_events() {
        let repo = setup().await;
        let now = Utc::now();

        let mut old_event = AuditEvent::new(Severity::Info, EventCategory::System, "old");
        old_event.timestamp = now - Duration::days(10);
        repo.append(&old_event).await.unwrap();

        let mut recent_event = AuditEvent::new(Severity::Info, EventCategory::System, "recent");
        recent_event.timestamp = now;
        repo.append(&recent_event).await.unwrap();

        let deleted = repo
            .delete_before(now - Duration::days(5))
            .await
            .unwrap();
        assert_eq!(deleted, 1);

        let remaining = repo.count(&AuditFilters::default()).await.unwrap();
        assert_eq!(remaining, 1);
    }

    #[tokio::test]
    async fn delete_before_past_deletes_nothing() {
        let repo = setup().await;
        repo.append(&AuditEvent::new(Severity::Info, EventCategory::System, "event"))
            .await
            .unwrap();

        let deleted = repo
            .delete_before(Utc::now() - Duration::days(365))
            .await
            .unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn get_stats_returns_aggregates() {
        let repo = setup().await;
        let now = Utc::now();

        repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "rule 1"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "rule 2"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Error, EventCategory::System, "error"))
            .await
            .unwrap();
        repo.append(&AuditEvent::new(Severity::Warning, EventCategory::Connection, "conn"))
            .await
            .unwrap();

        let stats = repo
            .get_stats(now - Duration::hours(1), now + Duration::hours(1))
            .await
            .unwrap();

        assert_eq!(stats.total, 4);
        assert_eq!(*stats.by_category.get("Rule").unwrap(), 2);
        assert_eq!(*stats.by_category.get("System").unwrap(), 1);
        assert_eq!(*stats.by_category.get("Connection").unwrap(), 1);
        assert_eq!(*stats.by_severity.get("Info").unwrap(), 2);
        assert_eq!(*stats.by_severity.get("Error").unwrap(), 1);
        assert_eq!(*stats.by_severity.get("Warning").unwrap(), 1);
    }

    #[tokio::test]
    async fn get_stats_empty_range() {
        let repo = setup().await;
        let now = Utc::now();

        repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "event"))
            .await
            .unwrap();

        // Query a range far in the past
        let stats = repo
            .get_stats(
                now - Duration::days(365),
                now - Duration::days(364),
            )
            .await
            .unwrap();

        assert_eq!(stats.total, 0);
        assert!(stats.by_category.is_empty());
        assert!(stats.by_severity.is_empty());
    }
}
