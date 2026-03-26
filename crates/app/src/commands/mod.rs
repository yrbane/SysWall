use syswall_domain::entities::{
    DecisionAction, DecisionGranularity, PendingDecisionId, RuleCriteria, RuleEffect, RuleId,
    RuleScope, RuleSource,
};

/// Command to create a new rule.
/// Commande pour créer une nouvelle règle.
#[derive(Debug, Clone)]
pub struct CreateRuleCommand {
    pub name: String,
    pub priority: u32,
    pub criteria: RuleCriteria,
    pub effect: RuleEffect,
    pub scope: RuleScope,
    pub source: RuleSource,
}

/// Command to update an existing rule.
/// Commande pour mettre à jour une règle existante.
#[derive(Debug, Clone)]
pub struct UpdateRuleCommand {
    pub id: RuleId,
    pub name: Option<String>,
    pub priority: Option<u32>,
    pub criteria: Option<RuleCriteria>,
    pub effect: Option<RuleEffect>,
    pub scope: Option<RuleScope>,
    pub enabled: Option<bool>,
}

/// Command to respond to a pending decision.
/// Commande pour répondre à une décision en attente.
#[derive(Debug, Clone)]
pub struct RespondToDecisionCommand {
    pub pending_decision_id: PendingDecisionId,
    pub action: DecisionAction,
    pub granularity: DecisionGranularity,
}
