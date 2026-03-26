use async_trait::async_trait;
use std::sync::Mutex;

use syswall_domain::entities::{Rule, RuleId};
use syswall_domain::errors::DomainError;
use syswall_domain::events::FirewallStatus;
use syswall_domain::ports::FirewallEngine;

/// Record of a call made to the firewall engine.
/// Enregistrement d'un appel effectué au moteur de pare-feu.
#[derive(Debug, Clone)]
pub enum FirewallCall {
    ApplyRule(RuleId),
    RemoveRule(RuleId),
    SyncAll(usize),
}

/// In-memory fake firewall engine for testing.
/// Moteur de pare-feu factice en mémoire pour les tests.
pub struct FakeFirewallEngine {
    pub calls: Mutex<Vec<FirewallCall>>,
}

impl Default for FakeFirewallEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeFirewallEngine {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl FirewallEngine for FakeFirewallEngine {
    async fn apply_rule(&self, rule: &Rule) -> Result<(), DomainError> {
        self.calls
            .lock()
            .unwrap()
            .push(FirewallCall::ApplyRule(rule.id));
        Ok(())
    }

    async fn remove_rule(&self, rule_id: &RuleId) -> Result<(), DomainError> {
        self.calls
            .lock()
            .unwrap()
            .push(FirewallCall::RemoveRule(*rule_id));
        Ok(())
    }

    async fn sync_all_rules(&self, rules: &[Rule]) -> Result<(), DomainError> {
        self.calls
            .lock()
            .unwrap()
            .push(FirewallCall::SyncAll(rules.len()));
        Ok(())
    }

    async fn get_status(&self) -> Result<FirewallStatus, DomainError> {
        Ok(FirewallStatus {
            enabled: true,
            active_rules_count: 0,
            nftables_synced: true,
            uptime_secs: 0,
            version: "0.1.0".to_string(),
        })
    }
}
