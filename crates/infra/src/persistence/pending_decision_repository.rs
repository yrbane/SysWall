use async_trait::async_trait;
use rusqlite::OptionalExtension;
use std::sync::Arc;

use syswall_domain::entities::{
    ConnectionSnapshot, PendingDecision, PendingDecisionId, PendingDecisionStatus,
};
use syswall_domain::errors::DomainError;
use syswall_domain::ports::PendingDecisionRepository;

use super::database::Database;

/// SQLite-backed implementation of the pending decision repository.
/// Implémentation du dépôt de décisions en attente adossée à SQLite.
pub struct SqlitePendingDecisionRepository {
    db: Arc<Database>,
}

impl SqlitePendingDecisionRepository {
    /// Create a new repository backed by the given database.
    /// Crée un nouveau dépôt adossé à la base de données donnée.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn row_to_pending_decision(row: &rusqlite::Row) -> Result<PendingDecision, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let snapshot_json: String = row.get(1)?;
        let requested_at_str: String = row.get(2)?;
        let expires_at_str: String = row.get(3)?;
        let deduplication_key: String = row.get(4)?;
        let status_str: String = row.get(5)?;

        let status = match status_str.as_str() {
            "Pending" => PendingDecisionStatus::Pending,
            "Resolved" => PendingDecisionStatus::Resolved,
            "Expired" => PendingDecisionStatus::Expired,
            "Cancelled" => PendingDecisionStatus::Cancelled,
            _ => PendingDecisionStatus::Pending,
        };

        Ok(PendingDecision {
            id: PendingDecisionId::from_uuid(id_str.parse().unwrap()),
            connection_snapshot: serde_json::from_str::<ConnectionSnapshot>(&snapshot_json)
                .unwrap(),
            requested_at: requested_at_str.parse().unwrap(),
            expires_at: expires_at_str.parse().unwrap(),
            deduplication_key,
            status,
        })
    }
}

#[async_trait]
impl PendingDecisionRepository for SqlitePendingDecisionRepository {
    async fn create(&self, request: &PendingDecision) -> Result<(), DomainError> {
        let request = request.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                conn.execute(
                    "INSERT INTO pending_decisions (id, snapshot_json, requested_at, expires_at, deduplication_key, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        request.id.as_uuid().to_string(),
                        serde_json::to_string(&request.connection_snapshot).unwrap(),
                        request.requested_at.to_rfc3339(),
                        request.expires_at.to_rfc3339(),
                        request.deduplication_key,
                        format!("{:?}", request.status),
                    ],
                )
                .map_err(|e| {
                    DomainError::Infrastructure(format!("Failed to create pending decision: {}", e))
                })?;
                Ok(())
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn list_pending(&self) -> Result<Vec<PendingDecision>, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let mut stmt = conn
                    .prepare("SELECT id, snapshot_json, requested_at, expires_at, deduplication_key, status FROM pending_decisions WHERE status = 'Pending'")
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let results = stmt
                    .query_map([], Self::row_to_pending_decision)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(results)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn resolve(&self, id: &PendingDecisionId) -> Result<(), DomainError> {
        let id_str = id.as_uuid().to_string();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                conn.execute(
                    "UPDATE pending_decisions SET status = 'Resolved' WHERE id = ?1",
                    rusqlite::params![id_str],
                )
                .map_err(|e| {
                    DomainError::Infrastructure(format!(
                        "Failed to resolve pending decision: {}",
                        e
                    ))
                })?;
                Ok(())
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn expire_overdue(&self) -> Result<Vec<PendingDecision>, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let now = chrono::Utc::now().to_rfc3339();

                // First, fetch the overdue pending decisions
                let mut stmt = conn
                    .prepare("SELECT id, snapshot_json, requested_at, expires_at, deduplication_key, status FROM pending_decisions WHERE status = 'Pending' AND expires_at < ?1")
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let expired: Vec<PendingDecision> = stmt
                    .query_map(rusqlite::params![now], Self::row_to_pending_decision)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                // Then update their status
                conn.execute(
                    "UPDATE pending_decisions SET status = 'Expired' WHERE status = 'Pending' AND expires_at < ?1",
                    rusqlite::params![now],
                )
                .map_err(|e| DomainError::Infrastructure(format!("Failed to expire overdue: {}", e)))?;

                Ok(expired)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn find_by_dedup_key(
        &self,
        key: &str,
    ) -> Result<Option<PendingDecision>, DomainError> {
        let key = key.to_string();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let mut stmt = conn
                    .prepare("SELECT id, snapshot_json, requested_at, expires_at, deduplication_key, status FROM pending_decisions WHERE deduplication_key = ?1 AND status = 'Pending' LIMIT 1")
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let result = stmt
                    .query_row(rusqlite::params![key], Self::row_to_pending_decision)
                    .optional()
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                Ok(result)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::value_objects::*;

    fn test_snapshot() -> ConnectionSnapshot {
        ConnectionSnapshot {
            protocol: Protocol::Tcp,
            source: SocketAddress::new("10.0.0.1".parse().unwrap(), Port::new(5000).unwrap()),
            destination: SocketAddress::new("8.8.8.8".parse().unwrap(), Port::new(443).unwrap()),
            direction: Direction::Outbound,
            process_name: Some("curl".to_string()),
            process_path: None,
            user: Some("seb".to_string()),
            hostname: None,
        }
    }

    fn test_pending_decision() -> PendingDecision {
        PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            deduplication_key: "curl:8.8.8.8:443:tcp".to_string(),
            status: PendingDecisionStatus::Pending,
        }
    }

    async fn setup() -> SqlitePendingDecisionRepository {
        let db = Arc::new(Database::open_in_memory().unwrap());
        SqlitePendingDecisionRepository::new(db)
    }

    #[tokio::test]
    async fn create_and_list_pending() {
        let repo = setup().await;
        let pd = test_pending_decision();
        repo.create(&pd).await.unwrap();

        let pending = repo.list_pending().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].deduplication_key, "curl:8.8.8.8:443:tcp");
    }

    #[tokio::test]
    async fn resolve_removes_from_pending_list() {
        let repo = setup().await;
        let pd = test_pending_decision();
        repo.create(&pd).await.unwrap();
        repo.resolve(&pd.id).await.unwrap();

        let pending = repo.list_pending().await.unwrap();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn expire_overdue_marks_expired() {
        let repo = setup().await;
        let mut pd = test_pending_decision();
        pd.expires_at = Utc::now() - chrono::Duration::minutes(1);
        repo.create(&pd).await.unwrap();

        let expired = repo.expire_overdue().await.unwrap();
        assert_eq!(expired.len(), 1);

        let pending = repo.list_pending().await.unwrap();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn find_by_dedup_key_returns_pending() {
        let repo = setup().await;
        let pd = test_pending_decision();
        repo.create(&pd).await.unwrap();

        let found = repo
            .find_by_dedup_key("curl:8.8.8.8:443:tcp")
            .await
            .unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn find_by_dedup_key_returns_none_for_unknown() {
        let repo = setup().await;
        let found = repo.find_by_dedup_key("unknown:key").await.unwrap();
        assert!(found.is_none());
    }
}
