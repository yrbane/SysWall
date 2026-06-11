//! Property tests du PolicyEngine : invariants vérifiés sur entrées arbitraires.
//! PolicyEngine property tests: invariants checked against arbitrary inputs.

use chrono::{Duration, Utc};
use proptest::prelude::*;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;

use syswall_domain::entities::*;
use syswall_domain::events::DefaultPolicy;
use syswall_domain::services::PolicyEngine;
use syswall_domain::value_objects::*;

// --- Stratégies / Strategies ---

fn arb_ip() -> impl Strategy<Value = IpAddr> {
    prop_oneof![
        any::<[u8; 4]>().prop_map(|b| IpAddr::V4(Ipv4Addr::from(b))),
        any::<[u8; 16]>().prop_map(|b| IpAddr::V6(Ipv6Addr::from(b))),
    ]
}

fn arb_port() -> impl Strategy<Value = Port> {
    (1u16..=u16::MAX).prop_map(|p| Port::new(p).expect("port non nul par construction"))
}

fn arb_protocol() -> impl Strategy<Value = Protocol> {
    prop_oneof![
        Just(Protocol::Tcp),
        Just(Protocol::Udp),
        Just(Protocol::Icmp),
        any::<u8>().prop_map(Protocol::Other),
    ]
}

fn arb_direction() -> impl Strategy<Value = Direction> {
    prop_oneof![Just(Direction::Inbound), Just(Direction::Outbound)]
}

fn arb_socket_address() -> impl Strategy<Value = SocketAddress> {
    (arb_ip(), arb_port()).prop_map(|(ip, port)| SocketAddress::new(ip, port))
}

fn arb_process() -> impl Strategy<Value = Option<ProcessInfo>> {
    proptest::option::of("[a-z]{1,12}".prop_map(|name| ProcessInfo {
        pid: 1234,
        name,
        path: Some(
            ExecutablePath::new(PathBuf::from("/usr/bin/app")).expect("chemin absolu valide"),
        ),
        cmdline: None,
        icon: None,
    }))
}

fn arb_connection() -> impl Strategy<Value = Connection> {
    (
        arb_protocol(),
        arb_socket_address(),
        arb_socket_address(),
        arb_direction(),
        arb_process(),
    )
        .prop_map(
            |(protocol, source, destination, direction, process)| Connection {
                // Valeurs figées pour un shrinking déterministe / pinned for deterministic shrinking
                id: ConnectionId::from_uuid(uuid::Uuid::nil()),
                protocol,
                source,
                destination,
                direction,
                state: ConnectionState::New,
                process,
                user: None,
                bytes_sent: 0,
                bytes_received: 0,
                started_at: chrono::DateTime::from_timestamp(0, 0).expect("epoch valide"),
                verdict: ConnectionVerdict::Unknown,
                matched_rule: None,
                remote_hostname: None,
            },
        )
}

fn arb_policy() -> impl Strategy<Value = DefaultPolicy> {
    prop_oneof![
        Just(DefaultPolicy::Ask),
        Just(DefaultPolicy::Allow),
        Just(DefaultPolicy::Block),
    ]
}

fn arb_ip_matcher() -> impl Strategy<Value = IpMatcher> {
    prop_oneof![
        arb_ip().prop_map(IpMatcher::Exact),
        (arb_ip(), 0u8..=128).prop_map(|(network, prefix_len)| IpMatcher::Cidr {
            network,
            prefix_len,
        }),
        (arb_ip(), arb_ip()).prop_map(|(start, end)| IpMatcher::Range { start, end }),
    ]
}

fn arb_port_matcher() -> impl Strategy<Value = PortMatcher> {
    prop_oneof![
        arb_port().prop_map(PortMatcher::Exact),
        (arb_port(), arb_port()).prop_map(|(a, b)| {
            let (start, end) = if a.value() <= b.value() {
                (a, b)
            } else {
                (b, a)
            };
            PortMatcher::Range { start, end }
        }),
    ]
}

fn arb_criteria() -> impl Strategy<Value = RuleCriteria> {
    (
        proptest::option::of(arb_ip_matcher()),
        proptest::option::of(arb_port_matcher()),
        proptest::option::of(arb_protocol()),
        proptest::option::of(arb_direction()),
    )
        .prop_map(
            |(remote_ip, remote_port, protocol, direction)| RuleCriteria {
                application: None,
                user: None,
                remote_ip,
                remote_port,
                local_port: None,
                protocol,
                direction,
                schedule: None,
            },
        )
}

fn arb_effect() -> impl Strategy<Value = RuleEffect> {
    prop_oneof![
        Just(RuleEffect::Allow),
        Just(RuleEffect::Block),
        Just(RuleEffect::Ask),
        Just(RuleEffect::Observe),
    ]
}

fn arb_rule() -> impl Strategy<Value = Rule> {
    (
        0u32..1000,
        any::<bool>(),
        arb_criteria(),
        arb_effect(),
        any::<bool>(),
    )
        .prop_map(|(priority, enabled, criteria, effect, expired)| Rule {
            id: RuleId::new(),
            name: "prop rule".to_string(),
            priority: RulePriority::new(priority),
            enabled,
            criteria,
            effect,
            scope: if expired {
                RuleScope::Temporary {
                    expires_at: Utc::now() - Duration::hours(1),
                }
            } else {
                RuleScope::Permanent
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        })
}

/// Règle décisive (Allow/Block) toujours active : l'ordre de matching est observable.
/// Decisive rule (Allow/Block), always enabled: matching order is observable.
fn arb_decisive_rule() -> impl Strategy<Value = Rule> {
    (
        0u32..1000,
        arb_criteria(),
        prop_oneof![Just(RuleEffect::Allow), Just(RuleEffect::Block)],
    )
        .prop_map(|(priority, criteria, effect)| Rule {
            id: RuleId::new(),
            name: "decisive rule".to_string(),
            priority: RulePriority::new(priority),
            enabled: true,
            criteria,
            effect,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        })
}

// --- Helpers ---

/// Connexion sortante fixe vers l'IP donnée (port 443).
/// Fixed outbound connection to the given IP (port 443).
fn connection_to(ip: IpAddr) -> Connection {
    Connection {
        id: ConnectionId::from_uuid(uuid::Uuid::nil()),
        protocol: Protocol::Tcp,
        source: SocketAddress::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)),
            Port::new(45000).expect("port non nul"),
        ),
        destination: SocketAddress::new(ip, Port::new(443).expect("port non nul")),
        direction: Direction::Outbound,
        state: ConnectionState::New,
        process: None,
        user: None,
        bytes_sent: 0,
        bytes_received: 0,
        started_at: chrono::DateTime::from_timestamp(0, 0).expect("epoch valide"),
        verdict: ConnectionVerdict::Unknown,
        matched_rule: None,
        remote_hostname: None,
    }
}

/// Connexion sortante fixe vers le port distant donné.
/// Fixed outbound connection to the given remote port.
fn connection_to_port(port: u16) -> Connection {
    let mut conn = connection_to(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)));
    conn.destination.port = Port::new(port).expect("port non nul");
    conn
}

// --- Propriétés / Properties ---

proptest! {
    /// Sans règle, le verdict découle uniquement de la politique par défaut.
    /// With no rules, the verdict derives solely from the default policy.
    #[test]
    fn empty_rules_apply_default_policy(conn in arb_connection(), policy in arb_policy()) {
        let eval = PolicyEngine::evaluate(&conn, &[], policy);
        let expected = match policy {
            DefaultPolicy::Ask => ConnectionVerdict::PendingDecision,
            DefaultPolicy::Allow => ConnectionVerdict::Allowed,
            DefaultPolicy::Block => ConnectionVerdict::Blocked,
        };
        prop_assert_eq!(eval.verdict, expected);
        prop_assert!(eval.matched_rule_id.is_none());
    }

    /// evaluate ne panique jamais, quelles que soient les entrées.
    /// evaluate never panics, whatever the inputs.
    #[test]
    fn evaluate_never_panics(
        conn in arb_connection(),
        mut rules in proptest::collection::vec(arb_rule(), 0..20),
        policy in arb_policy(),
    ) {
        rules.sort_by_key(|r| r.priority);
        let _ = PolicyEngine::evaluate(&conn, &rules, policy);
    }

    /// Une règle matchée est toujours active, non expirée, et matche réellement.
    /// A matched rule is always enabled, not expired, and actually matches.
    #[test]
    fn matched_rule_is_enabled_not_expired_and_matches(
        conn in arb_connection(),
        mut rules in proptest::collection::vec(arb_rule(), 0..20),
        policy in arb_policy(),
    ) {
        rules.sort_by_key(|r| r.priority);
        let eval = PolicyEngine::evaluate(&conn, &rules, policy);
        if let Some(id) = eval.matched_rule_id {
            let rule = rules.iter().find(|r| r.id == id)
                .expect("matched_rule_id référence une règle de la liste");
            prop_assert!(rule.enabled);
            prop_assert!(!rule.is_expired());
            prop_assert!(PolicyEngine::matches(&rule.criteria, &conn));
        }
    }

    /// La première règle décisive (triée par priorité) qui matche gagne.
    /// The first matching decisive rule (sorted by priority) wins.
    #[test]
    fn first_matching_decisive_rule_wins(
        conn in arb_connection(),
        mut rules in proptest::collection::vec(arb_decisive_rule(), 0..20),
    ) {
        rules.sort_by_key(|r| r.priority);
        let expected = rules
            .iter()
            .find(|r| PolicyEngine::matches(&r.criteria, &conn))
            .map(|r| r.id);
        let eval = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Block);
        prop_assert_eq!(eval.matched_rule_id, expected);
    }

    /// Un matcher IP d'une famille ne matche jamais une IP de l'autre famille.
    /// An IP matcher of one family never matches an IP of the other family.
    #[test]
    fn ip_family_mismatch_never_matches(v4 in any::<[u8; 4]>(), v6 in any::<[u8; 16]>()) {
        let matcher_v4 = IpMatcher::Exact(IpAddr::V4(Ipv4Addr::from(v4)));
        let criteria_v4 = RuleCriteria {
            remote_ip: Some(matcher_v4),
            application: None,
            user: None,
            remote_port: None,
            local_port: None,
            protocol: None,
            direction: None,
            schedule: None,
        };
        prop_assert!(!PolicyEngine::matches(
            &criteria_v4,
            &connection_to(IpAddr::V6(Ipv6Addr::from(v6))),
        ));
        let matcher_v6 = IpMatcher::Cidr {
            network: IpAddr::V6(Ipv6Addr::from(v6)),
            prefix_len: 64,
        };
        let criteria_v6 = RuleCriteria {
            remote_ip: Some(matcher_v6),
            application: None,
            user: None,
            remote_port: None,
            local_port: None,
            protocol: None,
            direction: None,
            schedule: None,
        };
        prop_assert!(!PolicyEngine::matches(
            &criteria_v6,
            &connection_to(IpAddr::V4(Ipv4Addr::from(v4))),
        ));
        let matcher_v6_exact = IpMatcher::Exact(IpAddr::V6(Ipv6Addr::from(v6)));
        let criteria_v6_exact = RuleCriteria {
            remote_ip: Some(matcher_v6_exact),
            application: None,
            user: None,
            remote_port: None,
            local_port: None,
            protocol: None,
            direction: None,
            schedule: None,
        };
        prop_assert!(!PolicyEngine::matches(
            &criteria_v6_exact,
            &connection_to(IpAddr::V4(Ipv4Addr::from(v4))),
        ));
        let matcher_v4_cidr = IpMatcher::Cidr {
            network: IpAddr::V4(Ipv4Addr::from(v4)),
            prefix_len: 24,
        };
        let criteria_v4_cidr = RuleCriteria {
            remote_ip: Some(matcher_v4_cidr),
            application: None,
            user: None,
            remote_port: None,
            local_port: None,
            protocol: None,
            direction: None,
            schedule: None,
        };
        prop_assert!(!PolicyEngine::matches(
            &criteria_v4_cidr,
            &connection_to(IpAddr::V6(Ipv6Addr::from(v6))),
        ));
    }

    /// Les bornes d'un PortMatcher::Range sont inclusives.
    /// PortMatcher::Range bounds are inclusive.
    #[test]
    fn port_range_bounds_inclusive(start in 1u16..=u16::MAX, end in 1u16..=u16::MAX) {
        let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
        let matcher = PortMatcher::Range {
            start: Port::new(lo).expect("port non nul"),
            end: Port::new(hi).expect("port non nul"),
        };
        let criteria = RuleCriteria {
            remote_port: Some(matcher),
            application: None,
            user: None,
            remote_ip: None,
            local_port: None,
            protocol: None,
            direction: None,
            schedule: None,
        };
        prop_assert!(PolicyEngine::matches(&criteria, &connection_to_port(lo)));
        prop_assert!(PolicyEngine::matches(&criteria, &connection_to_port(hi)));
    }
}
