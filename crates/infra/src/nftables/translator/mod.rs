//! Traduction des règles du domaine en expressions nftables.
//! Translation of domain rules into nftables expressions.

mod criteria;
mod verdict;
pub mod interception_chain;

use syswall_domain::entities::{Rule, RuleEffect};
use syswall_domain::value_objects::Direction;

use super::types::TranslatedRule;
use criteria::{build_ip_expressions, build_port_expressions, resolve_username_to_uid};
use verdict::build_verdict;

/// Traduit une Rule du domaine en arguments d'expressions nft.
/// Retourne None si la regle ne doit pas produire de regle nft (effet Ask).
///
/// Translate a domain Rule into nft expression arguments.
/// Returns None if the rule should not produce an nft rule (Ask effect).
pub fn translate_rule(rule: &Rule) -> Option<TranslatedRule> {
    if rule.effect == RuleEffect::Ask {
        return None;
    }

    let chains = get_target_chains(rule);
    let mut expressions: Vec<String> = Vec::new();
    let criteria = &rule.criteria;

    // Protocol match
    if let Some(ref proto) = criteria.protocol {
        use syswall_domain::value_objects::Protocol;
        let proto_str = match proto {
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
            Protocol::Icmp => "icmp",
            Protocol::Other(n) => {
                expressions.extend(["meta".into(), "l4proto".into(), n.to_string()]);
                // Skip the standard path
                ""
            }
        };
        if !proto_str.is_empty() {
            expressions.extend([
                "meta".to_string(),
                "l4proto".to_string(),
                proto_str.to_string(),
            ]);
        }
    }

    // Determine if outbound for IP direction
    let is_outbound = criteria.direction != Some(Direction::Inbound);

    // Remote IP match
    if let Some(ref ip_matcher) = criteria.remote_ip {
        expressions.extend(build_ip_expressions(ip_matcher, is_outbound));
    }

    // Remote port match (dport)
    if let Some(ref port_matcher) = criteria.remote_port {
        expressions.extend(build_port_expressions(
            port_matcher,
            criteria.protocol,
            "dport",
        ));
    }

    // Local port match (sport)
    if let Some(ref port_matcher) = criteria.local_port {
        expressions.extend(build_port_expressions(
            port_matcher,
            criteria.protocol,
            "sport",
        ));
    }

    // User match (meta skuid)
    if let Some(uid) = criteria
        .user
        .as_ref()
        .and_then(|u| resolve_username_to_uid(u))
    {
        expressions.extend(["meta".into(), "skuid".into(), uid.to_string()]);
    }

    // Verdict
    expressions.extend(build_verdict(rule.effect));

    // Comment with rule UUID for tracking
    let uuid_str = rule.id.as_uuid().to_string();
    expressions.extend([
        "comment".to_string(),
        format!("\"syswall:{}\"", uuid_str),
    ]);

    Some(TranslatedRule {
        chains,
        expressions,
    })
}

/// Détermine dans quelles chaînes nftables une règle doit être placée.
/// Determine which nftables chains a rule should be placed in.
pub fn get_target_chains(rule: &Rule) -> Vec<String> {
    match rule.criteria.direction {
        Some(Direction::Inbound) => vec!["input".to_string()],
        Some(Direction::Outbound) => vec!["output".to_string()],
        None => vec!["input".to_string(), "output".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn test_rule(effect: RuleEffect, criteria: RuleCriteria) -> Rule {
        Rule {
            id: RuleId::new(),
            name: "Test rule".to_string(),
            priority: RulePriority::new(100),
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
    fn ask_effect_produces_no_nft_rule() {
        let rule = test_rule(RuleEffect::Ask, RuleCriteria::default());
        assert!(translate_rule(&rule).is_none());
    }

    #[test]
    fn allow_tcp_port_443_outbound() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        assert_eq!(translated.chains, vec!["output"]);
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("meta l4proto tcp"));
        assert!(expr_str.contains("tcp dport 443"));
        assert!(expr_str.contains("accept"));
        assert!(expr_str.contains(&format!("syswall:{}", rule.id.as_uuid())));
    }

    #[test]
    fn block_ip_cidr_outbound() {
        let rule = test_rule(
            RuleEffect::Block,
            RuleCriteria {
                remote_ip: Some(IpMatcher::Cidr {
                    network: "10.0.0.0".parse().unwrap(),
                    prefix_len: 8,
                }),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        assert_eq!(translated.chains, vec!["output"]);
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("ip daddr 10.0.0.0/8"));
        assert!(expr_str.contains("drop"));
    }

    #[test]
    fn block_ip_cidr_inbound() {
        let rule = test_rule(
            RuleEffect::Block,
            RuleCriteria {
                remote_ip: Some(IpMatcher::Cidr {
                    network: "10.0.0.0".parse().unwrap(),
                    prefix_len: 8,
                }),
                direction: Some(Direction::Inbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        assert_eq!(translated.chains, vec!["input"]);
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("ip saddr 10.0.0.0/8"));
    }

    #[test]
    fn no_direction_produces_both_chains() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        assert_eq!(translated.chains, vec!["input", "output"]);
    }

    #[test]
    fn observe_effect_produces_log_and_accept() {
        let rule = test_rule(
            RuleEffect::Observe,
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("log prefix"));
        assert!(expr_str.contains("syswall-observe:"));
        assert!(expr_str.contains("accept"));
    }

    #[test]
    fn port_range_translated() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                remote_port: Some(PortMatcher::Range {
                    start: Port::new(8000).unwrap(),
                    end: Port::new(9000).unwrap(),
                }),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("tcp dport 8000-9000"));
    }

    #[test]
    fn rule_comment_contains_uuid() {
        let rule = test_rule(RuleEffect::Allow, RuleCriteria::default());
        let uuid_str = rule.id.as_uuid().to_string();
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains(&format!("syswall:{}", uuid_str)));
    }

    #[test]
    fn exact_ip_outbound() {
        let rule = test_rule(
            RuleEffect::Block,
            RuleCriteria {
                remote_ip: Some(IpMatcher::Exact("93.184.216.34".parse().unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("ip daddr 93.184.216.34"));
    }

    #[test]
    fn ipv6_address_uses_ip6() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                remote_ip: Some(IpMatcher::Exact("::1".parse().unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("ip6 daddr ::1"));
    }

    #[test]
    fn local_port_uses_sport() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                local_port: Some(PortMatcher::Exact(Port::new(8080).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("tcp sport 8080"));
    }

    #[test]
    fn udp_protocol_translated() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                remote_port: Some(PortMatcher::Exact(Port::new(53).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("meta l4proto udp"));
        assert!(expr_str.contains("udp dport 53"));
    }

    #[test]
    fn icmp_protocol_translated() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Icmp),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("meta l4proto icmp"));
    }

    #[test]
    fn get_target_chains_outbound() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        assert_eq!(get_target_chains(&rule), vec!["output"]);
    }

    #[test]
    fn get_target_chains_inbound() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                direction: Some(Direction::Inbound),
                ..Default::default()
            },
        );
        assert_eq!(get_target_chains(&rule), vec!["input"]);
    }

    #[test]
    fn get_target_chains_no_direction() {
        let rule = test_rule(RuleEffect::Allow, RuleCriteria::default());
        assert_eq!(get_target_chains(&rule), vec!["input", "output"]);
    }

    #[test]
    fn ip_range_outbound() {
        let rule = test_rule(
            RuleEffect::Block,
            RuleCriteria {
                remote_ip: Some(IpMatcher::Range {
                    start: "10.0.0.1".parse().unwrap(),
                    end: "10.0.0.255".parse().unwrap(),
                }),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("ip daddr 10.0.0.1-10.0.0.255"));
    }

    #[test]
    fn other_protocol_uses_raw_number() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Other(47)),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("meta l4proto 47"));
    }

    #[test]
    fn block_verdict_produces_drop() {
        let result = verdict::build_verdict(RuleEffect::Block);
        assert_eq!(result, vec!["drop"]);
    }

    #[test]
    fn allow_verdict_produces_accept() {
        let result = verdict::build_verdict(RuleEffect::Allow);
        assert_eq!(result, vec!["accept"]);
    }

    #[test]
    fn observe_verdict_produces_log_and_accept() {
        let result = verdict::build_verdict(RuleEffect::Observe);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "log");
        assert_eq!(result[1], "prefix");
        assert!(result[2].contains("syswall-observe:"));
        assert_eq!(result[3], "accept");
    }

    #[test]
    fn combined_protocol_ip_port_rule() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                remote_ip: Some(IpMatcher::Exact("93.184.216.34".parse().unwrap())),
                remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("meta l4proto tcp"));
        assert!(expr_str.contains("ip daddr 93.184.216.34"));
        assert!(expr_str.contains("tcp dport 443"));
        assert!(expr_str.contains("accept"));
        assert!(expr_str.contains("comment"));
    }

    #[test]
    fn ipv6_cidr_outbound() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                remote_ip: Some(IpMatcher::Cidr {
                    network: "fe80::".parse().unwrap(),
                    prefix_len: 10,
                }),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("ip6 daddr fe80::/10"));
    }

    #[test]
    fn local_port_range() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                local_port: Some(PortMatcher::Range {
                    start: Port::new(1024).unwrap(),
                    end: Port::new(65535).unwrap(),
                }),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("tcp sport 1024-65535"));
    }

    #[test]
    fn application_matcher_is_ignored() {
        // Application matching is userspace-only, not in nft rules
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                application: Some(AppMatcher::ByName("firefox".to_string())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        // Should not contain "firefox" anywhere in nft expressions
        assert!(!expr_str.contains("firefox"));
        // But should still have accept and comment
        assert!(expr_str.contains("accept"));
        assert!(expr_str.contains("comment"));
    }

    #[test]
    fn port_without_explicit_protocol_defaults_to_tcp() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                remote_port: Some(PortMatcher::Exact(Port::new(80).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("tcp dport 80"));
    }
}
