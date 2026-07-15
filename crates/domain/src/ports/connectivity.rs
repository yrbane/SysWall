use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use thiserror::Error;

use crate::errors::DomainError;

/// Outcome of a connectivity probe.
/// Résultat d'une sonde de connectivité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// At least one endpoint responded (including ConnectionRefused: the network is reachable).
    /// Au moins un endpoint a répondu (y compris ConnectionRefused : le réseau est joignable).
    Reachable,
    /// All endpoints timed out (network is likely lost).
    /// Tous les endpoints ont timeouté (le réseau est probablement perdu).
    Unreachable,
}

/// Errors emitted by a connectivity probe (configuration only — runtime failures map to Unreachable).
/// Erreurs émises par une sonde de connectivité (configuration uniquement — les erreurs d'exécution donnent Unreachable).
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ProbeError {
    #[error("Probe configuration error: {0}")]
    Configuration(String),
}

/// Port: tests whether the host has external network connectivity.
/// Port : teste si l'hôte a une connectivité réseau externe.
#[async_trait]
pub trait ConnectivityProbe: Send + Sync {
    async fn probe(&self) -> Result<ProbeOutcome, ProbeError>;
}

/// Closure exécutée quand la connectivité est perdue. Effectue le rollback réel.
/// Closure executed when connectivity is lost. Performs the actual rollback.
pub type ArmedRollback = Box<
    dyn FnOnce() -> Pin<Box<dyn Future<Output = Result<(), DomainError>> + Send + 'static>>
        + Send
        + 'static,
>;

/// Arme un rollback différé qui s'exécutera si la connectivité est perdue.
/// Arms a deferred rollback that will execute if connectivity is lost.
#[async_trait]
pub trait LockoutGuard: Send + Sync {
    async fn arm_rollback(
        &self,
        rolled_back_count: usize,
        rollback: ArmedRollback,
    ) -> Result<(), DomainError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_outcome_equality() {
        assert_eq!(ProbeOutcome::Reachable, ProbeOutcome::Reachable);
        assert_ne!(ProbeOutcome::Reachable, ProbeOutcome::Unreachable);
    }

    #[test]
    fn probe_error_displays_configuration() {
        let err = ProbeError::Configuration("empty endpoints".into());
        assert_eq!(
            err.to_string(),
            "Probe configuration error: empty endpoints"
        );
    }

    #[test]
    fn armed_rollback_type_compiles() {
        let _f: ArmedRollback = Box::new(|| Box::pin(async { Ok(()) }));
    }
}
