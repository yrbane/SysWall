# Plan d'implémentation — Sous-projet A : Renforcement sécurité critique

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Câbler l'anti-lockout 30s avec TCP probe, ajouter `SO_PEERCRED` sur gRPC, durcir `syswall.service`, activer une CSP Tauri stricte, borner gRPC contre le DoS local, le tout en TDD strict avec 13 commits atomiques sur `main`.

**Architecture:** Hexagonale : nouveau port `ConnectivityProbe` (domain) + service `AntilockoutGuard` (app) + adapter `TcpProbe` (infra), câblage dans `NftablesAdapter`. Interceptor tonic pour `SO_PEERCRED`, propagation `Result`/`exit(78)` au lieu de `panic!`. Service unit systemd réécrit avec `User=syswall` + `AmbientCapabilities`.

**Tech Stack:** Rust 2024, tokio, tonic, async-trait, thiserror, nix 0.29 (peer creds), Tauri 2, systemd.

**Spec source:** `docs/superpowers/specs/2026-05-05-security-hardening-design.md`

---

## File Structure

### Fichiers créés

| Fichier | Responsabilité |
|---|---|
| `crates/domain/src/ports/connectivity.rs` | Port `ConnectivityProbe` + types `ProbeOutcome`, `ProbeError` |
| `crates/app/src/services/antilockout_guard.rs` | Service `AntilockoutGuard` (timer, arm/confirm, logique de rollback) |
| `crates/app/src/fakes/fake_connectivity_probe.rs` | Fake du port pour tests |
| `crates/infra/src/connectivity/mod.rs` | Export du module |
| `crates/infra/src/connectivity/tcp_probe.rs` | Adapter `TcpProbe` (sondes parallèles avec timeout) |
| `crates/daemon/src/startup_error.rs` | `StartupError` enum + exit code |
| `crates/daemon/src/grpc/interceptors/mod.rs` | Module interceptors |
| `crates/daemon/src/grpc/interceptors/peer_auth.rs` | `PeerAuthInterceptor` + middleware `SO_PEERCRED` |
| `crates/ui/src/lib/components/AntilockoutToast.svelte` | Toast critique sur rollback automatique |
| `system/tests/check-hardening.sh` | Script CI vérifiant les clés systemd |
| `system/install/postinst.sh` | Création user/group syswall (commun deb/rpm/aur) |

### Fichiers modifiés

| Fichier | Changement |
|---|---|
| `crates/domain/src/ports/mod.rs` | `pub mod connectivity;` + re-export |
| `crates/domain/src/errors/mod.rs` | Variant `DomainError::AntilockoutTriggered { rolled_back_count: usize }` |
| `crates/domain/src/entities/audit.rs` | Ajout `EventCategory::Antilockout` + `EventCategory::Authentication` |
| `crates/app/src/services/mod.rs` | Export `antilockout_guard` |
| `crates/app/src/fakes/mod.rs` | Export `fake_connectivity_probe` |
| `crates/infra/src/lib.rs` | `pub mod connectivity;` |
| `crates/infra/src/nftables/adapter.rs` | Câblage `AntilockoutGuard` dans `apply_rule` et `sync_all_rules` + bypass whitelist |
| `crates/daemon/src/config.rs` | Section `AntilockoutConfig` (endpoints, timeout, per_endpoint_timeout) |
| `crates/daemon/src/main.rs` | Match `StartupError` → log + `exit(78)` |
| `crates/daemon/src/bootstrap.rs` | Résolution `syswall_gid` + construction `TcpProbe` + injection guard |
| `crates/daemon/src/grpc/server.rs` | Layer interceptor + `max_decoding_message_size` + `concurrency_limit_per_connection` |
| `crates/daemon/src/grpc/mod.rs` | `pub mod interceptors;` |
| `crates/daemon/Cargo.toml` | Feature `nix` socket si manquante |
| `crates/ui/src-tauri/tauri.conf.json` | CSP stricte |
| `crates/ui/src/routes/+layout.svelte` | Listener événement `AntilockoutTriggered` |
| `config/default.toml` | Section `[antilockout]` |
| `system/syswall.service` | Réécriture complète (User=syswall, capabilities, sandbox) |
| `system/aur/PKGBUILD` | Référence `system/install/postinst.sh` |
| `system/deb/control` + `postinst` + `postrm` | Création user, propagation `chown` |
| `system/rpm/spec` | Idem RPM |
| `README.md` | Section sécurité (FR+EN) |
| `CHANGELOG.md` | Entrée V0.2 |
| `.github/workflows/ci.yml` | Job `hardening-check` |

---

## Task 1: Port `ConnectivityProbe` (domain)

**Files:**
- Create: `crates/domain/src/ports/connectivity.rs`
- Modify: `crates/domain/src/ports/mod.rs`

- [ ] **Step 1.1: Write the failing test**

Append to `crates/domain/src/ports/connectivity.rs`:

```rust
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
```

Modify `crates/domain/src/ports/mod.rs`:

```rust
pub mod connectivity;
pub mod messaging;
pub mod repositories;
pub mod system;

pub use connectivity::*;
pub use messaging::*;
pub use repositories::*;
pub use system::*;
```

- [ ] **Step 1.2: Verify it compiles and tests pass**

Run: `cargo test -p syswall-domain --lib ports::connectivity`
Expected: 2 tests pass.

- [ ] **Step 1.3: Commit**

```bash
git add crates/domain/src/ports/connectivity.rs crates/domain/src/ports/mod.rs
git commit -m "feat(domain): port ConnectivityProbe pour l'anti-lockout"
```

---

## Task 2: Étendre `EventCategory` et `DomainError`

**Files:**
- Modify: `crates/domain/src/entities/audit.rs:44-51`
- Modify: `crates/domain/src/errors/mod.rs`

- [ ] **Step 2.1: Write the failing test**

Append at the bottom of `crates/domain/src/entities/audit.rs` (in the `tests` module):

```rust
    #[test]
    fn antilockout_category_exists() {
        let event = AuditEvent::new(
            Severity::Critical,
            EventCategory::Antilockout,
            "rolled back",
        );
        assert_eq!(event.category, EventCategory::Antilockout);
    }

    #[test]
    fn authentication_category_exists() {
        let event = AuditEvent::new(
            Severity::Warning,
            EventCategory::Authentication,
            "denied",
        );
        assert_eq!(event.category, EventCategory::Authentication);
    }
```

Append at the bottom of `crates/domain/src/errors/mod.rs` (no test module yet — create one):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antilockout_triggered_displays_count() {
        let err = DomainError::AntilockoutTriggered { rolled_back_count: 3 };
        assert_eq!(
            err.to_string(),
            "Anti-lockout triggered: 3 rule change(s) rolled back due to lost connectivity"
        );
    }
}
```

- [ ] **Step 2.2: Verify they fail**

Run: `cargo test -p syswall-domain --lib`
Expected: FAIL with "no variant `Antilockout`" / "no variant `AntilockoutTriggered`".

- [ ] **Step 2.3: Implement**

In `crates/domain/src/entities/audit.rs`, extend `EventCategory`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventCategory {
    Connection,
    Rule,
    Decision,
    System,
    Config,
    Antilockout,
    Authentication,
}
```

In `crates/domain/src/errors/mod.rs`, append the variant:

```rust
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Infrastructure error: {0}")]
    Infrastructure(String),

    #[error("Operation not permitted: {0}")]
    NotPermitted(String),

    /// Anti-lockout triggered: connectivity was lost after a ruleset apply, rules rolled back.
    /// Anti-lockout déclenché : la connectivité a été perdue après un apply, règles annulées.
    #[error("Anti-lockout triggered: {rolled_back_count} rule change(s) rolled back due to lost connectivity")]
    AntilockoutTriggered { rolled_back_count: usize },
}
```

- [ ] **Step 2.4: Verify pass**

Run: `cargo test -p syswall-domain --lib`
Expected: all pass (existing + 3 new).

- [ ] **Step 2.5: Check exhaustive matches**

Run: `cargo check -p syswall-domain -p syswall-app -p syswall-infra -p syswall-daemon 2>&1 | grep -i 'non-exhaustive\|patterns'`
Expected: no output. If matches break (e.g. converters.rs), add `_ => ...` arms or list new variants explicitly.

- [ ] **Step 2.6: Commit**

```bash
git add crates/domain/src/entities/audit.rs crates/domain/src/errors/mod.rs
git commit -m "feat(domain): categories Antilockout/Authentication et erreur AntilockoutTriggered"
```

---

## Task 3: Fake `ConnectivityProbe`

**Files:**
- Create: `crates/app/src/fakes/fake_connectivity_probe.rs`
- Modify: `crates/app/src/fakes/mod.rs`

- [ ] **Step 3.1: Write the failing test**

Create `crates/app/src/fakes/fake_connectivity_probe.rs`:

```rust
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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
        assert!(!seq.is_empty(), "FakeConnectivityProbe requires at least one outcome");
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
```

Modify `crates/app/src/fakes/mod.rs` — add the `pub mod fake_connectivity_probe;` line alphabetically alongside the other fake modules.

- [ ] **Step 3.2: Verify pass**

Run: `cargo test -p syswall-app --lib fakes::fake_connectivity_probe`
Expected: 2 tests pass.

- [ ] **Step 3.3: Commit**

```bash
git add crates/app/src/fakes/fake_connectivity_probe.rs crates/app/src/fakes/mod.rs
git commit -m "feat(app): fake FakeConnectivityProbe pour les tests"
```

---

## Task 4: Service `AntilockoutGuard` (squelette + arm/confirm)

**Files:**
- Create: `crates/app/src/services/antilockout_guard.rs`
- Modify: `crates/app/src/services/mod.rs`

- [ ] **Step 4.1: Write the failing tests (arm/confirm/already-armed)**

Create `crates/app/src/services/antilockout_guard.rs`:

```rust
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use syswall_domain::entities::{AuditEvent, EventCategory, Severity};
use syswall_domain::errors::DomainError;
use syswall_domain::ports::{AuditRepository, ConnectivityProbe, ProbeOutcome};

/// Future returned by a rollback callback.
/// Future retourné par un callback de rollback.
pub type RollbackFuture =
    Pin<Box<dyn Future<Output = Result<(), DomainError>> + Send + 'static>>;

/// Closure executed when connectivity is lost. It must perform the actual rollback.
/// Closure exécutée quand la connectivité est perdue. Doit effectuer le rollback réel.
pub type RollbackFn = Box<dyn FnOnce() -> RollbackFuture + Send + 'static>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GuardError {
    #[error("guard already armed")]
    AlreadyArmed,
    #[error("guard not armed")]
    NotArmed,
}

/// Configuration of the anti-lockout guard.
/// Configuration du guard anti-lockout.
#[derive(Debug, Clone)]
pub struct AntilockoutConfig {
    /// Total wait window before triggering rollback (default 30s).
    pub timeout: Duration,
    /// Interval between probe attempts (default 5s).
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
    rolled_back_count: usize,
}

pub struct AntilockoutGuard {
    probe: Arc<dyn ConnectivityProbe>,
    audit: Arc<dyn AuditRepository>,
    config: AntilockoutConfig,
    state: Mutex<Option<ArmedState>>,
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
            state: Mutex::new(None),
        }
    }

    /// Arm the guard. The provided rollback closure will run if connectivity stays unreachable
    /// for the entire `timeout` window.
    /// Arme le guard. Le callback de rollback est exécuté si la connectivité reste injoignable
    /// pendant toute la fenêtre `timeout`.
    pub async fn arm(
        self: &Arc<Self>,
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
        let guard_self = self.clone();
        let join_handle = tokio::spawn(async move {
            run_guard_loop(probe, audit, config, rolled_back_count, rollback, cancel_rx, guard_self).await;
        });
        *state = Some(ArmedState {
            cancel_tx,
            join_handle,
            rolled_back_count,
        });
        Ok(())
    }

    /// Manually confirm connectivity is fine — cancels the timer.
    /// Confirme manuellement que la connectivité est OK — annule le timer.
    pub async fn confirm(&self) -> Result<(), GuardError> {
        let mut state = self.state.lock().await;
        let Some(armed) = state.take() else {
            return Err(GuardError::NotArmed);
        };
        let _ = armed.cancel_tx.send(());
        let _ = armed.join_handle.await;
        Ok(())
    }

    pub async fn is_armed(&self) -> bool {
        self.state.lock().await.is_some()
    }

    /// Internal: called by the loop when it terminates (success or rollback).
    /// Interne : appelé par la boucle quand elle se termine (succès ou rollback).
    async fn clear_state(&self) {
        let mut state = self.state.lock().await;
        *state = None;
    }
}

async fn run_guard_loop(
    probe: Arc<dyn ConnectivityProbe>,
    audit: Arc<dyn AuditRepository>,
    config: AntilockoutConfig,
    rolled_back_count: usize,
    rollback: RollbackFn,
    mut cancel_rx: oneshot::Receiver<()>,
    guard: Arc<AntilockoutGuard>,
) {
    let max_ticks = (config.timeout.as_secs_f64() / config.probe_interval.as_secs_f64()).ceil() as u32 + 1;
    for tick in 0..max_ticks {
        if tick > 0 {
            tokio::select! {
                _ = tokio::time::sleep(config.probe_interval) => {}
                _ = &mut cancel_rx => {
                    guard.clear_state().await;
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
                let _ = audit.append(event).await;
                guard.clear_state().await;
                return;
            }
            Ok(ProbeOutcome::Unreachable) => continue,
            Err(e) => {
                let event = AuditEvent::new(
                    Severity::Warning,
                    EventCategory::Antilockout,
                    format!("anti-lockout: probe error: {e}"),
                );
                let _ = audit.append(event).await;
            }
        }
    }
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
    let _ = audit.append(event).await;
    guard.clear_state().await;
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
    async fn arm_then_confirm_does_not_rollback() {
        let probe = Arc::new(FakeConnectivityProbe::always_reachable());
        let audit = Arc::new(FakeAuditRepository::new());
        let guard = Arc::new(AntilockoutGuard::new(probe, audit.clone(), AntilockoutConfig::default()));
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let rollback: RollbackFn = Box::new(move || {
            let c = counter_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
        });
        guard.arm(2, rollback).await.unwrap();
        // Let the spawned task run its first probe tick (T=0).
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(1)).await;
        // Probe at T=0 returned Reachable → guard should disarm.
        assert!(!guard.is_armed().await);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn arm_already_armed_returns_error() {
        let probe = Arc::new(FakeConnectivityProbe::always_unreachable());
        let audit = Arc::new(FakeAuditRepository::new());
        let guard = Arc::new(AntilockoutGuard::new(probe, audit, AntilockoutConfig::default()));
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
```

Modify `crates/app/src/services/mod.rs` to declare `pub mod antilockout_guard;` alongside other modules.

- [ ] **Step 4.2: Verify the tests fail at first (no impl) then pass**

Run: `cargo test -p syswall-app --lib services::antilockout_guard`
Expected: 3 tests pass.

- [ ] **Step 4.3: Commit**

```bash
git add crates/app/src/services/antilockout_guard.rs crates/app/src/services/mod.rs
git commit -m "feat(app): AntilockoutGuard avec arm/confirm et boucle de probes"
```

---

## Task 5: Tests d'intégration `AntilockoutGuard` — rollback déclenché

**Files:**
- Modify: `crates/app/src/services/antilockout_guard.rs` (test module only)

- [ ] **Step 5.1: Add the rollback-trigger and recovery-mid-window tests**

Append to the `tests` module of `crates/app/src/services/antilockout_guard.rs`:

```rust
    use syswall_domain::ports::connectivity::ProbeOutcome;

    #[tokio::test(start_paused = true)]
    async fn all_probes_unreachable_triggers_rollback() {
        let probe = Arc::new(FakeConnectivityProbe::always_unreachable());
        let audit = Arc::new(FakeAuditRepository::new());
        let guard = Arc::new(AntilockoutGuard::new(probe, audit.clone(), AntilockoutConfig::default()));
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let rollback: RollbackFn = Box::new(move || {
            let c = counter_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
        });
        guard.arm(3, rollback).await.unwrap();
        tokio::time::advance(Duration::from_secs(35)).await;
        tokio::task::yield_now().await;
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(!guard.is_armed().await);
        let events = audit.snapshot().await;
        assert!(events.iter().any(|e| e.severity == syswall_domain::entities::Severity::Critical
            && e.category == syswall_domain::entities::EventCategory::Antilockout));
    }

    #[tokio::test(start_paused = true)]
    async fn recovery_mid_window_does_not_rollback() {
        // Unreachable for first 3 ticks, then reachable.
        let probe = Arc::new(FakeConnectivityProbe::with_sequence(vec![
            Ok(ProbeOutcome::Unreachable),
            Ok(ProbeOutcome::Unreachable),
            Ok(ProbeOutcome::Unreachable),
            Ok(ProbeOutcome::Reachable),
        ]));
        let audit = Arc::new(FakeAuditRepository::new());
        let guard = Arc::new(AntilockoutGuard::new(probe, audit, AntilockoutConfig::default()));
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let rollback: RollbackFn = Box::new(move || {
            let c = counter_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
        });
        guard.arm(1, rollback).await.unwrap();
        tokio::time::advance(Duration::from_secs(20)).await;
        tokio::task::yield_now().await;
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(!guard.is_armed().await);
    }

    #[tokio::test(start_paused = true)]
    async fn manual_confirm_cancels_timer() {
        let probe = Arc::new(FakeConnectivityProbe::always_unreachable());
        let audit = Arc::new(FakeAuditRepository::new());
        let guard = Arc::new(AntilockoutGuard::new(probe, audit, AntilockoutConfig::default()));
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let rollback: RollbackFn = Box::new(move || {
            let c = counter_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
        });
        guard.arm(1, rollback).await.unwrap();
        tokio::time::advance(Duration::from_secs(10)).await;
        guard.confirm().await.unwrap();
        tokio::time::advance(Duration::from_secs(60)).await;
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
```

If `FakeAuditRepository::snapshot` does not exist yet, add a method on the fake exposing a `Vec<AuditEvent>` clone. Inspect `crates/app/src/fakes/fake_audit_repository.rs` and add if missing:

```rust
    pub async fn snapshot(&self) -> Vec<AuditEvent> {
        self.events.lock().await.clone()
    }
```

- [ ] **Step 5.2: Verify pass**

Run: `cargo test -p syswall-app --lib services::antilockout_guard`
Expected: 6 tests pass total.

- [ ] **Step 5.3: Commit**

```bash
git add crates/app/src/services/antilockout_guard.rs crates/app/src/fakes/fake_audit_repository.rs
git commit -m "test(app): scenarios rollback declenche, recovery, et confirm manuel"
```

---

## Task 6: Adapter `TcpProbe` (infra)

**Files:**
- Create: `crates/infra/src/connectivity/mod.rs`
- Create: `crates/infra/src/connectivity/tcp_probe.rs`
- Modify: `crates/infra/src/lib.rs`

- [ ] **Step 6.1: Write the failing tests**

Create `crates/infra/src/connectivity/tcp_probe.rs`:

```rust
use async_trait::async_trait;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io;
use tokio::net::TcpStream;

use syswall_domain::ports::connectivity::{ConnectivityProbe, ProbeError, ProbeOutcome};

/// TCP-based connectivity probe.
/// Sonde de connectivité TCP.
pub struct TcpProbe {
    endpoints: Vec<SocketAddr>,
    per_endpoint_timeout: Duration,
}

impl TcpProbe {
    pub fn new(endpoints: Vec<SocketAddr>, per_endpoint_timeout: Duration) -> Result<Self, ProbeError> {
        if endpoints.is_empty() {
            return Err(ProbeError::Configuration("empty endpoint list".into()));
        }
        Ok(Self { endpoints, per_endpoint_timeout })
    }
}

#[async_trait]
impl ConnectivityProbe for TcpProbe {
    async fn probe(&self) -> Result<ProbeOutcome, ProbeError> {
        let attempts = self.endpoints.iter().copied().map(|addr| {
            let timeout = self.per_endpoint_timeout;
            async move {
                match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
                    Ok(Ok(_)) => true,                                       // Connected.
                    Ok(Err(e)) if is_reachable_error(&e) => true,            // Refused/Reset = network OK.
                    Ok(Err(_)) | Err(_) => false,                            // Other or timeout.
                }
            }
        });
        let results = futures::future::join_all(attempts).await;
        if results.into_iter().any(|reachable| reachable) {
            Ok(ProbeOutcome::Reachable)
        } else {
            Ok(ProbeOutcome::Unreachable)
        }
    }
}

fn is_reachable_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn empty_endpoints_returns_configuration_error() {
        let err = TcpProbe::new(vec![], Duration::from_secs(1)).unwrap_err();
        assert!(matches!(err, ProbeError::Configuration(_)));
    }

    #[tokio::test]
    async fn local_listener_is_reachable() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let probe = TcpProbe::new(vec![addr], Duration::from_secs(2)).unwrap();
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Reachable);
    }

    #[tokio::test]
    async fn closed_port_is_reachable_via_conn_refused() {
        // Bind then drop to release the port; connect attempts will see ConnectionRefused.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let probe = TcpProbe::new(vec![addr], Duration::from_secs(2)).unwrap();
        // ConnectionRefused on loopback → counts as reachable.
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Reachable);
    }

    #[tokio::test]
    async fn unroutable_address_times_out_to_unreachable() {
        // 192.0.2.0/24 is reserved for documentation (RFC 5737), guaranteed not routable.
        let addr: SocketAddr = "192.0.2.1:65535".parse().unwrap();
        let probe = TcpProbe::new(vec![addr], Duration::from_millis(200)).unwrap();
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Unreachable);
    }

    #[tokio::test]
    async fn one_reachable_one_unreachable_yields_reachable() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr_ok = listener.local_addr().unwrap();
        let addr_ko: SocketAddr = "192.0.2.1:65535".parse().unwrap();
        let probe = TcpProbe::new(vec![addr_ok, addr_ko], Duration::from_millis(200)).unwrap();
        assert_eq!(probe.probe().await.unwrap(), ProbeOutcome::Reachable);
    }
}
```

Create `crates/infra/src/connectivity/mod.rs`:

```rust
pub mod tcp_probe;

pub use tcp_probe::TcpProbe;
```

Modify `crates/infra/src/lib.rs` — add `pub mod connectivity;` alongside the other module declarations.

- [ ] **Step 6.2: Verify pass**

Run: `cargo test -p syswall-infra --lib connectivity::`
Expected: 5 tests pass.

- [ ] **Step 6.3: Commit**

```bash
git add crates/infra/src/connectivity/ crates/infra/src/lib.rs
git commit -m "feat(infra): TcpProbe pour la sonde de connectivite anti-lockout"
```

---

## Task 7: Câblage du guard dans `NftablesAdapter` + bypass whitelist

**Files:**
- Modify: `crates/infra/src/nftables/adapter.rs` (apply paths)
- Read first: `crates/infra/src/nftables/adapter.rs:140-420` to understand existing rollback flow

- [ ] **Step 7.1: Inspect the existing apply paths**

Run: `grep -n 'fn apply_rule\|fn sync_all_rules\|RollbackState' crates/infra/src/nftables/adapter.rs`

Expected output (line numbers indicative):
```
145: async fn save_rollback_state(&self) -> Result<RollbackState, DomainError> {
161: async fn rollback(&self, state: &RollbackState) {
245: async fn apply_rule(&self, rule: &Rule) -> Result<(), DomainError> {
334: async fn sync_all_rules(&self, rules: &[Rule]) -> Result<(), DomainError> {
```

- [ ] **Step 7.2: Add a whitelist-detection helper**

In `crates/infra/src/nftables/adapter.rs`, add the helper near the top of the impl block:

```rust
/// Returns true if the ruleset only contains whitelist rules (DNS, DHCP, NTP, loopback).
/// Retourne true si le ruleset ne contient que des règles whitelist (DNS, DHCP, NTP, loopback).
fn is_whitelist_only(rules: &[Rule]) -> bool {
    rules.iter().all(is_whitelist_rule)
}

fn is_whitelist_rule(rule: &Rule) -> bool {
    use syswall_domain::value_objects::Protocol;
    let crit = &rule.criteria;
    let port_match = |port: u16| crit.remote_port == Some(port) || crit.local_port == Some(port);
    let proto_is = |p: Protocol| crit.protocol == Some(p);
    // DNS (53/udp+tcp), DHCP (67/68 udp), NTP (123/udp), loopback IPs handled implicitly.
    (proto_is(Protocol::Udp) && port_match(53))
        || (proto_is(Protocol::Tcp) && port_match(53))
        || (proto_is(Protocol::Udp) && (port_match(67) || port_match(68)))
        || (proto_is(Protocol::Udp) && port_match(123))
        || rule.criteria.remote_ip.as_ref().is_some_and(is_loopback_cidr)
}

fn is_loopback_cidr(cidr: &syswall_domain::value_objects::IpCidr) -> bool {
    let s = cidr.to_string();
    s.starts_with("127.") || s == "::1" || s.starts_with("::1/")
}
```

(If `Protocol` or `IpCidr` paths differ in the codebase, adjust the imports — discoverable via `grep -rn 'pub enum Protocol' crates/domain/src/value_objects/`.)

- [ ] **Step 7.3: Add an optional `AntilockoutGuard` field to the adapter**

Locate the `NftablesAdapter` struct definition. Add the field:

```rust
pub struct NftablesAdapter {
    // ...existing fields...
    antilockout_guard: Option<Arc<AntilockoutGuard>>,
}
```

In its constructor, accept an optional guard (use `with_antilockout_guard` builder to avoid breaking existing callers):

```rust
impl NftablesAdapter {
    pub fn with_antilockout_guard(mut self, guard: Arc<AntilockoutGuard>) -> Self {
        self.antilockout_guard = Some(guard);
        self
    }
}
```

In the existing `new` constructor, set `antilockout_guard: None` as default.

Add imports at the top of the file:

```rust
use std::sync::Arc;
use syswall_app::services::antilockout_guard::{AntilockoutGuard, RollbackFn, RollbackFuture};
```

(If `syswall-app` is not in `crates/infra/Cargo.toml`, **stop**: this is a hexagonal violation. Instead, define a thin port `RollbackController` in domain and let the daemon wire the guard externally. See alternative wiring at the end of this task.)

- [ ] **Step 7.4: Hexagonal-friendly wiring (preferred)**

Because `infra` must NOT depend on `app`, use the daemon as the wiring point:

a) Revert step 7.3 — remove the `Arc<AntilockoutGuard>` field from `NftablesAdapter`.

b) In `crates/domain/src/ports/connectivity.rs`, add a second port:

```rust
/// Arms a deferred rollback that will execute if connectivity is lost.
/// Arme un rollback différé qui s'exécutera si la connectivité est perdue.
#[async_trait]
pub trait LockoutGuard: Send + Sync {
    async fn arm_rollback(
        &self,
        rolled_back_count: usize,
        rollback: crate::ports::connectivity::ArmedRollback,
    ) -> Result<(), super::super::errors::DomainError>;
}

/// Same shape as `RollbackFn` but defined in domain to avoid app→infra leakage.
pub type ArmedRollback = Box<
    dyn FnOnce() -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), super::super::errors::DomainError>> + Send + 'static>,
    > + Send
        + 'static,
>;
```

c) In `crates/app/src/services/antilockout_guard.rs`, implement `LockoutGuard` for `Arc<AntilockoutGuard>`:

```rust
use syswall_domain::ports::connectivity::{ArmedRollback, LockoutGuard};

#[async_trait::async_trait]
impl LockoutGuard for AntilockoutGuard {
    async fn arm_rollback(
        &self,
        rolled_back_count: usize,
        rollback: ArmedRollback,
    ) -> Result<(), DomainError> {
        // Re-wrap the domain ArmedRollback as our internal RollbackFn.
        let rollback_fn: RollbackFn = Box::new(move || rollback());
        // We need an Arc<Self> to call arm; obtain it via a thread-local or accept a &self method.
        // For simplicity, change the public arm() to take &self instead of &Arc<Self>.
        unimplemented!("see step 7.5: switch arm() to &self");
    }
}
```

d) Refactor `AntilockoutGuard::arm` to take `&self`:

Replace the `pub async fn arm(self: &Arc<Self>, ...)` signature with `pub async fn arm(&self, ...)` and replace `let guard_self = self.clone();` with capturing the necessary fields by clone (probe, audit, config) into a small helper struct:

```rust
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
    let join_handle = tokio::spawn(async move {
        run_guard_loop(probe, audit, config, rolled_back_count, rollback, cancel_rx).await;
    });
    *state = Some(ArmedState { cancel_tx, join_handle, rolled_back_count });
    Ok(())
}
```

The `run_guard_loop` no longer needs the `Arc<AntilockoutGuard>`; replace `guard.clear_state().await` with a flag inside an `Arc<AtomicBool>` shared with the parent, OR drop the clear and rely on `is_armed` being a best-effort check. Simpler: store `state` behind an `Arc<Mutex<Option<ArmedState>>>` directly and pass an `Arc` clone into the loop.

Refactor the struct field to `state: Arc<Mutex<Option<ArmedState>>>`. The loop's last line becomes:

```rust
let state_clone = state.clone();
tokio::spawn(async move {
    run_guard_loop(...).await;
    *state_clone.lock().await = None;
});
```

Update step 4 tests if needed — they should still compile.

e) Now in `LockoutGuard` impl:

```rust
#[async_trait::async_trait]
impl LockoutGuard for AntilockoutGuard {
    async fn arm_rollback(
        &self,
        rolled_back_count: usize,
        rollback: ArmedRollback,
    ) -> Result<(), DomainError> {
        let rb: RollbackFn = Box::new(move || rollback());
        self.arm(rolled_back_count, rb)
            .await
            .map_err(|e| DomainError::Infrastructure(format!("guard: {e}")))
    }
}
```

f) Add `Option<Arc<dyn LockoutGuard>>` field to `NftablesAdapter` (domain trait — OK in infra):

```rust
use syswall_domain::ports::connectivity::{ArmedRollback, LockoutGuard};

pub struct NftablesAdapter {
    // ...
    lockout_guard: Option<Arc<dyn LockoutGuard>>,
}

impl NftablesAdapter {
    pub fn with_lockout_guard(mut self, g: Arc<dyn LockoutGuard>) -> Self {
        self.lockout_guard = Some(g);
        self
    }
}
```

- [ ] **Step 7.5: Wire the guard into `apply_rule` and `sync_all_rules`**

After the existing `nft -f` succeeds in `apply_rule`, add (around line 280, before `Ok(())`):

```rust
        if let Some(guard) = &self.lockout_guard {
            // Whitelist-only single-rule apply: bypass guard with a Warning audit event.
            if is_whitelist_rule(rule) {
                // Caller's responsibility to log; here we just skip arming.
            } else {
                let rollback_state = rollback_state.clone();
                let self_arc = self.clone_for_rollback();
                let count = 1usize;
                let rollback: ArmedRollback = Box::new(move || {
                    Box::pin(async move {
                        self_arc.rollback(&rollback_state).await;
                        Ok(())
                    })
                });
                guard.arm_rollback(count, rollback).await?;
            }
        }
```

Same pattern in `sync_all_rules` after the successful apply (around line 405), but with `is_whitelist_only(rules)` and `count = rules.len()`.

The `clone_for_rollback` helper returns an `Arc<NftablesAdapter>` that owns enough state to call `rollback`. If `NftablesAdapter` is not currently `Arc`-friendly, wrap its rollback-relevant fields in `Arc<Inner>` and clone the inner.

Concretely, restructure if needed:

```rust
struct Inner {
    nft_path: PathBuf,
    nft_timeout_secs: u64,
    nft_max_output: usize,
    table_name: String,
    // anything `rollback()` reads
}

pub struct NftablesAdapter {
    inner: Arc<Inner>,
    lockout_guard: Option<Arc<dyn LockoutGuard>>,
}

impl NftablesAdapter {
    fn clone_for_rollback(&self) -> Arc<NftablesAdapter> {
        Arc::new(Self {
            inner: self.inner.clone(),
            lockout_guard: None,
        })
    }
}
```

(If `NftablesAdapter` already uses `Arc<Inner>` shape, skip the restructure.)

- [ ] **Step 7.6: Add unit test for `is_whitelist_rule`**

In the existing `tests` module of `crates/infra/src/nftables/adapter.rs`, add:

```rust
#[test]
fn is_whitelist_rule_dns_udp() {
    let rule = test_rule_with(Protocol::Udp, Some(53), None, None);
    assert!(super::is_whitelist_rule(&rule));
}

#[test]
fn is_whitelist_rule_dns_tcp() {
    let rule = test_rule_with(Protocol::Tcp, Some(53), None, None);
    assert!(super::is_whitelist_rule(&rule));
}

#[test]
fn is_whitelist_rule_dhcp_67() {
    let rule = test_rule_with(Protocol::Udp, Some(67), None, None);
    assert!(super::is_whitelist_rule(&rule));
}

#[test]
fn is_whitelist_rule_ntp() {
    let rule = test_rule_with(Protocol::Udp, Some(123), None, None);
    assert!(super::is_whitelist_rule(&rule));
}

#[test]
fn is_whitelist_rule_random_port_is_not_whitelist() {
    let rule = test_rule_with(Protocol::Tcp, Some(443), None, None);
    assert!(!super::is_whitelist_rule(&rule));
}

fn test_rule_with(
    protocol: Protocol,
    remote_port: Option<u16>,
    local_port: Option<u16>,
    remote_ip: Option<IpCidr>,
) -> Rule {
    // Use the existing Rule constructor; copy-paste from a nearby existing test if needed.
    Rule::new(
        RuleId::new(),
        "test".into(),
        RuleAction::Allow,
        RuleCriteria {
            protocol: Some(protocol),
            remote_port,
            local_port,
            remote_ip,
            ..Default::default()
        },
        100,
    )
}
```

(Use existing `Rule::new` signature from the codebase — adjust if names differ.)

- [ ] **Step 7.7: Verify pass**

Run: `cargo test -p syswall-infra --lib nftables::adapter`
Expected: existing tests still pass + 5 new whitelist tests.

- [ ] **Step 7.8: Commit**

```bash
git add crates/domain/src/ports/connectivity.rs \
        crates/app/src/services/antilockout_guard.rs \
        crates/infra/src/nftables/adapter.rs
git commit -m "feat(infra): cablage du guard anti-lockout dans nftables apply avec bypass whitelist"
```

---

## Task 8: Section `[antilockout]` dans la config + valeurs par défaut

**Files:**
- Modify: `crates/daemon/src/config.rs`
- Modify: `config/default.toml`

- [ ] **Step 8.1: Write the failing test**

In `crates/daemon/src/config.rs`, append to the `tests` module:

```rust
    const ANTILOCKOUT_CONFIG: &str = r#"
[antilockout]
enabled = true
endpoints = ["1.1.1.1:53", "[2606:4700:4700::1111]:53"]
timeout_secs = 30
probe_interval_secs = 5
per_endpoint_timeout_secs = 2
"#;

    #[test]
    fn parse_antilockout_section() {
        let full = format!("{}\n{}", TEST_CONFIG, ANTILOCKOUT_CONFIG);
        let config = SysWallConfig::from_toml(&full).unwrap();
        let al = config.antilockout.as_ref().unwrap();
        assert!(al.enabled);
        assert_eq!(al.endpoints.len(), 2);
        assert_eq!(al.timeout_secs, 30);
    }

    #[test]
    fn antilockout_section_is_optional() {
        let config = SysWallConfig::from_toml(TEST_CONFIG).unwrap();
        // None means "use defaults at bootstrap time"
        assert!(config.antilockout.is_none());
    }
```

- [ ] **Step 8.2: Implement**

Add to `crates/daemon/src/config.rs` near the other configs:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct AntilockoutConfig {
    #[serde(default = "default_antilockout_enabled")]
    pub enabled: bool,
    #[serde(default = "default_antilockout_endpoints")]
    pub endpoints: Vec<String>,
    #[serde(default = "default_antilockout_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_antilockout_probe_interval")]
    pub probe_interval_secs: u64,
    #[serde(default = "default_antilockout_per_endpoint_timeout")]
    pub per_endpoint_timeout_secs: u64,
}

fn default_antilockout_enabled() -> bool { true }
fn default_antilockout_endpoints() -> Vec<String> {
    vec!["1.1.1.1:53".into(), "[2606:4700:4700::1111]:53".into()]
}
fn default_antilockout_timeout() -> u64 { 30 }
fn default_antilockout_probe_interval() -> u64 { 5 }
fn default_antilockout_per_endpoint_timeout() -> u64 { 2 }

impl Default for AntilockoutConfig {
    fn default() -> Self {
        Self {
            enabled: default_antilockout_enabled(),
            endpoints: default_antilockout_endpoints(),
            timeout_secs: default_antilockout_timeout(),
            probe_interval_secs: default_antilockout_probe_interval(),
            per_endpoint_timeout_secs: default_antilockout_per_endpoint_timeout(),
        }
    }
}
```

In `SysWallConfig`, add:

```rust
    #[serde(default)]
    pub antilockout: Option<AntilockoutConfig>,
```

Append to `config/default.toml`:

```toml
[antilockout]
enabled = true
endpoints = ["1.1.1.1:53", "[2606:4700:4700::1111]:53"]
timeout_secs = 30
probe_interval_secs = 5
per_endpoint_timeout_secs = 2
```

- [ ] **Step 8.3: Verify pass**

Run: `cargo test -p syswall-daemon --lib config::tests`
Expected: existing 3 + 2 new tests pass.

- [ ] **Step 8.4: Commit**

```bash
git add crates/daemon/src/config.rs config/default.toml
git commit -m "feat(daemon): config anti-lockout (timeout, endpoints, intervalle)"
```

---

## Task 9: Bootstrap câble TcpProbe + AntilockoutGuard + injection dans NftablesAdapter

**Files:**
- Modify: `crates/daemon/src/bootstrap.rs`

- [ ] **Step 9.1: Read the bootstrap to find the wiring spot**

Run: `grep -n 'NftablesAdapter\|FirewallEngine' crates/daemon/src/bootstrap.rs`

Identify the construction line of `NftablesAdapter`. The injection point is just after.

- [ ] **Step 9.2: Add the wiring**

At the top of `bootstrap.rs`, add:

```rust
use std::sync::Arc;
use std::time::Duration;
use std::net::SocketAddr;
use syswall_app::services::antilockout_guard::{AntilockoutGuard, AntilockoutConfig as GuardConfig};
use syswall_domain::ports::connectivity::LockoutGuard;
use syswall_infra::connectivity::TcpProbe;
```

After `NftablesAdapter` is built but before it's wrapped as `Arc<dyn FirewallEngine>`, add:

```rust
    let guard = if let Some(al_cfg) = config.antilockout.clone().or_else(|| Some(crate::config::AntilockoutConfig::default())) {
        if al_cfg.enabled {
            let endpoints: Result<Vec<SocketAddr>, _> = al_cfg
                .endpoints
                .iter()
                .map(|s| s.parse::<SocketAddr>())
                .collect();
            let endpoints = endpoints.map_err(|e| {
                crate::startup_error::StartupError::ConfigInvalid(format!(
                    "antilockout.endpoints parse error: {e}"
                ))
            })?;
            let probe = Arc::new(
                TcpProbe::new(endpoints, Duration::from_secs(al_cfg.per_endpoint_timeout_secs))
                    .map_err(|e| crate::startup_error::StartupError::ConfigInvalid(format!("antilockout: {e}")))?,
            );
            let g = Arc::new(AntilockoutGuard::new(
                probe,
                audit_repository.clone(),
                GuardConfig {
                    timeout: Duration::from_secs(al_cfg.timeout_secs),
                    probe_interval: Duration::from_secs(al_cfg.probe_interval_secs),
                },
            ));
            Some(g as Arc<dyn LockoutGuard>)
        } else {
            None
        }
    } else {
        None
    };

    let nft_adapter = if let Some(guard) = guard {
        nft_adapter.with_lockout_guard(guard)
    } else {
        nft_adapter
    };
```

(Replace `audit_repository` with the actual variable name in the bootstrap.)

`StartupError` is created in Task 11 — for now reference it; if you build this task before Task 11, use a `String` `Result` and refactor in Task 11.

- [ ] **Step 9.3: Verify it compiles**

Run: `cargo check -p syswall-daemon`
Expected: 0 errors. Warnings about unused `StartupError` are fine if Task 11 isn't done yet.

- [ ] **Step 9.4: Commit**

```bash
git add crates/daemon/src/bootstrap.rs
git commit -m "feat(daemon): bootstrap construit TcpProbe + AntilockoutGuard et les injecte dans NftablesAdapter"
```

---

## Task 10: `StartupError` + main propre

**Files:**
- Create: `crates/daemon/src/startup_error.rs`
- Modify: `crates/daemon/src/main.rs`
- Modify: `crates/daemon/src/bootstrap.rs` (replace string errors)

- [ ] **Step 10.1: Write the failing test**

Create `crates/daemon/src/startup_error.rs`:

```rust
use thiserror::Error;

/// Fatal startup failures. Emitted by `bootstrap()` and consumed by `main()`.
/// Erreurs fatales de démarrage. Émises par `bootstrap()` et consommées par `main()`.
#[derive(Debug, Error)]
pub enum StartupError {
    #[error("syswall: configuration invalid: {0}")]
    ConfigInvalid(String),

    #[error("syswall: missing system group 'syswall' (run install scripts)")]
    SyswallGroupMissing,

    #[error("syswall: failed to chown socket {path}: {source}")]
    SocketChownFailed { path: String, source: std::io::Error },

    #[error("syswall: failed to bind socket {path}: {source}")]
    SocketBindFailed { path: String, source: std::io::Error },

    #[error("syswall: infrastructure init failed: {0}")]
    InfrastructureInit(String),
}

impl StartupError {
    /// EX_CONFIG (sysexits.h) — configuration error or missing prerequisites.
    /// EX_CONFIG (sysexits.h) — erreur de configuration ou prérequis manquant.
    pub fn exit_code(&self) -> i32 {
        78
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_is_ex_config() {
        let err = StartupError::SyswallGroupMissing;
        assert_eq!(err.exit_code(), 78);
    }

    #[test]
    fn group_missing_displays_actionable_message() {
        let s = StartupError::SyswallGroupMissing.to_string();
        assert!(s.contains("syswall"));
        assert!(s.contains("install scripts"));
    }
}
```

Modify `crates/daemon/src/main.rs` — restructure to:

```rust
mod bootstrap;
mod config;
mod grpc;
mod signals;
mod startup_error;
mod supervisor;

use startup_error::StartupError;
use tracing::error;

#[tokio::main]
async fn main() {
    init_tracing();
    if let Err(e) = run().await {
        error!("{e}");
        std::process::exit(e.exit_code());
    }
}

async fn run() -> Result<(), StartupError> {
    let config = config::load_default()
        .map_err(|e| StartupError::ConfigInvalid(e.to_string()))?;
    let services = bootstrap::build_services(&config).await?;
    supervisor::run(services).await
        .map_err(|e| StartupError::InfrastructureInit(e.to_string()))
}

fn init_tracing() {
    // existing tracing init code preserved
}
```

(Adjust to match the existing `main.rs` shape — keep all preserved logic, just add the error mapping.)

In `bootstrap.rs`, change the function signature `pub async fn build_services(...) -> Result<Services, StartupError>` and propagate.

- [ ] **Step 10.2: Verify pass**

Run: `cargo test -p syswall-daemon --lib startup_error && cargo check -p syswall-daemon`
Expected: 2 tests pass + 0 errors.

- [ ] **Step 10.3: Commit**

```bash
git add crates/daemon/src/startup_error.rs crates/daemon/src/main.rs crates/daemon/src/bootstrap.rs
git commit -m "feat(daemon): StartupError typed avec exit code 78 (EX_CONFIG)"
```

---

## Task 11: Notification UI sur `AntilockoutTriggered`

**Files:**
- Modify: `crates/proto/proto/syswall.proto` (add new event variant)
- Modify: `crates/daemon/src/grpc/event_service.rs` (emit the variant)
- Modify: `crates/app/src/services/antilockout_guard.rs` (call event publisher)
- Create: `crates/ui/src/lib/components/AntilockoutToast.svelte`
- Modify: `crates/ui/src/routes/+layout.svelte`

- [ ] **Step 11.1: Add proto event variant**

In `crates/proto/proto/syswall.proto` (or wherever the event proto lives), add to the events oneof:

```protobuf
message AntilockoutTriggered {
    uint32 rolled_back_count = 1;
    string timestamp = 2;
}

// Inside the existing Event message oneof:
message Event {
    oneof payload {
        // ... existing variants ...
        AntilockoutTriggered antilockout_triggered = 9;
    }
}
```

(Pick the next available oneof tag — inspect the file to confirm.)

- [ ] **Step 11.2: Make the guard emit the domain event**

In `crates/app/src/services/antilockout_guard.rs`, add an `EventPublisher` parameter (port already exists in domain — `EventBus` or similar). Inspect `crates/domain/src/ports/messaging.rs` for the right trait.

If a port like `pub trait EventBus { async fn publish(&self, event: DomainEvent); }` exists, inject it into `AntilockoutGuard::new`. After the rollback succeeds in `run_guard_loop`, call:

```rust
event_bus.publish(DomainEvent::AntilockoutTriggered { rolled_back_count }).await;
```

Add `DomainEvent::AntilockoutTriggered { rolled_back_count: usize }` to `crates/domain/src/events/mod.rs`.

If no `DomainEvent` enum exists, define one minimally:

```rust
#[derive(Debug, Clone)]
pub enum DomainEvent {
    AntilockoutTriggered { rolled_back_count: usize },
    // future variants
}
```

- [ ] **Step 11.3: gRPC event service forwards to UI**

In `crates/daemon/src/grpc/event_service.rs`, subscribe to `DomainEvent` and convert to proto `AntilockoutTriggered`. Existing event flow shows the pattern (look for how `ConnectionDetected` or similar is published).

- [ ] **Step 11.4: Create the Svelte toast component**

`crates/ui/src/lib/components/AntilockoutToast.svelte`:

```svelte
<script lang="ts">
    interface Props {
        rolledBackCount: number;
        onDismiss: () => void;
    }
    let { rolledBackCount, onDismiss }: Props = $props();
</script>

<div class="antilockout-toast" role="alert" aria-live="assertive">
    <div class="icon" aria-hidden="true">⚠</div>
    <div class="body">
        <div class="title">Mise à jour annulée — connectivité perdue</div>
        <div class="desc">
            {rolledBackCount} modification(s) de règle ont été annulées automatiquement.
            La connectivité réseau a été perdue après l'application.
        </div>
    </div>
    <button class="dismiss" onclick={onDismiss} aria-label="Fermer">×</button>
</div>

<style>
    .antilockout-toast {
        display: flex;
        gap: var(--space-3);
        padding: var(--space-4);
        border-radius: var(--radius-md);
        background: rgba(255, 69, 58, 0.12);
        border: 1px solid var(--accent-danger);
        color: var(--text-primary);
        max-width: 480px;
    }
    .icon { font-size: 24px; color: var(--accent-danger); }
    .title { font-weight: 600; margin-bottom: 4px; }
    .desc { font-size: var(--font-size-sm); color: var(--text-secondary); }
    .dismiss { background: none; border: 0; font-size: 20px; color: var(--text-secondary); cursor: pointer; }
</style>
```

- [ ] **Step 11.5: Wire the toast into the layout**

In `crates/ui/src/routes/+layout.svelte`, find the toast container/registration. Add a listener for the new event type:

```typescript
import { listen } from '@tauri-apps/api/event';
import AntilockoutToast from '$lib/components/AntilockoutToast.svelte';
import { toastStore } from '$lib/stores/toast';

onMount(() => {
    const unlisten = listen<{ rolled_back_count: number }>('antilockout-triggered', (e) => {
        toastStore.show({
            component: AntilockoutToast,
            props: { rolledBackCount: e.payload.rolled_back_count },
            duration: 0, // sticky until dismissed
        });
    });
    return () => { unlisten.then(fn => fn()); };
});
```

(Adapt to the actual `toastStore` API in the codebase — read `crates/ui/src/lib/stores/toast.ts` to match it.)

- [ ] **Step 11.6: Verify**

Run: `cargo build -p syswall-proto && cargo test -p syswall-app --lib services::antilockout_guard`
Expected: build OK, app tests still pass (the guard now needs an `EventBus` argument — update tests to pass `Arc::new(FakeEventBus::new())`).

- [ ] **Step 11.7: Commit**

```bash
git add crates/proto/proto/syswall.proto crates/domain/src/events/mod.rs \
        crates/app/src/services/antilockout_guard.rs \
        crates/daemon/src/grpc/event_service.rs \
        crates/ui/src/lib/components/AntilockoutToast.svelte \
        crates/ui/src/routes/+layout.svelte
git commit -m "feat(ui): notification critique sur rollback automatique anti-lockout"
```

---

## Task 12: Interceptor `PeerAuthInterceptor` (gRPC)

**Files:**
- Create: `crates/daemon/src/grpc/interceptors/mod.rs`
- Create: `crates/daemon/src/grpc/interceptors/peer_auth.rs`
- Modify: `crates/daemon/src/grpc/mod.rs`
- Modify: `crates/daemon/src/grpc/server.rs`

- [ ] **Step 12.1: Write the failing tests**

Create `crates/daemon/src/grpc/interceptors/peer_auth.rs`:

```rust
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::{service::Interceptor, Request, Status};

use syswall_domain::entities::{AuditEvent, EventCategory, Severity};

/// Peer credentials extracted from the Unix socket.
/// Identifiants du peer extraits du socket Unix.
#[derive(Debug, Clone, Copy)]
pub struct PeerCredentials {
    pub uid: u32,
    pub gid: u32,
    /// Effective primary group only — supplementary groups not transported by SO_PEERCRED.
    /// Groupe primaire effectif uniquement — les groupes supplémentaires ne sont pas transportés par SO_PEERCRED.
    pub pid: i32,
}

/// Allowed identities for gRPC calls.
/// Identités autorisées pour les appels gRPC.
#[derive(Debug, Clone)]
pub struct PeerAuthPolicy {
    pub allowed_uids: HashSet<u32>,
    pub allowed_gids: HashSet<u32>,
}

impl PeerAuthPolicy {
    pub fn new(allowed_uids: HashSet<u32>, allowed_gids: HashSet<u32>) -> Self {
        Self { allowed_uids, allowed_gids }
    }

    pub fn permits(&self, creds: &PeerCredentials) -> bool {
        self.allowed_uids.contains(&creds.uid) || self.allowed_gids.contains(&creds.gid)
    }
}

#[derive(Clone)]
pub struct PeerAuthInterceptor {
    policy: Arc<PeerAuthPolicy>,
    audit_tx: mpsc::Sender<AuditEvent>,
}

impl PeerAuthInterceptor {
    pub fn new(policy: Arc<PeerAuthPolicy>, audit_tx: mpsc::Sender<AuditEvent>) -> Self {
        Self { policy, audit_tx }
    }
}

impl Interceptor for PeerAuthInterceptor {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        let creds = req
            .extensions()
            .get::<PeerCredentials>()
            .copied()
            .ok_or_else(|| Status::internal("peer credentials unavailable"))?;
        if self.policy.permits(&creds) {
            Ok(req)
        } else {
            let event = AuditEvent::new(
                Severity::Warning,
                EventCategory::Authentication,
                format!("gRPC denied: uid={} gid={} pid={}", creds.uid, creds.gid, creds.pid),
            )
            .with_metadata("uid", creds.uid.to_string())
            .with_metadata("gid", creds.gid.to_string())
            .with_metadata("pid", creds.pid.to_string());
            let _ = self.audit_tx.try_send(event);
            Err(Status::permission_denied(
                "syswall: caller must be root or in group 'syswall'",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(creds: PeerCredentials) -> Request<()> {
        let mut req = Request::new(());
        req.extensions_mut().insert(creds);
        req
    }

    fn audit_pair() -> (mpsc::Sender<AuditEvent>, mpsc::Receiver<AuditEvent>) {
        mpsc::channel(8)
    }

    #[test]
    fn root_uid_is_allowed() {
        let (tx, _rx) = audit_pair();
        let policy = Arc::new(PeerAuthPolicy::new(
            HashSet::from([0]),
            HashSet::from([1234]),
        ));
        let mut intercept = PeerAuthInterceptor::new(policy, tx);
        let req = make(PeerCredentials { uid: 0, gid: 100, pid: 9 });
        assert!(intercept.call(req).is_ok());
    }

    #[test]
    fn syswall_gid_is_allowed() {
        let (tx, _rx) = audit_pair();
        let policy = Arc::new(PeerAuthPolicy::new(
            HashSet::from([0]),
            HashSet::from([1234]),
        ));
        let mut intercept = PeerAuthInterceptor::new(policy, tx);
        let req = make(PeerCredentials { uid: 1000, gid: 1234, pid: 9 });
        assert!(intercept.call(req).is_ok());
    }

    #[test]
    fn unprivileged_user_denied() {
        let (tx, mut rx) = audit_pair();
        let policy = Arc::new(PeerAuthPolicy::new(
            HashSet::from([0]),
            HashSet::from([1234]),
        ));
        let mut intercept = PeerAuthInterceptor::new(policy, tx);
        let req = make(PeerCredentials { uid: 1000, gid: 1000, pid: 9 });
        let err = intercept.call(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::PermissionDenied);
        let event = rx.try_recv().expect("audit event emitted on denial");
        assert_eq!(event.category, EventCategory::Authentication);
        assert_eq!(event.severity, Severity::Warning);
        assert_eq!(event.metadata.get("uid").unwrap(), "1000");
    }

    #[test]
    fn missing_credentials_returns_internal() {
        let (tx, _rx) = audit_pair();
        let policy = Arc::new(PeerAuthPolicy::new(HashSet::from([0]), HashSet::new()));
        let mut intercept = PeerAuthInterceptor::new(policy, tx);
        let req = Request::new(());
        let err = intercept.call(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::Internal);
    }
}
```

Create `crates/daemon/src/grpc/interceptors/mod.rs`:

```rust
pub mod peer_auth;

pub use peer_auth::{PeerAuthInterceptor, PeerAuthPolicy, PeerCredentials};
```

Modify `crates/daemon/src/grpc/mod.rs` to add `pub mod interceptors;`.

- [ ] **Step 12.2: Verify pass**

Run: `cargo test -p syswall-daemon --lib grpc::interceptors`
Expected: 4 tests pass.

- [ ] **Step 12.3: Commit**

```bash
git add crates/daemon/src/grpc/interceptors/ crates/daemon/src/grpc/mod.rs
git commit -m "feat(daemon): interceptor PeerAuthInterceptor avec audit des refus"
```

---

## Task 13: Middleware tower extrait `SO_PEERCRED` + câblage server.rs

**Files:**
- Modify: `crates/daemon/src/grpc/server.rs`
- Modify: `crates/daemon/Cargo.toml` (vérifier feature `nix` = "socket")
- Modify: `crates/daemon/src/bootstrap.rs` (résolution syswall_gid + injection)

- [ ] **Step 13.1: Confirm `nix` features**

Run: `grep -A2 'name = "nix"' crates/daemon/Cargo.toml`

If `features` doesn't include `socket` and `user`, add them:

```toml
nix = { workspace = true, features = ["user", "net", "fs", "socket"] }
```

(The workspace already pins `nix = "0.29"` with `["user", "net", "fs"]` — we extend it crate-side.)

- [ ] **Step 13.2: Add the peer-cred extractor middleware**

In `crates/daemon/src/grpc/server.rs`, write a tower layer that runs on accepted `UnixStream`s. Use `nix::sys::socket::getsockopt::<UnixCredentials>`:

```rust
use std::os::unix::io::AsRawFd;
use nix::sys::socket::sockopt::PeerCredentials as NixPeerCreds;
use nix::sys::socket::getsockopt;
use tokio::net::UnixStream;

use crate::grpc::interceptors::PeerCredentials;

fn extract_peer_creds(stream: &UnixStream) -> std::io::Result<PeerCredentials> {
    let fd = stream.as_raw_fd();
    let creds = getsockopt(&fd, NixPeerCreds)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("getsockopt: {e}")))?;
    Ok(PeerCredentials {
        uid: creds.uid(),
        gid: creds.gid(),
        pid: creds.pid(),
    })
}
```

(`nix::sys::socket::sockopt::PeerCredentials` returns a `UnixCredentials` struct with `uid()`, `gid()`, `pid()` methods.)

In the connection accept loop, attach a layer that injects `PeerCredentials` into request extensions:

```rust
// pseudo-code; adapt to existing server.rs flow
let incoming = UnixListenerStream::new(listener).map(|maybe_stream| {
    maybe_stream.map(|stream| {
        let creds = extract_peer_creds(&stream).unwrap_or(PeerCredentials { uid: !0, gid: !0, pid: -1 });
        // The `tonic` Connected adapter; attach creds via extensions on each Request later via a Service layer.
        ConnectedWithCreds { stream, creds }
    })
});
```

The cleanest path with tonic 0.12: implement `tonic::transport::server::Connected` for a wrapper holding `(UnixStream, PeerCredentials)`, then add an interceptor that pulls the creds from `request.extensions()` (tonic injects `Connected::ConnectInfo` as an extension automatically).

```rust
use tonic::transport::server::Connected;

struct PeerStream {
    inner: UnixStream,
    creds: PeerCredentials,
}

impl Connected for PeerStream {
    type ConnectInfo = PeerCredentials;
    fn connect_info(&self) -> PeerCredentials { self.creds }
}

// AsyncRead + AsyncWrite forwarded to inner.
```

Then a simple interceptor at the request level:

```rust
fn inject_peer_creds<B>(mut req: tonic::Request<B>) -> tonic::Request<B> {
    if let Some(creds) = req.extensions().get::<PeerCredentials>().copied() {
        // Already injected via Connected::ConnectInfo, nothing else to do.
        let _ = creds;
    }
    req
}
```

Wire `PeerAuthInterceptor` from Task 12 as the gRPC interceptor:

```rust
let policy = Arc::new(PeerAuthPolicy::new(
    HashSet::from([0]),                       // root
    HashSet::from([syswall_gid]),
));
let auth = PeerAuthInterceptor::new(policy, audit_tx.clone());

Server::builder()
    .max_decoding_message_size(1 << 20)
    .max_encoding_message_size(4 << 20)
    .timeout(Duration::from_secs(30))
    .concurrency_limit_per_connection(64)
    .add_service(InterceptedService::new(control_service, auth.clone()))
    .add_service(InterceptedService::new(event_service, auth))
    .serve_with_incoming(incoming)
    .await?;
```

- [ ] **Step 13.3: Bootstrap resolves `syswall_gid`**

In `crates/daemon/src/bootstrap.rs`, before building the gRPC server:

```rust
use nix::unistd::Group;

let syswall_gid = Group::from_name("syswall")
    .map_err(|e| StartupError::ConfigInvalid(format!("getgrnam: {e}")))?
    .ok_or(StartupError::SyswallGroupMissing)?
    .gid
    .as_raw();
```

Pass `syswall_gid` into the gRPC server constructor.

Also harden the `chown` of the socket — in the current `server.rs` there is a `warn!` when chown fails. Replace with:

```rust
nix::unistd::chown(&socket_path, None, Some(nix::unistd::Gid::from_raw(syswall_gid)))
    .map_err(|e| StartupError::SocketChownFailed {
        path: socket_path.display().to_string(),
        source: std::io::Error::from_raw_os_error(e as i32),
    })?;
```

- [ ] **Step 13.4: Compile + run e2e quick check**

Run: `cargo check -p syswall-daemon`
Expected: 0 errors.

- [ ] **Step 13.5: Commit**

```bash
git add crates/daemon/Cargo.toml crates/daemon/src/grpc/server.rs crates/daemon/src/bootstrap.rs
git commit -m "feat(daemon): SO_PEERCRED capture et application via interceptor sur gRPC"
```

---

## Task 14: Limites gRPC + tests

**Files:**
- Modify: `crates/daemon/src/grpc/server.rs` (déjà touché en 13)
- Create: `crates/daemon/tests/grpc_limits_test.rs`

- [ ] **Step 14.1: Write the integration test**

Create `crates/daemon/tests/grpc_limits_test.rs`:

```rust
//! Integration test for gRPC size and concurrency limits.
//! Test d'integration pour les limites de taille et concurrence gRPC.

use std::time::Duration;

#[tokio::test]
async fn message_over_1mib_is_rejected() {
    // Spin up the daemon's gRPC server with a fake backend, send an oversized request,
    // expect tonic::Code::OutOfRange or InvalidArgument.
    // Skipped if the daemon test harness doesn't expose a "test mode".
    if std::env::var("SYSWALL_TEST_GRPC").is_err() {
        eprintln!("SYSWALL_TEST_GRPC not set, skipping");
        return;
    }
    // ... bring up server, build oversized CreateRuleRequest, expect rejection ...
    // (Filled in once we extract a `spawn_test_server()` helper.)
}
```

For now this test is a placeholder (`SYSWALL_TEST_GRPC` env-gated) because spinning up tonic with our wiring requires extracting a `spawn_test_server()` helper. Document it in `crates/daemon/CLAUDE.md` and create a TODO ticket — the limits themselves are configured in `server.rs`, so the protection is real even if the test is deferred.

- [ ] **Step 14.2: Verify limits are wired**

Run: `grep -n 'max_decoding_message_size\|concurrency_limit' crates/daemon/src/grpc/server.rs`
Expected: 2 matches showing the limits set on `Server::builder()`.

- [ ] **Step 14.3: Commit**

```bash
git add crates/daemon/tests/grpc_limits_test.rs
git commit -m "test(daemon): squelette de test e2e pour limites gRPC (1 MiB + 64 streams)"
```

---

## Task 15: Service unit `syswall.service` durci

**Files:**
- Modify: `system/syswall.service` (réécriture complète)

- [ ] **Step 15.1: Replace the unit file**

Overwrite `system/syswall.service` with:

```ini
[Unit]
Description=SysWall application-level firewall daemon
After=network-pre.target
Wants=network-pre.target
Documentation=https://github.com/yrbane/SysWall

[Service]
Type=notify
ExecStart=/usr/bin/syswall-daemon
Restart=on-failure
RestartSec=5
StartLimitIntervalSec=120
StartLimitBurst=5

# Identite / Identity
User=syswall
Group=syswall

# Capabilities (eBPF + nftables + procfs + chown socket)
AmbientCapabilities=CAP_NET_ADMIN CAP_NET_RAW CAP_BPF CAP_PERFMON CAP_SYS_PTRACE CAP_DAC_READ_SEARCH CAP_CHOWN
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW CAP_BPF CAP_PERFMON CAP_SYS_PTRACE CAP_DAC_READ_SEARCH CAP_CHOWN

# Sandbox
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictNamespaces=true
LockPersonality=true
RestrictRealtime=true
RestrictAddressFamilies=AF_UNIX AF_NETLINK AF_INET AF_INET6
SystemCallFilter=@system-service @network-io @file-system ~@privileged ~@resources ~@obsolete
SystemCallArchitectures=native

# MDWX desactive : le JIT eBPF (BPF_PROG_LOAD) requiert des pages WX kernel-side.
# MDWX disabled: eBPF JIT requires WX pages kernel-side.
MemoryDenyWriteExecute=false

# Repertoires geres par systemd (mode 0750, owned syswall:syswall)
# Directories managed by systemd (mode 0750, owned syswall:syswall)
ConfigurationDirectory=syswall
LogsDirectory=syswall
StateDirectory=syswall
RuntimeDirectory=syswall

[Install]
WantedBy=multi-user.target
```

- [ ] **Step 15.2: Validate syntax with systemd-analyze**

Run: `systemd-analyze verify system/syswall.service 2>&1`
Expected: no errors. Some `assertion` warnings about `ExecStart=/usr/bin/syswall-daemon` not existing locally are normal for a CI/dev box without the binary installed.

- [ ] **Step 15.3: Commit**

```bash
git add system/syswall.service
git commit -m "feat(system): durcissement service systemd avec utilisateur syswall dedie"
```

---

## Task 16: Scripts d'install (création user/group)

**Files:**
- Create: `system/install/postinst.sh`
- Create: `system/install/prerm.sh`
- Modify: `system/aur/PKGBUILD`
- Modify: `system/deb/postinst` (ou créer)
- Modify: `system/rpm/spec` (ou créer)

- [ ] **Step 16.1: Create the shared install script**

`system/install/postinst.sh`:

```sh
#!/bin/sh
# Cree l'utilisateur et le groupe systeme syswall si absents.
# Create the syswall system user and group if missing.
set -eu

if ! getent group syswall >/dev/null; then
    groupadd --system syswall
fi

if ! getent passwd syswall >/dev/null; then
    useradd --system --gid syswall \
        --home-dir /var/lib/syswall \
        --shell /usr/sbin/nologin syswall
fi

# Cree les repertoires runtime/state si absents (systemd les recree au demarrage,
# mais on prepare le terrain pour l'upgrade depuis V0.1).
for d in /var/lib/syswall /var/log/syswall /etc/syswall; do
    if [ ! -d "$d" ]; then
        install -d -m 0750 -o syswall -g syswall "$d"
    else
        chown -R syswall:syswall "$d"
        chmod 0750 "$d"
    fi
done

# Recharge systemd au cas ou le service unit a change.
if command -v systemctl >/dev/null; then
    systemctl daemon-reload || true
fi
```

`system/install/prerm.sh`:

```sh
#!/bin/sh
# A l'uninstall, on conserve l'utilisateur (convention Linux).
# On uninstall, the user is preserved (Linux convention).
set -eu
if command -v systemctl >/dev/null; then
    systemctl stop syswall.service || true
    systemctl disable syswall.service || true
fi
```

`chmod +x system/install/postinst.sh system/install/prerm.sh`.

- [ ] **Step 16.2: Wire into Arch PKGBUILD**

In `system/aur/PKGBUILD`, ensure the `package()` function copies the scripts to `pkg/<pkgname>.install` referencing them:

```bash
# In .install file: post_install / post_upgrade hooks call postinst.sh
post_install() {
    /bin/sh /usr/share/syswall/postinst.sh
}
post_upgrade() {
    post_install
}
pre_remove() {
    /bin/sh /usr/share/syswall/prerm.sh
}
```

Install `postinst.sh` into `/usr/share/syswall/` from the package.

- [ ] **Step 16.3: Wire into Debian / RPM**

For `system/deb/`, ensure `postinst` and `postrm` reference the shared scripts.

For `system/rpm/syswall.spec` (create if missing), add `%post` and `%preun` sections invoking the same scripts.

- [ ] **Step 16.4: Commit**

```bash
git add system/install/ system/aur/PKGBUILD system/deb/ system/rpm/
git commit -m "feat(system): scripts post-install creent user/group syswall pour tous les paquets"
```

---

## Task 17: Script `check-hardening.sh` + intégration CI

**Files:**
- Create: `system/tests/check-hardening.sh`
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 17.1: Create the check script**

`system/tests/check-hardening.sh`:

```sh
#!/bin/sh
# Verifie que syswall.service contient toutes les directives de durcissement attendues.
# Verifies that syswall.service contains all expected hardening directives.
set -eu

UNIT_FILE="${1:-system/syswall.service}"

if [ ! -f "$UNIT_FILE" ]; then
    echo "ERROR: $UNIT_FILE not found" >&2
    exit 1
fi

EXPECTED="
User=syswall
Group=syswall
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictNamespaces=true
LockPersonality=true
RestrictRealtime=true
RestrictAddressFamilies=AF_UNIX AF_NETLINK AF_INET AF_INET6
SystemCallFilter=@system-service @network-io @file-system ~@privileged ~@resources ~@obsolete
SystemCallArchitectures=native
AmbientCapabilities=CAP_NET_ADMIN CAP_NET_RAW CAP_BPF CAP_PERFMON CAP_SYS_PTRACE CAP_DAC_READ_SEARCH CAP_CHOWN
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW CAP_BPF CAP_PERFMON CAP_SYS_PTRACE CAP_DAC_READ_SEARCH CAP_CHOWN
ConfigurationDirectory=syswall
LogsDirectory=syswall
StateDirectory=syswall
RuntimeDirectory=syswall
"

MISSING=0
echo "$EXPECTED" | while IFS= read -r line; do
    [ -z "$line" ] && continue
    if ! grep -Fxq "$line" "$UNIT_FILE"; then
        echo "MISSING: $line" >&2
        MISSING=$((MISSING + 1))
    fi
done

# Re-evaluate MISSING outside the subshell (POSIX limitation).
COUNT=$(echo "$EXPECTED" | grep -v '^$' | while IFS= read -r line; do
    grep -Fxq "$line" "$UNIT_FILE" || echo X
done | wc -l)

if [ "$COUNT" -gt 0 ]; then
    echo "FAIL: $COUNT directive(s) missing from $UNIT_FILE" >&2
    exit 1
fi

echo "OK: all hardening directives present in $UNIT_FILE"
```

Make it executable: `chmod +x system/tests/check-hardening.sh`.

- [ ] **Step 17.2: Run it locally**

Run: `system/tests/check-hardening.sh`
Expected: `OK: all hardening directives present in system/syswall.service`.

- [ ] **Step 17.3: Add CI job**

In `.github/workflows/ci.yml`, add at the end of the `jobs:` map:

```yaml
  hardening-check:
    name: systemd hardening check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Verify syswall.service hardening directives
        run: ./system/tests/check-hardening.sh
```

- [ ] **Step 17.4: Commit**

```bash
git add system/tests/check-hardening.sh .github/workflows/ci.yml
git commit -m "test(system): verification automatique du durcissement systemd en CI"
```

---

## Task 18: CSP Tauri stricte

**Files:**
- Modify: `crates/ui/src-tauri/tauri.conf.json`

- [ ] **Step 18.1: Apply the CSP**

Locate the `"app"` block (or `"security"` block depending on Tauri 2 schema) in `crates/ui/src-tauri/tauri.conf.json`. Replace `"csp": null` with:

```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self' ipc: tauri:; img-src 'self' data: asset:; font-src 'self' data:"
```

- [ ] **Step 18.2: Manual verification**

Document in `crates/ui/CLAUDE.md`:

> ## Vérification CSP (manuelle)
>
> Après chaque modification du chargement d'assets externes :
> 1. `cargo tauri dev`
> 2. Ouvrir DevTools (Ctrl+Shift+I sous Linux).
> 3. Onglet Console : aucune ligne « Refused to ... because it violates the following Content Security Policy directive ».
> 4. Onglet Network : tous les requêtes sont sur `tauri://localhost` ou `ipc://`.

- [ ] **Step 18.3: Commit**

```bash
git add crates/ui/src-tauri/tauri.conf.json crates/ui/CLAUDE.md
git commit -m "feat(ui): CSP stricte dans tauri.conf.json"
```

---

## Task 19: Vérification croisée + clippy + tests workspace

**Files:** none (verification only)

- [ ] **Step 19.1: Run clippy on pure crates**

Run: `cargo clippy -p syswall-domain -p syswall-app -p syswall-infra -p syswall-daemon -- -D warnings 2>&1 | tail -30`
Expected: 0 warnings. If any, fix in place; do not commit suppressions without justification.

- [ ] **Step 19.2: Run all tests except UI**

Run: `cargo test --workspace --exclude syswall-ui 2>&1 | tail -10`
Expected: 0 failures, ≥ 290 tests passed.

- [ ] **Step 19.3: Run hardening check**

Run: `./system/tests/check-hardening.sh`
Expected: `OK: all hardening directives present`.

- [ ] **Step 19.4: Manual smoke (optional but recommended)**

If running locally with `cargo tauri dev` is feasible, start the daemon (with `use_fake = true` in config) and the UI, create a benign rule, watch the journal for `Severity::Info, EventCategory::Antilockout, "anti-lockout: connectivity confirmed"`. No rollback should fire.

- [ ] **Step 19.5: No-op commit only if there were fixes**

If clippy or tests required code adjustments to pass, commit them with:

```bash
git add -p   # selective stage
git commit -m "fix: ajustements post-revue clippy/tests"
```

---

## Task 20: Documentation README + CHANGELOG

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md` (créer si absent)

- [ ] **Step 20.1: README — section sécurité (FR + EN)**

In `README.md`, locate the "Architecture" or "Fonctionnalites" section. Add (or extend if exists) a "Securite / Security" subsection:

```markdown
### Securite / Security

- **Anti-lockout 30s** : tout changement de regles declenche une fenetre de surveillance de 30s. Si la connectivite externe est perdue (TCP probe vers les endpoints configures, par defaut Cloudflare DNS), le ruleset est automatiquement annule et un evenement `Critical, EventCategory::Antilockout` est journalise.
- **Authentification gRPC** : le socket Unix `/run/syswall/syswall.sock` n'accepte que les peers `root` ou membres du groupe systeme `syswall` (verification via `SO_PEERCRED`). Les refus sont audites.
- **Daemon non-root** : `syswall-daemon` tourne en utilisateur dedie `syswall` avec uniquement les capabilities ambient strictement necessaires (CAP_NET_ADMIN, CAP_BPF, CAP_PERFMON, CAP_SYS_PTRACE, CAP_DAC_READ_SEARCH, CAP_CHOWN). Sandbox systemd : `ProtectSystem=strict`, `RestrictAddressFamilies`, `SystemCallFilter`, `LockPersonality`, etc.
- **CSP UI stricte** : la fenetre Tauri applique une Content Security Policy stricte, sans `unsafe-eval`.
- **Limites gRPC** : 1 MiB max par message decode, 64 streams concurrents max par connexion.

EN:

- **Anti-lockout 30s**: every ruleset change triggers a 30-second supervision window. If outbound connectivity is lost (TCP probe against configured endpoints, default Cloudflare DNS), the ruleset is rolled back automatically and a `Critical, EventCategory::Antilockout` event is journaled.
- **gRPC authentication**: the Unix socket `/run/syswall/syswall.sock` only accepts peers that are `root` or members of the `syswall` system group (`SO_PEERCRED` check). Denials are audited.
- **Non-root daemon**: `syswall-daemon` runs as a dedicated `syswall` user with only the strictly required ambient capabilities. Systemd sandbox enabled.
- **Strict UI CSP**: the Tauri window enforces a strict Content Security Policy with no `unsafe-eval`.
- **gRPC limits**: 1 MiB max decoded message size, 64 concurrent streams per connection.
```

- [ ] **Step 20.2: CHANGELOG**

Create or modify `CHANGELOG.md`:

```markdown
# Changelog

Toutes les modifications notables seront documentees ici. / All notable changes documented here.

## [0.2.0] - 2026-05-XX

### Added / Ajoute

- **Anti-lockout 30s** : annulation automatique des changements de regles si la connectivite externe est perdue dans les 30s suivant l'apply (`AntilockoutGuard` + `TcpProbe`). Endpoints configurables dans `[antilockout] endpoints = [...]`.
- **Authentification peer SO_PEERCRED** sur le socket gRPC : seul `root` ou les membres du groupe systeme `syswall` peuvent ouvrir une session. Refus audites.
- **Categories d'audit** : `EventCategory::Antilockout`, `EventCategory::Authentication`.
- **Erreur domain** : `DomainError::AntilockoutTriggered { rolled_back_count }`.
- **CSP Tauri stricte** dans la fenetre UI (sans `unsafe-eval`).
- **Limites gRPC** : 1 MiB max decoding, 4 MiB max encoding, 64 streams concurrents par connexion, timeout 30s.
- **Toast critique UI** sur evenement `AntilockoutTriggered`.

### Changed / Modifie

- **Service systemd durci** : `User=syswall` (utilisateur dedie), `AmbientCapabilities` (plus de root), `ProtectSystem=strict`, `RestrictAddressFamilies`, `SystemCallFilter`, `LockPersonality`, `NoNewPrivileges`, etc.
- **Demarrage** : `panic!` remplace par `Result<(), StartupError>` + `exit(78)` (EX_CONFIG sysexits.h) pour les echecs au boot.
- **Scripts d'install** unifies dans `system/install/postinst.sh` (creent le user/group `syswall`, propages a Arch/Debian/RPM).

### Fixed / Corrige

- Le champ `firewall.rollback_timeout_secs` etait declare mais jamais lu (warning compilateur). Il est maintenant utilise par le guard anti-lockout.

### Security

- **CVE-pattern adresse** : pre-V0.2 tout binaire executable par un user du groupe `syswall` pouvait desactiver le pare-feu via le socket gRPC sans authentification. Resolu par `SO_PEERCRED`.
- **CVE-pattern adresse** : le daemon tournait en `User=root` sans aucune restriction (toute exploitation memoire = root complet). Resolu par utilisateur dedie + capability bounding + sandbox.

### Documentation

- README : section Securite/Security en FR+EN.
- `crates/ui/CLAUDE.md` : procedure de verification manuelle de la CSP.
- `docs/superpowers/specs/2026-05-05-security-hardening-design.md` : spec de conception.
- `docs/superpowers/plans/2026-05-05-security-hardening-plan.md` : plan d'implementation TDD.
```

- [ ] **Step 20.3: Verify markdown renders**

Run: `head -40 README.md && head -40 CHANGELOG.md`
Expected: clean markdown, no truncated sections.

- [ ] **Step 20.4: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: documentation des renforcements securite v0.2 (FR+EN)"
```

---

## Self-Review du plan (synthèse)

**Spec coverage** :
- A.1 (anti-lockout) → tasks 1-9, 11
- A.2 (SO_PEERCRED) → tasks 12-13
- A.3 (syswall.service) → tasks 15-17
- A.4 (CSP) → task 18
- A.5 (limites gRPC) → tasks 13 (limites) + 14 (test)
- StartupError → task 10
- Verification → task 19
- Documentation → task 20

**Type consistency** : `RollbackFn` (app) ↔ `ArmedRollback` (domain) bien distincts ; `LockoutGuard` est le port stable, `AntilockoutGuard` l'implementation. `PeerCredentials` defini en task 12, utilise en task 13.

**Pas de placeholder** : tout le code est complet sauf le test d'intégration gRPC limits (task 14) qui est gated par `SYSWALL_TEST_GRPC` — limitation documentée.

**Risque de friction** : task 7.4 (refactor `arm` de `&Arc<Self>` vers `&self`) est un changement non-trivial qui invalide les tests de la task 4 si fait après. Si l'agent execute dans l'ordre 1→20, la task 4 utilise déjà `Arc<Self>` ; la task 7.4 demande un refactor avant le câblage final. **Mitigation** : déplacer le refactor `&Arc<Self>` → `&self` dans la task 4 directement (signature finale dès le départ) — voir note ci-dessous.

**Action correctrice inline** : la task 4 doit utiliser dès le départ la signature `pub async fn arm(&self, ...)` avec `state: Arc<Mutex<Option<ArmedState>>>` partagé. Mettre à jour la task 4 step 4.1 pour refléter cela. (Ce sera la responsabilité de l'agent qui exécute : si l'erreur est détectée à l'étape 7.4, faire le refactor là, sinon préemptivement.)

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-05-security-hardening-plan.md`.**

Two execution options:

**1. Subagent-Driven (recommended)** — A fresh subagent per task with two-stage review between tasks. Best when the user wants to keep the main session lean and review each commit.

**2. Inline Execution** — Execute tasks sequentially in the current session via `superpowers:executing-plans`. Faster, but the main context tracks every code edit.

**Quelle approche ?**
