//! String-to-enum parsing helpers shared across converter modules.
//! Fonctions d'analyse chaine->enum partagees entre les modules de conversion.

use syswall_domain::entities::{
    DecisionAction, DecisionGranularity, EventCategory, RuleEffect, RuleSource, Severity,
};

/// Parse a string to a Severity enum.
/// Analyse une chaîne vers l'énumération Severity.
// tonic::Status est imposé par l'API gRPC ; taille inévitable.
#[allow(clippy::result_large_err)]
pub(super) fn parse_severity(s: &str) -> Result<Severity, tonic::Status> {
    match s {
        "Debug" => Ok(Severity::Debug),
        "Info" => Ok(Severity::Info),
        "Warning" => Ok(Severity::Warning),
        "Error" => Ok(Severity::Error),
        "Critical" => Ok(Severity::Critical),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown severity: '{}'. Expected: Debug, Info, Warning, Error, Critical",
            s
        ))),
    }
}

/// Parse a string to an EventCategory enum.
/// Analyse une chaîne vers l'énumération EventCategory.
// tonic::Status est imposé par l'API gRPC ; taille inévitable.
#[allow(clippy::result_large_err)]
pub(super) fn parse_event_category(s: &str) -> Result<EventCategory, tonic::Status> {
    match s {
        "Connection" => Ok(EventCategory::Connection),
        "Rule" => Ok(EventCategory::Rule),
        "Decision" => Ok(EventCategory::Decision),
        "System" => Ok(EventCategory::System),
        "Config" => Ok(EventCategory::Config),
        "Antilockout" => Ok(EventCategory::Antilockout),
        "Authentication" => Ok(EventCategory::Authentication),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown category: '{}'. Expected: Connection, Rule, Decision, System, Config, Antilockout, Authentication",
            s
        ))),
    }
}

/// Parse a string to a RuleEffect enum.
/// Analyse une chaîne vers l'énumération RuleEffect.
// tonic::Status est imposé par l'API gRPC ; taille inévitable.
#[allow(clippy::result_large_err)]
pub(super) fn parse_rule_effect(s: &str) -> Result<RuleEffect, tonic::Status> {
    match s {
        "allow" => Ok(RuleEffect::Allow),
        "block" => Ok(RuleEffect::Block),
        "ask" => Ok(RuleEffect::Ask),
        "observe" => Ok(RuleEffect::Observe),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown rule effect: '{}'. Expected: allow, block, ask, observe",
            s
        ))),
    }
}

/// Parse a string to a RuleSource enum.
/// Analyse une chaîne vers l'énumération RuleSource.
// tonic::Status est imposé par l'API gRPC ; taille inévitable.
#[allow(clippy::result_large_err)]
pub(super) fn parse_rule_source(s: &str) -> Result<RuleSource, tonic::Status> {
    match s {
        "manual" => Ok(RuleSource::Manual),
        "auto_learning" => Ok(RuleSource::AutoLearning),
        "import" => Ok(RuleSource::Import),
        "system" => Ok(RuleSource::System),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown rule source: '{}'. Expected: manual, auto_learning, import, system",
            s
        ))),
    }
}

/// Parse a string to a DecisionAction enum.
/// Analyse une chaîne vers l'énumération DecisionAction.
// tonic::Status est imposé par l'API gRPC ; taille inévitable.
#[allow(clippy::result_large_err)]
pub(super) fn parse_decision_action(s: &str) -> Result<DecisionAction, tonic::Status> {
    match s {
        "allow_once" => Ok(DecisionAction::AllowOnce),
        "block_once" => Ok(DecisionAction::BlockOnce),
        "always_allow" => Ok(DecisionAction::AlwaysAllow),
        "always_block" => Ok(DecisionAction::AlwaysBlock),
        "create_rule" => Ok(DecisionAction::CreateRule),
        "ignore" => Ok(DecisionAction::Ignore),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown decision action: '{}'. Expected: allow_once, block_once, always_allow, always_block, create_rule, ignore",
            s
        ))),
    }
}

/// Parse a string to a DecisionGranularity enum.
/// Analyse une chaîne vers l'énumération DecisionGranularity.
// tonic::Status est imposé par l'API gRPC ; taille inévitable.
#[allow(clippy::result_large_err)]
pub(super) fn parse_decision_granularity(s: &str) -> Result<DecisionGranularity, tonic::Status> {
    match s {
        "app_only" => Ok(DecisionGranularity::AppOnly),
        "app_and_ip" => Ok(DecisionGranularity::AppAndIp),
        "app_and_port" => Ok(DecisionGranularity::AppAndPort),
        "app_and_domain" => Ok(DecisionGranularity::AppAndDomain),
        "app_and_protocol" => Ok(DecisionGranularity::AppAndProtocol),
        "full" => Ok(DecisionGranularity::Full),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown decision granularity: '{}'. Expected: app_only, app_and_ip, app_and_port, app_and_domain, app_and_protocol, full",
            s
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_rule_effects() {
        assert_eq!(parse_rule_effect("allow").unwrap(), RuleEffect::Allow);
        assert_eq!(parse_rule_effect("block").unwrap(), RuleEffect::Block);
        assert_eq!(parse_rule_effect("ask").unwrap(), RuleEffect::Ask);
        assert_eq!(parse_rule_effect("observe").unwrap(), RuleEffect::Observe);
        assert!(parse_rule_effect("bad").is_err());
    }

    #[test]
    fn parse_all_rule_sources() {
        assert_eq!(parse_rule_source("manual").unwrap(), RuleSource::Manual);
        assert_eq!(
            parse_rule_source("auto_learning").unwrap(),
            RuleSource::AutoLearning
        );
        assert_eq!(parse_rule_source("import").unwrap(), RuleSource::Import);
        assert_eq!(parse_rule_source("system").unwrap(), RuleSource::System);
        assert!(parse_rule_source("bad").is_err());
    }

    #[test]
    fn parse_all_decision_actions() {
        assert_eq!(
            parse_decision_action("allow_once").unwrap(),
            DecisionAction::AllowOnce
        );
        assert_eq!(
            parse_decision_action("block_once").unwrap(),
            DecisionAction::BlockOnce
        );
        assert_eq!(
            parse_decision_action("always_allow").unwrap(),
            DecisionAction::AlwaysAllow
        );
        assert_eq!(
            parse_decision_action("always_block").unwrap(),
            DecisionAction::AlwaysBlock
        );
        assert_eq!(
            parse_decision_action("create_rule").unwrap(),
            DecisionAction::CreateRule
        );
        assert_eq!(
            parse_decision_action("ignore").unwrap(),
            DecisionAction::Ignore
        );
        assert!(parse_decision_action("bad").is_err());
    }

    #[test]
    fn parse_all_decision_granularities() {
        assert_eq!(
            parse_decision_granularity("app_only").unwrap(),
            DecisionGranularity::AppOnly
        );
        assert_eq!(
            parse_decision_granularity("app_and_ip").unwrap(),
            DecisionGranularity::AppAndIp
        );
        assert_eq!(
            parse_decision_granularity("app_and_port").unwrap(),
            DecisionGranularity::AppAndPort
        );
        assert_eq!(
            parse_decision_granularity("app_and_domain").unwrap(),
            DecisionGranularity::AppAndDomain
        );
        assert_eq!(
            parse_decision_granularity("app_and_protocol").unwrap(),
            DecisionGranularity::AppAndProtocol
        );
        assert_eq!(
            parse_decision_granularity("full").unwrap(),
            DecisionGranularity::Full
        );
        assert!(parse_decision_granularity("bad").is_err());
    }
}
