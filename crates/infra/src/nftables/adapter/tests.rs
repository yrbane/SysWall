//! Tests unitaires pour le module adapter.
//! Unit tests for the adapter module.
use super::*;
use chrono::Utc;
use syswall_domain::entities::{RuleCriteria, RuleEffect, RuleId, RuleScope, RuleSource};
use syswall_domain::value_objects::{Protocol, RulePriority};

/// Construit une Rule de test avec protocole et ports optionnels.
/// Builds a test Rule with optional protocol and ports.
fn build_test_rule(
    protocol: Option<Protocol>,
    remote_port: Option<u16>,
    local_port: Option<u16>,
    remote_ip: Option<syswall_domain::entities::IpMatcher>,
) -> Rule {
    use syswall_domain::entities::PortMatcher;
    use syswall_domain::value_objects::Port;
    Rule {
        id: RuleId::new(),
        name: "whitelist test rule".to_string(),
        priority: RulePriority::new(100),
        enabled: true,
        criteria: RuleCriteria {
            protocol,
            remote_port: remote_port.map(|p| PortMatcher::Exact(Port::new(p).unwrap())),
            local_port: local_port.map(|p| PortMatcher::Exact(Port::new(p).unwrap())),
            remote_ip,
            ..Default::default()
        },
        effect: RuleEffect::Allow,
        scope: RuleScope::Permanent,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        source: RuleSource::Manual,
    }
}

#[test]
fn whitelist_dns_udp() {
    let rule = build_test_rule(Some(Protocol::Udp), Some(53), None, None);
    assert!(whitelist::is_whitelist_rule(&rule));
}

#[test]
fn whitelist_dns_tcp() {
    let rule = build_test_rule(Some(Protocol::Tcp), Some(53), None, None);
    assert!(whitelist::is_whitelist_rule(&rule));
}

#[test]
fn whitelist_dhcp_67() {
    let rule = build_test_rule(Some(Protocol::Udp), Some(67), None, None);
    assert!(whitelist::is_whitelist_rule(&rule));
}

#[test]
fn whitelist_ntp() {
    let rule = build_test_rule(Some(Protocol::Udp), Some(123), None, None);
    assert!(whitelist::is_whitelist_rule(&rule));
}

#[test]
fn whitelist_random_port_is_not_whitelist() {
    let rule = build_test_rule(Some(Protocol::Tcp), Some(443), None, None);
    assert!(!whitelist::is_whitelist_rule(&rule));
}

#[test]
fn nftables_config_default_values() {
    let config = NftablesConfig::default();
    assert_eq!(config.table_name, "syswall");
    assert_eq!(config.nft_binary_path, PathBuf::from("/usr/sbin/nft"));
    assert_eq!(config.command_timeout, Duration::from_secs(5));
    assert_eq!(config.max_output_bytes, 1_048_576);
}

#[test]
fn adapter_fails_with_missing_nft_binary() {
    let config = NftablesConfig {
        nft_binary_path: PathBuf::from("/nonexistent/nft"),
        ..Default::default()
    };
    let result = NftablesFirewallAdapter::new(config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        DomainError::Infrastructure(msg) => {
            assert!(msg.contains("nft binary not found"));
        }
        _ => panic!("Expected Infrastructure error"),
    }
}

#[test]
fn whitelist_dhcp_68() {
    let rule = build_test_rule(Some(Protocol::Udp), Some(68), None, None);
    assert!(whitelist::is_whitelist_rule(&rule));
}

#[test]
fn whitelist_loopback_ipv4() {
    let mut rule = build_test_rule(Some(Protocol::Tcp), Some(443), None, None);
    rule.criteria.remote_ip = Some(syswall_domain::entities::IpMatcher::Exact(
        "127.0.0.1".parse().unwrap(),
    ));
    assert!(whitelist::is_whitelist_rule(&rule));
}
