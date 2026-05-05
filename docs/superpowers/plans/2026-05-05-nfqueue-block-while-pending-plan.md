# NFQUEUE Block-While-Pending Implementation Plan — SysWall sub-project C.2

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a real packet-interception layer using NFQUEUE so that the first packet of every new outbound flow is held in the kernel until either an existing rule provides a verdict or the user resolves a `PendingDecision`. Includes baked-in deduplication so that multiple flows sharing the same `(app, remote_ip, remote_port, protocol)` tuple wait on a single decision.

**Architecture:** New domain port `PacketInterceptor` + handler trait `PacketDecisionHandler`. Infra adapter `NfqueueInterceptor` reads from NFQUEUE via the `nfq` crate, parses packets with `etherparse`, and asks the handler for a verdict. `LearningService` implements the handler — it consults the rule set, falls back on the existing `PolicyEngine`, and on `PendingDecision` waits up to 28 s on a per-decision broadcast channel that the user resolution unblocks. Fail-open via nft `bypass` flag. Degraded-mode boot if NFQUEUE cannot be opened.

**Tech Stack:** Rust 2024, tokio, async-trait, `nfq = "0.4"`, `etherparse = "0.16"`, nftables.

**Spec source:** `docs/superpowers/specs/2026-05-05-nfqueue-block-while-pending-design.md`

---

## Conventions for every task

- Comments and commit messages in **French**.
- Code identifiers in English.
- **NEVER add `Co-Authored-By Claude` lines** in any commit.
- Each commit must compile (`cargo check -p <crate>`) and pass tests (`cargo test -p <crate>`).
- TDD: write the failing test first, then implementation.
- Hexagonal: domain ↑, app ↑, infra ↑, daemon wires. NEVER make domain or app import infra/`nfq`/`etherparse`.

---

## File Structure

### New files

| File | Responsibility |
|---|---|
| `crates/domain/src/ports/interception.rs` | `PacketInterceptor` trait + `PacketDecisionHandler` trait + `PacketVerdict` enum |
| `crates/app/src/fakes/fake_packet_interceptor.rs` | Programmable fake interceptor for tests |
| `crates/app/src/services/learning_service/verdict_broadcast.rs` | `VerdictBroadcasts` struct managing `HashMap<PendingDecisionId, broadcast::Sender<PacketVerdict>>` |
| `crates/infra/src/nfqueue/mod.rs` | Module re-exports |
| `crates/infra/src/nfqueue/interceptor.rs` | `NfqueueInterceptor` adapter |
| `crates/infra/src/nfqueue/parser.rs` | Packet parsing (IPv4/IPv6 + TCP/UDP) → `Connection` |
| `crates/daemon/tests/nfqueue_smoke_test.rs` | Env-gated smoke test |

### Modified files

| File | Change |
|---|---|
| `crates/domain/src/ports/mod.rs` | `pub mod interception;` |
| `crates/app/src/fakes/mod.rs` | declare new fake |
| `crates/app/src/services/learning_service.rs` | maybe move to `learning_service/mod.rs` if growing past 400 LOC; add `PacketDecisionHandler` impl, integrate `VerdictBroadcasts` |
| `crates/app/src/services/mod.rs` | adjust re-exports if learning_service is split |
| `crates/infra/Cargo.toml` | add `nfq` and `etherparse` deps |
| `crates/infra/src/lib.rs` | `pub mod nfqueue;` |
| `crates/infra/src/nftables/translator/system_rules.rs` (or equivalent) | append new chain `interception` with `iif lo accept`, whitelist passes, `ct state new queue num 0 bypass` |
| `crates/daemon/src/config.rs` | `NfqueueConfig` struct + `nfqueue: Option<NfqueueConfig>` field |
| `crates/daemon/src/bootstrap.rs` | construct `NfqueueInterceptor`, spawn worker, degraded mode on failure |
| `config/default.toml` | append `[nfqueue]` block |
| `README.md` | section "Blocage actif (NFQUEUE)" in FR+EN |
| `CHANGELOG.md` | new section under `[0.2.0]` for NFQUEUE |

---

## Task 1: Domain port `PacketInterceptor`

**Files:**
- Create: `crates/domain/src/ports/interception.rs`
- Modify: `crates/domain/src/ports/mod.rs`

- [ ] **Step 1.1: Write the failing test**

Create `crates/domain/src/ports/interception.rs`:

```rust
use async_trait::async_trait;
use std::sync::Arc;

use crate::entities::Connection;
use crate::errors::DomainError;

/// Verdict to apply to a captured packet.
/// Verdict à appliquer à un paquet capturé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketVerdict {
    /// Allow the packet through.
    /// Laisser passer le paquet.
    Accept,
    /// Drop the packet (kernel-side).
    /// Jeter le paquet (côté kernel).
    Drop,
}

/// Handler resolving a verdict for a captured connection.
/// Handler résolvant un verdict pour une connexion capturée.
#[async_trait]
pub trait PacketDecisionHandler: Send + Sync {
    async fn decide(&self, connection: &Connection) -> Result<PacketVerdict, DomainError>;
}

/// Intercepts the first packet of every new flow and asks for a verdict.
/// Intercepte le premier paquet de chaque nouveau flux et demande un verdict.
#[async_trait]
pub trait PacketInterceptor: Send + Sync {
    /// Run the interception loop until the cancel token fires.
    /// Lance la boucle d'interception jusqu'au déclenchement du cancel token.
    async fn run(
        &self,
        handler: Arc<dyn PacketDecisionHandler>,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<(), DomainError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_equality() {
        assert_eq!(PacketVerdict::Accept, PacketVerdict::Accept);
        assert_ne!(PacketVerdict::Accept, PacketVerdict::Drop);
    }
}
```

Modify `crates/domain/src/ports/mod.rs` to add:

```rust
pub mod interception;
// ... existing modules ...
pub use interception::*;
```

(Place alphabetically among existing modules.)

- [ ] **Step 1.2: Verify**

Run: `cargo test -p syswall-domain --lib ports::interception 2>&1 | tail`
Expected: 1 test passes.

`tokio_util` should already be in workspace deps; if not, add `tokio-util = { workspace = true }` to `crates/domain/Cargo.toml` with feature `["rt"]`. Verify with `grep tokio-util crates/domain/Cargo.toml`.

- [ ] **Step 1.3: Commit**

```bash
git add crates/domain/src/ports/interception.rs crates/domain/src/ports/mod.rs crates/domain/Cargo.toml
git commit -m "feat(domain): port PacketInterceptor + PacketDecisionHandler pour le blocage actif"
```

---

## Task 2: Fake `PacketInterceptor` (app)

**Files:**
- Create: `crates/app/src/fakes/fake_packet_interceptor.rs`
- Modify: `crates/app/src/fakes/mod.rs`

- [ ] **Step 2.1: Write the failing test + implementation**

Create `crates/app/src/fakes/fake_packet_interceptor.rs`:

```rust
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

use syswall_domain::entities::Connection;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::interception::{
    PacketDecisionHandler, PacketInterceptor, PacketVerdict,
};

/// Programmable fake `PacketInterceptor` for tests.
/// Fake programmable du `PacketInterceptor` pour les tests.
#[derive(Debug, Default)]
pub struct FakePacketInterceptor {
    /// Connections to inject into the handler.
    /// Connexions à injecter dans le handler.
    pub injectable: Arc<Mutex<Vec<Connection>>>,
    /// Verdicts captured from the handler, in order.
    /// Verdicts capturés depuis le handler, dans l'ordre.
    pub captured: Arc<Mutex<Vec<PacketVerdict>>>,
}

impl FakePacketInterceptor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inject(&self, conn: Connection) {
        self.injectable
            .lock()
            .expect("Mutex jamais empoisonne dans un test")
            .push(conn);
    }

    pub fn captured_verdicts(&self) -> Vec<PacketVerdict> {
        self.captured
            .lock()
            .expect("Mutex jamais empoisonne dans un test")
            .clone()
    }
}

#[async_trait]
impl PacketInterceptor for FakePacketInterceptor {
    async fn run(
        &self,
        handler: Arc<dyn PacketDecisionHandler>,
        cancel: CancellationToken,
    ) -> Result<(), DomainError> {
        // Dépile les connections injectées et appelle le handler.
        // Drains injected connections and calls the handler.
        loop {
            if cancel.is_cancelled() {
                return Ok(());
            }
            let next = {
                let mut q = self
                    .injectable
                    .lock()
                    .expect("Mutex jamais empoisonne dans un test");
                q.pop()
            };
            match next {
                Some(conn) => {
                    let verdict = handler.decide(&conn).await?;
                    self.captured
                        .lock()
                        .expect("Mutex jamais empoisonne dans un test")
                        .push(verdict);
                }
                None => {
                    tokio::task::yield_now().await;
                    if cancel.is_cancelled() {
                        return Ok(());
                    }
                    break;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysAccept;
    #[async_trait]
    impl PacketDecisionHandler for AlwaysAccept {
        async fn decide(&self, _conn: &Connection) -> Result<PacketVerdict, DomainError> {
            Ok(PacketVerdict::Accept)
        }
    }

    fn dummy_connection() -> Connection {
        // Build a minimal Connection — adapt to the real constructor present in your codebase.
        // The implementer should inspect `crates/domain/src/entities/connection.rs` for the public API.
        Connection::default()
    }

    #[tokio::test]
    async fn fake_runs_handler_on_each_injected_connection() {
        let fake = FakePacketInterceptor::new();
        fake.inject(dummy_connection());
        fake.inject(dummy_connection());
        let cancel = CancellationToken::new();
        fake.run(Arc::new(AlwaysAccept), cancel).await.unwrap();
        assert_eq!(fake.captured_verdicts().len(), 2);
        assert!(fake.captured_verdicts().iter().all(|v| *v == PacketVerdict::Accept));
    }
}
```

If `Connection::default()` does not exist on the real type, the implementer must build a Connection literal matching the real struct's fields. Use `grep -n 'pub struct Connection' crates/domain/src/entities/connection.rs` and reproduce a minimal valid instance.

Modify `crates/app/src/fakes/mod.rs` to declare `pub mod fake_packet_interceptor;` alphabetically.

- [ ] **Step 2.2: Verify**

Run: `cargo test -p syswall-app --lib fakes::fake_packet_interceptor 2>&1 | tail`
Expected: 1 test passes.

- [ ] **Step 2.3: Commit**

```bash
git add crates/app/src/fakes/fake_packet_interceptor.rs crates/app/src/fakes/mod.rs
git commit -m "feat(app): fake FakePacketInterceptor pour tests d'integration NFQUEUE"
```

---

## Task 3: `VerdictBroadcasts` helper

**Files:**
- Create: `crates/app/src/services/learning_service/verdict_broadcast.rs` (if learning_service is split into a directory; otherwise create as `crates/app/src/services/verdict_broadcast.rs`)

- [ ] **Step 3.1: Decide whether to split learning_service.rs**

Run: `wc -l crates/app/src/services/learning_service.rs`
If ≥ 350 LOC, split now into a directory:
```bash
mkdir -p crates/app/src/services/learning_service
git mv crates/app/src/services/learning_service.rs crates/app/src/services/learning_service/mod.rs
```
Otherwise keep it as a single file and place `verdict_broadcast.rs` alongside it as `crates/app/src/services/verdict_broadcast.rs` (then add `mod verdict_broadcast;` in `services/mod.rs`).

The expected post-Task-7 size of learning_service is ~450 LOC, so splitting is recommended. Pick the directory layout.

- [ ] **Step 3.2: Write the failing test + implementation**

`crates/app/src/services/learning_service/verdict_broadcast.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use syswall_domain::entities::PendingDecisionId;
use syswall_domain::ports::interception::PacketVerdict;

/// Manages broadcast channels keyed by `PendingDecisionId`. Multiple captured packets
/// awaiting the same decision subscribe to the same channel; resolution publishes
/// the verdict to all subscribers and removes the channel.
/// Gère les canaux broadcast indexés par `PendingDecisionId`. Plusieurs paquets
/// capturés attendant la même décision s'abonnent au même canal ; la résolution
/// publie le verdict à tous les abonnés et retire le canal.
#[derive(Debug, Default)]
pub struct VerdictBroadcasts {
    inner: Arc<Mutex<HashMap<PendingDecisionId, broadcast::Sender<PacketVerdict>>>>,
}

impl VerdictBroadcasts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a broadcast for a decision.
    /// Récupère ou crée un broadcast pour une décision.
    pub async fn get_or_create(&self, id: PendingDecisionId) -> broadcast::Sender<PacketVerdict> {
        let mut map = self.inner.lock().await;
        map.entry(id)
            .or_insert_with(|| broadcast::channel(64).0)
            .clone()
    }

    /// Subscribe a receiver to an existing broadcast (creating if absent).
    /// Abonne un receveur à un broadcast existant (création si absent).
    pub async fn subscribe(&self, id: PendingDecisionId) -> broadcast::Receiver<PacketVerdict> {
        let sender = self.get_or_create(id).await;
        sender.subscribe()
    }

    /// Publish a verdict and remove the broadcast.
    /// Publie un verdict et retire le broadcast.
    pub async fn publish_and_remove(&self, id: PendingDecisionId, verdict: PacketVerdict) {
        let mut map = self.inner.lock().await;
        if let Some(sender) = map.remove(&id) {
            // Best-effort send: receivers may have already dropped (timeouts).
            // Envoi best-effort : les receveurs peuvent avoir déjà été abandonnés.
            let _ = sender.send(verdict);
        }
    }

    /// Number of active broadcasts (for tests/observability).
    /// Nombre de broadcasts actifs (pour tests/observabilité).
    pub async fn active_count(&self) -> usize {
        self.inner.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_propagates_verdict_to_subscribers() {
        let broadcasts = VerdictBroadcasts::new();
        let id = PendingDecisionId::new();
        let mut rx1 = broadcasts.subscribe(id).await;
        let mut rx2 = broadcasts.subscribe(id).await;
        broadcasts.publish_and_remove(id, PacketVerdict::Accept).await;
        assert_eq!(rx1.recv().await.unwrap(), PacketVerdict::Accept);
        assert_eq!(rx2.recv().await.unwrap(), PacketVerdict::Accept);
        assert_eq!(broadcasts.active_count().await, 0);
    }

    #[tokio::test]
    async fn publish_unknown_id_is_noop() {
        let broadcasts = VerdictBroadcasts::new();
        let id = PendingDecisionId::new();
        broadcasts.publish_and_remove(id, PacketVerdict::Drop).await;
        assert_eq!(broadcasts.active_count().await, 0);
    }
}
```

In `crates/app/src/services/learning_service/mod.rs` (or wherever the parent lives), add `mod verdict_broadcast;` and `pub use verdict_broadcast::VerdictBroadcasts;` if external visibility is needed. Otherwise keep it `pub(super)`.

- [ ] **Step 3.3: Verify**

Run: `cargo test -p syswall-app --lib services::learning_service::verdict_broadcast 2>&1 | tail` (or `services::verdict_broadcast` if not nested)
Expected: 2 tests pass.

- [ ] **Step 3.4: Commit**

```bash
git add crates/app/src/services/learning_service/ crates/app/src/services/mod.rs
git commit -m "feat(app): VerdictBroadcasts pour synchroniser les verdicts NFQUEUE par PendingDecision"
```

---

## Task 4: `LearningService` implements `PacketDecisionHandler`

**Files:**
- Modify: `crates/app/src/services/learning_service/mod.rs` (or `learning_service.rs`)

- [ ] **Step 4.1: Add PolicyEngine dependency to LearningService**

Currently the service has `pending_repo`, `decision_repo`, `notifier`, `event_bus`, `config`. To implement `PacketDecisionHandler`, it also needs:
- `policy_engine: Arc<PolicyEngine>` (or use the static `PolicyEngine::evaluate` if it's a free function — verify with `grep -n 'impl PolicyEngine\|pub fn evaluate' crates/domain/src/services/policy_engine/`)
- `rule_repo: Arc<dyn RuleRepository>` to fetch the current ruleset
- `default_policy: DefaultPolicy` from config
- `verdict_broadcasts: Arc<VerdictBroadcasts>`

Update `LearningService::new` signature to accept the new dependencies. All existing call sites in `crates/daemon/src/bootstrap.rs` will need updating.

- [ ] **Step 4.2: Write failing tests for the 7 handler scenarios**

Add to the `tests` module of the learning_service file:

```rust
#[tokio::test]
async fn decide_existing_allow_rule_returns_accept() {
    // Build LearningService with a FakeRuleRepository pre-seeded with one
    // rule that matches the test connection and has effect Allow.
    // Inject a connection, call service.decide(&conn).await.
    // Assert verdict == PacketVerdict::Accept.
    // Assert no PendingDecision was created (FakePendingDecisionRepository::pending_count() == 0).
    todo!("implementer écrit le test complet en suivant cette spec")
}

#[tokio::test]
async fn decide_no_rule_default_block_returns_drop() {
    // FakeRuleRepository empty + default_policy = DefaultPolicy::Block.
    // Assert verdict == Drop, no pending created.
    todo!("implementer écrit le test complet en suivant cette spec")
}

// ... 5 more (see spec)
```

The implementer must write the actual test bodies — the spec lists the 7 scenarios. The pattern: build dependencies as fakes, instantiate `LearningService::new(...)`, call `service.decide(&conn).await`, assert.

For tests 3-5 (waiting on user resolution), use `tokio::time::pause()` + a parallel task that simulates user resolution by calling `service.respond_to_decision(...)` (the existing method, which should be updated to also publish to `verdict_broadcasts`).

- [ ] **Step 4.3: Implement `PacketDecisionHandler` for `LearningService`**

```rust
use std::time::Duration;
use syswall_domain::ports::interception::{PacketDecisionHandler, PacketVerdict};
use syswall_domain::value_objects::ConnectionVerdict;
use syswall_domain::events::DefaultPolicy;

#[async_trait::async_trait]
impl PacketDecisionHandler for LearningService {
    async fn decide(&self, connection: &Connection) -> Result<PacketVerdict, DomainError> {
        // 1. Evaluate via PolicyEngine.
        let rules = self.rule_repo.list_enabled().await?;
        let evaluation = PolicyEngine::evaluate(connection, &rules, self.default_policy);
        match evaluation.verdict {
            ConnectionVerdict::Allowed => Ok(PacketVerdict::Accept),
            ConnectionVerdict::Blocked | ConnectionVerdict::Ignored => Ok(PacketVerdict::Drop),
            ConnectionVerdict::PendingDecision => self.pending_verdict_for(connection).await,
        }
    }
}

impl LearningService {
    async fn pending_verdict_for(&self, conn: &Connection) -> Result<PacketVerdict, DomainError> {
        let snapshot = ConnectionSnapshot::from_connection(conn);
        let dedup_key = Self::dedup_key(&snapshot);

        // Debounce: existing pending?
        if let Some(existing) = self.pending_repo.find_by_dedup_key(&dedup_key).await? {
            if existing.status == PendingDecisionStatus::Pending && !existing.is_expired() {
                return self.wait_for_verdict(existing.id).await;
            }
        }

        // Create new PendingDecision.
        let pd = PendingDecision::new(
            snapshot,
            dedup_key,
            self.config.prompt_timeout_secs,
        );
        self.pending_repo.create(&pd).await?;
        self.event_bus
            .publish(DomainEvent::PendingDecisionCreated { id: pd.id })
            .await?;
        self.notifier.notify_pending_decision(&pd).await?;

        self.wait_for_verdict(pd.id).await
    }

    async fn wait_for_verdict(&self, id: PendingDecisionId) -> Result<PacketVerdict, DomainError> {
        let mut rx = self.verdict_broadcasts.subscribe(id).await;
        match tokio::time::timeout(Duration::from_secs(28), rx.recv()).await {
            Ok(Ok(verdict)) => Ok(verdict),
            Ok(Err(_)) => Ok(PacketVerdict::Drop), // sender dropped → kernel will drop on its own; play safe.
            Err(_) => {
                // 28s elapsed — the kernel will drop after 30s anyway. Play safe.
                Ok(PacketVerdict::Drop)
            }
        }
    }
}
```

The signature of `LearningService::new` must change to accept:
- `rule_repo: Arc<dyn RuleRepository>`
- `default_policy: DefaultPolicy`
- `verdict_broadcasts: Arc<VerdictBroadcasts>`

Update all call sites (most importantly `crates/daemon/src/bootstrap.rs`).

The `PolicyEngine::evaluate` signature: inspect `crates/domain/src/services/policy_engine/evaluator.rs`. It's currently `pub fn evaluate(connection: &Connection, rules: &[Rule], default_policy: DefaultPolicy) -> PolicyEvaluation` (sub-project B). Use it directly.

`Connection` and `ConnectionSnapshot` may differ — verify with `grep -n 'fn dedup_key\|ConnectionSnapshot' crates/domain/src/entities/connection.rs`. The current `LearningService::dedup_key` takes a `ConnectionSnapshot`; reuse that.

- [ ] **Step 4.4: Update `respond_to_decision` to publish to `verdict_broadcasts`**

In the existing `LearningService::respond_to_decision` (the method that handles `RespondToDecisionCommand`), after persisting the user's resolution, call:

```rust
let verdict = match action {
    DecisionAction::AllowOnce | DecisionAction::AlwaysAllow => PacketVerdict::Accept,
    DecisionAction::CreateRule { effect: RuleEffect::Allow, .. } => PacketVerdict::Accept,
    DecisionAction::BlockOnce | DecisionAction::AlwaysBlock => PacketVerdict::Drop,
    DecisionAction::CreateRule { effect: RuleEffect::Block, .. } => PacketVerdict::Drop,
    DecisionAction::Ignore => PacketVerdict::Drop,
    // any other variants: default to Drop (fail-safe).
    _ => PacketVerdict::Drop,
};
self.verdict_broadcasts.publish_and_remove(decision_id, verdict).await;
```

Adapt the match to the actual `DecisionAction` variants in `crates/domain/src/entities/decision.rs`.

- [ ] **Step 4.5: Verify all 7 tests pass**

```bash
cargo test -p syswall-app --lib services::learning_service 2>&1 | tail
cargo clippy -p syswall-app --all-targets -- -D warnings 2>&1 | tail
```

Expected: 7+ tests pass (existing + 7 new), 0 clippy warnings.

- [ ] **Step 4.6: Commit**

```bash
git add crates/app/src/services/learning_service/ crates/app/src/services/mod.rs
# Plus any callers that needed signature updates:
git add crates/daemon/src/bootstrap.rs
git commit -m "feat(app): LearningService implemente PacketDecisionHandler avec debounce + timeout"
```

---

## Task 5: NFQUEUE adapter skeleton + IPv4 TCP parsing

**Files:**
- Modify: `crates/infra/Cargo.toml` (add `nfq` and `etherparse`)
- Create: `crates/infra/src/nfqueue/mod.rs`
- Create: `crates/infra/src/nfqueue/parser.rs`
- Create: `crates/infra/src/nfqueue/interceptor.rs`
- Modify: `crates/infra/src/lib.rs`

- [ ] **Step 5.1: Add deps**

In `crates/infra/Cargo.toml`, add to `[dependencies]`:

```toml
nfq = "0.4"
etherparse = "0.16"
```

(If you want them workspace-managed, also add to `Cargo.toml` workspace dependencies and use `workspace = true`. The current pattern: most deps are workspace-pinned. Follow the existing pattern.)

Verify: `cargo check -p syswall-infra 2>&1 | tail`. Expected: 0 errors.

- [ ] **Step 5.2: Write failing tests for parsing**

Create `crates/infra/src/nfqueue/parser.rs`:

```rust
//! Packet parsing: raw bytes from NFQUEUE -> domain Connection.
//! Parsing de paquets : octets bruts NFQUEUE -> Connection domain.

use etherparse::{IpHeaders, NetSlice, SlicedPacket};
use std::net::IpAddr;

use syswall_domain::entities::Connection;
use syswall_domain::errors::DomainError;
use syswall_domain::value_objects::Protocol;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("packet too short (< {0} bytes)")]
    TooShort(usize),
    #[error("unsupported L3: {0}")]
    UnsupportedL3(String),
    #[error("unsupported L4: {0}")]
    UnsupportedL4(String),
    #[error("etherparse: {0}")]
    Etherparse(String),
}

impl From<ParseError> for DomainError {
    fn from(e: ParseError) -> Self {
        DomainError::Validation(format!("packet parse: {e}"))
    }
}

/// Parse a raw packet (starting at the IP header) into a domain Connection.
/// Parse un paquet brut (commençant à l'en-tête IP) en Connection domain.
pub fn parse_packet(bytes: &[u8]) -> Result<Connection, ParseError> {
    let parsed = SlicedPacket::from_ip(bytes).map_err(|e| ParseError::Etherparse(e.to_string()))?;

    let (src_ip, dst_ip): (IpAddr, IpAddr) = match &parsed.net {
        Some(NetSlice::Ipv4(h)) => (
            IpAddr::V4(h.header().source_addr()),
            IpAddr::V4(h.header().destination_addr()),
        ),
        Some(NetSlice::Ipv6(h)) => (
            IpAddr::V6(h.header().source_addr()),
            IpAddr::V6(h.header().destination_addr()),
        ),
        _ => return Err(ParseError::UnsupportedL3("non-IP".into())),
    };

    let (protocol, src_port, dst_port) = match &parsed.transport {
        Some(etherparse::TransportSlice::Tcp(t)) => (Protocol::Tcp, t.source_port(), t.destination_port()),
        Some(etherparse::TransportSlice::Udp(u)) => (Protocol::Udp, u.source_port(), u.destination_port()),
        _ => return Err(ParseError::UnsupportedL4("non-TCP/UDP".into())),
    };

    // Build the Connection with whatever fields the real type has.
    // The implementer must adapt the construction to the actual domain Connection API.
    // The 5-tuple above is the minimum necessary information.
    Ok(Connection::new_outbound(src_ip, src_port, dst_ip, dst_port, protocol))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid IPv4 + TCP SYN packet.
    /// Constructed with hand-crafted bytes for deterministic testing.
    fn ipv4_tcp_syn() -> Vec<u8> {
        // 20-byte IPv4 header (src 10.0.0.1 → dst 1.2.3.4, total length 40, proto TCP)
        // + 20-byte TCP header (src port 12345 → dst port 443, SYN flag).
        // Use etherparse builder to avoid hand-bit-twiddling errors.
        let mut buffer = Vec::new();
        let builder = etherparse::PacketBuilder::ipv4(
            [10, 0, 0, 1],   // src
            [1, 2, 3, 4],    // dst
            64,              // ttl
        )
        .tcp(12345, 443, 0, 1024); // src port, dst port, seq, window
        let payload: &[u8] = &[];
        builder.write(&mut buffer, payload).expect("etherparse builder ok");
        buffer
    }

    #[test]
    fn parses_ipv4_tcp_into_connection() {
        let bytes = ipv4_tcp_syn();
        let conn = parse_packet(&bytes).expect("parse ok");
        // Adapt these assertions to the actual Connection API.
        assert!(matches!(conn.protocol(), Protocol::Tcp));
    }

    #[test]
    fn rejects_truncated_packet() {
        let result = parse_packet(&[0u8; 4]);
        assert!(result.is_err());
    }
}
```

The implementer must adapt `Connection::new_outbound` and `conn.protocol()` to the real API. Inspect `crates/domain/src/entities/connection.rs` first.

- [ ] **Step 5.3: Verify**

Run: `cargo test -p syswall-infra --lib nfqueue::parser 2>&1 | tail`
Expected: 2 tests pass.

- [ ] **Step 5.4: Skeleton interceptor**

Create `crates/infra/src/nfqueue/interceptor.rs`:

```rust
//! NFQUEUE adapter: read packets, ask handler for verdict, forward to kernel.
//! Adapter NFQUEUE : lit les paquets, demande au handler un verdict, le transmet au kernel.

use async_trait::async_trait;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use syswall_domain::errors::DomainError;
use syswall_domain::ports::interception::{
    PacketDecisionHandler, PacketInterceptor, PacketVerdict,
};

use super::parser::parse_packet;

/// Overflow policy when the queue is saturated.
/// Politique en cas de saturation de la queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// Drop the packet (fail-closed for the saturated burst).
    /// Jeter le paquet (fail-closed sur les bursts saturés).
    Block,
    /// Accept the packet (fail-open for the saturated burst).
    /// Laisser passer le paquet (fail-open sur les bursts saturés).
    Accept,
}

/// NFQUEUE-based packet interceptor.
/// Intercepteur de paquets basé sur NFQUEUE.
pub struct NfqueueInterceptor {
    queue_num: u16,
    max_queued: u32,
    overflow: OverflowPolicy,
}

impl NfqueueInterceptor {
    pub fn new(queue_num: u16, max_queued: u32, overflow: OverflowPolicy) -> Self {
        Self {
            queue_num,
            max_queued,
            overflow,
        }
    }
}

#[async_trait]
impl PacketInterceptor for NfqueueInterceptor {
    async fn run(
        &self,
        handler: Arc<dyn PacketDecisionHandler>,
        cancel: CancellationToken,
    ) -> Result<(), DomainError> {
        // Open the queue. The `nfq` crate's `Queue::open` returns Result.
        let mut queue = nfq::Queue::open()
            .map_err(|e| DomainError::Infrastructure(format!("nfq open: {e}")))?;
        queue
            .bind(self.queue_num)
            .map_err(|e| DomainError::Infrastructure(format!("nfq bind {}: {e}", self.queue_num)))?;
        queue
            .set_queue_max_len(self.queue_num, self.max_queued)
            .map_err(|e| DomainError::Infrastructure(format!("nfq set_queue_max_len: {e}")))?;
        info!(target: "nfqueue", queue_num = self.queue_num, max_queued = self.max_queued, "interception started");

        // Note: `nfq` is a sync API. We run the read loop on a blocking task
        // and forward each packet to the async handler via tokio::spawn.
        loop {
            if cancel.is_cancelled() {
                info!(target: "nfqueue", "interception cancelled");
                return Ok(());
            }

            // Try to recv a message with a small timeout (so we can check cancel periodically).
            // The `nfq` API may not have a timeout — use `set_recv_timeout` or poll fd.
            // Implementer: pick the right primitive after reading the nfq docs.
            let mut msg = match queue.recv() {
                Ok(m) => m,
                Err(e) => {
                    warn!(target: "nfqueue", "recv error: {e}");
                    continue;
                }
            };

            let verdict = match parse_packet(msg.get_payload()) {
                Ok(conn) => match handler.decide(&conn).await {
                    Ok(v) => v,
                    Err(e) => {
                        error!(target: "nfqueue", "handler error: {e}");
                        // Fail-closed on handler errors.
                        PacketVerdict::Drop
                    }
                },
                Err(e) => {
                    warn!(target: "nfqueue", "parse error: {e}");
                    // Unparseable: accept (fail-open) so we don't disrupt unknown protocols.
                    PacketVerdict::Accept
                }
            };

            let nfq_verdict = match verdict {
                PacketVerdict::Accept => nfq::Verdict::Accept,
                PacketVerdict::Drop => nfq::Verdict::Drop,
            };
            msg.set_verdict(nfq_verdict);
            if let Err(e) = queue.verdict(msg) {
                warn!(target: "nfqueue", "verdict send error: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_policy_equality() {
        assert_eq!(OverflowPolicy::Block, OverflowPolicy::Block);
        assert_ne!(OverflowPolicy::Block, OverflowPolicy::Accept);
    }
}
```

Note: the actual `nfq` 0.4 API may differ. The implementer must consult `cargo doc --open -p nfq` (or `https://docs.rs/nfq`) and adapt method names. The structure above is illustrative.

The async-over-sync bridge needs care. The simplest approach: spawn a `tokio::task::spawn_blocking` for the recv loop and use a tokio channel to forward parsed packets to async-aware verdict resolvers. Picking that strategy is implementer judgment — document the choice in code comments.

Create `crates/infra/src/nfqueue/mod.rs`:

```rust
pub mod interceptor;
pub mod parser;

pub use interceptor::{NfqueueInterceptor, OverflowPolicy};
```

Add to `crates/infra/src/lib.rs`: `pub mod nfqueue;` (alphabetically).

- [ ] **Step 5.5: Verify**

```bash
cargo check -p syswall-infra 2>&1 | tail
cargo clippy -p syswall-infra --all-targets -- -D warnings 2>&1 | tail
```

Expected: 0 errors, 0 warnings (the `nfq` API mismatches will surface here — fix iteratively).

- [ ] **Step 5.6: Commit**

```bash
git add crates/infra/Cargo.toml crates/infra/src/nfqueue/ crates/infra/src/lib.rs Cargo.lock
git commit -m "feat(infra): NfqueueInterceptor skeleton + parsing IPv4 TCP/UDP via etherparse"
```

---

## Task 6: IPv6 + UDP parsing tests

**Files:**
- Modify: `crates/infra/src/nfqueue/parser.rs` (add tests)

- [ ] **Step 6.1: Add 2 more tests**

Append to the `tests` module:

```rust
#[test]
fn parses_ipv6_udp_into_connection() {
    let mut buffer = Vec::new();
    let builder = etherparse::PacketBuilder::ipv6(
        [0u8; 16],   // src ::
        [0u8; 16],   // dst :: (whatever, just deterministic)
        64,
    )
    .udp(12345, 53);
    let payload: &[u8] = b"\x12\x34"; // 2 bytes payload
    builder.write(&mut buffer, payload).expect("etherparse builder ok");

    let conn = parse_packet(&buffer).expect("parse ok");
    assert!(matches!(conn.protocol(), Protocol::Udp));
}

#[test]
fn parses_ipv4_udp() {
    let mut buffer = Vec::new();
    let builder = etherparse::PacketBuilder::ipv4([10, 0, 0, 1], [1, 1, 1, 1], 64).udp(54321, 53);
    let payload: &[u8] = b"\x00\x00";
    builder.write(&mut buffer, payload).expect("etherparse builder ok");
    let conn = parse_packet(&buffer).expect("parse ok");
    assert!(matches!(conn.protocol(), Protocol::Udp));
}
```

- [ ] **Step 6.2: Verify**

```bash
cargo test -p syswall-infra --lib nfqueue::parser 2>&1 | tail
```

Expected: 4 tests pass.

- [ ] **Step 6.3: Commit**

```bash
git add crates/infra/src/nfqueue/parser.rs
git commit -m "test(infra/nfqueue): parsing IPv6 et UDP"
```

---

## Task 7: Config `[nfqueue]` section

**Files:**
- Modify: `crates/daemon/src/config.rs`
- Modify: `config/default.toml`

- [ ] **Step 7.1: Write failing tests + struct**

Append to `crates/daemon/src/config.rs` near the other configs:

```rust
/// NFQUEUE-based active blocking configuration.
/// Configuration du blocage actif via NFQUEUE.
#[derive(Debug, Clone, Deserialize)]
pub struct NfqueueConfig {
    #[serde(default = "default_nfq_enabled")]
    pub enabled: bool,
    #[serde(default = "default_nfq_queue_num")]
    pub queue_num: u16,
    #[serde(default = "default_nfq_max_queued")]
    pub max_queued: u32,
    #[serde(default = "default_nfq_overflow_policy")]
    pub overflow_policy: String, // "block" or "accept"
}

fn default_nfq_enabled() -> bool { true }
fn default_nfq_queue_num() -> u16 { 0 }
fn default_nfq_max_queued() -> u32 { 1024 }
fn default_nfq_overflow_policy() -> String { "block".to_string() }

impl Default for NfqueueConfig {
    fn default() -> Self {
        Self {
            enabled: default_nfq_enabled(),
            queue_num: default_nfq_queue_num(),
            max_queued: default_nfq_max_queued(),
            overflow_policy: default_nfq_overflow_policy(),
        }
    }
}
```

Add field to `SysWallConfig`:
```rust
#[serde(default)]
pub nfqueue: Option<NfqueueConfig>,
```

Add tests in the `tests` module:

```rust
const NFQ_TOML: &str = r#"
[nfqueue]
enabled = true
queue_num = 7
max_queued = 2048
overflow_policy = "accept"
"#;

#[test]
fn parse_nfqueue_section() {
    let full = format!("{}\n{}", TEST_CONFIG, NFQ_TOML);
    let config = SysWallConfig::from_toml(&full).unwrap();
    let nq = config.nfqueue.as_ref().unwrap();
    assert!(nq.enabled);
    assert_eq!(nq.queue_num, 7);
    assert_eq!(nq.max_queued, 2048);
    assert_eq!(nq.overflow_policy, "accept");
}

#[test]
fn nfqueue_section_is_optional() {
    let cfg = SysWallConfig::from_toml(TEST_CONFIG).unwrap();
    assert!(cfg.nfqueue.is_none());
}

#[test]
fn nfqueue_default_values() {
    let cfg = NfqueueConfig::default();
    assert!(cfg.enabled);
    assert_eq!(cfg.queue_num, 0);
    assert_eq!(cfg.max_queued, 1024);
    assert_eq!(cfg.overflow_policy, "block");
}
```

Append to `config/default.toml`:

```toml

[nfqueue]
enabled = true
queue_num = 0
max_queued = 1024
overflow_policy = "block"
```

- [ ] **Step 7.2: Verify**

```bash
cargo test -p syswall-daemon config 2>&1 | tail
```

Expected: 3 new tests pass.

- [ ] **Step 7.3: Commit**

```bash
git add crates/daemon/src/config.rs config/default.toml
git commit -m "feat(daemon): config [nfqueue] (queue_num, max_queued, overflow_policy)"
```

---

## Task 8: Bootstrap wires `NfqueueInterceptor` with degraded mode

**Files:**
- Modify: `crates/daemon/src/bootstrap.rs`

- [ ] **Step 8.1: Add wiring**

After `LearningService` is constructed and the cancel token is available, add:

```rust
use std::sync::Arc;
use syswall_app::services::learning_service::VerdictBroadcasts;
use syswall_domain::ports::interception::{PacketDecisionHandler, PacketInterceptor};
use syswall_infra::nfqueue::{NfqueueInterceptor, OverflowPolicy};

let nfq_cfg = config.nfqueue.clone().unwrap_or_default();
if nfq_cfg.enabled {
    let overflow = match nfq_cfg.overflow_policy.as_str() {
        "accept" => OverflowPolicy::Accept,
        _ => OverflowPolicy::Block,
    };
    let interceptor: Arc<dyn PacketInterceptor> = Arc::new(NfqueueInterceptor::new(
        nfq_cfg.queue_num,
        nfq_cfg.max_queued,
        overflow,
    ));
    let handler: Arc<dyn PacketDecisionHandler> = learning_service.clone();
    let cancel_token = supervisor_cancel_token.child_token();
    tokio::spawn(async move {
        match interceptor.run(handler, cancel_token).await {
            Ok(()) => tracing::info!(target: "nfqueue", "interception loop terminated cleanly"),
            Err(e) => tracing::error!(target: "nfqueue", "interception failed (mode degrade): {e}"),
        }
    });
} else {
    tracing::warn!(target: "nfqueue", "interception disabled by config — observation-only mode");
}
```

The variable name `supervisor_cancel_token` is illustrative — use the actual cancel-token plumbing already in `bootstrap.rs`.

`learning_service` must be an `Arc<LearningService>` for the handler upcast to work. If it's currently a value, wrap it in `Arc::new(...)` after construction.

- [ ] **Step 8.2: Verify**

```bash
cargo check -p syswall-daemon 2>&1 | tail
cargo test -p syswall-daemon 2>&1 | tail
```

Expected: 0 errors, all tests pass.

- [ ] **Step 8.3: Commit**

```bash
git add crates/daemon/src/bootstrap.rs
git commit -m "feat(daemon): bootstrap lance NfqueueInterceptor avec mode degrade en cas d'echec"
```

---

## Task 9: nft `interception` chain

**Files:**
- Modify: `crates/infra/src/nftables/translator/system_rules.rs` (or wherever the boot ruleset is built)
- Possibly: `crates/infra/src/nftables/adapter/mod.rs` (to install the new chain at startup)

- [ ] **Step 9.1: Inspect existing system_rules.rs**

```bash
cat crates/infra/src/nftables/translator/system_rules.rs
```

Identify where the whitelist rules (DNS/DHCP/NTP/loopback) are emitted at boot. The new `interception` chain follows the same pattern.

- [ ] **Step 9.2: Append the interception chain**

After the existing whitelist rules, add:

```rust
/// Build the interception chain that forwards new outbound flows to NFQUEUE.
/// Construit la chaîne d'interception qui transfère les nouveaux flux sortants vers NFQUEUE.
pub fn build_interception_chain(queue_num: u16) -> Vec<String> {
    vec![
        // Bypass loopback to avoid IPC deadlock.
        // Bypass loopback pour éviter les deadlocks IPC.
        format!("add chain inet syswall interception {{ type filter hook output priority 0 \\; policy accept \\; }}"),
        format!("add rule inet syswall interception iif lo accept"),
        // System whitelist (DNS/DHCP/NTP) is added on a separate, higher-priority chain
        // already present from sub-project A. Those rules' `accept` short-circuits before this queue.
        // La whitelist système (DNS/DHCP/NTP) est sur une chaîne séparée à priorité plus haute,
        // déjà présente depuis le sous-projet A. Le `accept` de ces règles court-circuite avant la queue.
        format!("add rule inet syswall interception ct state new queue num {} bypass", queue_num),
    ]
}
```

This function is called from the `NftablesAdapter` initialization flow. Find the boot-time rule installation in `crates/infra/src/nftables/adapter/` (likely in `apply.rs` or `mod.rs`) and call `build_interception_chain(nfq_cfg.queue_num)` to emit the rules.

The actual nft command syntax may need adjustment based on the existing patterns in the codebase. **Strongly prefer adapting to the existing pattern** rather than introducing new syntax.

- [ ] **Step 9.3: Verify**

```bash
cargo check -p syswall-infra 2>&1 | tail
cargo test -p syswall-infra 2>&1 | tail
```

Expected: 0 errors, tests pass (no new tests for this — manual integration test required).

If the `bypass` flag isn't recognized by nftables in the test environment, document this as a runtime warning. Modern nftables (≥ 1.0) supports `bypass`.

- [ ] **Step 9.4: Commit**

```bash
git add crates/infra/src/nftables/
git commit -m "feat(infra/nftables): chaine interception avec queue NFQUEUE et bypass loopback"
```

---

## Task 10: Audit events for boot success/fail and timeouts

**Files:**
- Modify: `crates/infra/src/nfqueue/interceptor.rs` (boot-success log → audit event)
- Modify: `crates/app/src/services/learning_service/mod.rs` (timeout audit, queue overflow audit)

- [ ] **Step 10.1: Inject audit repository into NfqueueInterceptor (or log via tracing only)**

The cleanest path: keep the interceptor pure (no audit dep); emit `tracing` events only. The bootstrap wraps the interceptor in a small layer that listens to a status-channel and writes audit events.

Simpler alternative: in the bootstrap's spawn block, on `Err`, call `audit_repo.append(&AuditEvent::new(Severity::Error, EventCategory::System, format!("nfqueue: boot failed: {e}"))).await`.

Pick whichever fits the existing patterns. Do NOT introduce new traits unless necessary.

- [ ] **Step 10.2: Add timeout audit in `wait_for_verdict`**

In `LearningService::wait_for_verdict`, on timeout, write:

```rust
let event = AuditEvent::new(
    Severity::Warning,
    EventCategory::Decision,
    "decision timeout: kernel will drop packet",
)
.with_metadata("decision_id", id.to_string());
let _ = self.audit_repo.append(&event).await;
```

This requires `LearningService` to have an `audit_repo: Arc<dyn AuditRepository>` field. Add it if missing — update `new()` signature and call sites in `bootstrap.rs`.

- [ ] **Step 10.3: Verify + commit**

```bash
cargo test -p syswall-app 2>&1 | tail
git add crates/app/src/services/ crates/daemon/src/bootstrap.rs
git commit -m "feat(app): audit events sur timeout de decision et statut NFQUEUE au boot"
```

---

## Task 11: Smoke test gated by env var

**Files:**
- Create: `crates/daemon/tests/nfqueue_smoke_test.rs`

- [ ] **Step 11.1: Create the placeholder smoke test**

```rust
//! NFQUEUE smoke test — requires CAP_NET_ADMIN + a real kernel queue.
//! Test fumée NFQUEUE — nécessite CAP_NET_ADMIN + une vraie queue kernel.
//!
//! Activated only when SYSWALL_TEST_NFQUEUE is set; otherwise skipped.
//! Activé uniquement si SYSWALL_TEST_NFQUEUE est défini ; sinon ignoré.

#[tokio::test]
async fn nfqueue_open_and_close() {
    if std::env::var("SYSWALL_TEST_NFQUEUE").is_err() {
        eprintln!("SYSWALL_TEST_NFQUEUE not set, skipping");
        return;
    }
    // The implementer fills this in: open queue, bind, set max_len, close.
    // Verifies basic plumbing without exercising packet flow.
    eprintln!("smoke test placeholder — see crates/daemon/CLAUDE.md for run instructions");
}
```

- [ ] **Step 11.2: Document run instructions**

Append to `crates/daemon/CLAUDE.md` (create if absent):

```markdown
## Smoke test NFQUEUE

```bash
sudo SYSWALL_TEST_NFQUEUE=1 cargo test -p syswall-daemon --test nfqueue_smoke_test -- --nocapture
```

Requires `CAP_NET_ADMIN` (or root). The test does not actually queue packets — it
just verifies that the queue can be opened and bound.

Pour exercer le flux complet :
1. Lancer le démon en root : `sudo cargo run --bin syswall-daemon`
2. Vérifier `dmesg | grep nfnetlink_queue` pour s'assurer que le module est chargé.
3. `sudo nft list ruleset | grep queue` doit montrer la règle d'interception.
```

- [ ] **Step 11.3: Commit**

```bash
git add crates/daemon/tests/nfqueue_smoke_test.rs crates/daemon/CLAUDE.md
git commit -m "test(daemon): smoke test NFQUEUE gated par SYSWALL_TEST_NFQUEUE"
```

---

## Task 12: Documentation README + CHANGELOG

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 12.1: README section "Blocage actif (NFQUEUE)"**

In `README.md`, locate the "Securite renforcee (V0.2)" section added in sub-project A. Append a new bullet OR a new section right after:

```markdown
### Blocage actif (NFQUEUE) / Active blocking (V0.2)

- **Premier paquet retenu** : chaque nouveau flux sortant est suspendu côté kernel (`nfnetlink_queue`) jusqu'à ce qu'une règle existante l'autorise/refuse, ou que l'utilisateur réponde au popup. Plus de fenêtre où la première requête passe avant la décision.
- **Déduplication intégrée** : Firefox qui ouvre 200 sockets vers le même `cdn.example.com:443/tcp` ne déclenche qu'**un seul** popup ; tous les paquets suivants attendent la même décision.
- **Fail-open** : si le démon SysWall meurt ou ne consomme plus la queue, le kernel laisse passer (`bypass` dans la règle nft). On ne coupe jamais internet à cause d'un bug du daemon.
- **Mode dégradé** : si NFQUEUE ne peut pas être ouvert (pas de `CAP_NET_ADMIN`, kernel sans `nfnetlink_queue`), le démon démarre en observation-only et journalise un événement `Severity::Error, Category::System` clairement identifiable.
- **Configuration** :
  ```toml
  [nfqueue]
  enabled = true
  queue_num = 0
  max_queued = 1024
  overflow_policy = "block"   # ou "accept"
  ```

EN:

- **First packet held**: every new outbound flow is suspended kernel-side (`nfnetlink_queue`) until either an existing rule provides a verdict or the user resolves the popup. No more leak of the first packet.
- **Built-in deduplication**: Firefox opening 200 sockets to the same `cdn.example.com:443/tcp` triggers **one** popup; all subsequent packets wait on the same decision.
- **Fail-open**: if the SysWall daemon crashes or stops consuming the queue, the kernel lets the packet through (`bypass` flag). Internet is never cut by a daemon bug.
- **Degraded mode**: if NFQUEUE cannot be opened (no `CAP_NET_ADMIN`, kernel without `nfnetlink_queue`), the daemon starts in observation-only and journals a clearly identifiable `Severity::Error, Category::System` event.
- **Configuration**: see TOML block above.
```

- [ ] **Step 12.2: CHANGELOG entry**

In `CHANGELOG.md`, under `## [0.2.0] - 2026-05-05`, append a new subsection (after the existing `### Code Hygiene`):

```markdown
### Active Blocking (NFQUEUE)

- **Nouveau port `PacketInterceptor`** + adapter `NfqueueInterceptor` (`crates/infra/src/nfqueue/`) : intercepte le premier paquet de chaque nouveau flux sortant via `nfnetlink_queue` et synchronise le verdict avec la décision utilisateur.
- **`LearningService::decide`** : nouveau handler qui consulte `PolicyEngine`, gère la création de `PendingDecision`, et attend (≤ 28 s) le verdict via `VerdictBroadcasts`.
- **Déduplication baked-in** : un seul popup par `(app, remote_ip, remote_port, protocol)` même sous burst ; les paquets suivants s'abonnent au broadcast existant.
- **Règle nft `interception`** : `iif lo accept` puis `ct state new queue num 0 bypass` ajoutée au boot.
- **Mode dégradé** : daemon démarre même si NFQUEUE échoue ; observation-only avec audit `Severity::Error, Category::System`.
- **Config `[nfqueue]`** : `enabled`, `queue_num`, `max_queued`, `overflow_policy`.
- **Limite documentée** : timeout 28 s par décision (kernel jette à 30 s) ; audit `Severity::Warning, Category::Decision` sur expiration.

Dépendances Cargo ajoutées : `nfq = "0.4"`, `etherparse = "0.16"` (workspace, dans `crates/infra/Cargo.toml`).
```

- [ ] **Step 12.3: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: documentation NFQUEUE block-while-pending v0.2 (FR+EN)"
```

---

## Final verification

```bash
echo "=== workspace clippy ==="
cargo clippy --workspace --exclude ui --all-targets -- -D warnings 2>&1 | tail
echo "=== workspace tests ==="
cargo test --workspace --exclude ui 2>&1 | grep result | tail
echo "=== hardening check ==="
./system/tests/check-hardening.sh
```

Expected:
- 0 clippy warnings.
- 308+ tests pass (existing) + at least 7-10 new (verdict broadcast + handler + parser).
- Hardening still OK.

If a test fails, fix it in the appropriate task. Don't bundle unrelated work.

---

## Self-Review

**Spec coverage:**
- New port `PacketInterceptor` + handler → Task 1.
- Fakes → Task 2.
- `VerdictBroadcasts` debounce machinery → Task 3.
- `LearningService` handler with 7 scenarios → Task 4.
- NfqueueInterceptor + parsing → Tasks 5, 6.
- Config → Task 7.
- Bootstrap with degraded mode → Task 8.
- nft chain → Task 9.
- Audit events → Task 10.
- Smoke test → Task 11.
- Docs → Task 12.

All 12 spec sections covered.

**Placeholder scan:** Each step has either complete code, a clear delegation to existing patterns ("inspect file X for the actual API"), or an explicit assertion. The `Connection::new_outbound` placeholder is acknowledged as needing implementer adaptation since the real type can't be assumed without reading.

**Type consistency:** `PacketInterceptor` / `PacketDecisionHandler` / `PacketVerdict` consistent throughout. `VerdictBroadcasts` API is `subscribe`/`publish_and_remove` consistently.

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-05-nfqueue-block-while-pending-plan.md`.**

12 tasks, ~12-15 commits. **Subagent-Driven recommended** — most tasks are mechanical with the spec providing context.
