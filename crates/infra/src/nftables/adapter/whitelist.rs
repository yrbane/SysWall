//! Détection des règles whitelist (DNS, DHCP, NTP, loopback) — bypass du guard anti-lockout.
//! Whitelist rule detection (DNS, DHCP, NTP, loopback) — bypass the lockout guard.

use syswall_domain::entities::{IpMatcher, Rule};
use syswall_domain::value_objects::Protocol;

/// Retourne true si toutes les règles sont whitelist (DNS/DHCP/NTP/loopback).
/// Returns true if all rules are whitelist (DNS/DHCP/NTP/loopback).
pub(super) fn is_whitelist_only(rules: &[Rule]) -> bool {
    !rules.is_empty() && rules.iter().all(is_whitelist_rule)
}

/// Retourne true si une règle individuelle correspond à un trafic réseau fondamental.
/// Returns true if a single rule matches fundamental network traffic.
pub(super) fn is_whitelist_rule(rule: &Rule) -> bool {
    let crit = &rule.criteria;
    let port_match = |p: u16| {
        let matches_matcher = |m: &syswall_domain::entities::PortMatcher| match m {
            syswall_domain::entities::PortMatcher::Exact(port) => port.value() == p,
            syswall_domain::entities::PortMatcher::Range { start, end } => {
                start.value() <= p && p <= end.value()
            }
        };
        crit.remote_port.as_ref().is_some_and(matches_matcher)
            || crit.local_port.as_ref().is_some_and(matches_matcher)
    };
    let proto_is = |p: Protocol| crit.protocol == Some(p);
    let is_loopback = crit.remote_ip.as_ref().is_some_and(|ip| match ip {
        IpMatcher::Exact(addr) => addr.is_loopback(),
        IpMatcher::Cidr { network, .. } => network.is_loopback(),
        IpMatcher::Range { start, end } => start.is_loopback() && end.is_loopback(),
    });
    (proto_is(Protocol::Udp) && port_match(53))
        || (proto_is(Protocol::Tcp) && port_match(53))
        || (proto_is(Protocol::Udp) && (port_match(67) || port_match(68)))
        || (proto_is(Protocol::Udp) && port_match(123))
        || is_loopback
}
