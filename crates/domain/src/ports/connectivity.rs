use async_trait::async_trait;
use thiserror::Error;

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
        assert_eq!(err.to_string(), "Probe configuration error: empty endpoints");
    }
}
