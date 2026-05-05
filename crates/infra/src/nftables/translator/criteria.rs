//! Traduction des critères de règle (IP, port, utilisateur) en expressions nft.
//! Translation of rule criteria (IP, port, user) into nft expressions.

use std::net::IpAddr;

use syswall_domain::entities::{IpMatcher, PortMatcher};
use syswall_domain::value_objects::Protocol;

/// Résout un nom d'utilisateur en UID numérique.
/// Returns None if the user cannot be found.
///
/// Resolve a username to a numeric UID.
/// Retourne None si l'utilisateur est introuvable.
pub(super) fn resolve_username_to_uid(username: &str) -> Option<u32> {
    nix::unistd::User::from_name(username)
        .ok()
        .flatten()
        .map(|u| u.uid.as_raw())
}

/// Construit les expressions nft pour la correspondance IP selon la direction.
/// For outbound: remote IP is destination (daddr).
/// For inbound: remote IP is source (saddr).
///
/// Build nft expressions for IP matching based on direction.
pub(super) fn build_ip_expressions(ip_matcher: &IpMatcher, is_outbound: bool) -> Vec<String> {
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

/// Construit les expressions nft pour la correspondance de port.
/// Build nft expressions for port matching.
pub(super) fn build_port_expressions(
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
