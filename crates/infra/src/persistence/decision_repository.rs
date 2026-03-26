use async_trait::async_trait;
use std::sync::Arc;

use syswall_domain::entities::Decision;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::DecisionRepository;

use super::database::Database;

/// SQLite-backed implementation of the decision repository.
/// Implémentation du dépôt de décisions adossée à SQLite.
pub struct SqliteDecisionRepository {
    db: Arc<Database>,
}

impl SqliteDecisionRepository {
    /// Create a new repository backed by the given database.
    /// Crée un nouveau dépôt adossé à la base de données donnée.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DecisionRepository for SqliteDecisionRepository {
    async fn save(&self, decision: &Decision) -> Result<(), DomainError> {
        let decision = decision.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO decisions (id, pending_decision_id, snapshot_json, action, granularity, decided_at, generated_rule_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        decision.id.as_uuid().to_string(),
                        decision.pending_decision_id.as_uuid().to_string(),
                        serde_json::to_string(&decision.connection_snapshot).unwrap(),
                        serde_json::to_string(&decision.action).unwrap().trim_matches('"'),
                        serde_json::to_string(&decision.granularity).unwrap().trim_matches('"'),
                        decision.decided_at.to_rfc3339(),
                        decision.generated_rule.map(|r| r.as_uuid().to_string()),
                    ],
                )
                .map_err(|e| {
                    DomainError::Infrastructure(format!("Failed to save decision: {}", e))
                })?;
                Ok(())
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
    use syswall_domain::entities::*;
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
        }
    }

    fn test_decision() -> Decision {
        Decision {
            id: DecisionId::new(),
            pending_decision_id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            action: DecisionAction::AllowOnce,
            granularity: DecisionGranularity::AppOnly,
            decided_at: Utc::now(),
            generated_rule: None,
        }
    }

    async fn setup() -> SqliteDecisionRepository {
        let db = Arc::new(Database::open_in_memory().unwrap());
        SqliteDecisionRepository::new(db)
    }

    #[tokio::test]
    async fn save_decision_succeeds() {
        let repo = setup().await;
        let decision = test_decision();
        let result = repo.save(&decision).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn save_decision_with_generated_rule() {
        let repo = setup().await;
        let mut decision = test_decision();
        decision.generated_rule = Some(RuleId::new());
        let result = repo.save(&decision).await;
        assert!(result.is_ok());
    }
}
