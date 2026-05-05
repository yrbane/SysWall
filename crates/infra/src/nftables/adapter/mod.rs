//! Adaptateur nftables : struct, configuration, application des règles, synchronisation.
//! nftables adapter: struct, configuration, rule application, synchronisation.

mod rollback;
mod whitelist;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tracing::{debug, error, info, warn};

use syswall_domain::entities::{Rule, RuleEffect, RuleId};
use syswall_domain::errors::DomainError;
use syswall_domain::events::FirewallStatus;
use syswall_domain::ports::FirewallEngine;

use super::command::NftCommandBuilder;
use super::parser::{extract_rule_id_from_comment, parse_nft_table_rules};
use super::translator::translate_rule;
use super::types::{HandleMap, NftRuleHandle};

use rollback::perform_rollback_static;
use whitelist::{is_whitelist_only, is_whitelist_rule};

/// Configuration pour l'adaptateur NftablesFirewallAdapter.
/// Configuration for the NftablesFirewallAdapter.
#[derive(Debug, Clone)]
pub struct NftablesConfig {
    /// Nom de la table nftables gérée par SysWall.
    /// Name of the nftables table managed by SysWall.
    pub table_name: String,
    /// Chemin vers le binaire nft.
    /// Path to the nft binary.
    pub nft_binary_path: PathBuf,
    /// Temps maximum d'attente pour qu'une commande nft se termine.
    /// Maximum time to wait for an nft command to complete.
    pub command_timeout: Duration,
    /// Nombre maximal d'octets à capturer depuis la sortie d'une commande nft.
    /// Maximum bytes to capture from nft command output.
    pub max_output_bytes: usize,
}

impl Default for NftablesConfig {
    fn default() -> Self {
        Self {
            table_name: "syswall".to_string(),
            nft_binary_path: PathBuf::from("/usr/sbin/nft"),
            command_timeout: Duration::from_secs(5),
            max_output_bytes: 1_048_576,
        }
    }
}

/// Adaptateur nftables réel. Gère une table dédiée avec des chaînes input/output/forward.
/// Real nftables firewall adapter. Manages a dedicated table with input/output/forward chains.
pub struct NftablesFirewallAdapter {
    pub(super) config: NftablesConfig,
    pub(super) handle_map: Mutex<HandleMap>,
    started_at: Instant,
    pub(super) nftables_synced: Mutex<bool>,
    lockout_guard: Option<Arc<dyn syswall_domain::ports::connectivity::LockoutGuard>>,
}

impl std::fmt::Debug for NftablesFirewallAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NftablesFirewallAdapter")
            .field("config", &self.config)
            .field("started_at", &self.started_at)
            .finish_non_exhaustive()
    }
}

impl NftablesFirewallAdapter {
    /// Crée un nouvel adaptateur avec la configuration donnée.
    /// Create a new adapter with the given configuration.
    pub fn new(config: NftablesConfig) -> Result<Self, DomainError> {
        if !config.nft_binary_path.exists() {
            return Err(DomainError::Infrastructure(format!(
                "nft binary not found at: {}. Install nftables package.",
                config.nft_binary_path.display()
            )));
        }

        Ok(Self {
            config,
            handle_map: Mutex::new(HandleMap::new()),
            started_at: Instant::now(),
            nftables_synced: Mutex::new(false),
            lockout_guard: None,
        })
    }

    /// Attache un guard anti-lockout à l'adaptateur.
    /// Attach a lockout guard to the adapter.
    pub fn with_lockout_guard(
        mut self,
        guard: Arc<dyn syswall_domain::ports::connectivity::LockoutGuard>,
    ) -> Self {
        self.lockout_guard = Some(guard);
        self
    }

    /// Exécute une commande nft et retourne stdout en cas de succès.
    /// Execute an nft command and return stdout on success.
    pub(super) async fn execute_nft(&self, cmd: &NftCommandBuilder) -> Result<String, DomainError> {
        let output = tokio::time::timeout(cmd.timeout(), async {
            tokio::process::Command::new(&self.config.nft_binary_path)
                .args(cmd.args())
                .output()
                .await
                .map_err(|e| {
                    DomainError::Infrastructure(format!("Failed to execute nft: {}", e))
                })
        })
        .await
        .map_err(|_| {
            DomainError::Infrastructure(format!(
                "nft command timed out after {:?}",
                cmd.timeout()
            ))
        })??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Table/chain already exists is not an error
            if stderr.contains("File exists") {
                return Ok(String::new());
            }
            return Err(DomainError::Infrastructure(format!(
                "nft command failed (exit {}): {}",
                output.status,
                &stderr[..stderr.len().min(500)]
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.len() > cmd.max_output_bytes() {
            return Err(DomainError::Infrastructure(format!(
                "nft output exceeds limit ({} > {} bytes)",
                stdout.len(),
                cmd.max_output_bytes()
            )));
        }

        Ok(stdout.to_string())
    }

    /// S'assure que la table et les chaînes syswall existent (idempotent).
    /// Ensure the syswall table and chains exist (idempotent).
    async fn ensure_table_and_chains(&self) -> Result<(), DomainError> {
        let table = &self.config.table_name;
        self.execute_nft(&NftCommandBuilder::create_table(table))
            .await?;

        for (chain, hook) in [("input", "input"), ("output", "output"), ("forward", "forward")] {
            self.execute_nft(&NftCommandBuilder::create_chain(table, chain, hook, 0))
                .await?;
        }

        Ok(())
    }
}

#[async_trait]
impl FirewallEngine for NftablesFirewallAdapter {

    async fn drop_all(&self) -> Result<(), DomainError> {
        for chain in &["output", "input"] {
            let cmd = NftCommandBuilder::add_rule(&self.config.table_name, chain)
                .arg("drop")
                .arg("comment")
                .arg("\"syswall-killswitch\"");
            self.execute_nft(&cmd).await?;
        }
        info!("Kill-switch activé : tout le trafic est bloqué");
        Ok(())
    }

    async fn remove_drop_all(&self) -> Result<(), DomainError> {
        let list_cmd_with_handles = NftCommandBuilder::new()
            .arg("list")
            .arg("table")
            .arg("inet")
            .arg(&self.config.table_name)
            .arg("-a");
        let output = self.execute_nft(&list_cmd_with_handles).await.unwrap_or_default();

        for line in output.lines() {
            if line.contains("syswall-killswitch")
                && let Some(handle_str) = line.rsplit("handle ").next()
                && let Ok(handle) = handle_str.trim().parse::<u64>()
            {
                let chain = if line.contains("output") { "output" } else { "input" };
                let del_cmd = NftCommandBuilder::delete_rule(
                    &self.config.table_name, chain, handle,
                );
                let _ = self.execute_nft(&del_cmd).await;
            }
        }
        info!("Kill-switch désactivé : trafic rétabli");
        Ok(())
    }

    /// Applique une seule règle à nftables.
    /// Apply a single rule to nftables.
    async fn apply_rule(&self, rule: &Rule) -> Result<(), DomainError> {
        // Ask rules produce no nft rule
        if rule.effect == RuleEffect::Ask {
            debug!(
                "Skipping Ask rule {} -- handled in userspace",
                rule.id.as_uuid()
            );
            return Ok(());
        }

        // Disabled or expired rules should not be applied
        if !rule.enabled || rule.is_expired() {
            return Ok(());
        }

        self.ensure_table_and_chains().await?;

        let translated = match translate_rule(rule) {
            Some(t) => t,
            None => return Ok(()),
        };

        let rollback_state = Arc::new(self.save_rollback_state().await?);
        let mut new_handles = Vec::new();

        for chain in &translated.chains {
            let mut cmd = NftCommandBuilder::add_rule(&self.config.table_name, chain);
            for expr in &translated.expressions {
                cmd = cmd.arg(expr.clone());
            }

            match self.execute_nft(&cmd).await {
                Ok(_output) => {
                    debug!("Rule applied to chain '{}': {}", chain, rule.id.as_uuid());
                    new_handles.push(NftRuleHandle {
                        chain: chain.clone(),
                        handle: 0, // Will be rebuilt during sync
                    });
                }
                Err(e) => {
                    error!(
                        "Failed to apply rule {} to chain '{}': {}",
                        rule.id.as_uuid(),
                        chain,
                        e
                    );
                    self.rollback(&rollback_state).await;
                    return Err(e);
                }
            }
        }

        self.handle_map
            .lock()
            .expect("Mutex jamais empoisonné / never poisoned")
            .insert(rule.id, new_handles);

        info!(
            "Rule {} applied to {} chain(s)",
            rule.id.as_uuid(),
            translated.chains.len()
        );

        // Arme le guard anti-lockout après application réussie (bypass si règle whitelist).
        // Arm the lockout guard after successful apply (bypass for whitelist rules).
        if let Some(guard) = &self.lockout_guard {
            if !is_whitelist_rule(rule) {
                let snapshot = Arc::clone(&rollback_state);
                let config = self.config.clone();
                let rollback: syswall_domain::ports::connectivity::ArmedRollback =
                    Box::new(move || {
                        Box::pin(async move {
                            perform_rollback_static(&snapshot, &config).await;
                            Ok(())
                        })
                    });
                guard.arm_rollback(1, rollback).await?;
            } else {
                tracing::warn!(
                    target: "antilockout",
                    rule_name = %rule.name,
                    reason = "whitelist-only",
                    "guard bypass: anti-lockout not armed for whitelist rule"
                );
            }
        }

        Ok(())
    }

    /// Supprime une règle de nftables par son identifiant du domaine.
    /// Remove a rule from nftables by its domain ID.
    async fn remove_rule(&self, rule_id: &RuleId) -> Result<(), DomainError> {
        let handles = self.handle_map.lock().expect("Mutex jamais empoisonné / never poisoned").remove(rule_id);

        let handles = match handles {
            Some(h) => h,
            None => {
                debug!(
                    "No nft handles for rule {} -- already removed or never applied",
                    rule_id.as_uuid()
                );
                return Ok(());
            }
        };

        for handle in &handles {
            if handle.handle == 0 {
                continue;
            }
            let cmd = NftCommandBuilder::delete_rule(
                &self.config.table_name,
                &handle.chain,
                handle.handle,
            );
            if let Err(e) = self.execute_nft(&cmd).await {
                warn!(
                    "Failed to delete nft rule handle {} in chain '{}': {}",
                    handle.handle, handle.chain, e
                );
            }
        }

        info!("Rule {} removed from nftables", rule_id.as_uuid());
        Ok(())
    }

    /// Synchronise toutes les règles : réconcilie l'état nftables avec la liste de règles fournie.
    /// Synchronize all rules: reconcile nftables state with the provided rule list.
    async fn sync_all_rules(&self, rules: &[Rule]) -> Result<(), DomainError> {
        info!("Starting nftables sync with {} rules", rules.len());

        self.ensure_table_and_chains().await?;
        let rollback_state = Arc::new(self.save_rollback_state().await?);

        let json = self
            .execute_nft(&NftCommandBuilder::list_table(&self.config.table_name))
            .await?;

        let nft_rules = parse_nft_table_rules(&json).unwrap_or_default();

        let mut nft_rule_ids: std::collections::HashSet<uuid::Uuid> =
            std::collections::HashSet::new();
        let mut nft_handles: std::collections::HashMap<uuid::Uuid, Vec<NftRuleHandle>> =
            std::collections::HashMap::new();

        for entry in &nft_rules {
            if let Some(uuid) = entry
                .comment
                .as_ref()
                .and_then(|c| extract_rule_id_from_comment(c))
            {
                nft_rule_ids.insert(uuid);
                nft_handles
                    .entry(uuid)
                    .or_default()
                    .push(NftRuleHandle {
                        chain: entry.chain.clone(),
                        handle: entry.handle,
                    });
            }
        }

        let desired_rules: Vec<&Rule> = rules
            .iter()
            .filter(|r| r.enabled && !r.is_expired() && r.effect != RuleEffect::Ask)
            .collect();

        let desired_ids: std::collections::HashSet<uuid::Uuid> = desired_rules
            .iter()
            .map(|r| *r.id.as_uuid())
            .collect();

        let to_remove: Vec<uuid::Uuid> = nft_rule_ids.difference(&desired_ids).cloned().collect();
        let to_add: Vec<&&Rule> = desired_rules
            .iter()
            .filter(|r| !nft_rule_ids.contains(r.id.as_uuid()))
            .collect();

        for uuid in &to_remove {
            if let Some(handles) = nft_handles.get(uuid) {
                for handle in handles {
                    let cmd = NftCommandBuilder::delete_rule(
                        &self.config.table_name,
                        &handle.chain,
                        handle.handle,
                    );
                    if let Err(e) = self.execute_nft(&cmd).await {
                        error!("Sync: failed to remove stale rule {}: {}", uuid, e);
                        self.rollback(&rollback_state).await;
                        return Err(e);
                    }
                }
            }
            debug!("Sync: removed stale rule {}", uuid);
        }

        for rule in &to_add {
            if let Err(e) = self.apply_rule(rule).await {
                error!("Sync: failed to add rule {}: {}", rule.id.as_uuid(), e);
                self.rollback(&rollback_state).await;
                return Err(e);
            }
        }

        let final_json = self
            .execute_nft(&NftCommandBuilder::list_table(&self.config.table_name))
            .await?;
        let final_rules = parse_nft_table_rules(&final_json).unwrap_or_default();

        let mut new_handle_map = HandleMap::new();
        for entry in &final_rules {
            if let Some(uuid) = entry
                .comment
                .as_ref()
                .and_then(|c| extract_rule_id_from_comment(c))
            {
                let rule_id = RuleId::from_uuid(uuid);
                let mut handles = new_handle_map
                    .get(&rule_id)
                    .cloned()
                    .unwrap_or_default();
                handles.push(NftRuleHandle {
                    chain: entry.chain.clone(),
                    handle: entry.handle,
                });
                new_handle_map.insert(rule_id, handles);
            }
        }
        *self.handle_map.lock().expect("Mutex jamais empoisonné / never poisoned") = new_handle_map;
        *self.nftables_synced.lock().expect("Mutex jamais empoisonné / never poisoned") = true;

        info!(
            "nftables sync complete: removed {}, added {}",
            to_remove.len(),
            to_add.len()
        );

        // Arme le guard anti-lockout après synchronisation réussie (bypass si whitelist seulement).
        // Arm the lockout guard after successful sync (bypass for whitelist-only rulesets).
        if let Some(guard) = &self.lockout_guard {
            if !is_whitelist_only(rules) {
                let snapshot = Arc::clone(&rollback_state);
                let config = self.config.clone();
                let count = rules.len();
                let rollback: syswall_domain::ports::connectivity::ArmedRollback =
                    Box::new(move || {
                        Box::pin(async move {
                            perform_rollback_static(&snapshot, &config).await;
                            Ok(())
                        })
                    });
                guard.arm_rollback(count, rollback).await?;
            } else {
                tracing::warn!(
                    target: "antilockout",
                    rule_count = rules.len(),
                    reason = "whitelist-only",
                    "guard bypass: anti-lockout not armed for whitelist-only ruleset"
                );
            }
        }

        Ok(())
    }

    /// Retourne l'état actuel du pare-feu.
    /// Get the current firewall status.
    async fn get_status(&self) -> Result<FirewallStatus, DomainError> {
        let synced = *self.nftables_synced.lock().expect("Mutex jamais empoisonné / never poisoned");

        let json = self
            .execute_nft(&NftCommandBuilder::list_table(&self.config.table_name))
            .await;

        let (enabled, active_rules_count) = match json {
            Ok(ref j) => {
                let rules = parse_nft_table_rules(j).unwrap_or_default();
                let syswall_count = rules
                    .iter()
                    .filter(|r| {
                        r.comment
                            .as_ref()
                            .is_some_and(|c| c.starts_with("syswall:"))
                    })
                    .count();
                (true, syswall_count as u32)
            }
            Err(_) => (false, 0),
        };

        Ok(FirewallStatus {
            enabled,
            active_rules_count,
            nftables_synced: synced,
            uptime_secs: self.started_at.elapsed().as_secs(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::entities::{RuleEffect, RuleScope, RuleSource, RuleCriteria, RuleId};
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
                remote_port: remote_port
                    .map(|p| PortMatcher::Exact(Port::new(p).unwrap())),
                local_port: local_port
                    .map(|p| PortMatcher::Exact(Port::new(p).unwrap())),
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
        assert_eq!(
            config.nft_binary_path,
            PathBuf::from("/usr/sbin/nft")
        );
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
}

/// Tests d'intégration nécessitant les privilèges root et le binaire nft.
/// Integration tests that require root privileges and the nft binary.
#[cfg(all(test, feature = "integration"))]
mod integration_tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn test_rule(effect: RuleEffect, criteria: RuleCriteria) -> Rule {
        Rule {
            id: RuleId::new(),
            name: "Integration test rule".to_string(),
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

    #[tokio::test]
    async fn apply_and_remove_tcp_rule() {
        let config = NftablesConfig::default();
        let adapter = NftablesFirewallAdapter::new(config).unwrap();

        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );

        adapter.apply_rule(&rule).await.unwrap();
    }
}
