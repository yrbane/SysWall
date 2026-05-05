use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use syswall_domain::entities::{AuditEvent, EventCategory, Severity};
use syswall_domain::errors::DomainError;
use syswall_domain::ports::connectivity::{ConnectivityProbe, ProbeOutcome};
use syswall_domain::ports::AuditRepository;

/// Future retourné par un callback de rollback.
/// Future returned by a rollback callback.
pub type RollbackFuture =
    Pin<Box<dyn Future<Output = Result<(), DomainError>> + Send + 'static>>;

/// Closure exécutée quand la connectivité est perdue. Effectue le rollback réel.
/// Closure executed when connectivity is lost. Performs the actual rollback.
pub type RollbackFn = Box<dyn FnOnce() -> RollbackFuture + Send + 'static>;

/// Erreurs émises par le guard anti-lockout.
/// Errors emitted by the anti-lockout guard.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GuardError {
    #[error("guard already armed")]
    AlreadyArmed,
    #[error("guard not armed")]
    NotArmed,
}

/// Configuration du guard anti-lockout.
/// Configuration of the anti-lockout guard.
#[derive(Debug, Clone)]
pub struct AntilockoutConfig {
    /// Fenêtre d'attente totale avant rollback (par défaut 30 s).
    /// Total wait window before triggering rollback (default 30 s).
    pub timeout: Duration,
    /// Intervalle entre les sondes (par défaut 5 s).
    /// Interval between probe attempts (default 5 s).
    pub probe_interval: Duration,
}

impl Default for AntilockoutConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            probe_interval: Duration::from_secs(5),
        }
    }
}

struct ArmedState {
    cancel_tx: oneshot::Sender<()>,
    join_handle: JoinHandle<()>,
}

pub struct AntilockoutGuard {
    probe: Arc<dyn ConnectivityProbe>,
    audit: Arc<dyn AuditRepository>,
    config: AntilockoutConfig,
    state: Arc<Mutex<Option<ArmedState>>>,
}

impl AntilockoutGuard {
    pub fn new(
        probe: Arc<dyn ConnectivityProbe>,
        audit: Arc<dyn AuditRepository>,
        config: AntilockoutConfig,
    ) -> Self {
        Self {
            probe,
            audit,
            config,
            state: Arc::new(Mutex::new(None)),
        }
    }

    /// Arme le guard. Le callback de rollback est exécuté si la connectivité reste
    /// injoignable pendant toute la fenêtre `timeout`.
    /// Arm the guard. The provided rollback closure will run if connectivity stays
    /// unreachable for the entire `timeout` window.
    pub async fn arm(
        &self,
        rolled_back_count: usize,
        rollback: RollbackFn,
    ) -> Result<(), GuardError> {
        let mut state = self.state.lock().await;
        if state.is_some() {
            return Err(GuardError::AlreadyArmed);
        }
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let probe = self.probe.clone();
        let audit = self.audit.clone();
        let config = self.config.clone();
        let state_clone = self.state.clone();
        let join_handle = tokio::spawn(async move {
            run_guard_loop(probe, audit, config, rolled_back_count, rollback, cancel_rx).await;
            *state_clone.lock().await = None;
        });
        *state = Some(ArmedState { cancel_tx, join_handle });
        Ok(())
    }

    /// Confirme manuellement que la connectivité est OK — annule le timer.
    /// Manually confirm connectivity is fine — cancels the timer.
    pub async fn confirm(&self) -> Result<(), GuardError> {
        let mut state = self.state.lock().await;
        let Some(armed) = state.take() else {
            return Err(GuardError::NotArmed);
        };
        // On envoie le signal d'annulation et on abandonne la tâche.
        // Awaiting the handle sous le lock provoquerait un deadlock car la tâche
        // tente elle-même d'acquérir le lock en fin d'exécution.
        let _ = armed.cancel_tx.send(());
        armed.join_handle.abort();
        Ok(())
    }

    pub async fn is_armed(&self) -> bool {
        self.state.lock().await.is_some()
    }
}

async fn run_guard_loop(
    probe: Arc<dyn ConnectivityProbe>,
    audit: Arc<dyn AuditRepository>,
    config: AntilockoutConfig,
    rolled_back_count: usize,
    rollback: RollbackFn,
    mut cancel_rx: oneshot::Receiver<()>,
) {
    let max_ticks = (config.timeout.as_secs_f64() / config.probe_interval.as_secs_f64()).ceil()
        as u32
        + 1;
    for tick in 0..max_ticks {
        if tick > 0 {
            tokio::select! {
                _ = tokio::time::sleep(config.probe_interval) => {}
                _ = &mut cancel_rx => {
                    return;
                }
            }
        }
        match probe.probe().await {
            Ok(ProbeOutcome::Reachable) => {
                let event = AuditEvent::new(
                    Severity::Info,
                    EventCategory::Antilockout,
                    "anti-lockout: connectivity confirmed",
                )
                .with_metadata("tick", tick.to_string());
                let _ = audit.append(&event).await;
                return;
            }
            Ok(ProbeOutcome::Unreachable) => continue,
            Err(e) => {
                let event = AuditEvent::new(
                    Severity::Warning,
                    EventCategory::Antilockout,
                    format!("anti-lockout: probe error: {e}"),
                );
                let _ = audit.append(&event).await;
            }
        }
    }
    // Toutes les tentatives épuisées : déclencher le rollback.
    // All ticks exhausted: trigger rollback.
    let rollback_result = rollback().await;
    let event = match &rollback_result {
        Ok(_) => AuditEvent::new(
            Severity::Critical,
            EventCategory::Antilockout,
            "anti-lockout: connectivity lost, ruleset rolled back",
        ),
        Err(e) => AuditEvent::new(
            Severity::Critical,
            EventCategory::Antilockout,
            format!("anti-lockout: rollback failed: {e}"),
        ),
    }
    .with_metadata("rolled_back_count", rolled_back_count.to_string());
    let _ = audit.append(&event).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::fake_audit_repository::FakeAuditRepository;
    use crate::fakes::fake_connectivity_probe::FakeConnectivityProbe;

    fn noop_rollback() -> RollbackFn {
        Box::new(|| Box::pin(async { Ok(()) }))
    }

    #[tokio::test(start_paused = true)]
    async fn arm_then_probe_reachable_does_not_rollback() {
        let probe = Arc::new(FakeConnectivityProbe::always_reachable());
        let audit = Arc::new(FakeAuditRepository::new());
        let guard = AntilockoutGuard::new(probe, audit.clone(), AntilockoutConfig::default());
        guard.arm(2, noop_rollback()).await.unwrap();
        // Céder la main pour que la tâche spawned exécute son premier probe à T=0.
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert!(!guard.is_armed().await);
    }

    #[tokio::test(start_paused = true)]
    async fn arm_already_armed_returns_error() {
        let probe = Arc::new(FakeConnectivityProbe::always_unreachable());
        let audit = Arc::new(FakeAuditRepository::new());
        let guard = AntilockoutGuard::new(probe, audit, AntilockoutConfig::default());
        guard.arm(1, noop_rollback()).await.unwrap();
        let err = guard.arm(1, noop_rollback()).await.unwrap_err();
        assert_eq!(err, GuardError::AlreadyArmed);
        let _ = guard.confirm().await;
    }

    #[tokio::test(start_paused = true)]
    async fn confirm_when_not_armed_returns_error() {
        let probe = Arc::new(FakeConnectivityProbe::always_reachable());
        let audit = Arc::new(FakeAuditRepository::new());
        let guard = AntilockoutGuard::new(probe, audit, AntilockoutConfig::default());
        let err = guard.confirm().await.unwrap_err();
        assert_eq!(err, GuardError::NotArmed);
    }
}
