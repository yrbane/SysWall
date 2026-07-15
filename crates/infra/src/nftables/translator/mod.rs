//! Traduction des règles du domaine en expressions nftables.
//! Translation of domain rules into nftables expressions.

mod criteria;
pub mod interception_chain;
mod verdict;

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
    expressions.extend(["comment".to_string(), format!("\"syswall:{}\"", uuid_str)]);

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
mod tests;
