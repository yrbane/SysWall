use syswall_domain::entities::{
    IpMatcher, PortMatcher, RuleCriteria, RuleEffect, RuleScope, RuleSource,
};
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{RuleFilters, RuleRepository};
use syswall_domain::value_objects::{Direction, Port, Protocol};
use tracing::info;

use crate::commands::CreateRuleCommand;
use crate::services::rule_service::RuleService;

/// Ensure the system whitelist exists. Creates default rules on first start.
/// S'assure que la liste blanche systeme existe. Cree les regles par defaut au premier demarrage.
pub async fn ensure_system_whitelist(
    rule_service: &RuleService,
    rule_repo: &dyn RuleRepository,
) -> Result<(), DomainError> {
    let existing = rule_repo
        .find_all(
            &RuleFilters {
                source: Some(RuleSource::System),
                ..Default::default()
            },
            &Pagination {
                offset: 0,
                limit: 1,
            },
        )
        .await?;

    if !existing.is_empty() {
        info!(
            "System whitelist already exists ({} rules found)",
            existing.len()
        );
        return Ok(());
    }

    info!("Creating system whitelist rules (first start)...");

    let whitelist = vec![
        create_system_rule(
            "Allow DNS (UDP)",
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                remote_port: Some(PortMatcher::Exact(Port::new(53).unwrap())),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow DNS (TCP)",
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                remote_port: Some(PortMatcher::Exact(Port::new(53).unwrap())),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow DHCP Client",
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                local_port: Some(PortMatcher::Exact(Port::new(68).unwrap())),
                remote_port: Some(PortMatcher::Exact(Port::new(67).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow DHCP Server Response",
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                local_port: Some(PortMatcher::Exact(Port::new(67).unwrap())),
                remote_port: Some(PortMatcher::Exact(Port::new(68).unwrap())),
                direction: Some(Direction::Inbound),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow Loopback (IPv4)",
            RuleCriteria {
                remote_ip: Some(IpMatcher::Cidr {
                    network: "127.0.0.0".parse().unwrap(),
                    prefix_len: 8,
                }),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow Loopback (IPv6)",
            RuleCriteria {
                remote_ip: Some(IpMatcher::Exact("::1".parse().unwrap())),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow NTP",
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                remote_port: Some(PortMatcher::Exact(Port::new(123).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        ),
    ];

    for cmd in whitelist {
        rule_service.create_rule(cmd).await?;
    }

    info!("System whitelist created successfully (7 rules)");
    Ok(())
}

fn create_system_rule(name: &str, criteria: RuleCriteria) -> CreateRuleCommand {
    CreateRuleCommand {
        name: name.to_string(),
        priority: 0,
        criteria,
        effect: RuleEffect::Allow,
        scope: RuleScope::Permanent,
        source: RuleSource::System,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn creates_whitelist_on_first_start() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let rule_service = RuleService::new(rule_repo.clone(), firewall, event_bus);

        ensure_system_whitelist(&rule_service, rule_repo.as_ref())
            .await
            .unwrap();

        let all_rules = rule_repo
            .find_all(
                &RuleFilters::default(),
                &Pagination {
                    offset: 0,
                    limit: 100,
                },
            )
            .await
            .unwrap();

        assert_eq!(all_rules.len(), 7);
        assert!(all_rules.iter().all(|r| r.source == RuleSource::System));
        assert!(all_rules.iter().all(|r| r.effect == RuleEffect::Allow));
        assert!(all_rules.iter().all(|r| r.enabled));
    }

    #[tokio::test]
    async fn skips_if_system_rules_exist() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let rule_service =
            RuleService::new(rule_repo.clone(), firewall.clone(), event_bus.clone());

        // Create whitelist first time
        ensure_system_whitelist(&rule_service, rule_repo.as_ref())
            .await
            .unwrap();

        // Call again -- should not create duplicates
        let rule_service2 = RuleService::new(rule_repo.clone(), firewall, event_bus);
        ensure_system_whitelist(&rule_service2, rule_repo.as_ref())
            .await
            .unwrap();

        let all_rules = rule_repo
            .find_all(
                &RuleFilters::default(),
                &Pagination {
                    offset: 0,
                    limit: 100,
                },
            )
            .await
            .unwrap();

        assert_eq!(all_rules.len(), 7);
    }

    #[tokio::test]
    async fn whitelist_contains_dns_rules() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let rule_service = RuleService::new(rule_repo.clone(), firewall, event_bus);

        ensure_system_whitelist(&rule_service, rule_repo.as_ref())
            .await
            .unwrap();

        let all_rules = rule_repo
            .find_all(
                &RuleFilters::default(),
                &Pagination {
                    offset: 0,
                    limit: 100,
                },
            )
            .await
            .unwrap();

        let dns_rules: Vec<_> = all_rules
            .iter()
            .filter(|r| r.name.contains("DNS"))
            .collect();
        assert_eq!(dns_rules.len(), 2); // UDP + TCP
    }
}
