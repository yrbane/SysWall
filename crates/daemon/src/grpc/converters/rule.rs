//! Conversions entre Rule (domaine) et son équivalent proto.
//! Conversions between Rule (domain) and its proto equivalent.

use syswall_app::commands::CreateRuleCommand;
use syswall_domain::entities::{Rule, RuleCriteria, RuleEffect, RuleScope, RuleSource};
use syswall_proto::syswall::{CreateRuleRequest, RuleMessage};

use super::parsers::{parse_rule_effect, parse_rule_source};

/// Convert a domain Rule to a proto RuleMessage.
/// Convertit une Rule du domaine en RuleMessage proto.
pub fn rule_to_proto(rule: &Rule) -> RuleMessage {
    let effect = match rule.effect {
        RuleEffect::Allow => "allow",
        RuleEffect::Block => "block",
        RuleEffect::Ask => "ask",
        RuleEffect::Observe => "observe",
    };

    let source = match rule.source {
        RuleSource::Manual => "manual",
        RuleSource::AutoLearning => "auto_learning",
        RuleSource::Import => "import",
        RuleSource::System => "system",
    };

    RuleMessage {
        id: rule.id.as_uuid().to_string(),
        name: rule.name.clone(),
        priority: rule.priority.value(),
        enabled: rule.enabled,
        criteria_json: serde_json::to_string(&rule.criteria).unwrap_or_default(),
        effect: effect.to_string(),
        scope_json: serde_json::to_string(&rule.scope).unwrap_or_default(),
        source: source.to_string(),
        created_at: rule.created_at.to_rfc3339(),
        updated_at: rule.updated_at.to_rfc3339(),
    }
}

/// Convert a proto CreateRuleRequest to a domain CreateRuleCommand.
/// Convertit une CreateRuleRequest proto en CreateRuleCommand du domaine.
// tonic::Status est imposé par l'API gRPC ; taille inévitable.
#[allow(clippy::result_large_err)]
pub fn proto_to_create_rule_cmd(
    req: &CreateRuleRequest,
) -> Result<CreateRuleCommand, tonic::Status> {
    if req.name.is_empty() {
        return Err(tonic::Status::invalid_argument("Rule name must not be empty"));
    }

    let criteria: RuleCriteria = serde_json::from_str(&req.criteria_json)
        .map_err(|e| tonic::Status::invalid_argument(format!("Invalid criteria_json: {}", e)))?;

    let effect = parse_rule_effect(&req.effect)?;

    let scope: RuleScope = serde_json::from_str(&req.scope_json)
        .map_err(|e| tonic::Status::invalid_argument(format!("Invalid scope_json: {}", e)))?;

    let source = parse_rule_source(&req.source)?;

    Ok(CreateRuleCommand {
        name: req.name.clone(),
        priority: req.priority,
        criteria,
        effect,
        scope,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn test_rule() -> Rule {
        Rule {
            id: RuleId::new(),
            name: "Test rule".to_string(),
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

    #[test]
    fn rule_to_proto_all_fields() {
        let rule = test_rule();
        let msg = rule_to_proto(&rule);

        assert_eq!(msg.id, rule.id.as_uuid().to_string());
        assert_eq!(msg.name, "Test rule");
        assert_eq!(msg.priority, 10);
        assert!(msg.enabled);
        assert_eq!(msg.effect, "allow");
        assert_eq!(msg.source, "manual");
        assert!(!msg.criteria_json.is_empty());
        assert!(!msg.scope_json.is_empty());
        assert!(!msg.created_at.is_empty());
        assert!(!msg.updated_at.is_empty());
    }

    #[test]
    fn create_rule_request_valid() {
        let req = CreateRuleRequest {
            name: "Block SSH".to_string(),
            priority: 5,
            criteria_json: serde_json::to_string(&RuleCriteria::default()).unwrap(),
            effect: "block".to_string(),
            scope_json: serde_json::to_string(&RuleScope::Permanent).unwrap(),
            source: "manual".to_string(),
        };

        let cmd = proto_to_create_rule_cmd(&req).unwrap();
        assert_eq!(cmd.name, "Block SSH");
        assert_eq!(cmd.priority, 5);
        assert_eq!(cmd.effect, RuleEffect::Block);
        assert_eq!(cmd.source, RuleSource::Manual);
    }

    #[test]
    fn create_rule_request_invalid_json() {
        let req = CreateRuleRequest {
            name: "Bad".to_string(),
            priority: 1,
            criteria_json: "not json".to_string(),
            effect: "allow".to_string(),
            scope_json: "\"Permanent\"".to_string(),
            source: "manual".to_string(),
        };

        let err = proto_to_create_rule_cmd(&req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn create_rule_request_empty_name() {
        let req = CreateRuleRequest {
            name: String::new(),
            priority: 1,
            criteria_json: "{}".to_string(),
            effect: "allow".to_string(),
            scope_json: "\"Permanent\"".to_string(),
            source: "manual".to_string(),
        };

        let err = proto_to_create_rule_cmd(&req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }
}
