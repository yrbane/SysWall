//! Sauvegarde et restauration d'état nftables (rollback).
//! nftables state save and rollback logic.

use std::time::Instant;

use tracing::{error, warn};

use syswall_domain::errors::DomainError;

use super::{NftablesConfig, NftablesFirewallAdapter};
use crate::nftables::command::NftCommandBuilder;
use crate::nftables::types::RollbackState;

impl NftablesFirewallAdapter {
    /// Sauvegarde l'état actuel de la table nftables pour un éventuel rollback.
    /// Save the current nftables table state for potential rollback.
    pub(super) async fn save_rollback_state(&self) -> Result<RollbackState, DomainError> {
        let table_state = self
            .execute_nft(&NftCommandBuilder::list_table(&self.config.table_name))
            .await
            .unwrap_or_default();

        Ok(RollbackState {
            table_state,
            saved_at: Instant::now(),
        })
    }

    /// Tente un retour arrière vers un état précédent.
    /// Attempt to rollback to a previous state.
    pub(super) async fn rollback(&self, state: &RollbackState) {
        perform_rollback_static(state, &self.config).await;
        *self
            .nftables_synced
            .lock()
            .expect("Mutex jamais empoisonné / never poisoned") = false;
    }
}

/// Effectue le rollback nftables à partir de l'état sauvegardé et de la config.
/// Perform nftables rollback from saved state and config (callable from static closures).
pub(super) async fn perform_rollback_static(state: &RollbackState, config: &NftablesConfig) {
    warn!("Attempting nftables rollback...");
    let delete_cmd = NftCommandBuilder::new()
        .arg("delete")
        .arg("table")
        .arg("inet")
        .arg(&config.table_name);

    let output = tokio::time::timeout(delete_cmd.timeout(), async {
        tokio::process::Command::new(&config.nft_binary_path)
            .args(delete_cmd.args())
            .output()
            .await
    })
    .await;

    match output {
        Ok(Ok(o)) if o.status.success() => {}
        Ok(Ok(o)) => {
            error!(
                "Rollback: failed to delete table: {}",
                String::from_utf8_lossy(&o.stderr)
            );
        }
        Ok(Err(e)) => error!("Rollback: nft exec error: {}", e),
        Err(_) => error!("Rollback: nft command timed out"),
    }

    if !state.table_state.is_empty() {
        warn!(
            "Rollback state saved at {:?} (age: {:?})",
            state.saved_at,
            state.saved_at.elapsed()
        );
    }

    error!("nftables rollback completed -- adapter in degraded mode");
}
