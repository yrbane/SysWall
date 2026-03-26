use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

use crate::value_objects::{Direction, ExecutablePath, Port, Protocol, RulePriority};

/// Unique identifier for a rule.
/// Identifiant unique d'une règle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuleId(Uuid);

impl RuleId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

/// What a rule does when it matches.
/// Ce que fait une règle lorsqu'elle correspond.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleEffect {
    Allow,
    Block,
    Ask,
    Observe,
}

/// Rule lifetime scope.
/// Portée temporelle d'une règle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleScope {
    Permanent,
    Temporary { expires_at: DateTime<Utc> },
}

/// How the rule was created.
/// Comment la règle a été créée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleSource {
    Manual,
    AutoLearning,
    Import,
    System,
}

/// Application matching criteria.
/// Critères de correspondance d'application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppMatcher {
    ByName(String),
    ByPath(ExecutablePath),
    ByHash(String),
}

/// IP matching criteria.
/// Critères de correspondance IP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpMatcher {
    Exact(IpAddr),
    Cidr { network: IpAddr, prefix_len: u8 },
    Range { start: IpAddr, end: IpAddr },
}

/// Port matching criteria.
/// Critères de correspondance de port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortMatcher {
    Exact(Port),
    Range { start: Port, end: Port },
}

/// Time schedule for rule application.
/// Horaire d'application d'une règle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub days: Vec<chrono::Weekday>,
    pub start_time: chrono::NaiveTime,
    pub end_time: chrono::NaiveTime,
}

/// All matching criteria for a rule. All present fields must match (AND logic).
/// None means "match anything" for that dimension.
///
/// Tous les critères de correspondance d'une règle. Tous les champs présents doivent correspondre (logique ET).
/// None signifie « correspond à tout » pour cette dimension.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleCriteria {
    pub application: Option<AppMatcher>,
    pub user: Option<String>,
    pub remote_ip: Option<IpMatcher>,
    pub remote_port: Option<PortMatcher>,
    pub local_port: Option<PortMatcher>,
    pub protocol: Option<Protocol>,
    pub direction: Option<Direction>,
    pub schedule: Option<Schedule>,
}

/// A firewall rule.
/// Une règle de pare-feu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: RuleId,
    pub name: String,
    pub priority: RulePriority,
    pub enabled: bool,
    pub criteria: RuleCriteria,
    pub effect: RuleEffect,
    pub scope: RuleScope,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub source: RuleSource,
}

impl Rule {
    /// System rules cannot be deleted, only disabled.
    /// Les règles système ne peuvent pas être supprimées, seulement désactivées.
    pub fn is_system(&self) -> bool {
        self.source == RuleSource::System
    }

    /// Check if a temporary rule has expired.
    /// Vérifie si une règle temporaire a expiré.
    pub fn is_expired(&self) -> bool {
        match &self.scope {
            RuleScope::Permanent => false,
            RuleScope::Temporary { expires_at } => Utc::now() > *expires_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rule(effect: RuleEffect, source: RuleSource) -> Rule {
        Rule {
            id: RuleId::new(),
            name: "Test rule".to_string(),
            priority: RulePriority::new(100),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source,
        }
    }

    #[test]
    fn system_rule_detected() {
        let rule = test_rule(RuleEffect::Allow, RuleSource::System);
        assert!(rule.is_system());
    }

    #[test]
    fn manual_rule_not_system() {
        let rule = test_rule(RuleEffect::Block, RuleSource::Manual);
        assert!(!rule.is_system());
    }

    #[test]
    fn permanent_rule_never_expired() {
        let rule = test_rule(RuleEffect::Allow, RuleSource::Manual);
        assert!(!rule.is_expired());
    }

    #[test]
    fn expired_temporary_rule_detected() {
        let mut rule = test_rule(RuleEffect::Allow, RuleSource::Manual);
        rule.scope = RuleScope::Temporary {
            expires_at: Utc::now() - chrono::Duration::hours(1),
        };
        assert!(rule.is_expired());
    }

    #[test]
    fn future_temporary_rule_not_expired() {
        let mut rule = test_rule(RuleEffect::Allow, RuleSource::Manual);
        rule.scope = RuleScope::Temporary {
            expires_at: Utc::now() + chrono::Duration::hours(1),
        };
        assert!(!rule.is_expired());
    }

    #[test]
    fn default_criteria_matches_everything() {
        let criteria = RuleCriteria::default();
        assert!(criteria.application.is_none());
        assert!(criteria.protocol.is_none());
        assert!(criteria.direction.is_none());
    }
}
