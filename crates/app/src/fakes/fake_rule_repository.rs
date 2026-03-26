use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use syswall_domain::entities::{Rule, RuleId};
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{RuleFilters, RuleRepository};

/// In-memory fake rule repository for testing.
/// Dépôt de règles factice en mémoire pour les tests.
pub struct FakeRuleRepository {
    rules: Mutex<HashMap<RuleId, Rule>>,
}

impl FakeRuleRepository {
    pub fn new() -> Self {
        Self {
            rules: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl RuleRepository for FakeRuleRepository {
    async fn save(&self, rule: &Rule) -> Result<(), DomainError> {
        self.rules.lock().unwrap().insert(rule.id, rule.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &RuleId) -> Result<Option<Rule>, DomainError> {
        Ok(self.rules.lock().unwrap().get(id).cloned())
    }

    async fn find_all(
        &self,
        _filters: &RuleFilters,
        pagination: &Pagination,
    ) -> Result<Vec<Rule>, DomainError> {
        let rules = self.rules.lock().unwrap();
        let mut all: Vec<Rule> = rules.values().cloned().collect();
        all.sort_by_key(|r| r.priority);
        let start = pagination.offset as usize;
        let end = (start + pagination.limit as usize).min(all.len());
        if start >= all.len() {
            return Ok(vec![]);
        }
        Ok(all[start..end].to_vec())
    }

    async fn delete(&self, id: &RuleId) -> Result<(), DomainError> {
        self.rules.lock().unwrap().remove(id);
        Ok(())
    }

    async fn list_enabled_ordered(&self) -> Result<Vec<Rule>, DomainError> {
        let rules = self.rules.lock().unwrap();
        let mut enabled: Vec<Rule> = rules.values().filter(|r| r.enabled).cloned().collect();
        enabled.sort_by_key(|r| r.priority);
        Ok(enabled)
    }
}
