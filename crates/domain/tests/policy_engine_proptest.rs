//! Property tests du PolicyEngine : invariants vérifiés sur entrées arbitraires.
//! PolicyEngine property tests: invariants checked against arbitrary inputs.

use chrono::Utc;
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
                id: ConnectionId::new(),
                protocol,
                source,
                destination,
                direction,
                state: ConnectionState::New,
                process,
                user: None,
                bytes_sent: 0,
                bytes_received: 0,
                started_at: Utc::now(),
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
}
