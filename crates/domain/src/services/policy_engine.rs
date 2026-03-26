use std::net::IpAddr;

use crate::entities::{
    AppMatcher, Connection, ConnectionVerdict, IpMatcher, PortMatcher, Rule, RuleCriteria,
    RuleEffect, RuleId,
};
use crate::events::DefaultPolicy;
use crate::value_objects::Port;

/// Result of evaluating a connection against rules.
/// Résultat de l'évaluation d'une connexion par rapport aux règles.
#[derive(Debug, Clone)]
pub struct PolicyEvaluation {
    pub verdict: ConnectionVerdict,
    pub matched_rule_id: Option<RuleId>,
    pub reason: EvaluationReason,
}

/// Why a particular verdict was reached.
/// Pourquoi un verdict particulier a été atteint.
#[derive(Debug, Clone)]
pub enum EvaluationReason {
    MatchedRule {
        rule_id: RuleId,
        effect: RuleEffect,
    },
    NoMatchingRule,
    PendingUserDecision,
    DefaultPolicyApplied {
        policy: DefaultPolicy,
    },
}

/// Pure domain service -- no I/O, no ports.
/// Service de domaine pur -- pas d'E/S, pas de ports.
pub struct PolicyEngine;

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

    /// Check if a single criteria set matches a connection (Specification pattern).
    /// Vérifie si un ensemble de critères correspond à une connexion (patron Spécification).
    pub fn matches(criteria: &RuleCriteria, connection: &Connection) -> bool {
        if let Some(ref app_matcher) = criteria.application {
            if let Some(ref process) = connection.process {
                let matched = match app_matcher {
                    AppMatcher::ByName(name) => process.name == *name,
                    AppMatcher::ByPath(path) => {
                        process.path.as_ref().is_some_and(|p| p == path)
                    }
                    AppMatcher::ByHash(_hash) => false, // Hash matching deferred to sub-project 2
                };
                if !matched {
                    return false;
                }
            } else {
                // No process info available, can't match application criteria
                return false;
            }
        }

        if let Some(ref user) = criteria.user {
            match &connection.user {
                Some(u) if u.name == *user => {}
                _ => return false,
            }
        }

        if let Some(ref ip_matcher) = criteria.remote_ip {
            let remote_ip =
                if connection.direction == crate::value_objects::Direction::Outbound {
                    connection.destination.ip
                } else {
                    connection.source.ip
                };
            if !Self::matches_ip(ip_matcher, remote_ip) {
                return false;
            }
        }

        if let Some(ref port_matcher) = criteria.remote_port {
            let remote_port =
                if connection.direction == crate::value_objects::Direction::Outbound {
                    connection.destination.port
                } else {
                    connection.source.port
                };
            if !Self::matches_port(port_matcher, remote_port) {
                return false;
            }
        }

        if let Some(ref port_matcher) = criteria.local_port {
            let local_port =
                if connection.direction == crate::value_objects::Direction::Outbound {
                    connection.source.port
                } else {
                    connection.destination.port
                };
            if !Self::matches_port(port_matcher, local_port) {
                return false;
            }
        }

        if let Some(ref proto) = criteria.protocol
            && connection.protocol != *proto {
                return false;
            }

        if let Some(ref dir) = criteria.direction
            && connection.direction != *dir {
                return false;
            }

        // Schedule matching deferred to sub-project 4

        true
    }

    fn matches_ip(matcher: &IpMatcher, ip: IpAddr) -> bool {
        match matcher {
            IpMatcher::Exact(expected) => ip == *expected,
            IpMatcher::Cidr {
                network,
                prefix_len,
            } => Self::ip_in_cidr(ip, *network, *prefix_len),
            IpMatcher::Range { start, end } => ip >= *start && ip <= *end,
        }
    }

    fn ip_in_cidr(ip: IpAddr, network: IpAddr, prefix_len: u8) -> bool {
        match (ip, network) {
            (IpAddr::V4(ip), IpAddr::V4(net)) => {
                if prefix_len > 32 {
                    return false;
                }
                let mask = if prefix_len == 0 {
                    0u32
                } else {
                    !0u32 << (32 - prefix_len)
                };
                (u32::from(ip) & mask) == (u32::from(net) & mask)
            }
            (IpAddr::V6(ip), IpAddr::V6(net)) => {
                if prefix_len > 128 {
                    return false;
                }
                let ip_bits = u128::from(ip);
                let net_bits = u128::from(net);
                let mask = if prefix_len == 0 {
                    0u128
                } else {
                    !0u128 << (128 - prefix_len)
                };
                (ip_bits & mask) == (net_bits & mask)
            }
            _ => false, // v4 vs v6 mismatch
        }
    }

    fn matches_port(matcher: &PortMatcher, port: Port) -> bool {
        match matcher {
            PortMatcher::Exact(expected) => port == *expected,
            PortMatcher::Range { start, end } => {
                port.value() >= start.value() && port.value() <= end.value()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::*;
    use crate::value_objects::*;
    use chrono::Utc;

    fn test_connection() -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol: Protocol::Tcp,
            source: SocketAddress::new(
                "192.168.1.100".parse().unwrap(),
                Port::new(45000).unwrap(),
            ),
            destination: SocketAddress::new(
                "93.184.216.34".parse().unwrap(),
                Port::new(443).unwrap(),
            ),
            direction: Direction::Outbound,
            state: ConnectionState::New,
            process: Some(ProcessInfo {
                pid: 1234,
                name: "firefox".to_string(),
                path: Some(ExecutablePath::new("/usr/bin/firefox".into()).unwrap()),
                cmdline: None,
            }),
            user: Some(SystemUser {
                uid: 1000,
                name: "seb".to_string(),
            }),
            bytes_sent: 0,
            bytes_received: 0,
            started_at: Utc::now(),
            verdict: ConnectionVerdict::Unknown,
            matched_rule: None,
        }
    }

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

    // --- evaluate() tests ---

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

    // --- matches() tests ---

    #[test]
    fn empty_criteria_matches_everything() {
        let conn = test_connection();
        assert!(PolicyEngine::matches(&RuleCriteria::default(), &conn));
    }

    #[test]
    fn app_name_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByName("firefox".to_string())),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn app_name_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByName("chrome".to_string())),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn app_path_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByPath(
                ExecutablePath::new("/usr/bin/firefox".into()).unwrap(),
            )),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn protocol_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            protocol: Some(Protocol::Tcp),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn protocol_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            protocol: Some(Protocol::Udp),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn direction_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            direction: Some(Direction::Outbound),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn direction_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            direction: Some(Direction::Inbound),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_ip_exact_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_ip: Some(IpMatcher::Exact("93.184.216.34".parse().unwrap())),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_ip_cidr_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_ip: Some(IpMatcher::Cidr {
                network: "93.184.216.0".parse().unwrap(),
                prefix_len: 24,
            }),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_ip_cidr_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_ip: Some(IpMatcher::Cidr {
                network: "10.0.0.0".parse().unwrap(),
                prefix_len: 8,
            }),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_port_exact_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_port_range_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_port: Some(PortMatcher::Range {
                start: Port::new(400).unwrap(),
                end: Port::new(500).unwrap(),
            }),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn user_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            user: Some("seb".to_string()),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn user_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            user: Some("root".to_string()),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn combined_criteria_all_must_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByName("firefox".to_string())),
            protocol: Some(Protocol::Tcp),
            direction: Some(Direction::Outbound),
            remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn combined_criteria_one_fails_all_fails() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByName("firefox".to_string())),
            protocol: Some(Protocol::Udp), // wrong protocol
            direction: Some(Direction::Outbound),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn no_process_info_fails_app_criteria() {
        let mut conn = test_connection();
        conn.process = None;
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByName("firefox".to_string())),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn local_port_match_outbound() {
        let conn = test_connection();
        // For outbound, local port is source port
        let criteria = RuleCriteria {
            local_port: Some(PortMatcher::Exact(Port::new(45000).unwrap())),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn local_port_no_match_outbound() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            local_port: Some(PortMatcher::Exact(Port::new(80).unwrap())),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn ip_range_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_ip: Some(IpMatcher::Range {
                start: "93.184.216.0".parse().unwrap(),
                end: "93.184.216.255".parse().unwrap(),
            }),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn ip_range_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_ip: Some(IpMatcher::Range {
                start: "10.0.0.0".parse().unwrap(),
                end: "10.255.255.255".parse().unwrap(),
            }),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_port_exact_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_port: Some(PortMatcher::Exact(Port::new(80).unwrap())),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn user_criteria_no_user_on_connection() {
        let mut conn = test_connection();
        conn.user = None;
        let criteria = RuleCriteria {
            user: Some("seb".to_string()),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
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
