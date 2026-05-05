//! Evaluation logic: resolve which rule applies given matched candidates and priority.
//! Logique d'évaluation : résoudre quelle règle s'applique parmi les candidates et la priorité.

use crate::entities::{Connection, ConnectionVerdict, Rule, RuleEffect};
use crate::events::DefaultPolicy;

use super::{EvaluationReason, PolicyEngine, PolicyEvaluation};

impl PolicyEngine {
    /// Evaluate a connection against a list of rules (must be sorted by priority).
    /// Returns the evaluation result including verdict and reason.
    ///
    /// Évalue une connexion par rapport à une liste de règles (triées par priorité).
    /// Retourne le résultat de l'évaluation incluant le verdict et la raison.
    pub fn evaluate(
        connection: &Connection,
        rules: &[Rule],
        default_policy: DefaultPolicy,
    ) -> PolicyEvaluation {
        for rule in rules {
            if !rule.enabled || rule.is_expired() {
                continue;
            }
            if Self::matches(&rule.criteria, connection) {
                let verdict = match rule.effect {
                    RuleEffect::Allow => ConnectionVerdict::Allowed,
                    RuleEffect::Block => ConnectionVerdict::Blocked,
                    RuleEffect::Ask => ConnectionVerdict::PendingDecision,
                    RuleEffect::Observe => ConnectionVerdict::Ignored,
                };
                return PolicyEvaluation {
                    verdict,
                    matched_rule_id: Some(rule.id),
                    reason: EvaluationReason::MatchedRule {
                        rule_id: rule.id,
                        effect: rule.effect,
                    },
                };
            }
        }

        // No rule matched -- apply default policy
        // Aucune règle ne correspond -- appliquer la politique par défaut
        match default_policy {
            DefaultPolicy::Ask => PolicyEvaluation {
                verdict: ConnectionVerdict::PendingDecision,
                matched_rule_id: None,
                reason: EvaluationReason::NoMatchingRule,
            },
            DefaultPolicy::Allow => PolicyEvaluation {
                verdict: ConnectionVerdict::Allowed,
                matched_rule_id: None,
                reason: EvaluationReason::DefaultPolicyApplied {
                    policy: DefaultPolicy::Allow,
                },
            },
            DefaultPolicy::Block => PolicyEvaluation {
                verdict: ConnectionVerdict::Blocked,
                matched_rule_id: None,
                reason: EvaluationReason::DefaultPolicyApplied {
                    policy: DefaultPolicy::Block,
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::PolicyEngine;
    use super::super::matcher::tests::test_connection;
    use crate::entities::*;
    use crate::events::DefaultPolicy;
    use crate::value_objects::*;
    use chrono::Utc;

    fn make_rule(priority: u32, effect: RuleEffect, criteria: RuleCriteria) -> Rule {
        Rule {
            id: RuleId::new(),
            name: format!("Rule p{}", priority),
            priority: RulePriority::new(priority),
            enabled: true,
            criteria,
            effect,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        }
    }

    #[test]
    fn no_rules_default_ask() {
        let conn = test_connection();
        let result = PolicyEngine::evaluate(&conn, &[], DefaultPolicy::Ask);
        assert_eq!(result.verdict, ConnectionVerdict::PendingDecision);
        assert!(result.matched_rule_id.is_none());
    }

    #[test]
    fn no_rules_default_block() {
        let conn = test_connection();
        let result = PolicyEngine::evaluate(&conn, &[], DefaultPolicy::Block);
        assert_eq!(result.verdict, ConnectionVerdict::Blocked);
    }

    #[test]
    fn no_rules_default_allow() {
        let conn = test_connection();
        let result = PolicyEngine::evaluate(&conn, &[], DefaultPolicy::Allow);
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
    }

    #[test]
    fn first_matching_rule_wins_by_priority() {
        let conn = test_connection();
        let rules = vec![
            make_rule(10, RuleEffect::Allow, RuleCriteria::default()),
            make_rule(20, RuleEffect::Block, RuleCriteria::default()),
        ];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Block);
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
        assert_eq!(result.matched_rule_id, Some(rules[0].id));
    }

    #[test]
    fn disabled_rule_skipped() {
        let conn = test_connection();
        let mut rule = make_rule(1, RuleEffect::Block, RuleCriteria::default());
        rule.enabled = false;
        let rules = vec![rule];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Allow);
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
    }

    #[test]
    fn expired_rule_skipped() {
        let conn = test_connection();
        let mut rule = make_rule(1, RuleEffect::Block, RuleCriteria::default());
        rule.scope = RuleScope::Temporary {
            expires_at: Utc::now() - chrono::Duration::hours(1),
        };
        let rules = vec![rule];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Allow);
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
    }

    #[test]
    fn ask_effect_returns_pending_decision() {
        let conn = test_connection();
        let rule = make_rule(1, RuleEffect::Ask, RuleCriteria::default());
        let rules = vec![rule];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Block);
        assert_eq!(result.verdict, ConnectionVerdict::PendingDecision);
    }

    #[test]
    fn observe_effect_returns_ignored() {
        let conn = test_connection();
        let rule = make_rule(1, RuleEffect::Observe, RuleCriteria::default());
        let rules = vec![rule];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Block);
        assert_eq!(result.verdict, ConnectionVerdict::Ignored);
    }

    #[test]
    fn non_matching_rule_falls_through_to_next() {
        let conn = test_connection();
        let rules = vec![
            make_rule(
                1,
                RuleEffect::Block,
                RuleCriteria {
                    application: Some(AppMatcher::ByName("chrome".to_string())),
                    ..Default::default()
                },
            ),
            make_rule(2, RuleEffect::Allow, RuleCriteria::default()),
        ];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Block);
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
        assert_eq!(result.matched_rule_id, Some(rules[1].id));
    }
}
