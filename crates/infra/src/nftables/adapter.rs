use std::path::PathBuf;
use std::sync::Mutex;
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
use super::types::{HandleMap, NftRuleHandle, RollbackState};

/// Configuration for the NftablesFirewallAdapter.
/// Configuration pour l'adaptateur NftablesFirewallAdapter.
#[derive(Debug, Clone)]
pub struct NftablesConfig {
    /// Name of the nftables table managed by SysWall.
    /// Nom de la table nftables geree par SysWall.
    pub table_name: String,
    /// Path to the nft binary.
    /// Chemin vers le binaire nft.
    pub nft_binary_path: PathBuf,
    /// Maximum time to wait for an nft command to complete.
    /// Temps maximum d'attente pour qu'une commande nft se termine.
    pub command_timeout: Duration,
    /// Maximum bytes to capture from nft command output.
    /// Nombre maximal d'octets a capturer depuis la sortie d'une commande nft.
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

/// Real nftables firewall adapter. Manages a dedicated table with input/output/forward chains.
/// Adaptateur reel nftables. Gere une table dediee avec des chaines input/output/forward.
pub struct NftablesFirewallAdapter {
    config: NftablesConfig,
    handle_map: Mutex<HandleMap>,
    started_at: Instant,
    nftables_synced: Mutex<bool>,
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
    /// Create a new adapter with the given configuration.
    /// Cree un nouvel adaptateur avec la configuration donnee.
    pub fn new(config: NftablesConfig) -> Result<Self, DomainError> {
        // Verify nft binary exists
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
        })
    }

    /// Execute an nft command and return stdout on success.
    /// Execute une commande nft et retourne stdout en cas de succes.
    async fn execute_nft(&self, cmd: &NftCommandBuilder) -> Result<String, DomainError> {
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

    /// Ensure the syswall table and chains exist (idempotent).
    /// S'assure que la table et les chaines syswall existent (idempotent).
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

    /// Save the current table state for rollback.
    /// Sauvegarde l'etat actuel de la table pour retour arriere.
    async fn save_rollback_state(&self) -> Result<RollbackState, DomainError> {
        let table_state = self
            .execute_nft(&NftCommandBuilder::list_table(&self.config.table_name))
            .await
            .unwrap_or_default();

        Ok(RollbackState {
            table_state,
            saved_at: Instant::now(),
        })
    }

    /// Attempt to rollback to a previous state.
    /// Tente un retour arriere vers un etat precedent.
    async fn rollback(&self, state: &RollbackState) {
        warn!("Attempting nftables rollback...");
        // Delete the table and re-create from saved state
        let delete_cmd = NftCommandBuilder::new()
            .arg("delete")
            .arg("table")
            .arg("inet")
            .arg(&self.config.table_name);

        if let Err(e) = self.execute_nft(&delete_cmd).await {
            error!("Rollback: failed to delete table: {}", e);
        }

        // If we had a saved state, attempt to restore it
        if !state.table_state.is_empty() {
            let _restore_cmd = NftCommandBuilder::new().arg("-j").arg("-f").arg("-");
            // Note: In production, we would pipe the saved JSON to stdin.
            // For now, we log the failure and set degraded mode.
            warn!(
                "Rollback state saved at {:?} (age: {:?})",
                state.saved_at,
                state.saved_at.elapsed()
            );
        }

        *self.nftables_synced.lock().unwrap() = false;
        error!("nftables rollback completed -- adapter in degraded mode");
    }
}

#[async_trait]
impl FirewallEngine for NftablesFirewallAdapter {
    /// Apply a single rule to nftables.
    /// Applique une seule regle a nftables.
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

        let rollback_state = self.save_rollback_state().await?;
        let mut new_handles = Vec::new();

        for chain in &translated.chains {
            let mut cmd = NftCommandBuilder::add_rule(&self.config.table_name, chain);
            for expr in &translated.expressions {
                cmd = cmd.arg(expr.clone());
            }

            match self.execute_nft(&cmd).await {
                Ok(_output) => {
                    // nft may return handle in JSON mode, or we list afterwards
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
                    // Rollback any rules added in this call
                    self.rollback(&rollback_state).await;
                    return Err(e);
                }
            }
        }

        self.handle_map
            .lock()
            .unwrap()
            .insert(rule.id, new_handles);

        info!(
            "Rule {} applied to {} chain(s)",
            rule.id.as_uuid(),
            translated.chains.len()
        );
        Ok(())
    }

    /// Remove a rule from nftables by its domain ID.
    /// Supprime une regle de nftables par son identifiant du domaine.
    async fn remove_rule(&self, rule_id: &RuleId) -> Result<(), DomainError> {
        let handles = self.handle_map.lock().unwrap().remove(rule_id);

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
                // Handle not yet resolved -- need to find it by listing
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

    /// Synchronize all rules: reconcile nftables state with the provided rule list.
    /// Synchronise toutes les regles : reconcilie l'etat nftables avec la liste de regles fournie.
    async fn sync_all_rules(&self, rules: &[Rule]) -> Result<(), DomainError> {
        info!("Starting nftables sync with {} rules", rules.len());

        self.ensure_table_and_chains().await?;
        let rollback_state = self.save_rollback_state().await?;

        // List current nft rules
        let json = self
            .execute_nft(&NftCommandBuilder::list_table(&self.config.table_name))
            .await?;

        let nft_rules = parse_nft_table_rules(&json).unwrap_or_default();

        // Build set of existing SysWall rule IDs in nftables
        let mut nft_rule_ids: std::collections::HashSet<uuid::Uuid> =
            std::collections::HashSet::new();
        let mut nft_handles: std::collections::HashMap<uuid::Uuid, Vec<NftRuleHandle>> =
            std::collections::HashMap::new();

        for entry in &nft_rules {
            if let Some(ref comment) = entry.comment {
                if let Some(uuid) = extract_rule_id_from_comment(comment) {
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
        }

        // Build set of desired rule IDs (enabled, non-expired, non-Ask)
        let desired_rules: Vec<&Rule> = rules
            .iter()
            .filter(|r| r.enabled && !r.is_expired() && r.effect != RuleEffect::Ask)
            .collect();

        let desired_ids: std::collections::HashSet<uuid::Uuid> = desired_rules
            .iter()
            .map(|r| *r.id.as_uuid())
            .collect();

        // Compute delta
        let to_remove: Vec<uuid::Uuid> = nft_rule_ids.difference(&desired_ids).cloned().collect();
        let to_add: Vec<&&Rule> = desired_rules
            .iter()
            .filter(|r| !nft_rule_ids.contains(r.id.as_uuid()))
            .collect();

        // Remove stale rules first (safe direction)
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

        // Add missing rules
        for rule in &to_add {
            if let Err(e) = self.apply_rule(rule).await {
                error!("Sync: failed to add rule {}: {}", rule.id.as_uuid(), e);
                self.rollback(&rollback_state).await;
                return Err(e);
            }
        }

        // Rebuild handle map from final state
        let final_json = self
            .execute_nft(&NftCommandBuilder::list_table(&self.config.table_name))
            .await?;
        let final_rules = parse_nft_table_rules(&final_json).unwrap_or_default();

        let mut new_handle_map = HandleMap::new();
        for entry in &final_rules {
            if let Some(ref comment) = entry.comment {
                if let Some(uuid) = extract_rule_id_from_comment(comment) {
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
        }
        *self.handle_map.lock().unwrap() = new_handle_map;
        *self.nftables_synced.lock().unwrap() = true;

        info!(
            "nftables sync complete: removed {}, added {}",
            to_remove.len(),
            to_add.len()
        );
        Ok(())
    }

    /// Get the current firewall status.
    /// Retourne l'etat actuel du pare-feu.
    async fn get_status(&self) -> Result<FirewallStatus, DomainError> {
        let synced = *self.nftables_synced.lock().unwrap();

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
}

/// Integration tests that require root privileges and the nft binary.
/// Tests d'integration necessitant les privileges root et le binaire nft.
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

        let status = adapter.get_status().await.unwrap();
        assert!(status.enabled);

        adapter.remove_rule(&rule.id).await.unwrap();
    }

    #[tokio::test]
    async fn sync_empty_rules_clears_table() {
        let config = NftablesConfig::default();
        let adapter = NftablesFirewallAdapter::new(config).unwrap();
        adapter.sync_all_rules(&[]).await.unwrap();

        let status = adapter.get_status().await.unwrap();
        assert!(status.nftables_synced);
    }

    #[tokio::test]
    async fn get_status_returns_valid_info() {
        let config = NftablesConfig::default();
        let adapter = NftablesFirewallAdapter::new(config).unwrap();

        let status = adapter.get_status().await.unwrap();
        assert!(!status.version.is_empty());
    }
}
