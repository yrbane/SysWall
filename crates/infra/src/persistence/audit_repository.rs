use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use syswall_domain::entities::{AuditEvent, AuditStats, EventCategory, EventId, Severity};
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

    /// Build the WHERE clause and collect parameters from AuditFilters.
    /// Construit la clause WHERE et collecte les paramètres depuis AuditFilters.
    fn build_where_clause(
        filters: &AuditFilters,
    ) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
        let mut clauses: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref severity) = filters.severity {
            clauses.push("severity = ?".to_string());
            params.push(Box::new(
                serde_json::to_string(severity)
                    .unwrap()
                    .trim_matches('"')
                    .to_string(),
            ));
        }
        if let Some(ref category) = filters.category {
            clauses.push("category = ?".to_string());
            params.push(Box::new(
                serde_json::to_string(category)
                    .unwrap()
                    .trim_matches('"')
                    .to_string(),
            ));
        }
        if let Some(ref search) = filters.search {
            clauses.push("description LIKE ?".to_string());
            params.push(Box::new(format!("%{}%", search)));
        }
        if let Some(ref from) = filters.from {
            clauses.push("timestamp >= ?".to_string());
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(ref to) = filters.to {
            clauses.push("timestamp <= ?".to_string());
            params.push(Box::new(to.to_rfc3339()));
        }

        let where_sql = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };

        (where_sql, params)
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
        filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError> {
        let filters = filters.clone();
        let offset = pagination.offset;
        let limit = pagination.limit;
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let (where_sql, mut params) = Self::build_where_clause(&filters);
                let sql = format!(
                    "SELECT id, timestamp, severity, category, description, metadata_json \
                     FROM audit_events{} ORDER BY timestamp DESC LIMIT ? OFFSET ?",
                    where_sql
                );
                params.push(Box::new(limit as i64));
                params.push(Box::new(offset as i64));

                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();

                let mut stmt = conn
                    .prepare(&sql)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let events = stmt
                    .query_map(param_refs.as_slice(), Self::row_to_audit_event)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(events)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn count(&self, filters: &AuditFilters) -> Result<u64, DomainError> {
        let filters = filters.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let (where_sql, params) = Self::build_where_clause(&filters);
                let sql = format!("SELECT COUNT(*) FROM audit_events{}", where_sql);

                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();

                let count: i64 = conn
                    .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                Ok(count as u64)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn append_batch(&self, events: &[AuditEvent]) -> Result<(), DomainError> {
        let events = events.to_vec();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let tx = conn.unchecked_transaction().map_err(|e| {
                    DomainError::Infrastructure(format!("Failed to start transaction: {}", e))
                })?;
                {
                    let mut stmt = tx
                        .prepare_cached(
                            "INSERT INTO audit_events (id, timestamp, severity, category, description, metadata_json)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        )
                        .map_err(|e| {
                            DomainError::Infrastructure(format!("Failed to prepare statement: {}", e))
                        })?;
                    for event in &events {
                        stmt.execute(rusqlite::params![
                            event.id.as_uuid().to_string(),
                            event.timestamp.to_rfc3339(),
                            serde_json::to_string(&event.severity)
                                .unwrap()
                                .trim_matches('"'),
                            serde_json::to_string(&event.category)
                                .unwrap()
                                .trim_matches('"'),
                            event.description,
                            serde_json::to_string(&event.metadata).unwrap(),
                        ])
                        .map_err(|e| {
                            DomainError::Infrastructure(format!(
                                "Failed to insert batch event: {}",
                                e
                            ))
                        })?;
                    }
                }
                tx.commit().map_err(|e| {
                    DomainError::Infrastructure(format!("Failed to commit batch: {}", e))
                })?;
                Ok(())
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn delete_before(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let deleted = conn
                    .execute(
                        "DELETE FROM audit_events WHERE timestamp < ?1",
                        rusqlite::params![before.to_rfc3339()],
                    )
                    .map_err(|e| {
                        DomainError::Infrastructure(format!(
                            "Failed to delete old audit events: {}",
                            e
                        ))
                    })?;
                Ok(deleted as u64)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn get_stats(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<AuditStats, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let from_str = from.to_rfc3339();
                let to_str = to.to_rfc3339();

                // Total count in range
                let total: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM audit_events WHERE timestamp >= ?1 AND timestamp <= ?2",
                        rusqlite::params![from_str, to_str],
                        |row| row.get(0),
                    )
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                // Counts per category
                let mut stmt = conn
                    .prepare(
                        "SELECT category, COUNT(*) FROM audit_events \
                         WHERE timestamp >= ?1 AND timestamp <= ?2 \
                         GROUP BY category",
                    )
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                let by_category: HashMap<String, u64> = stmt
                    .query_map(rusqlite::params![from_str, to_str], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
                    })
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                // Counts per severity
                let mut stmt = conn
                    .prepare(
                        "SELECT severity, COUNT(*) FROM audit_events \
                         WHERE timestamp >= ?1 AND timestamp <= ?2 \
                         GROUP BY severity",
                    )
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                let by_severity: HashMap<String, u64> = stmt
                    .query_map(rusqlite::params![from_str, to_str], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
                    })
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(AuditStats {
                    total: total as u64,
                    by_category,
                    by_severity,
                })
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
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
