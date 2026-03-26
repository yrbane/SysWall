use async_trait::async_trait;
use rusqlite::OptionalExtension;
use std::sync::Arc;

use syswall_domain::entities::{Rule, RuleCriteria, RuleEffect, RuleId, RuleScope, RuleSource};
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{RuleFilters, RuleRepository};
use syswall_domain::value_objects::RulePriority;

use super::database::Database;

/// SQLite-backed implementation of the rule repository.
/// Implémentation du dépôt de règles adossée à SQLite.
pub struct SqliteRuleRepository {
    db: Arc<Database>,
}

impl SqliteRuleRepository {
    /// Create a new repository backed by the given database.
    /// Crée un nouveau dépôt adossé à la base de données donnée.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn row_to_rule(row: &rusqlite::Row) -> Result<Rule, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let name: String = row.get(1)?;
        let priority: u32 = row.get(2)?;
        let enabled: bool = row.get(3)?;
        let criteria_json: String = row.get(4)?;
        let effect_str: String = row.get(5)?;
        let scope_json: String = row.get(6)?;
        let source_str: String = row.get(7)?;
        let created_at_str: String = row.get(8)?;
        let updated_at_str: String = row.get(9)?;

        Ok(Rule {
            id: RuleId::from_uuid(id_str.parse().unwrap()),
            name,
            priority: RulePriority::new(priority),
            enabled,
            criteria: serde_json::from_str::<RuleCriteria>(&criteria_json).unwrap_or_default(),
            effect: serde_json::from_str::<RuleEffect>(&format!("\"{}\"", effect_str)).unwrap(),
            scope: serde_json::from_str::<RuleScope>(&scope_json).unwrap(),
            source: serde_json::from_str::<RuleSource>(&format!("\"{}\"", source_str)).unwrap(),
            created_at: created_at_str.parse().unwrap(),
            updated_at: updated_at_str.parse().unwrap(),
        })
    }
}

#[async_trait]
impl RuleRepository for SqliteRuleRepository {
    async fn save(&self, rule: &Rule) -> Result<(), DomainError> {
        let rule = rule.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO rules (id, name, priority, enabled, criteria_json, effect, scope_json, source, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    rusqlite::params![
                        rule.id.as_uuid().to_string(),
                        rule.name,
                        rule.priority.value(),
                        rule.enabled,
                        serde_json::to_string(&rule.criteria).unwrap(),
                        serde_json::to_string(&rule.effect).unwrap().trim_matches('"'),
                        serde_json::to_string(&rule.scope).unwrap(),
                        serde_json::to_string(&rule.source).unwrap().trim_matches('"'),
                        rule.created_at.to_rfc3339(),
                        rule.updated_at.to_rfc3339(),
                    ],
                )
                .map_err(|e| DomainError::Infrastructure(format!("Failed to save rule: {}", e)))?;
                Ok(())
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn find_by_id(&self, id: &RuleId) -> Result<Option<Rule>, DomainError> {
        let id_str = id.as_uuid().to_string();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let mut stmt = conn
                    .prepare("SELECT id, name, priority, enabled, criteria_json, effect, scope_json, source, created_at, updated_at FROM rules WHERE id = ?1")
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let result = stmt
                    .query_row(rusqlite::params![id_str], Self::row_to_rule)
                    .optional()
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                Ok(result)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn find_all(
        &self,
        _filters: &RuleFilters,
        pagination: &Pagination,
    ) -> Result<Vec<Rule>, DomainError> {
        let offset = pagination.offset;
        let limit = pagination.limit;
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let mut stmt = conn
                    .prepare("SELECT id, name, priority, enabled, criteria_json, effect, scope_json, source, created_at, updated_at FROM rules ORDER BY priority ASC LIMIT ?1 OFFSET ?2")
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let rules = stmt
                    .query_map(rusqlite::params![limit, offset], Self::row_to_rule)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(rules)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn delete(&self, id: &RuleId) -> Result<(), DomainError> {
        let id_str = id.as_uuid().to_string();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                conn.execute("DELETE FROM rules WHERE id = ?1", rusqlite::params![id_str])
                    .map_err(|e| DomainError::Infrastructure(format!("Failed to delete rule: {}", e)))?;
                Ok(())
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn list_enabled_ordered(&self) -> Result<Vec<Rule>, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let mut stmt = conn
                    .prepare("SELECT id, name, priority, enabled, criteria_json, effect, scope_json, source, created_at, updated_at FROM rules WHERE enabled = 1 ORDER BY priority ASC")
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let rules = stmt
                    .query_map([], Self::row_to_rule)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(rules)
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

    async fn setup() -> (SqliteRuleRepository, Arc<Database>) {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let repo = SqliteRuleRepository::new(db.clone());
        (repo, db)
    }

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

    #[tokio::test]
    async fn save_then_find_by_id() {
        let (repo, _) = setup().await;
        let rule = test_rule();
        repo.save(&rule).await.unwrap();

        let found = repo.find_by_id(&rule.id).await.unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.name, "Test Rule");
        assert_eq!(found.priority, RulePriority::new(10));
        assert_eq!(found.effect, RuleEffect::Allow);
    }

    #[tokio::test]
    async fn delete_then_find_by_id_returns_none() {
        let (repo, _) = setup().await;
        let rule = test_rule();
        repo.save(&rule).await.unwrap();
        repo.delete(&rule.id).await.unwrap();

        let found = repo.find_by_id(&rule.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn list_enabled_ordered_by_priority() {
        let (repo, _) = setup().await;

        let mut r1 = test_rule();
        r1.priority = RulePriority::new(20);
        r1.name = "Second".to_string();

        let mut r2 = test_rule();
        r2.priority = RulePriority::new(5);
        r2.name = "First".to_string();

        let mut r3 = test_rule();
        r3.priority = RulePriority::new(10);
        r3.enabled = false;
        r3.name = "Disabled".to_string();

        repo.save(&r1).await.unwrap();
        repo.save(&r2).await.unwrap();
        repo.save(&r3).await.unwrap();

        let enabled = repo.list_enabled_ordered().await.unwrap();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0].name, "First");
        assert_eq!(enabled[1].name, "Second");
    }

    #[tokio::test]
    async fn find_by_id_nonexistent_returns_none() {
        let (repo, _) = setup().await;
        let found = repo.find_by_id(&RuleId::new()).await.unwrap();
        assert!(found.is_none());
    }
}
