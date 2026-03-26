use std::sync::Arc;

use chrono::Utc;
use syswall_domain::entities::{Rule, RuleId};
use syswall_domain::errors::DomainError;
use syswall_domain::events::{DomainEvent, Pagination};
use syswall_domain::ports::{EventBus, FirewallEngine, RuleFilters, RuleRepository};
use syswall_domain::value_objects::RulePriority;

use crate::commands::CreateRuleCommand;

/// Service for managing firewall rules (CRUD + firewall sync).
/// Service de gestion des règles de pare-feu (CRUD + synchronisation pare-feu).
pub struct RuleService {
    rule_repo: Arc<dyn RuleRepository>,
    firewall: Arc<dyn FirewallEngine>,
    event_bus: Arc<dyn EventBus>,
}

impl RuleService {
    pub fn new(
        rule_repo: Arc<dyn RuleRepository>,
        firewall: Arc<dyn FirewallEngine>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            rule_repo,
            firewall,
            event_bus,
        }
    }

    /// Create a new rule, persist it, apply it to the firewall, and publish an event.
    /// Crée une nouvelle règle, la persiste, l'applique au pare-feu, et publie un événement.
    pub async fn create_rule(&self, cmd: CreateRuleCommand) -> Result<Rule, DomainError> {
        let now = Utc::now();
        let rule = Rule {
            id: RuleId::new(),
            name: cmd.name,
            priority: RulePriority::new(cmd.priority),
            enabled: true,
            criteria: cmd.criteria,
            effect: cmd.effect,
            scope: cmd.scope,
            created_at: now,
            updated_at: now,
            source: cmd.source,
        };

        self.rule_repo.save(&rule).await?;
        self.firewall.apply_rule(&rule).await?;
        let _ = self
            .event_bus
            .publish(DomainEvent::RuleCreated(rule.clone()))
            .await;

        Ok(rule)
    }

    /// Delete a rule. System rules cannot be deleted.
    /// Supprime une règle. Les règles système ne peuvent pas être supprimées.
    pub async fn delete_rule(&self, id: &RuleId) -> Result<(), DomainError> {
        let rule = self
            .rule_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound(format!("Rule {:?}", id)))?;

        if rule.is_system() {
            return Err(DomainError::NotPermitted(
                "System rules cannot be deleted".to_string(),
            ));
        }

        self.firewall.remove_rule(id).await?;
        self.rule_repo.delete(id).await?;
        let _ = self.event_bus.publish(DomainEvent::RuleDeleted(*id)).await;

        Ok(())
    }

    /// Enable or disable a rule and update the firewall accordingly.
    /// Active ou désactive une règle et met à jour le pare-feu en conséquence.
    pub async fn toggle_rule(&self, id: &RuleId, enabled: bool) -> Result<Rule, DomainError> {
        let mut rule = self
            .rule_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound(format!("Rule {:?}", id)))?;

        rule.enabled = enabled;
        rule.updated_at = Utc::now();
        self.rule_repo.save(&rule).await?;

        if enabled {
            self.firewall.apply_rule(&rule).await?;
        } else {
            self.firewall.remove_rule(id).await?;
        }

        let _ = self
            .event_bus
            .publish(DomainEvent::RuleUpdated(rule.clone()))
            .await;

        Ok(rule)
    }

    /// List rules with optional filters and pagination.
    /// Liste les règles avec filtres et pagination optionnels.
    pub async fn list_rules(
        &self,
        filters: &RuleFilters,
        pagination: &Pagination,
    ) -> Result<Vec<Rule>, DomainError> {
        self.rule_repo.find_all(filters, pagination).await
    }

    /// Get a single rule by ID.
    /// Récupère une seule règle par identifiant.
    pub async fn get_rule(&self, id: &RuleId) -> Result<Option<Rule>, DomainError> {
        self.rule_repo.find_by_id(id).await
    }

    /// List all enabled rules ordered by priority (for PolicyEngine evaluation).
    /// Liste toutes les règles activées triées par priorité (pour l'évaluation du PolicyEngine).
    pub async fn list_enabled_ordered(&self) -> Result<Vec<Rule>, DomainError> {
        self.rule_repo.list_enabled_ordered().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::*;
    use syswall_domain::entities::*;

    fn setup() -> (RuleService, Arc<FakeRuleRepository>, Arc<FakeFirewallEngine>) {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let service = RuleService::new(rule_repo.clone(), firewall.clone(), event_bus);
        (service, rule_repo, firewall)
    }

    #[tokio::test]
    async fn create_rule_persists_and_applies() {
        let (service, repo, firewall) = setup();
        let cmd = CreateRuleCommand {
            name: "Block SSH".to_string(),
            priority: 10,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Block,
            scope: RuleScope::Permanent,
            source: RuleSource::Manual,
        };

        let rule = service.create_rule(cmd).await.unwrap();
        assert_eq!(rule.name, "Block SSH");

        // Verify persisted
        let found = repo.find_by_id(&rule.id).await.unwrap();
        assert!(found.is_some());

        // Verify firewall was called
        let calls = firewall.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(matches!(calls[0], FirewallCall::ApplyRule(_)));
    }

    #[tokio::test]
    async fn delete_system_rule_rejected() {
        let (service, repo, _) = setup();
        let rule = Rule {
            id: RuleId::new(),
            name: "DNS".to_string(),
            priority: RulePriority::new(0),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::System,
        };
        repo.save(&rule).await.unwrap();

        let result = service.delete_rule(&rule.id).await;
        assert!(matches!(result, Err(DomainError::NotPermitted(_))));
    }

    #[tokio::test]
    async fn toggle_rule_updates_firewall() {
        let (service, _repo, firewall) = setup();
        let cmd = CreateRuleCommand {
            name: "Test".to_string(),
            priority: 10,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            source: RuleSource::Manual,
        };
        let rule = service.create_rule(cmd).await.unwrap();

        // Disable
        let updated = service.toggle_rule(&rule.id, false).await.unwrap();
        assert!(!updated.enabled);

        // Verify firewall remove was called
        let calls = firewall.calls.lock().unwrap();
        assert!(calls
            .iter()
            .any(|c| matches!(c, FirewallCall::RemoveRule(_))));
    }

    #[tokio::test]
    async fn delete_nonexistent_rule_returns_not_found() {
        let (service, _, _) = setup();
        let result = service.delete_rule(&RuleId::new()).await;
        assert!(matches!(result, Err(DomainError::NotFound(_))));
    }
}
