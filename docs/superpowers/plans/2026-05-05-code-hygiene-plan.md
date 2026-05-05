# Code Hygiene Implementation Plan — SysWall sub-project B

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eradicate the 486 production-path `unwrap()` calls, split 6 god-modules (>600 LOC), bump the workspace version to 0.2.0, fix 24 clippy warnings in `infra`, and lock the codebase against regressions via a strict workspace clippy CI gate — all without changing observable behavior.

**Architecture:** Per-crate gradual hardening. For each crate, classify every `unwrap()` as **infallible** (logic invariant, compile-time constant, locked single-thread state) or **fallible** (IO, parsing user input, recoverable error path). Infallible → `expect("invariant en français")`. Fallible → propagate via `?`, possibly extending `DomainError`. After cleanup, lock the crate with `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]`. God-modules are split by responsibility, never by line count, with re-exports preserving the public API.

**Tech Stack:** Rust 2024, cargo workspace, clippy, rustfmt.

**Spec source:** `docs/superpowers/specs/2026-05-05-code-hygiene-design.md`
**Originating audit:** `docs/audit-2026-05-04.md`

---

## Current state (verified 2026-05-05)

| Crate | unwrap (prod) | expect (prod) | god-files (>600 LOC) |
|---|---|---|---|
| `domain` | 31 | 0 | `services/policy_engine.rs` (639) |
| `app` | 126 | 0 | `services/audit_service.rs` (628) |
| `daemon` | 35 | 3 | `grpc/converters.rs` (789) |
| `infra` | 288 | 0 | `nftables/adapter.rs` (785), `persistence/audit_repository.rs` (634), `nftables/translator.rs` (611) |
| `ebpf` | 6 | 0 | — |
| **Total** | **486** | **3** | **6 fichiers** |

CHANGELOG: 0.2.0 section already exists from sub-project A — needs date stamp at end of B.
Cargo.toml workspace: `version = "0.1.0"` (mismatch with packaged 0.2.0).
Cargo dep `infra → app`: present in `crates/infra/Cargo.toml`, no source-level usage.
CI: `cargo clippy` not gated with `-D warnings`.

---

## Conventions for every task in this plan

- Comments and commit messages in **French**.
- Code identifiers in English.
- **NEVER add `Co-Authored-By Claude` lines** in any commit.
- Each commit must compile (`cargo check -p <crate>`) and pass tests (`cargo test -p <crate>`).
- For `expect()` messages: explain **why** the value is guaranteed, not what it is. Bilingual EN+FR optional but preferred for non-trivial cases.
- After classifying any `unwrap()`, if the answer is "I can't prove it's infallible," it's **fallible** — propagate, don't `expect`.

---

## Task 1: Bump workspace version 0.1.0 → 0.2.0

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock` (auto-regenerated)
- Modify: `CHANGELOG.md`

- [ ] **Step 1.1: Edit workspace Cargo.toml**

In `/home/seb/Dev/SysWall/Cargo.toml`, change the line under `[workspace.package]`:
```toml
version = "0.1.0"
```
to:
```toml
version = "0.2.0"
```

- [ ] **Step 1.2: Verify all crates use workspace version**

Run: `grep -rn 'version' crates/*/Cargo.toml | grep -v workspace`
Expected: no output (or only output from crates that should NOT inherit, e.g., `crates/proto` if it has its own protocol versioning).
If a crate has its own hardcoded version, leave a one-line French comment explaining why and continue.

- [ ] **Step 1.3: Refresh Cargo.lock**

Run: `cargo update -p syswall-domain -p syswall-app -p syswall-infra -p syswall-daemon -p syswall-proto -p syswall-ebpf 2>&1 | tail`
Expected: lock file updated with `0.1.0` → `0.2.0` for the workspace members.

- [ ] **Step 1.4: Date the CHANGELOG**

In `/home/seb/Dev/SysWall/CHANGELOG.md`, locate `## [0.2.0] - 2026-05-XX` and replace `2026-05-XX` with today's date (`2026-05-05`).

- [ ] **Step 1.5: Verify build still works**

Run: `cargo check --workspace 2>&1 | tail`
Expected: 0 errors.

- [ ] **Step 1.6: Commit**

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: bump workspace version 0.1.0 -> 0.2.0 et date du changelog"
```

---

## Task 2: Remove unused `infra → app` Cargo dependency

**Files:**
- Modify: `crates/infra/Cargo.toml`

- [ ] **Step 2.1: Confirm the dependency is unused in source code**

Run: `grep -rn 'syswall_app\|use syswall_app\|extern crate syswall_app' crates/infra/src/`
Expected: no output (the dep is dead).

If output is non-empty, STOP — the dep is actually used. Document the usage and skip Task 2 entirely. Re-run after Task 5 (which may remove the usage).

- [ ] **Step 2.2: Remove the dep**

In `/home/seb/Dev/SysWall/crates/infra/Cargo.toml`, find the `[dependencies]` section and remove the line:
```toml
syswall-app = { path = "../app" }
```
or any similar form referencing `syswall-app`.

- [ ] **Step 2.3: Verify infra still builds**

Run: `cargo check -p syswall-infra 2>&1 | tail`
Expected: 0 errors.

- [ ] **Step 2.4: Verify the workspace still builds**

Run: `cargo check --workspace 2>&1 | tail`
Expected: 0 errors. (No reverse dep since `app` doesn't depend on infra in any case.)

- [ ] **Step 2.5: Commit**

```bash
git add crates/infra/Cargo.toml Cargo.lock
git commit -m "chore(infra): retire la dependance Cargo syswall-app non utilisee (rupture hexagonale)"
```

---

## Task 3: Eradicate `unwrap()` in `crates/domain/src/`

**Files:**
- Modify: every `.rs` file in `crates/domain/src/` containing `unwrap()` outside `#[cfg(test)]`.
- Modify: `crates/domain/src/lib.rs` (add `#![cfg_attr(...)]` at top, last step).

- [ ] **Step 3.1: List the production unwraps**

Run: `rg -n 'unwrap\(\)' crates/domain/src/ --type rust -g '!*test*' -g '!*tests*' > /tmp/domain-unwraps.txt && wc -l /tmp/domain-unwraps.txt`
Expected: 31 lines (or close).

- [ ] **Step 3.2: Classify each occurrence and apply the fix**

For each line in `/tmp/domain-unwraps.txt`:
- Open the file at the indicated line.
- Determine: is the value guaranteed by an invariant (compile-time literal, just-checked Some, just-built HashMap key, etc.)?
  - **Yes (infallible):** replace `unwrap()` with `expect("raison technique en francais")`. Examples:
    - `IpAddr::from_str("127.0.0.1").unwrap()` → `expect("litteral IPv4 valide compile-time")`
    - `map.get(&key).unwrap()` (where `key` was just inserted) → `expect("cle inseree ligne X plus haut")`
    - `Mutex::lock().unwrap()` → `expect("Mutex jamais empoisonne: pas de panic dans la section critique")`
  - **No (fallible):** propagate with `?`. If the surrounding function does not return `Result`, change its signature to `Result<_, DomainError>` and update callers up the chain. Use the appropriate `DomainError` variant (`Validation`, `NotFound`, `Infrastructure`, `NotPermitted`).

If you must extend `DomainError` to express a new failure mode, add the variant in `crates/domain/src/errors/mod.rs` with a French `#[error("...")]` message.

- [ ] **Step 3.3: Verify tests still pass**

Run: `cargo test -p syswall-domain 2>&1 | tail`
Expected: same number of tests passing as before (≥ 60).

- [ ] **Step 3.4: Activate the lint**

In `/home/seb/Dev/SysWall/crates/domain/src/lib.rs`, add at the very top (before any `pub mod`):

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
```

- [ ] **Step 3.5: Run clippy with the new lint**

Run: `cargo clippy -p syswall-domain --all-targets -- -D warnings 2>&1 | tail`
Expected: 0 warnings.

If you missed an unwrap, clippy will catch it now — fix it before committing.

- [ ] **Step 3.6: Commit**

```bash
git add crates/domain/src/
git commit -m "refactor(domain): eradique les unwrap() en prod et active deny(clippy::unwrap_used)"
```

---

## Task 4: Split `crates/domain/src/services/policy_engine.rs`

**Files:**
- Move: `crates/domain/src/services/policy_engine.rs` → `crates/domain/src/services/policy_engine/`
- Create: `crates/domain/src/services/policy_engine/mod.rs`
- Create: `crates/domain/src/services/policy_engine/matcher.rs`
- Create: `crates/domain/src/services/policy_engine/evaluator.rs`

- [ ] **Step 4.1: Inspect responsibilities**

Run: `grep -nE '^(pub )?(async )?fn |^impl |^struct |^enum ' crates/domain/src/services/policy_engine.rs`
This lists every type/function. Classify each into:
- **matcher** — pure boolean logic that decides if a `RuleCriteria` matches a given event/context (no side effects, no priority resolution).
- **evaluator** — orchestration: iterate rules, apply matcher, resolve priority, fall back to default policy, return decision.
- **mod** (orchestration root) — re-exports + the high-level `PolicyEngine` struct + `new` constructor + tests.

- [ ] **Step 4.2: Create the new directory**

Run: `mkdir -p crates/domain/src/services/policy_engine`

- [ ] **Step 4.3: Move the original to `mod.rs`**

Run: `git mv crates/domain/src/services/policy_engine.rs crates/domain/src/services/policy_engine/mod.rs`

- [ ] **Step 4.4: Extract matcher.rs**

Cut the matcher functions/methods from `mod.rs` and paste them into a new file `crates/domain/src/services/policy_engine/matcher.rs`. Add at the top of the new file:

```rust
//! Matching logic: evaluate whether a rule's criteria match a given event/context.
//! Logique de matching : déterminer si les critères d'une règle correspondent à un évènement/contexte.

use crate::entities::{Connection, Rule};
// Add imports as needed based on what was moved.
```

In `mod.rs`, add at the top:
```rust
mod matcher;
pub use matcher::*;
```

- [ ] **Step 4.5: Extract evaluator.rs**

Cut the evaluator functions (priority resolution, default policy fallback) from `mod.rs` and paste into `crates/domain/src/services/policy_engine/evaluator.rs`.

```rust
//! Evaluation logic: resolve which rule applies given matched candidates and priority.
//! Logique d'évaluation : résoudre quelle règle s'applique parmi les candidates et la priorité.

use crate::entities::Rule;
use crate::events::DefaultPolicy;
// Add imports as needed.
```

Add to `mod.rs`:
```rust
mod evaluator;
pub use evaluator::*;
```

- [ ] **Step 4.6: Move tests near their target**

If a test in the existing `tests` mod is testing only matcher logic, move it to `matcher.rs` `#[cfg(test)] mod tests`. Same for evaluator tests. Tests that exercise the whole engine stay in `mod.rs`.

- [ ] **Step 4.7: Verify line counts**

Run: `wc -l crates/domain/src/services/policy_engine/*.rs`
Expected: each file ≤ 400 LOC. If `mod.rs` is still > 400, extract more responsibilities.

- [ ] **Step 4.8: Verify tests + clippy**

```bash
cargo test -p syswall-domain --lib services::policy_engine 2>&1 | tail
cargo clippy -p syswall-domain --all-targets -- -D warnings 2>&1 | tail
```
Expected: same number of tests pass, 0 clippy warnings.

- [ ] **Step 4.9: Commit**

```bash
git add crates/domain/src/services/policy_engine/
git commit -m "refactor(domain): split policy_engine en matcher + evaluator (SRP)"
```

---

## Task 5: Eradicate `unwrap()` in `crates/app/src/`

**Files:**
- Modify: every `.rs` file in `crates/app/src/` containing `unwrap()` outside `#[cfg(test)]` and outside `crates/app/src/fakes/` (fakes use `unwrap` legitimately for test-only assertions).
- Modify: `crates/app/src/lib.rs` (add lint at top, last step).

- [ ] **Step 5.1: List production unwraps (excluding fakes)**

Run: `rg -n 'unwrap\(\)' crates/app/src/ --type rust -g '!*test*' -g '!*tests*' -g '!fakes/*' > /tmp/app-unwraps.txt && wc -l /tmp/app-unwraps.txt`
Expected: ~120 lines.

The `fakes/` directory is excluded because fakes are test-only infrastructure. They're allowed to `unwrap()` on internal state.

- [ ] **Step 5.2: Classify and fix in batches**

Process in batches of ~30 unwraps. After each batch:
```bash
cargo check -p syswall-app 2>&1 | tail
cargo test -p syswall-app 2>&1 | tail | grep result
```
Expected: 0 errors, ≥ 41 tests pass.

Apply the same classification rules as Task 3.

Common patterns in `app`:
- `services/audit_service.rs`: `lock().unwrap()` on `Mutex` → `expect("Mutex audit jamais empoisonne")`.
- `services/connection_service.rs`: `Option::unwrap()` after `is_some()` → either restructure with `if let Some(x) = ...` or `expect("verifie via is_some() ligne X")`.
- `services/learning_service.rs`: `Result::unwrap()` on internal channel sends → propagate via `?` or change to `let _ = ...` with a comment if the failure is genuinely ignorable.

- [ ] **Step 5.3: Activate the lint**

Add at the top of `/home/seb/Dev/SysWall/crates/app/src/lib.rs`:

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
```

But: the `fakes/` module IS production code structurally (compiled as part of `app` lib), even though it's only consumed by tests. To allow `unwrap()` in fakes, gate the deny by module:

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod commands;
#[allow(clippy::unwrap_used, clippy::expect_used)]
pub mod fakes;
pub mod services;
```

If the `pub mod fakes;` declaration already exists without the `#[allow]`, add it.

- [ ] **Step 5.4: Run clippy**

Run: `cargo clippy -p syswall-app --all-targets -- -D warnings 2>&1 | tail -20`
Expected: 0 warnings outside `fakes/`.

- [ ] **Step 5.5: Verify all tests still pass**

Run: `cargo test -p syswall-app 2>&1 | grep result`
Expected: 41+ pass, 0 fail.

- [ ] **Step 5.6: Commit**

```bash
git add crates/app/src/
git commit -m "refactor(app): eradique les unwrap() en prod et active deny(clippy::unwrap_used)"
```

If the eradication touched many files, you may split into multiple commits per service (e.g., one commit per `audit_service.rs`, `connection_service.rs`, etc.). Each commit must compile and pass tests independently.

---

## Task 6: Split `crates/app/src/services/audit_service.rs` (CQRS-light)

**Files:**
- Move: `crates/app/src/services/audit_service.rs` → `crates/app/src/services/audit_service/mod.rs`
- Create: `crates/app/src/services/audit_service/command.rs`
- Create: `crates/app/src/services/audit_service/query.rs`

- [ ] **Step 6.1: Identify the split axis**

Run: `grep -nE '^(pub )?(async )?fn |^impl ' crates/app/src/services/audit_service.rs`

Classify each method:
- **command** — mutates state: `record`, `append`, `flush`, `rotate`, `purge`.
- **query** — reads state: `list`, `find`, `filter`, `stats`, `export`.
- **mod** (root) — `AuditService` struct + `new` + cross-cutting helpers + tests that touch both sides.

- [ ] **Step 6.2: Create the directory and move**

```bash
mkdir -p crates/app/src/services/audit_service
git mv crates/app/src/services/audit_service.rs crates/app/src/services/audit_service/mod.rs
```

- [ ] **Step 6.3: Extract command.rs**

Move command-side methods (and only the impl blocks containing them) into `crates/app/src/services/audit_service/command.rs`:

```rust
//! Audit write side: record, batch flush, rotation, purge.
//! Cote ecriture audit : enregistrement, vidage par lots, rotation, purge.

use super::AuditService;
use syswall_domain::entities::AuditEvent;
use syswall_domain::errors::DomainError;
// Imports as needed.

impl AuditService {
    // Command methods relocated here.
}
```

Add to `mod.rs`:
```rust
mod command;
```

- [ ] **Step 6.4: Extract query.rs**

Move query-side methods into `crates/app/src/services/audit_service/query.rs` with the same impl-block pattern:

```rust
//! Audit read side: filtering, stats, export.
//! Cote lecture audit : filtrage, statistiques, export.

use super::AuditService;
// Imports as needed.

impl AuditService {
    // Query methods relocated here.
}
```

Add to `mod.rs`:
```rust
mod query;
```

- [ ] **Step 6.5: Verify line counts and tests**

```bash
wc -l crates/app/src/services/audit_service/*.rs
cargo test -p syswall-app --lib services::audit_service 2>&1 | grep result
cargo clippy -p syswall-app --all-targets -- -D warnings 2>&1 | tail
```
Expected: each file ≤ 400 LOC, all existing tests pass, 0 clippy warnings.

- [ ] **Step 6.6: Commit**

```bash
git add crates/app/src/services/audit_service/
git commit -m "refactor(app): split audit_service en command + query (CQRS-light)"
```

---

## Task 7: Eradicate `unwrap()` in `crates/daemon/src/`

**Files:**
- Modify: every `.rs` file in `crates/daemon/src/` containing `unwrap()` outside `#[cfg(test)]`.
- Modify: `crates/daemon/src/main.rs` (add lint at top).

- [ ] **Step 7.1: List production unwraps**

Run: `rg -n 'unwrap\(\)' crates/daemon/src/ --type rust -g '!*test*' -g '!*tests*' > /tmp/daemon-unwraps.txt && wc -l /tmp/daemon-unwraps.txt`
Expected: ~35.

- [ ] **Step 7.2: Classify and fix**

Common patterns in daemon:
- `signals.rs`: `tokio::signal::unix::signal(...).unwrap()` at boot → propagate via `?` to `StartupError::InfrastructureInit`.
- `grpc/server.rs`: `socket_path.parent().unwrap()` after path is known absolute → `expect("socket_path est absolu, parent garanti")`.
- `bootstrap.rs`: `config.rollback_timeout_secs.try_into().unwrap()` (u64 → u32) → if range-bounded use `expect("rollback_timeout_secs <= u32::MAX par construction config")`, else propagate.

For unwraps that occur during `tokio::spawn`ed tasks where the failure cannot bubble up, log the error with `tracing::error!` and gracefully exit the task — do NOT panic.

- [ ] **Step 7.3: Activate the lint**

In `/home/seb/Dev/SysWall/crates/daemon/src/main.rs`, add at the very top (before `mod` declarations):

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
```

- [ ] **Step 7.4: Clippy**

Run: `cargo clippy -p syswall-daemon --all-targets -- -D warnings 2>&1 | tail -20`
Expected: 0 warnings (in daemon's own code; infra-transitive warnings are addressed in Task 9).

If clippy still reports infra-transitive warnings, that's expected — they'll go away after Task 9. For now, scope your check: `cargo clippy -p syswall-daemon --no-deps -- -D warnings`.

- [ ] **Step 7.5: Verify**

Run: `cargo test -p syswall-daemon 2>&1 | grep result`
Expected: 40+ pass, 0 fail.

- [ ] **Step 7.6: Commit**

```bash
git add crates/daemon/src/
git commit -m "refactor(daemon): eradique les unwrap() en prod et active deny(clippy::unwrap_used)"
```

---

## Task 8: Split `crates/daemon/src/grpc/converters.rs` by entity

**Files:**
- Move: `crates/daemon/src/grpc/converters.rs` → `crates/daemon/src/grpc/converters/mod.rs`
- Create: `crates/daemon/src/grpc/converters/rule.rs`
- Create: `crates/daemon/src/grpc/converters/decision.rs`
- Create: `crates/daemon/src/grpc/converters/audit.rs`
- Create: `crates/daemon/src/grpc/converters/connection.rs`
- Create: `crates/daemon/src/grpc/converters/error.rs`

- [ ] **Step 8.1: Inventory the converters**

Run: `grep -nE '^(pub )?fn ' crates/daemon/src/grpc/converters.rs | head -30`

Group functions by domain entity. Typical groups:
- **rule** — `proto_to_rule`, `rule_to_proto`, `rule_action_to_proto`, etc.
- **decision** — `proto_to_decision`, `decision_action_to_proto`, etc.
- **audit** — `audit_event_to_proto`, `parse_event_category`, `parse_severity`, etc.
- **connection** — `connection_to_proto`, etc.
- **error** — `domain_error_to_status` and any other error mapping.

- [ ] **Step 8.2: Create the directory**

```bash
mkdir -p crates/daemon/src/grpc/converters
git mv crates/daemon/src/grpc/converters.rs crates/daemon/src/grpc/converters/mod.rs
```

- [ ] **Step 8.3: Extract each entity into its own file**

For each group, create a sub-file. Example for `rule.rs`:

```rust
//! Conversions entre Rule (domain) et son equivalent proto.
//! Conversions between Rule (domain) and its proto equivalent.

use syswall_domain::entities::{Rule, RuleAction, RuleCriteria, RuleId};
use syswall_proto::syswall as pb;

pub fn rule_to_proto(rule: &Rule) -> pb::Rule {
    // Body relocated.
}

pub fn proto_to_rule(p: &pb::Rule) -> Result<Rule, syswall_domain::errors::DomainError> {
    // Body relocated.
}

// Plus any private helpers used only by these conversions.

#[cfg(test)]
mod tests {
    // Tests relocated from the original tests module that exercise rule conversions.
}
```

Repeat for `decision.rs`, `audit.rs`, `connection.rs`, `error.rs`.

In `mod.rs`, after extraction, declare and re-export everything that was previously `pub`:

```rust
mod rule;
mod decision;
mod audit;
mod connection;
mod error;

pub use rule::*;
pub use decision::*;
pub use audit::*;
pub use connection::*;
pub use error::*;
```

- [ ] **Step 8.4: Verify**

```bash
wc -l crates/daemon/src/grpc/converters/*.rs
cargo test -p syswall-daemon 2>&1 | grep result
cargo clippy -p syswall-daemon --no-deps -- -D warnings 2>&1 | tail
```
Expected: each sub-file < 300 LOC, all tests pass, 0 clippy warnings.

- [ ] **Step 8.5: Commit**

```bash
git add crates/daemon/src/grpc/converters/
git commit -m "refactor(daemon): split converters.rs en sous-modules par entite (rule, decision, audit, connection, error)"
```

---

## Task 9: Fix the 24 clippy warnings in `infra`

**Files:**
- Modify: various files in `crates/infra/src/` flagged by clippy.

- [ ] **Step 9.1: List the warnings**

Run: `cargo clippy -p syswall-infra --all-targets 2>&1 | grep -E '^(warning|error)' | head -40`

Categorize:
- **collapsible_if** → fuse `if x { if y { ... } }` to `if x && y { ... }`.
- **needs_default** → add `impl Default for Type { fn default() -> Self { Self::new() } }`.
- **redundant_closure** → replace `|x| f(x)` with `f`.
- **needs_is_empty** → add `pub fn is_empty(&self) -> bool { self.len() == 0 }` to types with `len()`.
- **unused_import** → remove the import line.
- **unnecessary_to_string** → replace `s.to_string()` with `s.into()` or remove if `&str` works.

- [ ] **Step 9.2: Fix in groups**

Apply each fix in the file flagged. After each file:
```bash
cargo check -p syswall-infra 2>&1 | tail
```
Expected: 0 errors.

- [ ] **Step 9.3: Verify all warnings cleared**

Run: `cargo clippy -p syswall-infra --all-targets -- -D warnings 2>&1 | tail`
Expected: 0 warnings, 0 errors.

- [ ] **Step 9.4: Verify tests still pass**

Run: `cargo test -p syswall-infra 2>&1 | grep result`
Expected: same number of tests as before, 0 fail.

- [ ] **Step 9.5: Commit**

```bash
git add crates/infra/src/
git commit -m "fix(infra): corrige les 24 warnings clippy (collapsible_if, default, is_empty, etc.)"
```

If the changes are large, split into 2-3 commits per category (e.g., one for `collapsible_if`, one for `Default` impls, etc.).

---

## Task 10: Eradicate `unwrap()` in `crates/infra/src/`

**Files:**
- Modify: every `.rs` file in `crates/infra/src/` containing `unwrap()` outside `#[cfg(test)]`.
- Modify: `crates/infra/src/lib.rs` (add lint at top).

This is the biggest task — 288 unwraps. Process in batches by sub-module.

- [ ] **Step 10.1: Group by sub-module**

```bash
for d in conntrack dns event_bus nftables persistence process blocklist connectivity; do
  n=$(rg -n 'unwrap\(\)' crates/infra/src/$d --type rust -g '!*test*' -g '!*tests*' 2>/dev/null | wc -l)
  echo "$d: $n"
done
```

Process each sub-module in its own commit so the diff stays reviewable.

- [ ] **Step 10.2: Process each sub-module**

For each sub-module (e.g., `nftables`), repeat the classify-and-fix loop:
```bash
rg -n 'unwrap\(\)' crates/infra/src/nftables --type rust -g '!*test*' -g '!*tests*' > /tmp/infra-nftables-unwraps.txt
```

Apply classifications:
- **infallible:** `expect("invariant en francais")`. Common cases:
  - `RuleId::from_str(uuid).unwrap()` after a valid UUID was just created → `expect("UUID genere ligne X")`
  - `serde_json::from_str(json).unwrap()` on a self-produced JSON → `expect("JSON serialise par notre own to_string()")`
- **fallible:** propagate with `?`. The `infra` crate already uses `DomainError` extensively — extend variants if needed (rare).

After fixing a sub-module:
```bash
cargo check -p syswall-infra 2>&1 | tail
cargo test -p syswall-infra --lib <sub-module>:: 2>&1 | grep result
git add crates/infra/src/<sub-module>/
git commit -m "refactor(infra/<sub-module>): eradique les unwrap() en prod"
```

- [ ] **Step 10.3: Activate the lint after ALL sub-modules done**

In `/home/seb/Dev/SysWall/crates/infra/src/lib.rs`, add at the very top:

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
```

- [ ] **Step 10.4: Final verify**

```bash
cargo clippy -p syswall-infra --all-targets -- -D warnings 2>&1 | tail
cargo test -p syswall-infra 2>&1 | grep result
```
Expected: 0 warnings, ~150 tests pass (all pre-existing + Task 7 anti-lockout tests).

- [ ] **Step 10.5: Final commit for the lint**

```bash
git add crates/infra/src/lib.rs
git commit -m "refactor(infra): active deny(clippy::unwrap_used, clippy::expect_used) sur le crate"
```

Total commits for Task 10: ~7-10 (one per sub-module + one for the lint).

---

## Task 11: Split `crates/infra/src/persistence/audit_repository.rs`

**Files:**
- Move: `audit_repository.rs` → `audit_repository/mod.rs`
- Create: `audit_repository/queries.rs`
- Create: `audit_repository/writes.rs`
- Create: `audit_repository/migration.rs` (if there's a `CREATE TABLE` block in current code)

- [ ] **Step 11.1: Inventory**

Run: `grep -nE '^(pub )?(async )?fn |^impl ' crates/infra/src/persistence/audit_repository.rs | head -30`

Group:
- **queries** — methods returning data: `list`, `find_by_id`, `filter_by_severity`, `filter_by_date_range`, `count`, `stats`.
- **writes** — methods inserting/updating/deleting: `insert`, `insert_batch`, `delete_older_than`.
- **migration** — `CREATE TABLE` SQL or schema-related code.
- **mod** — `SqliteAuditRepository` struct, `new`, the `impl AuditRepository for SqliteAuditRepository` block (the trait methods just delegate to the sub-modules).

- [ ] **Step 11.2: Create directory and move**

```bash
mkdir -p crates/infra/src/persistence/audit_repository
git mv crates/infra/src/persistence/audit_repository.rs crates/infra/src/persistence/audit_repository/mod.rs
```

- [ ] **Step 11.3: Extract sub-modules**

Pattern (for `queries.rs`):
```rust
//! Audit query side: filtering, stats, retrieval.
//! Cote requete audit : filtrage, statistiques, lecture.

use super::SqliteAuditRepository;
use syswall_domain::entities::{AuditEvent, AuditStats, EventCategory, Severity};
use syswall_domain::errors::DomainError;

impl SqliteAuditRepository {
    pub(super) async fn run_query_list(&self, ...) -> Result<Vec<AuditEvent>, DomainError> {
        // Body relocated.
    }
    // Other queries.
}
```

The trait `impl AuditRepository for SqliteAuditRepository` block in `mod.rs` then calls these `pub(super)` methods. This way the public API is unchanged.

Repeat for `writes.rs` and `migration.rs`.

In `mod.rs`:
```rust
mod queries;
mod writes;
mod migration;
```

- [ ] **Step 11.4: Verify**

```bash
wc -l crates/infra/src/persistence/audit_repository/*.rs
cargo test -p syswall-infra --lib persistence::audit_repository 2>&1 | grep result
cargo clippy -p syswall-infra --all-targets -- -D warnings 2>&1 | tail
```
Expected: each file ≤ 350 LOC, tests pass, 0 clippy warnings.

- [ ] **Step 11.5: Commit**

```bash
git add crates/infra/src/persistence/audit_repository/
git commit -m "refactor(infra): split audit_repository en queries + writes + migration"
```

---

## Task 12: Split `crates/infra/src/nftables/translator.rs` and `adapter.rs`

**Files:**
- Move: `translator.rs` → `translator/mod.rs`
- Create: `translator/criteria.rs`
- Create: `translator/action.rs`
- Create: `translator/system_rules.rs`
- Possibly: split `adapter.rs` (785 LOC) similarly.

- [ ] **Step 12.1: Translator inventory**

Run: `grep -nE '^(pub )?fn |^impl ' crates/infra/src/nftables/translator.rs`

Groups:
- **criteria** — translates `RuleCriteria` fields (proto, ports, IPs, app, user, schedule) to nft expressions.
- **action** — translates `RuleAction` to nft verdict (`accept`, `drop`, `reject`).
- **system_rules** — pre-built whitelist rules (DNS, DHCP, NTP, loopback) emitted on startup or rule sync.
- **mod** — orchestration: `translate_rule(&Rule) -> Vec<NftExpression>`, `translate_ruleset(&[Rule]) -> NftBuffer`, tests.

- [ ] **Step 12.2: Create directory and move**

```bash
mkdir -p crates/infra/src/nftables/translator
git mv crates/infra/src/nftables/translator.rs crates/infra/src/nftables/translator/mod.rs
```

- [ ] **Step 12.3: Extract sub-modules**

```rust
//! Translates RuleCriteria to nftables match expressions.
//! Traduit les RuleCriteria en expressions de match nftables.

use syswall_domain::entities::RuleCriteria;
// ...
```

Repeat for `action.rs` and `system_rules.rs`.

- [ ] **Step 12.4: Adapter inventory**

```bash
wc -l crates/infra/src/nftables/adapter.rs
grep -nE '^(pub )?(async )?fn |^impl ' crates/infra/src/nftables/adapter.rs | head -20
```

If `adapter.rs` is still > 600 LOC, identify split candidates:
- **apply** — `apply_rule`, `sync_all_rules`, `apply_ruleset_static`.
- **rollback** — `save_rollback_state`, `rollback`, `perform_rollback_static` (added in sub-project A).
- **whitelist** — `is_whitelist_only`, `is_whitelist_rule`, `is_loopback_cidr` (helpers added in sub-project A).
- **mod** — struct definition, `new`, `with_lockout_guard`, `impl FirewallEngine for NftablesAdapter`.

If splitting:
```bash
mkdir -p crates/infra/src/nftables/adapter
git mv crates/infra/src/nftables/adapter.rs crates/infra/src/nftables/adapter/mod.rs
# Then extract apply.rs, rollback.rs, whitelist.rs as Step 12.3 pattern.
```

If under 600 after the translator split (some logic might have been over-counted), leave `adapter.rs` as-is.

- [ ] **Step 12.5: Verify**

```bash
wc -l crates/infra/src/nftables/translator/*.rs
ls crates/infra/src/nftables/adapter* 2>/dev/null
cargo test -p syswall-infra --lib nftables 2>&1 | grep result
cargo clippy -p syswall-infra --all-targets -- -D warnings 2>&1 | tail
```
Expected: each file ≤ 400 LOC, all tests pass.

- [ ] **Step 12.6: Commit**

```bash
git add crates/infra/src/nftables/
git commit -m "refactor(infra): split translator (criteria + action + system_rules) et adapter si > 600 LOC"
```

If you split both `translator` and `adapter`, do two separate commits with their respective scopes.

---

## Task 13: Eradicate `unwrap()` in `crates/ebpf/src/`

**Files:**
- Modify: `crates/ebpf/src/lib.rs` (the only source file with prod unwraps).

- [ ] **Step 13.1: List**

Run: `rg -n 'unwrap\(\)' crates/ebpf/src/ --type rust -g '!*test*' -g '!*tests*'`
Expected: ~6 lines.

- [ ] **Step 13.2: Classify and fix**

Common patterns in `ebpf`:
- `program_mut("inet_sock_set_state").unwrap()` after `Ebpf::load`'s map of programs is built → `expect("Programme charge a partir d'un blob compile par notre Cargo.toml — invariant build-time")`. This is genuinely infallible: the program name comes from a compile-time link, not user input.
- `read_unaligned` on a fixed-size struct after the buffer length was just verified → already uses `if event_data.len() < ...` guard, so `expect("buffer >= size_of::<SocketEvent>() verifie ligne X")`.

- [ ] **Step 13.3: Activate the lint**

In `/home/seb/Dev/SysWall/crates/ebpf/src/lib.rs`, add at the top:

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
```

Note: `unsafe` blocks remain — they're justified for kernel ringbuf reads and are not affected by the lint.

- [ ] **Step 13.4: Verify**

```bash
cargo clippy -p syswall-ebpf --all-targets -- -D warnings 2>&1 | tail
cargo test -p syswall-ebpf 2>&1 | grep result
```
Expected: 0 warnings, all tests pass.

- [ ] **Step 13.5: Commit**

```bash
git add crates/ebpf/src/
git commit -m "refactor(ebpf): eradique les unwrap() en prod et active deny(clippy::unwrap_used)"
```

---

## Task 14: Activate `clippy --workspace --all-targets -- -D warnings` in CI

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 14.1: Inspect existing CI**

Run: `cat .github/workflows/ci.yml`

Find the existing clippy job (or create one if absent).

- [ ] **Step 14.2: Replace with strict workspace clippy**

If the file already has a clippy step, replace its command with:

```yaml
- name: cargo clippy (workspace, all targets, deny warnings)
  run: cargo clippy --workspace --all-targets -- -D warnings
```

If absent, add a new job at the end of the `jobs:` map:

```yaml
  clippy:
    name: clippy (workspace, deny warnings)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 14.3: Verify locally first**

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20`
Expected: 0 warnings.

If warnings remain (e.g., from `crates/ui/src-tauri/`), fix them now or scope the workspace check to exclude `syswall-ui` if Tauri-specific lints are noisy and out of scope:

```yaml
- run: cargo clippy --workspace --exclude syswall-ui --all-targets -- -D warnings
```

Document the exclusion in a YAML comment.

- [ ] **Step 14.4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: active cargo clippy --workspace --all-targets -- -D warnings"
```

---

## Task 15: Update CHANGELOG with a "Code Hygiene" section

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 15.1: Append the section**

In `/home/seb/Dev/SysWall/CHANGELOG.md`, locate the `## [0.2.0]` section. Add a new subsection (place it after `### Documentation`, before any `## [older]` entries):

```markdown
### Code Hygiene

- **`unwrap()` en production eradiques** : 486 occurrences remplacees par `?` (propagation) ou `expect("invariant en francais")` documentes. Les crates `domain`, `app`, `daemon`, `infra`, `ebpf` activent maintenant `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]`.
- **God-modules scindes** : `policy_engine`, `audit_service`, `converters`, `audit_repository`, `translator` (et `adapter` si > 600 LOC) sont decomposes en sous-modules par responsabilite (matcher/evaluator, command/query, par entite, queries/writes/migration, criteria/action/system_rules).
- **Version du workspace** : alignee a `0.2.0` (coherent avec les paquets systeme).
- **Dependance Cargo `infra -> app`** : retiree (etait inutilisee, violation hexagonale au niveau Cargo).
- **CI** : `cargo clippy --workspace --all-targets -- -D warnings` est maintenant un gate obligatoire.
- **24 warnings clippy `infra`** : tous corriges (collapsible_if, Default manquant, is_empty manquant, redundant closures, unused imports).
```

- [ ] **Step 15.2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: section Code Hygiene dans le CHANGELOG 0.2.0"
```

---

## Task 16: Final verification

**Files:** none (verification only)

- [ ] **Step 16.1: Workspace clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```
Expected: 0 warnings, 0 errors.

- [ ] **Step 16.2: Workspace tests**

```bash
cargo test --workspace --exclude syswall-ui 2>&1 | tail -10
```
Expected: 308+ tests passed, 0 failed.

- [ ] **Step 16.3: No production unwraps left**

```bash
for c in domain app daemon infra ebpf; do
  n=$(rg -n 'unwrap\(\)' crates/$c/src --type rust -g '!*test*' -g '!*tests*' -g '!fakes/*' 2>/dev/null | wc -l)
  echo "$c: $n unwrap()"
done
```
Expected: all zeros (or only inside `app/fakes/` which has the `#[allow]` exception).

- [ ] **Step 16.4: God-modules below threshold**

```bash
find crates -name '*.rs' -not -path '*/target/*' -exec wc -l {} + | sort -rn | head -10
```
Expected: no `.rs` source file exceeds 500 LOC unless commented as exempt.

- [ ] **Step 16.5: Hardening check still passes**

```bash
./system/tests/check-hardening.sh
```
Expected: `OK: all hardening directives present`.

- [ ] **Step 16.6: Workspace version**

```bash
grep '^version' Cargo.toml
```
Expected: `version = "0.2.0"`.

If everything passes: no commit needed for this task — the success report goes to the user. If anything fails, identify the root cause and fix it (which may require an additional commit in the appropriate crate).

---

## Self-Review

**Spec coverage:**
- B.1 (unwrap eradication) → Tasks 3, 5, 7, 10, 13 (one per crate).
- B.2 (version bump) → Task 1.
- B.3 (god-module split) → Tasks 4, 6, 8, 11, 12.
- B.4 (clippy infra) → Task 9.
- B.5 (remove infra→app dep) → Task 2.
- B.6 (CI clippy gate) → Task 14.
- CHANGELOG → Task 15.
- Final verification → Task 16.

All 6 spec sections covered.

**Placeholder scan:** All steps have specific commands and code samples. No "TBD" / "implement later" / "handle errors" without concrete guidance.

**Type consistency:** No new types introduced — all changes are mechanical (replacement of `unwrap` with `expect`/`?`, file moves with re-exports). Public APIs are preserved via `pub use submodule::*` patterns.

**Risks flagged in spec:**
- "Un `unwrap()` jugé infaillible cache un vrai bug" → addressed by the classification rule "if you can't prove it's infallible, it's fallible".
- "Split de fichier introduit erreurs visibilité" → addressed by `pub use submodule::*` pattern.
- "Suppression `infra → app` casse une feature cachée" → addressed by Step 2.1 grep gate.
- "Bump 0.2.0 conflit Cargo.lock" → addressed by `cargo update` step.
- "deny(unwrap_used) casse les builds aval" → only applies to the declaring crate (Rust language guarantee).

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-05-code-hygiene-plan.md`.**

Two execution options:

**1. Subagent-Driven (recommended)** — fresh subagent per task with two-stage review. Each task has a clear strategy and verification commands. The subagent does the per-line classification work.

**2. Inline Execution** — execute tasks sequentially in the current session.

For sub-project B specifically, I recommend **Subagent-Driven**: the unwrap eradication tasks (3, 5, 7, 10) are mechanical but voluminous, and a fresh subagent per crate will keep context clean.
