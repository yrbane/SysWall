use std::net::IpAddr;

use syswall_domain::entities::{IpMatcher, PortMatcher, Rule, RuleEffect};
use syswall_domain::value_objects::{Direction, Protocol};

use super::types::TranslatedRule;

/// Translate a domain Rule into nft expression arguments.
/// Returns None if the rule should not produce an nft rule (Ask effect).
///
/// Traduit une Rule du domaine en arguments d'expressions nft.
/// Retourne None si la regle ne doit pas produire de regle nft (effet Ask).
pub fn translate_rule(rule: &Rule) -> Option<TranslatedRule> {
    if rule.effect == RuleEffect::Ask {
        return None;
    }

    let chains = get_target_chains(rule);
    let mut expressions: Vec<String> = Vec::new();
    let criteria = &rule.criteria;

    // Protocol match
    if let Some(ref proto) = criteria.protocol {
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
    if let Some(ref username) = criteria.user {
        if let Some(uid) = resolve_username_to_uid(username) {
            expressions.extend(["meta".into(), "skuid".into(), uid.to_string()]);
        }
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

/// Determine which nftables chains a rule should be placed in.
/// Determine dans quelles chaines nftables une regle doit etre placee.
pub fn get_target_chains(rule: &Rule) -> Vec<String> {
    match rule.criteria.direction {
        Some(Direction::Inbound) => vec!["input".to_string()],
        Some(Direction::Outbound) => vec!["output".to_string()],
        None => vec!["input".to_string(), "output".to_string()],
    }
}

/// Resolve a username to a numeric UID.
/// Returns None if the user cannot be found.
///
/// Resout un nom d'utilisateur en UID numerique.
/// Retourne None si l'utilisateur est introuvable.
pub fn resolve_username_to_uid(username: &str) -> Option<u32> {
    nix::unistd::User::from_name(username)
        .ok()
        .flatten()
        .map(|u| u.uid.as_raw())
}

/// Build nft expressions for IP matching based on direction.
/// For outbound: remote IP is destination (daddr).
/// For inbound: remote IP is source (saddr).
///
/// Construit les expressions nft pour la correspondance IP selon la direction.
fn build_ip_expressions(ip_matcher: &IpMatcher, is_outbound: bool) -> Vec<String> {
    let direction_keyword = if is_outbound { "daddr" } else { "saddr" };

    match ip_matcher {
        IpMatcher::Exact(ip) => {
            let family = match ip {
                IpAddr::V4(_) => "ip",
                IpAddr::V6(_) => "ip6",
            };
            vec![
                family.to_string(),
                direction_keyword.to_string(),
                ip.to_string(),
            ]
        }
        IpMatcher::Cidr {
            network,
            prefix_len,
        } => {
            let family = match network {
                IpAddr::V4(_) => "ip",
                IpAddr::V6(_) => "ip6",
            };
            vec![
                family.to_string(),
                direction_keyword.to_string(),
                format!("{}/{}", network, prefix_len),
            ]
        }
        IpMatcher::Range { start, end } => {
            let family = match start {
                IpAddr::V4(_) => "ip",
                IpAddr::V6(_) => "ip6",
            };
            vec![
                family.to_string(),
                direction_keyword.to_string(),
                format!("{}-{}", start, end),
            ]
        }
    }
}

/// Build nft expressions for port matching.
/// Construit les expressions nft pour la correspondance de port.
fn build_port_expressions(
    port_matcher: &PortMatcher,
    protocol: Option<Protocol>,
    keyword: &str,
) -> Vec<String> {
    let proto_str = match protocol {
        Some(Protocol::Tcp) => "tcp",
        Some(Protocol::Udp) => "udp",
        _ => "tcp", // default to tcp if protocol not specified with port
    };

    match port_matcher {
        PortMatcher::Exact(port) => {
            vec![
                proto_str.to_string(),
                keyword.to_string(),
                port.value().to_string(),
            ]
        }
        PortMatcher::Range { start, end } => {
            vec![
                proto_str.to_string(),
                keyword.to_string(),
                format!("{}-{}", start.value(), end.value()),
            ]
        }
    }
}

/// Build the verdict expression (accept, drop, or log+accept for observe).
/// Construit l'expression de verdict (accept, drop, ou log+accept pour observe).
fn build_verdict(effect: RuleEffect) -> Vec<String> {
    match effect {
        RuleEffect::Allow => vec!["accept".to_string()],
        RuleEffect::Block => vec!["drop".to_string()],
        RuleEffect::Observe => vec![
            "log".to_string(),
            "prefix".to_string(),
            "\"syswall-observe: \"".to_string(),
            "accept".to_string(),
        ],
        RuleEffect::Ask => vec![], // should never reach here
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
        let result = build_verdict(RuleEffect::Block);
        assert_eq!(result, vec!["drop"]);
    }

    #[test]
    fn allow_verdict_produces_accept() {
        let result = build_verdict(RuleEffect::Allow);
        assert_eq!(result, vec!["accept"]);
    }

    #[test]
    fn observe_verdict_produces_log_and_accept() {
        let result = build_verdict(RuleEffect::Observe);
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
