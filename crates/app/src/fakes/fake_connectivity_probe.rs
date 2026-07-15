use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use syswall_domain::ports::connectivity::{ConnectivityProbe, ProbeError, ProbeOutcome};

/// Programmable fake for `ConnectivityProbe` used by tests.
/// Fake programmable pour `ConnectivityProbe` utilisé dans les tests.
#[derive(Debug, Clone)]
pub struct FakeConnectivityProbe {
    /// Pre-programmed sequence of outcomes; the last value repeats once exhausted.
    /// Séquence d'outcomes pré-programmée ; la dernière valeur se répète une fois épuisée.
    sequence: Arc<Vec<Result<ProbeOutcome, ProbeError>>>,
    cursor: Arc<AtomicUsize>,
    call_count: Arc<AtomicUsize>,
}

impl FakeConnectivityProbe {
    pub fn always_reachable() -> Self {
        Self::with_sequence(vec![Ok(ProbeOutcome::Reachable)])
    }

    pub fn always_unreachable() -> Self {
        Self::with_sequence(vec![Ok(ProbeOutcome::Unreachable)])
    }

    pub fn with_sequence(seq: Vec<Result<ProbeOutcome, ProbeError>>) -> Self {
        assert!(
            !seq.is_empty(),
            "FakeConnectivityProbe requires at least one outcome"
        );
        Self {
            sequence: Arc::new(seq),
            cursor: Arc::new(AtomicUsize::new(0)),
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ConnectivityProbe for FakeConnectivityProbe {
    async fn probe(&self) -> Result<ProbeOutcome, ProbeError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let idx = self
            .cursor
            .fetch_add(1, Ordering::SeqCst)
            .min(self.sequence.len() - 1);
        self.sequence[idx].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn always_reachable_returns_reachable() {
        let probe = FakeConnectivityProbe::always_reachable();
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Reachable);
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Reachable);
        assert_eq!(probe.call_count(), 2);
    }

    #[tokio::test]
    async fn sequence_repeats_last_value() {
        let probe = FakeConnectivityProbe::with_sequence(vec![
            Ok(ProbeOutcome::Unreachable),
            Ok(ProbeOutcome::Reachable),
        ]);
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Unreachable);
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Reachable);
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Reachable);
    }
}
