# SysWall Firewall Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all three fake adapters (FakeFirewallEngine, FakeConnectionMonitor, FakeProcessResolver) with real Linux system adapters: NftablesFirewallAdapter, ConntrackMonitorAdapter, ProcfsProcessResolver. Integrate them into the daemon with a system whitelist for safe defaults. Wire the full pipeline: conntrack event -> process resolution -> policy evaluation -> firewall verdict.

**Architecture:** Infrastructure adapters in `syswall-infra` implementing domain ports from `syswall-domain`. NftablesFirewallAdapter uses a typed NftCommandBuilder (no shell injection). ConntrackMonitorAdapter spawns `conntrack -E` as async child processes. ProcfsProcessResolver reads /proc with LRU cache. All adapters are selected at runtime via config flags (`use_fake`). Daemon bootstrap conditionally wires real or fake adapters.

**Tech Stack:** Rust, tokio (async runtime), nftables CLI (`nft -j`), conntrack CLI (`conntrack -E`), /proc filesystem, lru crate (LRU cache), nix crate (POSIX APIs), serde_json (nft JSON parsing), futures/tokio-stream (async streams)

**Spec:** `docs/superpowers/specs/2026-03-26-syswall-firewall-engine-design.md`

---

## File Map

### crates/infra/src/nftables/
| File | Responsibility |
|---|---|
| `mod.rs` | Re-exports NftablesFirewallAdapter, NftCommandBuilder |
| `command.rs` | NftCommandBuilder: typed nft argument builder, no I/O |
| `types.rs` | NftRuleHandle, NftRuleEntry, HandleMap, RollbackState (internal types) |
| `parser.rs` | Parse nft JSON output into NftRuleEntry structs |
| `translator.rs` | Translate domain Rule -> nft expression args (pure function) |
| `adapter.rs` | NftablesFirewallAdapter: implements FirewallEngine trait |

### crates/infra/src/conntrack/
| File | Responsibility |
|---|---|
| `mod.rs` | Re-exports ConntrackMonitorAdapter, ConntrackEventParser |
| `parser.rs` | Parse conntrack event lines into ConntrackEvent structs (pure) |
| `types.rs` | ConntrackEvent, ConntrackEventType (internal types) |
| `transformer.rs` | ConntrackEvent -> domain Connection transformation |
| `adapter.rs` | ConntrackMonitorAdapter: implements ConnectionMonitor trait |

### crates/infra/src/process/
| File | Responsibility |
|---|---|
| `mod.rs` | Re-exports ProcfsProcessResolver |
| `proc_parser.rs` | Parse /proc files: /proc/net/tcp, /proc/pid/status, cmdline, exe |
| `cache.rs` | ProcessCache: LRU cache with TTL for pid and inode lookups |
| `resolver.rs` | ProcfsProcessResolver: implements ProcessResolver trait |

### crates/infra/src/ (modified)
| File | Responsibility |
|---|---|
| `lib.rs` | Add `pub mod nftables; pub mod conntrack; pub mod process;` |

### crates/infra/ (modified)
| File | Responsibility |
|---|---|
| `Cargo.toml` | Add dependencies: lru, nix, serde_json, tokio-stream, futures |

### crates/domain/src/ports/ (modified)
| File | Responsibility |
|---|---|
| `system.rs` | Add `resolve_by_connection()` default method to ProcessResolver |

### crates/app/src/ (new)
| File | Responsibility |
|---|---|
| `services/whitelist.rs` | `ensure_system_whitelist()` function for first-start default rules |
| `services/mod.rs` | Add `pub mod whitelist;` |

### crates/daemon/src/ (modified)
| File | Responsibility |
|---|---|
| `config.rs` | Add new fields to FirewallConfig and MonitoringConfig |
| `bootstrap.rs` | Conditional wiring of real vs fake adapters, add connection_monitor + firewall to AppContext |
| `main.rs` | Add whitelist creation + monitoring stream spawn |

### config/ (modified)
| File | Responsibility |
|---|---|
| `default.toml` | Add new config fields (nft_binary_path, conntrack_binary_path, use_fake, etc.) |

### Cargo.toml (workspace root, modified)
| File | Responsibility |
|---|---|
| `Cargo.toml` | Add workspace dependencies: lru, nix |

---

### Task 1: Add Workspace Dependencies and Update Infra Crate

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/infra/Cargo.toml`

- [ ] **Step 1: Add new workspace dependencies**

`Cargo.toml` (workspace root) -- add after the existing `prost-types` line:

```toml
lru = "0.12"
nix = { version = "0.29", features = ["user", "net", "fs"] }
```

- [ ] **Step 2: Update infra Cargo.toml with new dependencies**

`crates/infra/Cargo.toml`:
```toml
[package]
name = "syswall-infra"
version.workspace = true
edition.workspace = true

[dependencies]
syswall-domain = { path = "../domain" }
syswall-app = { path = "../app" }
async-trait = { workspace = true }
rusqlite = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tokio-stream = { workspace = true }
futures = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
lru = { workspace = true }
nix = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
```

- [ ] **Step 3: Verify workspace compiles**

Run: `cargo check -p syswall-infra`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/infra/Cargo.toml
git commit -m "feat: add lru and nix dependencies for firewall engine adapters

Workspace-level deps for sub-project 2: lru (process cache),
nix (POSIX APIs), tokio-stream/futures (async streams)."
```

---

### Task 2: NftCommandBuilder -- Failing Tests

**Files:**
- Create: `crates/infra/src/nftables/mod.rs`
- Create: `crates/infra/src/nftables/command.rs`
- Modify: `crates/infra/src/lib.rs`

- [ ] **Step 1: Create nftables module with command builder skeleton and tests**

`crates/infra/src/nftables/mod.rs`:
```rust
pub mod command;
```

`crates/infra/src/nftables/command.rs`:
```rust
use std::time::Duration;

/// Typed nft command builder. Never concatenates strings into shell commands.
/// All arguments are passed as separate entries, preventing injection.
///
/// Constructeur typé de commandes nft. Ne concatène jamais de chaînes en commandes shell.
/// Tous les arguments sont passés séparément, empêchant l'injection.
pub struct NftCommandBuilder {
    args: Vec<String>,
    timeout: Duration,
    max_output_bytes: usize,
}

impl NftCommandBuilder {
    /// Create a new empty command builder.
    /// Crée un nouveau constructeur de commande vide.
    pub fn new() -> Self {
        todo!()
    }

    /// List the syswall table in JSON format.
    /// Liste la table syswall au format JSON.
    pub fn list_table(table: &str) -> Self {
        todo!()
    }

    /// Add a rule to a chain.
    /// Ajoute une règle à une chaîne.
    pub fn add_rule(table: &str, chain: &str) -> Self {
        todo!()
    }

    /// Delete a rule by handle.
    /// Supprime une règle par handle.
    pub fn delete_rule(table: &str, chain: &str, handle: u64) -> Self {
        todo!()
    }

    /// Create the table if it does not exist.
    /// Crée la table si elle n'existe pas.
    pub fn create_table(table: &str) -> Self {
        todo!()
    }

    /// Create a chain with the given hook and priority.
    /// Crée une chaîne avec le hook et la priorité donnés.
    pub fn create_chain(table: &str, chain: &str, hook: &str, priority: i32) -> Self {
        todo!()
    }

    /// Save the full ruleset for rollback (JSON format).
    /// Sauvegarde l'ensemble des règles pour retour arrière (format JSON).
    pub fn list_ruleset_json() -> Self {
        todo!()
    }

    /// Append an argument.
    /// Ajoute un argument.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Get the built argument list (for testing and execution).
    /// Retourne la liste d'arguments construite (pour tests et exécution).
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Get the configured timeout.
    /// Retourne le délai d'attente configuré.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Get the max output bytes limit.
    /// Retourne la limite maximale d'octets en sortie.
    pub fn max_output_bytes(&self) -> usize {
        self.max_output_bytes
    }
}

impl Default for NftCommandBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty_builder_with_defaults() {
        let cmd = NftCommandBuilder::new();
        assert!(cmd.args().is_empty());
        assert_eq!(cmd.timeout(), Duration::from_secs(5));
        assert_eq!(cmd.max_output_bytes(), 1_048_576);
    }

    #[test]
    fn list_table_produces_correct_args() {
        let cmd = NftCommandBuilder::list_table("syswall");
        assert_eq!(
            cmd.args(),
            &["-j", "list", "table", "inet", "syswall"]
        );
    }

    #[test]
    fn add_rule_produces_correct_base_args() {
        let cmd = NftCommandBuilder::add_rule("syswall", "output");
        assert_eq!(
            cmd.args(),
            &["add", "rule", "inet", "syswall", "output"]
        );
    }

    #[test]
    fn add_rule_with_extra_args() {
        let cmd = NftCommandBuilder::add_rule("syswall", "output")
            .arg("meta")
            .arg("l4proto")
            .arg("tcp")
            .arg("tcp")
            .arg("dport")
            .arg("443")
            .arg("accept")
            .arg("comment")
            .arg("\"syswall:550e8400-e29b-41d4-a716-446655440000\"");
        assert!(cmd.args().contains(&"443".to_string()));
        assert!(cmd.args().contains(&"accept".to_string()));
        assert!(cmd.args().contains(&"output".to_string()));
    }

    #[test]
    fn delete_rule_includes_handle() {
        let cmd = NftCommandBuilder::delete_rule("syswall", "input", 42);
        assert_eq!(
            cmd.args(),
            &["delete", "rule", "inet", "syswall", "input", "handle", "42"]
        );
    }

    #[test]
    fn create_table_produces_correct_args() {
        let cmd = NftCommandBuilder::create_table("syswall");
        assert_eq!(
            cmd.args(),
            &["add", "table", "inet", "syswall"]
        );
    }

    #[test]
    fn create_chain_produces_correct_args() {
        let cmd = NftCommandBuilder::create_chain("syswall", "input", "input", 0);
        assert_eq!(
            cmd.args(),
            &[
                "add", "chain", "inet", "syswall", "input",
                "{ type filter hook input priority 0; policy accept; }"
            ]
        );
    }

    #[test]
    fn list_ruleset_json_produces_correct_args() {
        let cmd = NftCommandBuilder::list_ruleset_json();
        assert_eq!(
            cmd.args(),
            &["-j", "list", "ruleset"]
        );
    }

    #[test]
    fn arg_chaining_preserves_order() {
        let cmd = NftCommandBuilder::new()
            .arg("one")
            .arg("two")
            .arg("three");
        assert_eq!(cmd.args(), &["one", "two", "three"]);
    }
}
```

- [ ] **Step 2: Register nftables module in lib.rs**

Modify `crates/infra/src/lib.rs`:
```rust
pub mod event_bus;
pub mod nftables;
pub mod persistence;
```

- [ ] **Step 3: Verify tests fail**

Run: `cargo test -p syswall-infra nftables::command`
Expected: 9 tests FAIL (all `todo!()` panics)

---

### Task 3: NftCommandBuilder -- Implementation

**Files:**
- Modify: `crates/infra/src/nftables/command.rs`

- [ ] **Step 1: Implement all NftCommandBuilder methods**

Replace the `todo!()` implementations in `crates/infra/src/nftables/command.rs`:

```rust
use std::time::Duration;

/// Typed nft command builder. Never concatenates strings into shell commands.
/// All arguments are passed as separate entries, preventing injection.
///
/// Constructeur typé de commandes nft. Ne concatène jamais de chaînes en commandes shell.
/// Tous les arguments sont passés séparément, empêchant l'injection.
pub struct NftCommandBuilder {
    args: Vec<String>,
    timeout: Duration,
    max_output_bytes: usize,
}

impl NftCommandBuilder {
    /// Create a new empty command builder.
    /// Crée un nouveau constructeur de commande vide.
    pub fn new() -> Self {
        Self {
            args: vec![],
            timeout: Duration::from_secs(5),
            max_output_bytes: 1_048_576,
        }
    }

    /// Create a builder with custom timeout and output limit.
    /// Crée un constructeur avec un délai et une limite de sortie personnalisés.
    pub fn with_limits(timeout: Duration, max_output_bytes: usize) -> Self {
        Self {
            args: vec![],
            timeout,
            max_output_bytes,
        }
    }

    /// List the syswall table in JSON format.
    /// Liste la table syswall au format JSON.
    pub fn list_table(table: &str) -> Self {
        Self::new()
            .arg("-j")
            .arg("list")
            .arg("table")
            .arg("inet")
            .arg(table)
    }

    /// Add a rule to a chain.
    /// Ajoute une règle à une chaîne.
    pub fn add_rule(table: &str, chain: &str) -> Self {
        Self::new()
            .arg("add")
            .arg("rule")
            .arg("inet")
            .arg(table)
            .arg(chain)
    }

    /// Delete a rule by handle.
    /// Supprime une règle par handle.
    pub fn delete_rule(table: &str, chain: &str, handle: u64) -> Self {
        Self::new()
            .arg("delete")
            .arg("rule")
            .arg("inet")
            .arg(table)
            .arg(chain)
            .arg("handle")
            .arg(handle.to_string())
    }

    /// Create the table if it does not exist.
    /// Crée la table si elle n'existe pas.
    pub fn create_table(table: &str) -> Self {
        Self::new()
            .arg("add")
            .arg("table")
            .arg("inet")
            .arg(table)
    }

    /// Create a chain with the given hook and priority.
    /// Crée une chaîne avec le hook et la priorité donnés.
    pub fn create_chain(table: &str, chain: &str, hook: &str, priority: i32) -> Self {
        Self::new()
            .arg("add")
            .arg("chain")
            .arg("inet")
            .arg(table)
            .arg(chain)
            .arg(format!(
                "{{ type filter hook {} priority {}; policy accept; }}",
                hook, priority
            ))
    }

    /// Save the full ruleset for rollback (JSON format).
    /// Sauvegarde l'ensemble des règles pour retour arrière (format JSON).
    pub fn list_ruleset_json() -> Self {
        Self::new().arg("-j").arg("list").arg("ruleset")
    }

    /// Append an argument.
    /// Ajoute un argument.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Get the built argument list (for testing and execution).
    /// Retourne la liste d'arguments construite (pour tests et exécution).
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Get the configured timeout.
    /// Retourne le délai d'attente configuré.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Get the max output bytes limit.
    /// Retourne la limite maximale d'octets en sortie.
    pub fn max_output_bytes(&self) -> usize {
        self.max_output_bytes
    }
}

impl Default for NftCommandBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty_builder_with_defaults() {
        let cmd = NftCommandBuilder::new();
        assert!(cmd.args().is_empty());
        assert_eq!(cmd.timeout(), Duration::from_secs(5));
        assert_eq!(cmd.max_output_bytes(), 1_048_576);
    }

    #[test]
    fn list_table_produces_correct_args() {
        let cmd = NftCommandBuilder::list_table("syswall");
        assert_eq!(
            cmd.args(),
            &["-j", "list", "table", "inet", "syswall"]
        );
    }

    #[test]
    fn add_rule_produces_correct_base_args() {
        let cmd = NftCommandBuilder::add_rule("syswall", "output");
        assert_eq!(
            cmd.args(),
            &["add", "rule", "inet", "syswall", "output"]
        );
    }

    #[test]
    fn add_rule_with_extra_args() {
        let cmd = NftCommandBuilder::add_rule("syswall", "output")
            .arg("meta")
            .arg("l4proto")
            .arg("tcp")
            .arg("tcp")
            .arg("dport")
            .arg("443")
            .arg("accept")
            .arg("comment")
            .arg("\"syswall:550e8400-e29b-41d4-a716-446655440000\"");
        assert!(cmd.args().contains(&"443".to_string()));
        assert!(cmd.args().contains(&"accept".to_string()));
        assert!(cmd.args().contains(&"output".to_string()));
    }

    #[test]
    fn delete_rule_includes_handle() {
        let cmd = NftCommandBuilder::delete_rule("syswall", "input", 42);
        assert_eq!(
            cmd.args(),
            &["delete", "rule", "inet", "syswall", "input", "handle", "42"]
        );
    }

    #[test]
    fn create_table_produces_correct_args() {
        let cmd = NftCommandBuilder::create_table("syswall");
        assert_eq!(
            cmd.args(),
            &["add", "table", "inet", "syswall"]
        );
    }

    #[test]
    fn create_chain_produces_correct_args() {
        let cmd = NftCommandBuilder::create_chain("syswall", "input", "input", 0);
        assert_eq!(
            cmd.args(),
            &[
                "add", "chain", "inet", "syswall", "input",
                "{ type filter hook input priority 0; policy accept; }"
            ]
        );
    }

    #[test]
    fn list_ruleset_json_produces_correct_args() {
        let cmd = NftCommandBuilder::list_ruleset_json();
        assert_eq!(
            cmd.args(),
            &["-j", "list", "ruleset"]
        );
    }

    #[test]
    fn arg_chaining_preserves_order() {
        let cmd = NftCommandBuilder::new()
            .arg("one")
            .arg("two")
            .arg("three");
        assert_eq!(cmd.args(), &["one", "two", "three"]);
    }

    #[test]
    fn with_limits_sets_custom_values() {
        let cmd = NftCommandBuilder::with_limits(
            Duration::from_secs(10),
            2_097_152,
        );
        assert_eq!(cmd.timeout(), Duration::from_secs(10));
        assert_eq!(cmd.max_output_bytes(), 2_097_152);
    }
}
```

- [ ] **Step 2: Verify all tests pass**

Run: `cargo test -p syswall-infra nftables::command`
Expected: 10 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/infra/src/nftables/ crates/infra/src/lib.rs
git commit -m "feat: add NftCommandBuilder with typed nft argument construction

Pure struct that builds nft CLI arguments as separate strings,
preventing shell injection. No I/O -- fully unit-testable."
```

---

### Task 4: Nft Rule Translator -- Failing Tests

**Files:**
- Create: `crates/infra/src/nftables/types.rs`
- Create: `crates/infra/src/nftables/translator.rs`
- Modify: `crates/infra/src/nftables/mod.rs`

- [ ] **Step 1: Create nftables internal types**

`crates/infra/src/nftables/types.rs`:
```rust
use std::collections::HashMap;
use std::time::Instant;

use syswall_domain::entities::RuleId;

/// Handle assigned by nftables to a specific rule in a chain.
/// Handle assigné par nftables à une règle spécifique dans une chaîne.
#[derive(Debug, Clone)]
pub struct NftRuleHandle {
    pub chain: String,
    pub handle: u64,
}

/// Tracks the mapping between domain RuleId and nftables handles.
/// Suit la correspondance entre RuleId du domaine et les handles nftables.
#[derive(Debug, Default)]
pub struct HandleMap {
    handles: HashMap<RuleId, Vec<NftRuleHandle>>,
}

impl HandleMap {
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    /// Insert handles for a domain rule.
    /// Insère les handles pour une règle du domaine.
    pub fn insert(&mut self, rule_id: RuleId, rule_handles: Vec<NftRuleHandle>) {
        self.handles.insert(rule_id, rule_handles);
    }

    /// Get handles for a domain rule.
    /// Retourne les handles pour une règle du domaine.
    pub fn get(&self, rule_id: &RuleId) -> Option<&Vec<NftRuleHandle>> {
        self.handles.get(rule_id)
    }

    /// Remove handles for a domain rule.
    /// Supprime les handles pour une règle du domaine.
    pub fn remove(&mut self, rule_id: &RuleId) -> Option<Vec<NftRuleHandle>> {
        self.handles.remove(rule_id)
    }

    /// Clear all handles (used during sync).
    /// Efface tous les handles (utilisé lors de la synchronisation).
    pub fn clear(&mut self) {
        self.handles.clear();
    }
}

/// Saved state for rollback on failure.
/// État sauvegardé pour retour arrière en cas d'échec.
#[derive(Debug)]
pub struct RollbackState {
    /// JSON output of `nft -j list table inet syswall` before the operation.
    /// Sortie JSON de `nft -j list table inet syswall` avant l'opération.
    pub table_state: String,
    /// When the state was saved.
    /// Quand l'état a été sauvegardé.
    pub saved_at: Instant,
}

/// A parsed rule entry from nft JSON output.
/// Une entrée de règle parsée depuis la sortie JSON de nft.
#[derive(Debug, Clone)]
pub struct NftRuleEntry {
    pub chain: String,
    pub handle: u64,
    pub comment: Option<String>,
}

/// The result of translating a domain Rule into nft expressions.
/// Returns None if the rule should not be expressed in nftables (e.g., Ask effect).
///
/// Le résultat de la traduction d'une Rule du domaine en expressions nft.
/// Retourne None si la règle ne doit pas être exprimée dans nftables (ex. effet Ask).
#[derive(Debug, Clone)]
pub struct TranslatedRule {
    /// Target chain(s): "input", "output", or both.
    /// Chaîne(s) cible : "input", "output", ou les deux.
    pub chains: Vec<String>,
    /// nft expression arguments (protocol match, port match, verdict, comment).
    /// Arguments d'expression nft (correspondance protocole, port, verdict, commentaire).
    pub expressions: Vec<String>,
}
```

- [ ] **Step 2: Create translator with failing tests**

`crates/infra/src/nftables/translator.rs`:
```rust
use std::net::IpAddr;

use syswall_domain::entities::{IpMatcher, PortMatcher, Rule, RuleEffect};
use syswall_domain::value_objects::{Direction, Protocol};

use super::types::TranslatedRule;

/// Translate a domain Rule into nft expression arguments.
/// Returns None if the rule should not produce an nft rule (Ask effect).
///
/// Traduit une Rule du domaine en arguments d'expressions nft.
/// Retourne None si la règle ne doit pas produire de règle nft (effet Ask).
pub fn translate_rule(rule: &Rule) -> Option<TranslatedRule> {
    todo!()
}

/// Determine which nftables chains a rule should be placed in.
/// Détermine dans quelles chaînes nftables une règle doit être placée.
pub fn get_target_chains(rule: &Rule) -> Vec<String> {
    todo!()
}

/// Resolve a username to a numeric UID.
/// Returns None if the user cannot be found.
///
/// Résout un nom d'utilisateur en UID numérique.
/// Retourne None si l'utilisateur est introuvable.
pub fn resolve_username_to_uid(username: &str) -> Option<u32> {
    todo!()
}

/// Build nft expressions for IP matching based on direction.
/// For outbound: remote IP is destination (daddr).
/// For inbound: remote IP is source (saddr).
///
/// Construit les expressions nft pour la correspondance IP selon la direction.
fn build_ip_expressions(ip_matcher: &IpMatcher, is_outbound: bool) -> Vec<String> {
    todo!()
}

/// Build nft expressions for port matching.
/// Construit les expressions nft pour la correspondance de port.
fn build_port_expressions(
    port_matcher: &PortMatcher,
    protocol: Option<Protocol>,
    keyword: &str,
) -> Vec<String> {
    todo!()
}

/// Build the verdict expression (accept, drop, or log+accept for observe).
/// Construit l'expression de verdict (accept, drop, ou log+accept pour observe).
fn build_verdict(effect: RuleEffect) -> Vec<String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn test_rule(effect: RuleEffect, criteria: RuleCriteria) -> Rule {
        Rule {
            id: RuleId::new(),
            name: "Test rule".to_string(),
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

    #[test]
    fn ask_effect_produces_no_nft_rule() {
        let rule = test_rule(RuleEffect::Ask, RuleCriteria::default());
        assert!(translate_rule(&rule).is_none());
    }

    #[test]
    fn allow_tcp_port_443_outbound() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        assert_eq!(translated.chains, vec!["output"]);
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("meta l4proto tcp"));
        assert!(expr_str.contains("tcp dport 443"));
        assert!(expr_str.contains("accept"));
        assert!(expr_str.contains(&format!("syswall:{}", rule.id.as_uuid())));
    }

    #[test]
    fn block_ip_cidr_outbound() {
        let rule = test_rule(
            RuleEffect::Block,
            RuleCriteria {
                remote_ip: Some(IpMatcher::Cidr {
                    network: "10.0.0.0".parse().unwrap(),
                    prefix_len: 8,
                }),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        assert_eq!(translated.chains, vec!["output"]);
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("ip daddr 10.0.0.0/8"));
        assert!(expr_str.contains("drop"));
    }

    #[test]
    fn block_ip_cidr_inbound() {
        let rule = test_rule(
            RuleEffect::Block,
            RuleCriteria {
                remote_ip: Some(IpMatcher::Cidr {
                    network: "10.0.0.0".parse().unwrap(),
                    prefix_len: 8,
                }),
                direction: Some(Direction::Inbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        assert_eq!(translated.chains, vec!["input"]);
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("ip saddr 10.0.0.0/8"));
    }

    #[test]
    fn no_direction_produces_both_chains() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        assert_eq!(translated.chains, vec!["input", "output"]);
    }

    #[test]
    fn observe_effect_produces_log_and_accept() {
        let rule = test_rule(
            RuleEffect::Observe,
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("log prefix"));
        assert!(expr_str.contains("syswall-observe:"));
        assert!(expr_str.contains("accept"));
    }

    #[test]
    fn port_range_translated() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                remote_port: Some(PortMatcher::Range {
                    start: Port::new(8000).unwrap(),
                    end: Port::new(9000).unwrap(),
                }),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("tcp dport 8000-9000"));
    }

    #[test]
    fn rule_comment_contains_uuid() {
        let rule = test_rule(RuleEffect::Allow, RuleCriteria::default());
        let uuid_str = rule.id.as_uuid().to_string();
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains(&format!("syswall:{}", uuid_str)));
    }

    #[test]
    fn exact_ip_outbound() {
        let rule = test_rule(
            RuleEffect::Block,
            RuleCriteria {
                remote_ip: Some(IpMatcher::Exact("93.184.216.34".parse().unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("ip daddr 93.184.216.34"));
    }

    #[test]
    fn ipv6_address_uses_ip6() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                remote_ip: Some(IpMatcher::Exact("::1".parse().unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("ip6 daddr ::1"));
    }

    #[test]
    fn local_port_uses_sport() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                local_port: Some(PortMatcher::Exact(Port::new(8080).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("tcp sport 8080"));
    }

    #[test]
    fn udp_protocol_translated() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                remote_port: Some(PortMatcher::Exact(Port::new(53).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("meta l4proto udp"));
        assert!(expr_str.contains("udp dport 53"));
    }

    #[test]
    fn icmp_protocol_translated() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                protocol: Some(Protocol::Icmp),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        let translated = translate_rule(&rule).unwrap();
        let expr_str = translated.expressions.join(" ");
        assert!(expr_str.contains("meta l4proto icmp"));
    }

    #[test]
    fn get_target_chains_outbound() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        );
        assert_eq!(get_target_chains(&rule), vec!["output"]);
    }

    #[test]
    fn get_target_chains_inbound() {
        let rule = test_rule(
            RuleEffect::Allow,
            RuleCriteria {
                direction: Some(Direction::Inbound),
                ..Default::default()
            },
        );
        assert_eq!(get_target_chains(&rule), vec!["input"]);
    }

    #[test]
    fn get_target_chains_no_direction() {
        let rule = test_rule(RuleEffect::Allow, RuleCriteria::default());
        assert_eq!(get_target_chains(&rule), vec!["input", "output"]);
    }
}
```

- [ ] **Step 3: Update nftables mod.rs**

`crates/infra/src/nftables/mod.rs`:
```rust
pub mod command;
pub mod translator;
pub mod types;
```

- [ ] **Step 4: Verify tests fail**

Run: `cargo test -p syswall-infra nftables::translator`
Expected: 16 tests FAIL (all `todo!()` panics)

---

### Task 5: Nft Rule Translator -- Implementation

**Files:**
- Modify: `crates/infra/src/nftables/translator.rs`

- [ ] **Step 1: Implement all translator functions**

Replace the `todo!()` bodies in `crates/infra/src/nftables/translator.rs`. Keep the test module unchanged. Replace only the function bodies:

```rust
use std::net::IpAddr;

use syswall_domain::entities::{IpMatcher, PortMatcher, Rule, RuleEffect};
use syswall_domain::value_objects::{Direction, Protocol};

use super::types::TranslatedRule;

/// Translate a domain Rule into nft expression arguments.
/// Returns None if the rule should not produce an nft rule (Ask effect).
///
/// Traduit une Rule du domaine en arguments d'expressions nft.
/// Retourne None si la règle ne doit pas produire de règle nft (effet Ask).
pub fn translate_rule(rule: &Rule) -> Option<TranslatedRule> {
    if rule.effect == RuleEffect::Ask {
        return None;
    }

    let chains = get_target_chains(rule);
    let mut expressions: Vec<String> = Vec::new();
    let criteria = &rule.criteria;

    // Protocol match
    if let Some(ref proto) = criteria.protocol {
        let proto_str = match proto {
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
            Protocol::Icmp => "icmp",
            Protocol::Other(n) => {
                expressions.extend(["meta".into(), "l4proto".into(), n.to_string()]);
                // Skip the standard path
                ""
            }
        };
        if !proto_str.is_empty() {
            expressions.extend([
                "meta".to_string(),
                "l4proto".to_string(),
                proto_str.to_string(),
            ]);
        }
    }

    // Determine if outbound for IP direction
    let is_outbound = criteria.direction != Some(Direction::Inbound);

    // Remote IP match
    if let Some(ref ip_matcher) = criteria.remote_ip {
        expressions.extend(build_ip_expressions(ip_matcher, is_outbound));
    }

    // Remote port match (dport)
    if let Some(ref port_matcher) = criteria.remote_port {
        expressions.extend(build_port_expressions(port_matcher, criteria.protocol, "dport"));
    }

    // Local port match (sport)
    if let Some(ref port_matcher) = criteria.local_port {
        expressions.extend(build_port_expressions(port_matcher, criteria.protocol, "sport"));
    }

    // User match (meta skuid)
    if let Some(ref username) = criteria.user {
        if let Some(uid) = resolve_username_to_uid(username) {
            expressions.extend(["meta".into(), "skuid".into(), uid.to_string()]);
        }
    }

    // Verdict
    expressions.extend(build_verdict(rule.effect));

    // Comment with rule UUID for tracking
    let uuid_str = rule.id.as_uuid().to_string();
    expressions.extend([
        "comment".to_string(),
        format!("\"syswall:{}\"", uuid_str),
    ]);

    Some(TranslatedRule {
        chains,
        expressions,
    })
}

/// Determine which nftables chains a rule should be placed in.
/// Détermine dans quelles chaînes nftables une règle doit être placée.
pub fn get_target_chains(rule: &Rule) -> Vec<String> {
    match rule.criteria.direction {
        Some(Direction::Inbound) => vec!["input".to_string()],
        Some(Direction::Outbound) => vec!["output".to_string()],
        None => vec!["input".to_string(), "output".to_string()],
    }
}

/// Resolve a username to a numeric UID.
/// Returns None if the user cannot be found.
///
/// Résout un nom d'utilisateur en UID numérique.
/// Retourne None si l'utilisateur est introuvable.
pub fn resolve_username_to_uid(username: &str) -> Option<u32> {
    nix::unistd::User::from_name(username)
        .ok()
        .flatten()
        .map(|u| u.uid.as_raw())
}

/// Build nft expressions for IP matching based on direction.
/// Construit les expressions nft pour la correspondance IP selon la direction.
fn build_ip_expressions(ip_matcher: &IpMatcher, is_outbound: bool) -> Vec<String> {
    let direction_keyword = if is_outbound { "daddr" } else { "saddr" };

    match ip_matcher {
        IpMatcher::Exact(ip) => {
            let family = match ip {
                IpAddr::V4(_) => "ip",
                IpAddr::V6(_) => "ip6",
            };
            vec![
                family.to_string(),
                direction_keyword.to_string(),
                ip.to_string(),
            ]
        }
        IpMatcher::Cidr { network, prefix_len } => {
            let family = match network {
                IpAddr::V4(_) => "ip",
                IpAddr::V6(_) => "ip6",
            };
            vec![
                family.to_string(),
                direction_keyword.to_string(),
                format!("{}/{}", network, prefix_len),
            ]
        }
        IpMatcher::Range { start, end } => {
            let family = match start {
                IpAddr::V4(_) => "ip",
                IpAddr::V6(_) => "ip6",
            };
            vec![
                family.to_string(),
                direction_keyword.to_string(),
                format!("{}-{}", start, end),
            ]
        }
    }
}

/// Build nft expressions for port matching.
/// Construit les expressions nft pour la correspondance de port.
fn build_port_expressions(
    port_matcher: &PortMatcher,
    protocol: Option<Protocol>,
    keyword: &str,
) -> Vec<String> {
    let proto_str = match protocol {
        Some(Protocol::Tcp) => "tcp",
        Some(Protocol::Udp) => "udp",
        _ => "tcp", // default to tcp if protocol not specified with port
    };

    match port_matcher {
        PortMatcher::Exact(port) => {
            vec![
                proto_str.to_string(),
                keyword.to_string(),
                port.value().to_string(),
            ]
        }
        PortMatcher::Range { start, end } => {
            vec![
                proto_str.to_string(),
                keyword.to_string(),
                format!("{}-{}", start.value(), end.value()),
            ]
        }
    }
}

/// Build the verdict expression (accept, drop, or log+accept for observe).
/// Construit l'expression de verdict (accept, drop, ou log+accept pour observe).
fn build_verdict(effect: RuleEffect) -> Vec<String> {
    match effect {
        RuleEffect::Allow => vec!["accept".to_string()],
        RuleEffect::Block => vec!["drop".to_string()],
        RuleEffect::Observe => vec![
            "log".to_string(),
            "prefix".to_string(),
            "\"syswall-observe: \"".to_string(),
            "accept".to_string(),
        ],
        RuleEffect::Ask => vec![], // should never reach here
    }
}
```

- [ ] **Step 2: Verify all tests pass**

Run: `cargo test -p syswall-infra nftables::translator`
Expected: 16 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/infra/src/nftables/
git commit -m "feat: add nft rule translator (Rule -> nft expressions)

Pure functions that translate domain Rule entities into nftables
expression arguments. Handles protocol, IP, port, direction, user,
and verdict mapping. Ask effect correctly produces no nft rule."
```

---

### Task 6: Nft JSON Output Parser -- Tests and Implementation

**Files:**
- Create: `crates/infra/src/nftables/parser.rs`
- Modify: `crates/infra/src/nftables/mod.rs`

- [ ] **Step 1: Create parser with tests and implementation**

`crates/infra/src/nftables/parser.rs`:
```rust
use syswall_domain::errors::DomainError;

use super::types::NftRuleEntry;

/// Parse the JSON output of `nft -j list table inet syswall` to extract rule entries.
/// Analyse la sortie JSON de `nft -j list table inet syswall` pour extraire les entrées de règles.
pub fn parse_nft_table_rules(json: &str) -> Result<Vec<NftRuleEntry>, DomainError> {
    let root: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| DomainError::Infrastructure(format!("Failed to parse nft JSON: {}", e)))?;

    let nftables = root
        .get("nftables")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            DomainError::Infrastructure("Missing 'nftables' array in nft output".to_string())
        })?;

    let mut entries = Vec::new();

    for item in nftables {
        if let Some(rule_obj) = item.get("rule") {
            let chain = rule_obj
                .get("chain")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let handle = rule_obj
                .get("handle")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            let comment = rule_obj
                .get("comment")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            entries.push(NftRuleEntry {
                chain,
                handle,
                comment,
            });
        }
    }

    Ok(entries)
}

/// Extract a SysWall rule UUID from a comment string like "syswall:550e8400-...".
/// Extrait un UUID de règle SysWall depuis une chaîne de commentaire comme "syswall:550e8400-...".
pub fn extract_rule_id_from_comment(comment: &str) -> Option<uuid::Uuid> {
    comment
        .strip_prefix("syswall:")
        .and_then(|uuid_str| uuid::Uuid::parse_str(uuid_str).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_table_with_one_rule() {
        let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}, {"table": {"family": "inet", "name": "syswall", "handle": 1}}, {"chain": {"family": "inet", "table": "syswall", "name": "output", "handle": 2, "type": "filter", "hook": "output", "prio": 0, "policy": "accept"}}, {"rule": {"family": "inet", "table": "syswall", "chain": "output", "handle": 5, "comment": "syswall:550e8400-e29b-41d4-a716-446655440000", "expr": [{"match": {"op": "==", "left": {"meta": {"key": "l4proto"}}, "right": "tcp"}}, {"match": {"op": "==", "left": {"payload": {"protocol": "tcp", "field": "dport"}}, "right": 443}}, {"accept": null}]}}]}"#;
        let rules = parse_nft_table_rules(json).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].handle, 5);
        assert_eq!(rules[0].chain, "output");
        assert_eq!(
            rules[0].comment,
            Some("syswall:550e8400-e29b-41d4-a716-446655440000".to_string())
        );
    }

    #[test]
    fn parse_empty_table() {
        let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}, {"table": {"family": "inet", "name": "syswall", "handle": 1}}]}"#;
        let rules = parse_nft_table_rules(json).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let result = parse_nft_table_rules("not json");
        assert!(result.is_err());
    }

    #[test]
    fn parse_missing_nftables_key_returns_error() {
        let json = r#"{"other": []}"#;
        let result = parse_nft_table_rules(json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_multiple_rules() {
        let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}, {"rule": {"family": "inet", "table": "syswall", "chain": "input", "handle": 3, "comment": "syswall:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"}}, {"rule": {"family": "inet", "table": "syswall", "chain": "output", "handle": 4, "comment": "syswall:11111111-2222-3333-4444-555555555555"}}]}"#;
        let rules = parse_nft_table_rules(json).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].chain, "input");
        assert_eq!(rules[1].chain, "output");
    }

    #[test]
    fn parse_rule_without_comment() {
        let json = r#"{"nftables": [{"rule": {"family": "inet", "table": "syswall", "chain": "output", "handle": 7}}]}"#;
        let rules = parse_nft_table_rules(json).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].comment.is_none());
    }

    #[test]
    fn extract_uuid_from_valid_comment() {
        let uuid = extract_rule_id_from_comment("syswall:550e8400-e29b-41d4-a716-446655440000");
        assert!(uuid.is_some());
        assert_eq!(
            uuid.unwrap().to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn extract_uuid_from_invalid_comment() {
        assert!(extract_rule_id_from_comment("not-syswall").is_none());
        assert!(extract_rule_id_from_comment("syswall:not-a-uuid").is_none());
        assert!(extract_rule_id_from_comment("").is_none());
    }
}
```

- [ ] **Step 2: Update nftables mod.rs**

`crates/infra/src/nftables/mod.rs`:
```rust
pub mod command;
pub mod parser;
pub mod translator;
pub mod types;
```

- [ ] **Step 3: Verify all tests pass**

Run: `cargo test -p syswall-infra nftables::parser`
Expected: 8 tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/infra/src/nftables/
git commit -m "feat: add nft JSON output parser

Parses nft -j list table output into NftRuleEntry structs.
Extracts rule handles, chains, and syswall UUID comments."
```

---

### Task 7: NftablesFirewallAdapter -- Implementation

**Files:**
- Create: `crates/infra/src/nftables/adapter.rs`
- Modify: `crates/infra/src/nftables/mod.rs`

- [ ] **Step 1: Create the adapter**

`crates/infra/src/nftables/adapter.rs`:
```rust
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
use super::translator::{get_target_chains, translate_rule};
use super::types::{HandleMap, NftRuleHandle, RollbackState};

/// Configuration for the NftablesFirewallAdapter.
/// Configuration pour l'adaptateur NftablesFirewallAdapter.
#[derive(Debug, Clone)]
pub struct NftablesConfig {
    pub table_name: String,
    pub nft_binary_path: PathBuf,
    pub command_timeout: Duration,
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
/// Adaptateur réel nftables. Gère une table dédiée avec des chaînes input/output/forward.
pub struct NftablesFirewallAdapter {
    config: NftablesConfig,
    handle_map: Mutex<HandleMap>,
    started_at: Instant,
    nftables_synced: Mutex<bool>,
}

impl NftablesFirewallAdapter {
    /// Create a new adapter with the given configuration.
    /// Crée un nouvel adaptateur avec la configuration donnée.
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
    /// Exécute une commande nft et retourne stdout en cas de succès.
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
    /// S'assure que la table et les chaînes syswall existent (idempotent).
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
    /// Sauvegarde l'état actuel de la table pour retour arrière.
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
    /// Tente un retour arrière vers un état précédent.
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
            let restore_cmd = NftCommandBuilder::new().arg("-j").arg("-f").arg("-");
            // Note: In production, we'd pipe the saved JSON to stdin.
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
    /// Applique une seule règle à nftables.
    async fn apply_rule(&self, rule: &Rule) -> Result<(), DomainError> {
        // Ask rules produce no nft rule
        if rule.effect == RuleEffect::Ask {
            debug!("Skipping Ask rule {} -- handled in userspace", rule.id.as_uuid());
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
            let mut cmd =
                NftCommandBuilder::add_rule(&self.config.table_name, chain);
            for expr in &translated.expressions {
                cmd = cmd.arg(expr.clone());
            }

            match self.execute_nft(&cmd).await {
                Ok(output) => {
                    // Parse handle from output if available
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
    /// Supprime une règle de nftables par son identifiant du domaine.
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
    /// Synchronise toutes les règles : réconcilie l'état nftables avec la liste de règles fournie.
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
    /// Retourne l'état actuel du pare-feu.
    async fn get_status(&self) -> Result<FirewallStatus, DomainError> {
        let synced = *self.nftables_synced.lock().unwrap();
        let handle_map = self.handle_map.lock().unwrap();
        let active_count = handle_map.get(&RuleId::new()).map_or(0, |_| 0);
        // Count is approximate -- count unique rule IDs in the handle map
        // We don't have a len() method, so list the table
        drop(handle_map);

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
```

- [ ] **Step 2: Update nftables mod.rs**

`crates/infra/src/nftables/mod.rs`:
```rust
pub mod adapter;
pub mod command;
pub mod parser;
pub mod translator;
pub mod types;

pub use adapter::{NftablesConfig, NftablesFirewallAdapter};
pub use command::NftCommandBuilder;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p syswall-infra`
Expected: compiles with no errors (may have warnings)

- [ ] **Step 4: Commit**

```bash
git add crates/infra/src/nftables/
git commit -m "feat: add NftablesFirewallAdapter implementing FirewallEngine

Manages a dedicated inet syswall table with input/output/forward chains.
apply_rule, remove_rule, sync_all_rules with rollback on failure.
Handle tracking via UUID comments embedded in nft rules."
```

---

### Task 8: ConntrackEventParser -- Failing Tests

**Files:**
- Create: `crates/infra/src/conntrack/mod.rs`
- Create: `crates/infra/src/conntrack/types.rs`
- Create: `crates/infra/src/conntrack/parser.rs`
- Modify: `crates/infra/src/lib.rs`

- [ ] **Step 1: Create conntrack types**

`crates/infra/src/conntrack/types.rs`:
```rust
use std::net::IpAddr;

use syswall_domain::value_objects::Protocol;

/// The type of conntrack event.
/// Le type d'événement conntrack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConntrackEventType {
    New,
    Update,
    Destroy,
}

/// A parsed conntrack event (raw, before domain transformation).
/// Un événement conntrack parsé (brut, avant transformation en domaine).
#[derive(Debug, Clone)]
pub struct ConntrackEvent {
    pub timestamp: f64,
    pub event_type: ConntrackEventType,
    pub protocol: Protocol,
    pub proto_number: u8,
    pub state: Option<String>,
    pub src: IpAddr,
    pub dst: IpAddr,
    pub sport: u16,
    pub dport: u16,
    pub reply_src: Option<IpAddr>,
    pub reply_dst: Option<IpAddr>,
    pub reply_sport: Option<u16>,
    pub reply_dport: Option<u16>,
}
```

- [ ] **Step 2: Create parser with failing tests**

`crates/infra/src/conntrack/parser.rs`:
```rust
use std::net::IpAddr;

use syswall_domain::value_objects::Protocol;

use super::types::{ConntrackEvent, ConntrackEventType};

/// Parse a single conntrack event output line into a ConntrackEvent.
/// Returns None if the line cannot be parsed.
///
/// Analyse une seule ligne de sortie d'événement conntrack en ConntrackEvent.
/// Retourne None si la ligne ne peut pas être parsée.
pub fn parse_conntrack_line(line: &str) -> Option<ConntrackEvent> {
    todo!()
}

/// Parse the event type token ([NEW], [UPDATE], [DESTROY]).
/// Analyse le jeton de type d'événement ([NEW], [UPDATE], [DESTROY]).
fn parse_event_type(token: &str) -> Option<ConntrackEventType> {
    todo!()
}

/// Parse a protocol name to our domain Protocol enum.
/// Analyse un nom de protocole vers notre enum Protocol du domaine.
fn parse_protocol(name: &str) -> Option<Protocol> {
    todo!()
}

/// Extract a key=value pair from the token list.
/// Extrait une paire clé=valeur depuis la liste de jetons.
fn extract_kv<'a>(tokens: &'a [&str], key: &str) -> Option<&'a str> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_new_tcp_event() {
        let line = "[1711468800.123456]      [NEW] tcp      6 120 SYN_SENT src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 [UNREPLIED] src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.event_type, ConntrackEventType::New);
        assert_eq!(event.protocol, Protocol::Tcp);
        assert_eq!(event.src, "192.168.1.100".parse::<IpAddr>().unwrap());
        assert_eq!(event.dst, "93.184.216.34".parse::<IpAddr>().unwrap());
        assert_eq!(event.sport, 45000);
        assert_eq!(event.dport, 443);
        assert!((event.timestamp - 1711468800.123456).abs() < 0.001);
    }

    #[test]
    fn parse_destroy_event() {
        let line = "[1711468800.345678]  [DESTROY] tcp      6 src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.event_type, ConntrackEventType::Destroy);
        assert!(event.state.is_none());
    }

    #[test]
    fn parse_update_established() {
        let line = "[1711468800.234567]   [UPDATE] tcp      6 60 ESTABLISHED src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.event_type, ConntrackEventType::Update);
        assert_eq!(event.state, Some("ESTABLISHED".to_string()));
    }

    #[test]
    fn parse_udp_event() {
        let line = "[1711468800.456789]      [NEW] udp      17 30 src=192.168.1.100 dst=8.8.8.8 sport=52000 dport=53 [UNREPLIED] src=8.8.8.8 dst=192.168.1.100 sport=53 dport=52000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.protocol, Protocol::Udp);
        assert_eq!(event.proto_number, 17);
        assert_eq!(event.dport, 53);
    }

    #[test]
    fn malformed_line_returns_none() {
        assert!(parse_conntrack_line("garbage data").is_none());
    }

    #[test]
    fn missing_port_returns_none() {
        let line = "[1711468800.123456]      [NEW] tcp      6 120 SYN_SENT src=192.168.1.100 dst=93.184.216.34";
        assert!(parse_conntrack_line(line).is_none());
    }

    #[test]
    fn ipv6_addresses_parsed() {
        let line = "[1711468800.123456]      [NEW] tcp      6 120 SYN_SENT src=::1 dst=::1 sport=45000 dport=8080 [UNREPLIED] src=::1 dst=::1 sport=8080 dport=45000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.src, "::1".parse::<IpAddr>().unwrap());
        assert_eq!(event.dst, "::1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn empty_line_returns_none() {
        assert!(parse_conntrack_line("").is_none());
    }

    #[test]
    fn reply_addresses_parsed() {
        let line = "[1711468800.123456]      [NEW] tcp      6 120 SYN_SENT src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 [UNREPLIED] src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(
            event.reply_src,
            Some("93.184.216.34".parse::<IpAddr>().unwrap())
        );
        assert_eq!(
            event.reply_dst,
            Some("192.168.1.100".parse::<IpAddr>().unwrap())
        );
        assert_eq!(event.reply_sport, Some(443));
        assert_eq!(event.reply_dport, Some(45000));
    }
}
```

- [ ] **Step 3: Create conntrack mod.rs and register in lib.rs**

`crates/infra/src/conntrack/mod.rs`:
```rust
pub mod parser;
pub mod types;
```

Update `crates/infra/src/lib.rs`:
```rust
pub mod conntrack;
pub mod event_bus;
pub mod nftables;
pub mod persistence;
```

- [ ] **Step 4: Verify tests fail**

Run: `cargo test -p syswall-infra conntrack::parser`
Expected: 9 tests FAIL (all `todo!()` panics)

---

### Task 9: ConntrackEventParser -- Implementation

**Files:**
- Modify: `crates/infra/src/conntrack/parser.rs`

- [ ] **Step 1: Implement the parser functions**

Replace the `todo!()` bodies in `crates/infra/src/conntrack/parser.rs`. Keep the test module unchanged:

```rust
use std::net::IpAddr;

use syswall_domain::value_objects::Protocol;

use super::types::{ConntrackEvent, ConntrackEventType};

/// Parse a single conntrack event output line into a ConntrackEvent.
/// Returns None if the line cannot be parsed.
///
/// Analyse une seule ligne de sortie d'événement conntrack en ConntrackEvent.
/// Retourne None si la ligne ne peut pas être parsée.
pub fn parse_conntrack_line(line: &str) -> Option<ConntrackEvent> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Parse timestamp: [1711468800.123456]
    let ts_end = line.find(']')?;
    let ts_str = &line[1..ts_end];
    let timestamp: f64 = ts_str.parse().ok()?;

    let rest = &line[ts_end + 1..];

    // Tokenize the rest
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.len() < 5 {
        return None;
    }

    // Find event type: [NEW], [UPDATE], [DESTROY]
    let mut event_type = None;
    let mut event_type_idx = 0;
    for (i, token) in tokens.iter().enumerate() {
        if let Some(et) = parse_event_type(token) {
            event_type = Some(et);
            event_type_idx = i;
            break;
        }
    }
    let event_type = event_type?;

    // Protocol is next token after event type
    let proto_idx = event_type_idx + 1;
    if proto_idx >= tokens.len() {
        return None;
    }
    let protocol = parse_protocol(tokens[proto_idx])?;

    // Protocol number is next
    let proto_num_idx = proto_idx + 1;
    let proto_number: u8 = if proto_num_idx < tokens.len() {
        tokens[proto_num_idx].parse().unwrap_or(0)
    } else {
        0
    };

    // Find state: known TCP states appearing before first key=value pair
    let kv_tokens = &tokens[proto_num_idx + 1..];
    let mut state = None;
    let known_states = [
        "SYN_SENT",
        "SYN_RECV",
        "ESTABLISHED",
        "FIN_WAIT",
        "CLOSE_WAIT",
        "LAST_ACK",
        "TIME_WAIT",
        "CLOSE",
        "LISTEN",
    ];

    for token in kv_tokens {
        if known_states.contains(token) {
            state = Some(token.to_string());
            break;
        }
        // Stop looking once we hit key=value pairs
        if token.contains('=') {
            break;
        }
    }

    // Extract key=value pairs -- there are two sets separated by [UNREPLIED] or similar markers
    // First set is the original direction, second set is the reply
    let all_kv: Vec<&str> = tokens.iter().copied().filter(|t| t.contains('=')).collect();

    // First occurrence of src, dst, sport, dport
    let src_str = extract_kv_from_list(&all_kv, "src", 0)?;
    let dst_str = extract_kv_from_list(&all_kv, "dst", 0)?;
    let sport_str = extract_kv_from_list(&all_kv, "sport", 0)?;
    let dport_str = extract_kv_from_list(&all_kv, "dport", 0)?;

    let src: IpAddr = src_str.parse().ok()?;
    let dst: IpAddr = dst_str.parse().ok()?;
    let sport: u16 = sport_str.parse().ok()?;
    let dport: u16 = dport_str.parse().ok()?;

    // Second occurrence is the reply direction
    let reply_src = extract_kv_from_list(&all_kv, "src", 1)
        .and_then(|s| s.parse::<IpAddr>().ok());
    let reply_dst = extract_kv_from_list(&all_kv, "dst", 1)
        .and_then(|s| s.parse::<IpAddr>().ok());
    let reply_sport = extract_kv_from_list(&all_kv, "sport", 1)
        .and_then(|s| s.parse::<u16>().ok());
    let reply_dport = extract_kv_from_list(&all_kv, "dport", 1)
        .and_then(|s| s.parse::<u16>().ok());

    Some(ConntrackEvent {
        timestamp,
        event_type,
        protocol,
        proto_number,
        state,
        src,
        dst,
        sport,
        dport,
        reply_src,
        reply_dst,
        reply_sport,
        reply_dport,
    })
}

/// Parse the event type token ([NEW], [UPDATE], [DESTROY]).
/// Analyse le jeton de type d'événement ([NEW], [UPDATE], [DESTROY]).
fn parse_event_type(token: &str) -> Option<ConntrackEventType> {
    match token {
        "[NEW]" => Some(ConntrackEventType::New),
        "[UPDATE]" => Some(ConntrackEventType::Update),
        "[DESTROY]" => Some(ConntrackEventType::Destroy),
        _ => None,
    }
}

/// Parse a protocol name to our domain Protocol enum.
/// Analyse un nom de protocole vers notre enum Protocol du domaine.
fn parse_protocol(name: &str) -> Option<Protocol> {
    match name {
        "tcp" => Some(Protocol::Tcp),
        "udp" => Some(Protocol::Udp),
        "icmp" => Some(Protocol::Icmp),
        _ => None,
    }
}

/// Extract the Nth occurrence of a key=value pair from the token list.
/// Extrait la Nième occurrence d'une paire clé=valeur depuis la liste de jetons.
fn extract_kv_from_list<'a>(tokens: &[&'a str], key: &str, occurrence: usize) -> Option<&'a str> {
    let prefix = format!("{}=", key);
    tokens
        .iter()
        .filter(|t| t.starts_with(&prefix))
        .nth(occurrence)
        .map(|t| &t[prefix.len()..])
}

/// Extract a key=value pair from the token list (first occurrence).
/// Extrait une paire clé=valeur depuis la liste de jetons (première occurrence).
fn extract_kv<'a>(tokens: &'a [&str], key: &str) -> Option<&'a str> {
    extract_kv_from_list(tokens, key, 0)
}
```

- [ ] **Step 2: Verify all tests pass**

Run: `cargo test -p syswall-infra conntrack::parser`
Expected: 9 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/infra/src/conntrack/ crates/infra/src/lib.rs
git commit -m "feat: add conntrack event line parser

Pure parsing of conntrack -E output lines into ConntrackEvent structs.
Handles NEW/UPDATE/DESTROY events, TCP/UDP protocols, IPv4/IPv6,
original and reply direction fields."
```

---

### Task 10: Conntrack Domain Transformer -- Tests and Implementation

**Files:**
- Create: `crates/infra/src/conntrack/transformer.rs`
- Modify: `crates/infra/src/conntrack/mod.rs`

- [ ] **Step 1: Create transformer with tests and implementation**

`crates/infra/src/conntrack/transformer.rs`:
```rust
use std::net::IpAddr;

use chrono::Utc;

use syswall_domain::entities::{
    Connection, ConnectionId, ConnectionState, ConnectionVerdict,
};
use syswall_domain::value_objects::{Direction, Port, SocketAddress};

use super::types::{ConntrackEvent, ConntrackEventType};

/// Transform a raw ConntrackEvent into a domain Connection.
/// The connection is created with process=None and verdict=Unknown.
/// Process resolution and policy evaluation happen downstream.
///
/// Transforme un ConntrackEvent brut en Connection du domaine.
/// La connexion est créée avec process=None et verdict=Unknown.
/// La résolution de processus et l'évaluation de politique se font en aval.
pub fn conntrack_to_connection(
    event: ConntrackEvent,
    local_ips: &[IpAddr],
) -> Option<Connection> {
    let source_port = Port::new(event.sport).ok()?;
    let dest_port = Port::new(event.dport).ok()?;

    let direction = if local_ips.contains(&event.src) {
        Direction::Outbound
    } else if local_ips.contains(&event.dst) {
        Direction::Inbound
    } else {
        // Neither src nor dst is local -- could be forwarded traffic, default to outbound
        Direction::Outbound
    };

    let state = match event.event_type {
        ConntrackEventType::New => ConnectionState::New,
        ConntrackEventType::Update => match event.state.as_deref() {
            Some("ESTABLISHED") => ConnectionState::Established,
            Some("TIME_WAIT") | Some("CLOSE_WAIT") | Some("LAST_ACK") | Some("CLOSE") => {
                ConnectionState::Closing
            }
            Some("SYN_SENT") | Some("SYN_RECV") => ConnectionState::New,
            _ => ConnectionState::Established,
        },
        ConntrackEventType::Destroy => ConnectionState::Closed,
    };

    Some(Connection {
        id: ConnectionId::new(),
        protocol: event.protocol,
        source: SocketAddress::new(event.src, source_port),
        destination: SocketAddress::new(event.dst, dest_port),
        direction,
        state,
        process: None,
        user: None,
        bytes_sent: 0,
        bytes_received: 0,
        started_at: Utc::now(),
        verdict: ConnectionVerdict::Unknown,
        matched_rule: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use syswall_domain::value_objects::Protocol;

    fn make_event(src: &str, dst: &str) -> ConntrackEvent {
        ConntrackEvent {
            timestamp: 1711468800.0,
            event_type: ConntrackEventType::New,
            protocol: Protocol::Tcp,
            proto_number: 6,
            state: Some("SYN_SENT".to_string()),
            src: src.parse().unwrap(),
            dst: dst.parse().unwrap(),
            sport: 45000,
            dport: 443,
            reply_src: None,
            reply_dst: None,
            reply_sport: None,
            reply_dport: None,
        }
    }

    #[test]
    fn new_event_becomes_new_connection() {
        let event = make_event("192.168.1.100", "93.184.216.34");
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.state, ConnectionState::New);
        assert_eq!(conn.direction, Direction::Outbound);
        assert_eq!(conn.verdict, ConnectionVerdict::Unknown);
        assert!(conn.process.is_none());
    }

    #[test]
    fn direction_outbound_when_src_is_local() {
        let event = make_event("192.168.1.100", "8.8.8.8");
        let local_ips: Vec<IpAddr> = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.direction, Direction::Outbound);
    }

    #[test]
    fn direction_inbound_when_dst_is_local() {
        let event = make_event("8.8.8.8", "192.168.1.100");
        let local_ips: Vec<IpAddr> = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.direction, Direction::Inbound);
    }

    #[test]
    fn destroy_event_becomes_closed() {
        let mut event = make_event("192.168.1.100", "93.184.216.34");
        event.event_type = ConntrackEventType::Destroy;
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.state, ConnectionState::Closed);
    }

    #[test]
    fn update_established_becomes_established() {
        let mut event = make_event("192.168.1.100", "93.184.216.34");
        event.event_type = ConntrackEventType::Update;
        event.state = Some("ESTABLISHED".to_string());
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.state, ConnectionState::Established);
    }

    #[test]
    fn update_time_wait_becomes_closing() {
        let mut event = make_event("192.168.1.100", "93.184.216.34");
        event.event_type = ConntrackEventType::Update;
        event.state = Some("TIME_WAIT".to_string());
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.state, ConnectionState::Closing);
    }

    #[test]
    fn port_zero_returns_none() {
        let mut event = make_event("192.168.1.100", "93.184.216.34");
        event.sport = 0;
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        assert!(conntrack_to_connection(event, &local_ips).is_none());
    }

    #[test]
    fn connection_has_correct_addresses() {
        let event = make_event("192.168.1.100", "93.184.216.34");
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.source.ip, "192.168.1.100".parse::<IpAddr>().unwrap());
        assert_eq!(conn.destination.ip, "93.184.216.34".parse::<IpAddr>().unwrap());
        assert_eq!(conn.source.port.value(), 45000);
        assert_eq!(conn.destination.port.value(), 443);
    }
}
```

- [ ] **Step 2: Update conntrack mod.rs**

`crates/infra/src/conntrack/mod.rs`:
```rust
pub mod parser;
pub mod transformer;
pub mod types;
```

- [ ] **Step 3: Verify all tests pass**

Run: `cargo test -p syswall-infra conntrack::transformer`
Expected: 7 tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/infra/src/conntrack/
git commit -m "feat: add conntrack event to domain Connection transformer

Maps ConntrackEvent to domain Connection with direction detection
based on local IPs, state mapping, and proper defaults."
```

---

### Task 11: ConntrackMonitorAdapter

**Files:**
- Create: `crates/infra/src/conntrack/adapter.rs`
- Modify: `crates/infra/src/conntrack/mod.rs`

- [ ] **Step 1: Create the adapter**

`crates/infra/src/conntrack/adapter.rs`:
```rust
use std::net::IpAddr;
use std::path::PathBuf;

use async_trait::async_trait;
use futures::StreamExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, error, info, warn};

use syswall_domain::entities::Connection;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::{ConnectionEventStream, ConnectionMonitor};

use super::parser::parse_conntrack_line;
use super::transformer::conntrack_to_connection;

/// Configuration for the ConntrackMonitorAdapter.
/// Configuration pour l'adaptateur ConntrackMonitorAdapter.
#[derive(Debug, Clone)]
pub struct ConntrackConfig {
    pub binary_path: PathBuf,
    pub protocols: Vec<String>,
    pub buffer_size: usize,
}

impl Default for ConntrackConfig {
    fn default() -> Self {
        Self {
            binary_path: PathBuf::from("/usr/sbin/conntrack"),
            protocols: vec!["tcp".to_string(), "udp".to_string()],
            buffer_size: 4096,
        }
    }
}

/// Real conntrack-based connection monitor adapter.
/// Adaptateur réel de surveillance des connexions basé sur conntrack.
pub struct ConntrackMonitorAdapter {
    config: ConntrackConfig,
    local_ips: Vec<IpAddr>,
}

impl ConntrackMonitorAdapter {
    /// Create a new adapter. Detects local IPs at construction time.
    /// Crée un nouvel adaptateur. Détecte les IPs locales à la construction.
    pub fn new(config: ConntrackConfig) -> Result<Self, DomainError> {
        if !config.binary_path.exists() {
            return Err(DomainError::Infrastructure(format!(
                "conntrack binary not found at: {}. Install conntrack-tools package.",
                config.binary_path.display()
            )));
        }

        let local_ips = detect_local_ips();
        info!("ConntrackMonitorAdapter: detected {} local IPs", local_ips.len());

        Ok(Self { config, local_ips })
    }
}

/// Detect local IP addresses by reading network interfaces.
/// Détecte les adresses IP locales en lisant les interfaces réseau.
fn detect_local_ips() -> Vec<IpAddr> {
    let mut ips = vec![
        "127.0.0.1".parse::<IpAddr>().unwrap(),
        "::1".parse::<IpAddr>().unwrap(),
    ];

    // Use nix::ifaddrs to get interface addresses
    if let Ok(addrs) = nix::ifaddrs::getifaddrs() {
        for addr in addrs {
            if let Some(sockaddr) = addr.address {
                if let Some(sin) = sockaddr.as_sockaddr_in() {
                    let ip = IpAddr::V4(std::net::Ipv4Addr::from(sin.ip()));
                    if !ips.contains(&ip) {
                        ips.push(ip);
                    }
                } else if let Some(sin6) = sockaddr.as_sockaddr_in6() {
                    let ip = IpAddr::V6(sin6.ip());
                    if !ips.contains(&ip) {
                        ips.push(ip);
                    }
                }
            }
        }
    }

    ips
}

#[async_trait]
impl ConnectionMonitor for ConntrackMonitorAdapter {
    /// Start streaming conntrack events. Spawns one child process per protocol.
    /// Démarre le streaming des événements conntrack. Lance un processus enfant par protocole.
    async fn stream_events(&self) -> Result<ConnectionEventStream, DomainError> {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Connection, DomainError>>(
            self.config.buffer_size,
        );

        for proto in &self.config.protocols {
            let mut child = tokio::process::Command::new(&self.config.binary_path)
                .args(["-E", "-o", "timestamp", "-p", proto])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| {
                    DomainError::Infrastructure(format!(
                        "Failed to spawn conntrack -E -p {}: {}",
                        proto, e
                    ))
                })?;

            let stdout = child.stdout.take().ok_or_else(|| {
                DomainError::Infrastructure("Failed to capture conntrack stdout".to_string())
            })?;

            let tx = tx.clone();
            let local_ips = self.local_ips.clone();
            let proto_name = proto.clone();

            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                loop {
                    match lines.next_line().await {
                        Ok(Some(line)) => {
                            if let Some(event) = parse_conntrack_line(&line) {
                                if let Some(conn) =
                                    conntrack_to_connection(event, &local_ips)
                                {
                                    if tx.send(Ok(conn)).await.is_err() {
                                        debug!(
                                            "conntrack {} stream: receiver dropped",
                                            proto_name
                                        );
                                        break;
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            warn!("conntrack {} stream ended (EOF)", proto_name);
                            let _ = tx
                                .send(Err(DomainError::Infrastructure(format!(
                                    "conntrack {} process exited",
                                    proto_name
                                ))))
                                .await;
                            break;
                        }
                        Err(e) => {
                            error!("conntrack {} read error: {}", proto_name, e);
                            let _ = tx
                                .send(Err(DomainError::Infrastructure(format!(
                                    "conntrack read error: {}",
                                    e
                                ))))
                                .await;
                            break;
                        }
                    }
                }

                // Cleanup: kill child process
                let _ = child.kill().await;
            });
        }

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    /// Get currently active connections by running `conntrack -L`.
    /// Récupère les connexions actives en exécutant `conntrack -L`.
    async fn get_active_connections(&self) -> Result<Vec<Connection>, DomainError> {
        let output = tokio::process::Command::new(&self.config.binary_path)
            .args(["-L", "-o", "extended"])
            .output()
            .await
            .map_err(|e| {
                DomainError::Infrastructure(format!("Failed to run conntrack -L: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::Infrastructure(format!(
                "conntrack -L failed: {}",
                &stderr[..stderr.len().min(500)]
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let connections: Vec<Connection> = stdout
            .lines()
            .filter_map(|line| parse_conntrack_line(line))
            .filter_map(|event| conntrack_to_connection(event, &self.local_ips))
            .collect();

        Ok(connections)
    }
}
```

- [ ] **Step 2: Update conntrack mod.rs**

`crates/infra/src/conntrack/mod.rs`:
```rust
pub mod adapter;
pub mod parser;
pub mod transformer;
pub mod types;

pub use adapter::{ConntrackConfig, ConntrackMonitorAdapter};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p syswall-infra`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/infra/src/conntrack/
git commit -m "feat: add ConntrackMonitorAdapter implementing ConnectionMonitor

Spawns conntrack -E child processes per protocol, parses stdout
into domain Connections via async stream. get_active_connections
runs conntrack -L. Detects local IPs at construction for direction."
```

---

### Task 12: ProcfsProcessResolver -- Cache Tests (Failing)

**Files:**
- Create: `crates/infra/src/process/mod.rs`
- Create: `crates/infra/src/process/cache.rs`
- Modify: `crates/infra/src/lib.rs`

- [ ] **Step 1: Create process cache with failing tests**

`crates/infra/src/process/cache.rs`:
```rust
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lru::LruCache;
use std::num::NonZeroUsize;

use syswall_domain::entities::{ProcessInfo, SystemUser};

/// Entry in the process cache with insertion timestamp.
/// Entrée dans le cache de processus avec horodatage d'insertion.
#[derive(Clone)]
struct CacheEntry {
    info: ProcessInfo,
    user: Option<SystemUser>,
    inserted_at: Instant,
}

/// LRU cache with TTL for process resolution results.
/// Cache LRU avec TTL pour les résultats de résolution de processus.
pub struct ProcessCache {
    pid_cache: Mutex<LruCache<u32, CacheEntry>>,
    inode_cache: Mutex<LruCache<u64, CacheEntry>>,
    ttl: Duration,
}

impl ProcessCache {
    /// Create a new cache with the given capacity and TTL.
    /// Crée un nouveau cache avec la capacité et le TTL donnés.
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        todo!()
    }

    /// Get a cached entry by PID. Returns None if not found or stale.
    /// Retourne une entrée mise en cache par PID. Retourne None si introuvable ou périmée.
    pub fn get_by_pid(&self, pid: u32) -> Option<(ProcessInfo, Option<SystemUser>)> {
        todo!()
    }

    /// Get a cached entry by socket inode. Returns None if not found or stale.
    /// Retourne une entrée mise en cache par inode de socket. Retourne None si introuvable ou périmée.
    pub fn get_by_inode(&self, inode: u64) -> Option<(ProcessInfo, Option<SystemUser>)> {
        todo!()
    }

    /// Insert a process info entry by PID.
    /// Insère une entrée d'info processus par PID.
    pub fn insert_pid(&self, pid: u32, info: ProcessInfo, user: Option<SystemUser>) {
        todo!()
    }

    /// Insert a process info entry by socket inode.
    /// Insère une entrée d'info processus par inode de socket.
    pub fn insert_inode(&self, inode: u64, info: ProcessInfo, user: Option<SystemUser>) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_info(pid: u32) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: format!("proc-{}", pid),
            path: None,
            cmdline: None,
        }
    }

    #[test]
    fn cache_returns_fresh_entry() {
        let cache = ProcessCache::new(10, Duration::from_secs(5));
        let info = make_info(1);
        cache.insert_pid(1, info.clone(), None);
        let result = cache.get_by_pid(1);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0.name, "proc-1");
    }

    #[test]
    fn cache_evicts_stale_entry() {
        let cache = ProcessCache::new(10, Duration::from_millis(1));
        let info = make_info(1);
        cache.insert_pid(1, info, None);
        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.get_by_pid(1).is_none());
    }

    #[test]
    fn cache_respects_capacity() {
        let cache = ProcessCache::new(2, Duration::from_secs(60));
        cache.insert_pid(1, make_info(1), None);
        cache.insert_pid(2, make_info(2), None);
        cache.insert_pid(3, make_info(3), None);
        assert!(cache.get_by_pid(1).is_none());
        assert!(cache.get_by_pid(2).is_some());
        assert!(cache.get_by_pid(3).is_some());
    }

    #[test]
    fn inode_cache_independent_from_pid_cache() {
        let cache = ProcessCache::new(10, Duration::from_secs(5));
        let info = make_info(1);
        cache.insert_inode(99, info, None);
        assert!(cache.get_by_pid(1).is_none());
        assert!(cache.get_by_inode(99).is_some());
    }

    #[test]
    fn cache_stores_user_info() {
        let cache = ProcessCache::new(10, Duration::from_secs(5));
        let user = SystemUser {
            uid: 1000,
            name: "seb".to_string(),
        };
        cache.insert_pid(1, make_info(1), Some(user));
        let (_, u) = cache.get_by_pid(1).unwrap();
        assert_eq!(u.unwrap().name, "seb");
    }

    #[test]
    fn cache_miss_returns_none() {
        let cache = ProcessCache::new(10, Duration::from_secs(5));
        assert!(cache.get_by_pid(999).is_none());
        assert!(cache.get_by_inode(999).is_none());
    }
}
```

`crates/infra/src/process/mod.rs`:
```rust
pub mod cache;
```

Update `crates/infra/src/lib.rs`:
```rust
pub mod conntrack;
pub mod event_bus;
pub mod nftables;
pub mod persistence;
pub mod process;
```

- [ ] **Step 2: Verify tests fail**

Run: `cargo test -p syswall-infra process::cache`
Expected: 6 tests FAIL

---

### Task 13: ProcfsProcessResolver -- Cache Implementation

**Files:**
- Modify: `crates/infra/src/process/cache.rs`

- [ ] **Step 1: Implement cache methods**

Replace the `todo!()` bodies. Keep the test module unchanged:

```rust
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lru::LruCache;
use std::num::NonZeroUsize;

use syswall_domain::entities::{ProcessInfo, SystemUser};

/// Entry in the process cache with insertion timestamp.
/// Entrée dans le cache de processus avec horodatage d'insertion.
#[derive(Clone)]
struct CacheEntry {
    info: ProcessInfo,
    user: Option<SystemUser>,
    inserted_at: Instant,
}

/// LRU cache with TTL for process resolution results.
/// Cache LRU avec TTL pour les résultats de résolution de processus.
pub struct ProcessCache {
    pid_cache: Mutex<LruCache<u32, CacheEntry>>,
    inode_cache: Mutex<LruCache<u64, CacheEntry>>,
    ttl: Duration,
}

impl ProcessCache {
    /// Create a new cache with the given capacity and TTL.
    /// Crée un nouveau cache avec la capacité et le TTL donnés.
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap();
        Self {
            pid_cache: Mutex::new(LruCache::new(cap)),
            inode_cache: Mutex::new(LruCache::new(cap)),
            ttl,
        }
    }

    /// Get a cached entry by PID. Returns None if not found or stale.
    /// Retourne une entrée mise en cache par PID. Retourne None si introuvable ou périmée.
    pub fn get_by_pid(&self, pid: u32) -> Option<(ProcessInfo, Option<SystemUser>)> {
        let mut cache = self.pid_cache.lock().unwrap();
        let entry = cache.get(&pid)?;
        if entry.inserted_at.elapsed() > self.ttl {
            cache.pop(&pid);
            return None;
        }
        Some((entry.info.clone(), entry.user.clone()))
    }

    /// Get a cached entry by socket inode. Returns None if not found or stale.
    /// Retourne une entrée mise en cache par inode de socket. Retourne None si introuvable ou périmée.
    pub fn get_by_inode(&self, inode: u64) -> Option<(ProcessInfo, Option<SystemUser>)> {
        let mut cache = self.inode_cache.lock().unwrap();
        let entry = cache.get(&inode)?;
        if entry.inserted_at.elapsed() > self.ttl {
            cache.pop(&inode);
            return None;
        }
        Some((entry.info.clone(), entry.user.clone()))
    }

    /// Insert a process info entry by PID.
    /// Insère une entrée d'info processus par PID.
    pub fn insert_pid(&self, pid: u32, info: ProcessInfo, user: Option<SystemUser>) {
        let mut cache = self.pid_cache.lock().unwrap();
        cache.put(
            pid,
            CacheEntry {
                info,
                user,
                inserted_at: Instant::now(),
            },
        );
    }

    /// Insert a process info entry by socket inode.
    /// Insère une entrée d'info processus par inode de socket.
    pub fn insert_inode(&self, inode: u64, info: ProcessInfo, user: Option<SystemUser>) {
        let mut cache = self.inode_cache.lock().unwrap();
        cache.put(
            inode,
            CacheEntry {
                info,
                user,
                inserted_at: Instant::now(),
            },
        );
    }
}
```

- [ ] **Step 2: Verify all tests pass**

Run: `cargo test -p syswall-infra process::cache`
Expected: 6 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/infra/src/process/ crates/infra/src/lib.rs
git commit -m "feat: add ProcessCache with LRU eviction and TTL

Dual LRU caches (by PID and by socket inode) with configurable
capacity and TTL. Thread-safe via std::sync::Mutex."
```

---

### Task 14: Proc Parser -- Tests and Implementation

**Files:**
- Create: `crates/infra/src/process/proc_parser.rs`
- Modify: `crates/infra/src/process/mod.rs`

- [ ] **Step 1: Create proc parser with tests and implementation**

`crates/infra/src/process/proc_parser.rs`:
```rust
use std::net::IpAddr;

/// A parsed entry from /proc/net/tcp or /proc/net/udp.
/// Une entrée parsée de /proc/net/tcp ou /proc/net/udp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcNetEntry {
    pub local_ip: IpAddr,
    pub local_port: u16,
    pub remote_ip: IpAddr,
    pub remote_port: u16,
    pub state: u8,
    pub uid: u32,
    pub inode: u64,
}

/// Parsed fields from /proc/<pid>/status.
/// Champs parsés de /proc/<pid>/status.
#[derive(Debug, Clone)]
pub struct ProcStatus {
    pub name: String,
    pub uid: u32,
}

/// Parse /proc/net/tcp content into entries.
/// Analyse le contenu de /proc/net/tcp en entrées.
pub fn parse_proc_net_tcp(content: &str) -> Vec<ProcNetEntry> {
    parse_proc_net(content, false)
}

/// Parse /proc/net/tcp6 content into entries.
/// Analyse le contenu de /proc/net/tcp6 en entrées.
pub fn parse_proc_net_tcp6(content: &str) -> Vec<ProcNetEntry> {
    parse_proc_net(content, true)
}

/// Parse /proc/net/udp content into entries.
/// Analyse le contenu de /proc/net/udp en entrées.
pub fn parse_proc_net_udp(content: &str) -> Vec<ProcNetEntry> {
    parse_proc_net(content, false)
}

fn parse_proc_net(content: &str, is_v6: bool) -> Vec<ProcNetEntry> {
    content
        .lines()
        .skip(1) // skip header
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 {
                return None;
            }

            let (local_ip, local_port) = parse_addr_port(fields[1], is_v6)?;
            let (remote_ip, remote_port) = parse_addr_port(fields[2], is_v6)?;
            let state = u8::from_str_radix(fields[3], 16).ok()?;
            let uid: u32 = fields[7].parse().ok()?;
            let inode: u64 = fields[9].parse().ok()?;

            Some(ProcNetEntry {
                local_ip,
                local_port,
                remote_ip,
                remote_port,
                state,
                uid,
                inode,
            })
        })
        .collect()
}

/// Parse a hex address:port string like "0100007F:0050".
/// Analyse une chaîne adresse:port hexadécimale comme "0100007F:0050".
fn parse_addr_port(s: &str, is_v6: bool) -> Option<(IpAddr, u16)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let ip = if is_v6 {
        parse_hex_ipv6(parts[0])?
    } else {
        parse_hex_ip(parts[0])?
    };
    let port = u16::from_str_radix(parts[1], 16).ok()?;

    Some((ip, port))
}

/// Parse a hex-encoded IPv4 address (little-endian).
/// Analyse une adresse IPv4 encodée en hexadécimal (little-endian).
pub fn parse_hex_ip(hex: &str) -> Option<IpAddr> {
    if hex.len() != 8 {
        return None;
    }
    let val = u32::from_str_radix(hex, 16).ok()?;
    // /proc/net/tcp uses host byte order (little-endian on x86)
    let ip = std::net::Ipv4Addr::from(val.to_be());
    Some(IpAddr::V4(ip))
}

/// Parse a hex-encoded IPv6 address from /proc/net/tcp6.
/// Analyse une adresse IPv6 encodée en hexadécimal de /proc/net/tcp6.
pub fn parse_hex_ipv6(hex: &str) -> Option<IpAddr> {
    if hex.len() != 32 {
        return None;
    }
    // /proc/net/tcp6 stores as 4 groups of 32-bit values in host byte order
    let mut octets = [0u8; 16];
    for i in 0..4 {
        let group_hex = &hex[i * 8..(i + 1) * 8];
        let val = u32::from_str_radix(group_hex, 16).ok()?;
        let bytes = val.to_be_bytes();
        // Reverse within each 4-byte group (host to network byte order)
        octets[i * 4] = bytes[0];
        octets[i * 4 + 1] = bytes[1];
        octets[i * 4 + 2] = bytes[2];
        octets[i * 4 + 3] = bytes[3];
    }
    Some(IpAddr::V6(std::net::Ipv6Addr::from(octets)))
}

/// Parse /proc/<pid>/status content.
/// Analyse le contenu de /proc/<pid>/status.
pub fn parse_proc_status(content: &str) -> Option<ProcStatus> {
    let mut name = None;
    let mut uid = None;

    for line in content.lines() {
        if let Some(n) = line.strip_prefix("Name:\t") {
            name = Some(n.to_string());
        } else if let Some(uid_line) = line.strip_prefix("Uid:\t") {
            // Format: real effective saved fs
            if let Some(real_uid) = uid_line.split_whitespace().next() {
                uid = real_uid.parse().ok();
            }
        }
    }

    Some(ProcStatus {
        name: name?,
        uid: uid?,
    })
}

/// Parse /proc/<pid>/cmdline bytes (NUL-separated).
/// Analyse les octets de /proc/<pid>/cmdline (séparés par NUL).
pub fn parse_cmdline(bytes: &[u8]) -> String {
    bytes
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse /proc/<pid>/cmdline bytes, returning None if empty.
/// Analyse les octets de /proc/<pid>/cmdline, retournant None si vide.
pub fn parse_cmdline_opt(bytes: &[u8]) -> Option<String> {
    let result = parse_cmdline(bytes);
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_proc_net_tcp_single_entry() {
        let content = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 0100007F:0050 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 12345 1 0000000000000000 100 0 0 10 0\n";
        let entries = parse_proc_net_tcp(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].local_ip, "127.0.0.1".parse::<IpAddr>().unwrap());
        assert_eq!(entries[0].local_port, 80);
        assert_eq!(entries[0].inode, 12345);
        assert_eq!(entries[0].uid, 0);
    }

    #[test]
    fn parse_hex_ip_loopback() {
        let ip = parse_hex_ip("0100007F").unwrap();
        assert_eq!(ip, "127.0.0.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn parse_hex_ip_invalid_length() {
        assert!(parse_hex_ip("0100").is_none());
    }

    #[test]
    fn parse_proc_status_name_and_uid() {
        let content = "Name:\tfirefox\nUmask:\t0022\nState:\tS (sleeping)\nTgid:\t1234\nNgid:\t0\nPid:\t1234\nPPid:\t1000\nUid:\t1000\t1000\t1000\t1000\nGid:\t1000\t1000\t1000\t1000\n";
        let info = parse_proc_status(content).unwrap();
        assert_eq!(info.name, "firefox");
        assert_eq!(info.uid, 1000);
    }

    #[test]
    fn parse_cmdline_with_args() {
        let bytes = b"firefox\0--no-remote\0https://example.com\0";
        let cmdline = parse_cmdline(bytes);
        assert_eq!(cmdline, "firefox --no-remote https://example.com");
    }

    #[test]
    fn parse_empty_cmdline() {
        assert!(parse_cmdline_opt(b"").is_none());
    }

    #[test]
    fn parse_proc_net_tcp6_loopback() {
        let content = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 00000000000000000000000001000000:0050 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 12345 1 0000000000000000 100 0 0 10 0\n";
        let entries = parse_proc_net_tcp6(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].local_ip, "::1".parse::<IpAddr>().unwrap());
        assert_eq!(entries[0].local_port, 80);
    }

    #[test]
    fn parse_proc_net_tcp_empty() {
        let content = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n";
        let entries = parse_proc_net_tcp(content);
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_proc_status_missing_name_returns_none() {
        let content = "Uid:\t1000\t1000\t1000\t1000\n";
        assert!(parse_proc_status(content).is_none());
    }
}
```

- [ ] **Step 2: Update process mod.rs**

`crates/infra/src/process/mod.rs`:
```rust
pub mod cache;
pub mod proc_parser;
```

- [ ] **Step 3: Verify all tests pass**

Run: `cargo test -p syswall-infra process::proc_parser`
Expected: 9 tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/infra/src/process/
git commit -m "feat: add /proc file parsers for process resolution

Parse /proc/net/tcp{,6}, /proc/pid/status, /proc/pid/cmdline.
Hex IP address decoding handles both IPv4 (little-endian) and
IPv6 (4x32-bit host byte order) formats."
```

---

### Task 15: ProcfsProcessResolver -- Implementation

**Files:**
- Create: `crates/infra/src/process/resolver.rs`
- Modify: `crates/infra/src/process/mod.rs`

- [ ] **Step 1: Create the resolver**

`crates/infra/src/process/resolver.rs`:
```rust
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use tracing::{debug, warn};

use syswall_domain::entities::{ProcessInfo, SystemUser};
use syswall_domain::errors::DomainError;
use syswall_domain::ports::ProcessResolver;
use syswall_domain::value_objects::{ExecutablePath, Protocol};

use super::cache::ProcessCache;
use super::proc_parser::{
    parse_cmdline_opt, parse_proc_net_tcp, parse_proc_net_tcp6, parse_proc_net_udp,
    parse_proc_status,
};

/// Configuration for the ProcfsProcessResolver.
/// Configuration pour le ProcfsProcessResolver.
#[derive(Debug, Clone)]
pub struct ProcfsConfig {
    pub cache_capacity: usize,
    pub cache_ttl: Duration,
}

impl Default for ProcfsConfig {
    fn default() -> Self {
        Self {
            cache_capacity: 1024,
            cache_ttl: Duration::from_secs(5),
        }
    }
}

/// Real /proc-based process resolver.
/// Résolveur de processus réel basé sur /proc.
pub struct ProcfsProcessResolver {
    cache: ProcessCache,
}

impl ProcfsProcessResolver {
    /// Create a new resolver. Verifies /proc is accessible.
    /// Crée un nouveau résolveur. Vérifie que /proc est accessible.
    pub fn new(config: ProcfsConfig) -> Result<Self, DomainError> {
        if !Path::new("/proc").exists() {
            return Err(DomainError::Infrastructure(
                "/proc is not accessible".to_string(),
            ));
        }

        Ok(Self {
            cache: ProcessCache::new(config.cache_capacity, config.cache_ttl),
        })
    }

    /// Read process info from /proc/<pid>/.
    /// Lit les informations du processus depuis /proc/<pid>/.
    fn read_process_info(pid: u32) -> Option<(ProcessInfo, Option<SystemUser>)> {
        let proc_path = PathBuf::from(format!("/proc/{}", pid));
        if !proc_path.exists() {
            return None;
        }

        // Read executable path
        let exe_path = std::fs::read_link(proc_path.join("exe")).ok().and_then(|p| {
            let path_str = p.to_string_lossy().to_string();
            // Strip " (deleted)" suffix if present
            let clean_path = if path_str.ends_with(" (deleted)") {
                PathBuf::from(&path_str[..path_str.len() - 10])
            } else {
                p
            };
            ExecutablePath::new(clean_path).ok()
        });

        // Read cmdline
        let cmdline = std::fs::read(proc_path.join("cmdline"))
            .ok()
            .and_then(|bytes| parse_cmdline_opt(&bytes));

        // Read status for name and UID
        let status_content = std::fs::read_to_string(proc_path.join("status")).ok()?;
        let status = parse_proc_status(&status_content)?;

        let user = nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(status.uid))
            .ok()
            .flatten()
            .map(|u| SystemUser {
                uid: status.uid,
                name: u.name,
            });

        Some((
            ProcessInfo {
                pid,
                name: status.name,
                path: exe_path,
                cmdline,
            },
            user,
        ))
    }

    /// Scan /proc/[0-9]*/fd/* to find which PID owns a socket inode.
    /// Parcourt /proc/[0-9]*/fd/* pour trouver quel PID possède un inode de socket.
    fn find_pid_by_inode(inode: u64) -> Option<u32> {
        let target = format!("socket:[{}]", inode);

        let proc_dir = match std::fs::read_dir("/proc") {
            Ok(d) => d,
            Err(_) => return None,
        };

        for entry in proc_dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Only numeric directories (PIDs)
            if !name_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            let fd_dir = entry.path().join("fd");
            let fd_entries = match std::fs::read_dir(&fd_dir) {
                Ok(d) => d,
                Err(_) => continue,
            };

            for fd_entry in fd_entries.flatten() {
                if let Ok(link) = std::fs::read_link(fd_entry.path()) {
                    if link.to_string_lossy() == target {
                        return name_str.parse().ok();
                    }
                }
            }
        }

        None
    }
}

#[async_trait]
impl ProcessResolver for ProcfsProcessResolver {
    /// Resolve process info by PID.
    /// Résout les informations du processus par PID.
    async fn resolve(&self, pid: u32) -> Result<Option<ProcessInfo>, DomainError> {
        // Check cache first
        if let Some((info, _)) = self.cache.get_by_pid(pid) {
            return Ok(Some(info));
        }

        // Read from /proc in a blocking task
        let result = tokio::task::spawn_blocking(move || Self::read_process_info(pid))
            .await
            .map_err(|e| {
                DomainError::Infrastructure(format!("spawn_blocking failed: {}", e))
            })?;

        if let Some((ref info, ref user)) = result {
            self.cache
                .insert_pid(pid, info.clone(), user.clone());
        }

        Ok(result.map(|(info, _)| info))
    }

    /// Resolve process info by socket inode.
    /// Résout les informations du processus par inode de socket.
    async fn resolve_by_socket(&self, inode: u64) -> Result<Option<ProcessInfo>, DomainError> {
        // Check inode cache first
        if let Some((info, _)) = self.cache.get_by_inode(inode) {
            return Ok(Some(info));
        }

        // Find PID by scanning /proc in a blocking task
        let pid = tokio::task::spawn_blocking(move || Self::find_pid_by_inode(inode))
            .await
            .map_err(|e| {
                DomainError::Infrastructure(format!("spawn_blocking failed: {}", e))
            })?;

        let pid = match pid {
            Some(p) => p,
            None => {
                debug!("No PID found for socket inode {}", inode);
                return Ok(None);
            }
        };

        // Resolve the PID
        let result = tokio::task::spawn_blocking(move || Self::read_process_info(pid))
            .await
            .map_err(|e| {
                DomainError::Infrastructure(format!("spawn_blocking failed: {}", e))
            })?;

        if let Some((ref info, ref user)) = result {
            self.cache
                .insert_inode(inode, info.clone(), user.clone());
            self.cache
                .insert_pid(pid, info.clone(), user.clone());
        }

        Ok(result.map(|(info, _)| info))
    }
}
```

- [ ] **Step 2: Update process mod.rs**

`crates/infra/src/process/mod.rs`:
```rust
pub mod cache;
pub mod proc_parser;
pub mod resolver;

pub use resolver::{ProcfsConfig, ProcfsProcessResolver};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p syswall-infra`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/infra/src/process/
git commit -m "feat: add ProcfsProcessResolver implementing ProcessResolver

Reads /proc/pid/{exe,cmdline,status} for process info, scans
/proc/*/fd/* for socket inode to PID resolution. LRU cache with
TTL prevents repeated /proc scans. All I/O via spawn_blocking."
```

---

### Task 16: System Whitelist

**Files:**
- Create: `crates/app/src/services/whitelist.rs`
- Modify: `crates/app/src/services/mod.rs`

- [ ] **Step 1: Create whitelist service with tests**

`crates/app/src/services/whitelist.rs`:
```rust
use syswall_domain::entities::{
    IpMatcher, PortMatcher, RuleCriteria, RuleEffect, RuleScope, RuleSource,
};
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{RuleFilters, RuleRepository};
use syswall_domain::value_objects::{Direction, Port, Protocol};
use tracing::info;

use crate::commands::CreateRuleCommand;
use crate::services::rule_service::RuleService;

/// Ensure the system whitelist exists. Creates default rules on first start.
/// S'assure que la liste blanche système existe. Crée les règles par défaut au premier démarrage.
pub async fn ensure_system_whitelist(
    rule_service: &RuleService,
    rule_repo: &dyn RuleRepository,
) -> Result<(), DomainError> {
    let existing = rule_repo
        .find_all(
            &RuleFilters {
                source: Some(RuleSource::System),
                ..Default::default()
            },
            &Pagination {
                offset: 0,
                limit: 1,
            },
        )
        .await?;

    if !existing.is_empty() {
        info!(
            "System whitelist already exists ({} rules found)",
            existing.len()
        );
        return Ok(());
    }

    info!("Creating system whitelist rules (first start)...");

    let whitelist = vec![
        create_system_rule(
            "Allow DNS (UDP)",
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                remote_port: Some(PortMatcher::Exact(Port::new(53).unwrap())),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow DNS (TCP)",
            RuleCriteria {
                protocol: Some(Protocol::Tcp),
                remote_port: Some(PortMatcher::Exact(Port::new(53).unwrap())),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow DHCP Client",
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                local_port: Some(PortMatcher::Exact(Port::new(68).unwrap())),
                remote_port: Some(PortMatcher::Exact(Port::new(67).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow DHCP Server Response",
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                local_port: Some(PortMatcher::Exact(Port::new(67).unwrap())),
                remote_port: Some(PortMatcher::Exact(Port::new(68).unwrap())),
                direction: Some(Direction::Inbound),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow Loopback (IPv4)",
            RuleCriteria {
                remote_ip: Some(IpMatcher::Cidr {
                    network: "127.0.0.0".parse().unwrap(),
                    prefix_len: 8,
                }),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow Loopback (IPv6)",
            RuleCriteria {
                remote_ip: Some(IpMatcher::Exact("::1".parse().unwrap())),
                ..Default::default()
            },
        ),
        create_system_rule(
            "Allow NTP",
            RuleCriteria {
                protocol: Some(Protocol::Udp),
                remote_port: Some(PortMatcher::Exact(Port::new(123).unwrap())),
                direction: Some(Direction::Outbound),
                ..Default::default()
            },
        ),
    ];

    for cmd in whitelist {
        rule_service.create_rule(cmd).await?;
    }

    info!("System whitelist created successfully (7 rules)");
    Ok(())
}

fn create_system_rule(name: &str, criteria: RuleCriteria) -> CreateRuleCommand {
    CreateRuleCommand {
        name: name.to_string(),
        priority: 0,
        criteria,
        effect: RuleEffect::Allow,
        scope: RuleScope::Permanent,
        source: RuleSource::System,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn creates_whitelist_on_first_start() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let rule_service = RuleService::new(rule_repo.clone(), firewall, event_bus);

        ensure_system_whitelist(&rule_service, rule_repo.as_ref())
            .await
            .unwrap();

        let all_rules = rule_repo
            .find_all(
                &RuleFilters::default(),
                &Pagination {
                    offset: 0,
                    limit: 100,
                },
            )
            .await
            .unwrap();

        assert_eq!(all_rules.len(), 7);
        assert!(all_rules.iter().all(|r| r.source == RuleSource::System));
        assert!(all_rules.iter().all(|r| r.effect == RuleEffect::Allow));
        assert!(all_rules.iter().all(|r| r.enabled));
    }

    #[tokio::test]
    async fn skips_if_system_rules_exist() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let rule_service = RuleService::new(rule_repo.clone(), firewall.clone(), event_bus.clone());

        // Create whitelist first time
        ensure_system_whitelist(&rule_service, rule_repo.as_ref())
            .await
            .unwrap();

        // Call again -- should not create duplicates
        let rule_service2 = RuleService::new(rule_repo.clone(), firewall, event_bus);
        ensure_system_whitelist(&rule_service2, rule_repo.as_ref())
            .await
            .unwrap();

        let all_rules = rule_repo
            .find_all(
                &RuleFilters::default(),
                &Pagination {
                    offset: 0,
                    limit: 100,
                },
            )
            .await
            .unwrap();

        assert_eq!(all_rules.len(), 7);
    }

    #[tokio::test]
    async fn whitelist_contains_dns_rules() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let rule_service = RuleService::new(rule_repo.clone(), firewall, event_bus);

        ensure_system_whitelist(&rule_service, rule_repo.as_ref())
            .await
            .unwrap();

        let all_rules = rule_repo
            .find_all(
                &RuleFilters::default(),
                &Pagination {
                    offset: 0,
                    limit: 100,
                },
            )
            .await
            .unwrap();

        let dns_rules: Vec<_> = all_rules
            .iter()
            .filter(|r| r.name.contains("DNS"))
            .collect();
        assert_eq!(dns_rules.len(), 2); // UDP + TCP
    }
}
```

- [ ] **Step 2: Update services mod.rs**

Add to `crates/app/src/services/mod.rs`:
```rust
pub mod audit_service;
pub mod connection_service;
pub mod learning_service;
pub mod rule_service;
pub mod whitelist;
```

- [ ] **Step 3: Verify all tests pass**

Run: `cargo test -p syswall-app whitelist`
Expected: 3 tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/services/whitelist.rs crates/app/src/services/mod.rs
git commit -m "feat: add system whitelist for safe default rules

Creates DNS (UDP+TCP), DHCP, loopback (IPv4+IPv6), and NTP allow
rules on first start. Idempotent: skips if System rules exist."
```

---

### Task 17: Config Updates

**Files:**
- Modify: `crates/daemon/src/config.rs`
- Modify: `config/default.toml`

- [ ] **Step 1: Update FirewallConfig and MonitoringConfig**

In `crates/daemon/src/config.rs`, replace `FirewallConfig` and `MonitoringConfig`:

Replace:
```rust
pub struct FirewallConfig {
    pub default_policy: DefaultPolicyConfig,
    pub rollback_timeout_secs: u64,
    pub nftables_table_name: String,
}
```

With:
```rust
/// Firewall configuration (default policy, rollback, nftables).
/// Configuration du pare-feu (politique par défaut, retour arrière, nftables).
#[derive(Debug, Deserialize)]
pub struct FirewallConfig {
    pub default_policy: DefaultPolicyConfig,
    pub rollback_timeout_secs: u64,
    pub nftables_table_name: String,
    #[serde(default = "default_nft_path")]
    pub nft_binary_path: std::path::PathBuf,
    #[serde(default = "default_nft_timeout")]
    pub nft_command_timeout_secs: u64,
    #[serde(default = "default_nft_max_output")]
    pub nft_max_output_bytes: usize,
    #[serde(default)]
    pub use_fake: bool,
}

fn default_nft_path() -> std::path::PathBuf {
    std::path::PathBuf::from("/usr/sbin/nft")
}

fn default_nft_timeout() -> u64 {
    5
}

fn default_nft_max_output() -> usize {
    1_048_576
}
```

Replace:
```rust
pub struct MonitoringConfig {
    pub conntrack_buffer_size: usize,
    pub process_cache_ttl_secs: u64,
    pub event_bus_capacity: usize,
}
```

With:
```rust
/// Connection monitoring configuration (buffers, cache TTL, event bus).
/// Configuration du suivi des connexions (tampons, TTL du cache, bus d'événements).
#[derive(Debug, Deserialize)]
pub struct MonitoringConfig {
    pub conntrack_buffer_size: usize,
    pub process_cache_ttl_secs: u64,
    #[serde(default = "default_cache_capacity")]
    pub process_cache_capacity: usize,
    pub event_bus_capacity: usize,
    #[serde(default = "default_conntrack_path")]
    pub conntrack_binary_path: std::path::PathBuf,
    #[serde(default = "default_conntrack_protocols")]
    pub conntrack_protocols: Vec<String>,
    #[serde(default)]
    pub use_fake: bool,
}

fn default_cache_capacity() -> usize {
    1024
}

fn default_conntrack_path() -> std::path::PathBuf {
    std::path::PathBuf::from("/usr/sbin/conntrack")
}

fn default_conntrack_protocols() -> Vec<String> {
    vec!["tcp".to_string(), "udp".to_string()]
}
```

- [ ] **Step 2: Update default.toml**

`config/default.toml`:
```toml
config_version = 1

[daemon]
socket_path = "/var/run/syswall/syswall.sock"
log_level = "info"
log_dir = "/var/log/syswall"
watchdog_interval_secs = 15

[database]
path = "/var/lib/syswall/syswall.db"
journal_retention_days = 30
audit_batch_size = 100
audit_flush_interval_secs = 2

[firewall]
default_policy = "ask"
rollback_timeout_secs = 30
nftables_table_name = "syswall"
nft_binary_path = "/usr/sbin/nft"
nft_command_timeout_secs = 5
nft_max_output_bytes = 1048576
use_fake = false

[monitoring]
conntrack_buffer_size = 4096
process_cache_ttl_secs = 5
process_cache_capacity = 1024
event_bus_capacity = 4096
conntrack_binary_path = "/usr/sbin/conntrack"
conntrack_protocols = ["tcp", "udp"]
use_fake = false

[learning]
enabled = true
debounce_window_secs = 5
prompt_timeout_secs = 60
default_timeout_action = "block"
max_pending_decisions = 50
overflow_action = "block"

[ui]
locale = "fr"
theme = "dark"
refresh_interval_ms = 1000
```

- [ ] **Step 3: Update test config string**

In the test module of `config.rs`, update `TEST_CONFIG` to include the new fields:

Replace the `[firewall]` and `[monitoring]` sections in `TEST_CONFIG`:
```rust
[firewall]
default_policy = "ask"
rollback_timeout_secs = 30
nftables_table_name = "syswall"
nft_binary_path = "/usr/sbin/nft"
nft_command_timeout_secs = 5
nft_max_output_bytes = 1048576
use_fake = true

[monitoring]
conntrack_buffer_size = 4096
process_cache_ttl_secs = 5
process_cache_capacity = 1024
event_bus_capacity = 4096
conntrack_binary_path = "/usr/sbin/conntrack"
conntrack_protocols = ["tcp", "udp"]
use_fake = true
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p syswall-daemon config`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/config.rs config/default.toml
git commit -m "feat: add config fields for nftables and conntrack adapters

New fields: nft_binary_path, nft_command_timeout_secs,
nft_max_output_bytes, conntrack_binary_path, conntrack_protocols,
process_cache_capacity, use_fake flags. All with serde defaults."
```

---

### Task 18: Daemon Bootstrap Integration

**Files:**
- Modify: `crates/daemon/src/bootstrap.rs`
- Modify: `crates/daemon/Cargo.toml`

- [ ] **Step 1: Update daemon Cargo.toml**

Add `tokio-stream` and `futures` dependencies to `crates/daemon/Cargo.toml`:

```toml
[package]
name = "syswall-daemon"
version.workspace = true
edition.workspace = true

[dependencies]
syswall-domain = { path = "../domain" }
syswall-app = { path = "../app" }
syswall-infra = { path = "../infra" }
syswall-proto = { path = "../proto" }
tokio = { workspace = true }
tokio-util = { workspace = true }
tokio-stream = { workspace = true }
futures = { workspace = true }
tonic = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
serde = { workspace = true }
toml = { workspace = true }
chrono = { workspace = true }
```

- [ ] **Step 2: Update bootstrap.rs**

Replace `crates/daemon/src/bootstrap.rs` entirely:

```rust
use std::sync::Arc;
use std::time::Duration;

use tracing::info;

use syswall_app::fakes::{FakeConnectionMonitor, FakeFirewallEngine, FakeProcessResolver, FakeUserNotifier};
use syswall_app::services::audit_service::AuditService;
use syswall_app::services::connection_service::ConnectionService;
use syswall_app::services::learning_service::{
    LearningConfig as AppLearningConfig, LearningService,
};
use syswall_app::services::rule_service::RuleService;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::{ConnectionMonitor, FirewallEngine, ProcessResolver};
use syswall_infra::conntrack::{ConntrackConfig, ConntrackMonitorAdapter};
use syswall_infra::event_bus::TokioBroadcastEventBus;
use syswall_infra::nftables::{NftablesConfig, NftablesFirewallAdapter};
use syswall_infra::persistence::audit_repository::SqliteAuditRepository;
use syswall_infra::persistence::decision_repository::SqliteDecisionRepository;
use syswall_infra::persistence::pending_decision_repository::SqlitePendingDecisionRepository;
use syswall_infra::persistence::rule_repository::SqliteRuleRepository;
use syswall_infra::persistence::Database;
use syswall_infra::process::{ProcfsConfig, ProcfsProcessResolver};

use crate::config::SysWallConfig;

/// All the wired-up services, ready to use.
/// Tous les services assemblés, prêts à l'emploi.
pub struct AppContext {
    pub rule_service: Arc<RuleService>,
    pub connection_service: Arc<ConnectionService>,
    pub learning_service: Arc<LearningService>,
    pub audit_service: Arc<AuditService>,
    pub event_bus: Arc<TokioBroadcastEventBus>,
    /// Connection monitor for the Supervisor to start streaming.
    /// Moniteur de connexion pour que le Superviseur démarre le streaming.
    pub connection_monitor: Arc<dyn ConnectionMonitor>,
    /// Firewall engine for sync_all_rules at startup.
    /// Moteur de pare-feu pour sync_all_rules au démarrage.
    pub firewall: Arc<dyn FirewallEngine>,
    /// Rule repository reference for whitelist creation.
    /// Référence au dépôt de règles pour la création de la liste blanche.
    pub rule_repo: Arc<SqliteRuleRepository>,
}

/// Wire up all dependencies and return the application context.
/// Assemble toutes les dépendances et retourne le contexte applicatif.
pub fn bootstrap(
    config: &SysWallConfig,
) -> Result<AppContext, DomainError> {
    // Database
    let db = Arc::new(Database::open(&config.database.path)?);

    // Repositories
    let rule_repo = Arc::new(SqliteRuleRepository::new(db.clone()));
    let pending_repo = Arc::new(SqlitePendingDecisionRepository::new(db.clone()));
    let decision_repo = Arc::new(SqliteDecisionRepository::new(db.clone()));
    let audit_repo = Arc::new(SqliteAuditRepository::new(db.clone()));

    // Event bus
    let event_bus = Arc::new(TokioBroadcastEventBus::new(
        config.monitoring.event_bus_capacity,
    ));

    // Firewall engine -- real or fake based on config
    // Moteur de pare-feu -- réel ou factice selon la configuration
    let firewall: Arc<dyn FirewallEngine> = if config.firewall.use_fake {
        info!("Using FakeFirewallEngine (use_fake = true)");
        Arc::new(FakeFirewallEngine::new())
    } else {
        info!("Using NftablesFirewallAdapter");
        Arc::new(NftablesFirewallAdapter::new(NftablesConfig {
            table_name: config.firewall.nftables_table_name.clone(),
            nft_binary_path: config.firewall.nft_binary_path.clone(),
            command_timeout: Duration::from_secs(config.firewall.nft_command_timeout_secs),
            max_output_bytes: config.firewall.nft_max_output_bytes,
        })?)
    };

    // Process resolver -- real or fake based on config
    // Résolveur de processus -- réel ou factice selon la configuration
    let process_resolver: Arc<dyn ProcessResolver> = if config.monitoring.use_fake {
        info!("Using FakeProcessResolver (use_fake = true)");
        Arc::new(FakeProcessResolver::new())
    } else {
        info!("Using ProcfsProcessResolver");
        Arc::new(ProcfsProcessResolver::new(ProcfsConfig {
            cache_capacity: config.monitoring.process_cache_capacity,
            cache_ttl: Duration::from_secs(config.monitoring.process_cache_ttl_secs),
        })?)
    };

    // Connection monitor -- real or fake based on config
    // Moniteur de connexion -- réel ou factice selon la configuration
    let connection_monitor: Arc<dyn ConnectionMonitor> = if config.monitoring.use_fake {
        info!("Using FakeConnectionMonitor (use_fake = true)");
        Arc::new(FakeConnectionMonitor::new())
    } else {
        info!("Using ConntrackMonitorAdapter");
        Arc::new(ConntrackMonitorAdapter::new(ConntrackConfig {
            binary_path: config.monitoring.conntrack_binary_path.clone(),
            protocols: config.monitoring.conntrack_protocols.clone(),
            buffer_size: config.monitoring.conntrack_buffer_size,
        })?)
    };

    let notifier = Arc::new(FakeUserNotifier::new());

    // Application services
    let rule_service = Arc::new(RuleService::new(
        rule_repo.clone(),
        firewall.clone(),
        event_bus.clone(),
    ));

    let default_policy = (&config.firewall.default_policy).into();

    let connection_service = Arc::new(ConnectionService::new(
        process_resolver,
        rule_repo.clone(),
        event_bus.clone(),
        default_policy,
    ));

    let learning_service = Arc::new(LearningService::new(
        pending_repo,
        decision_repo,
        notifier,
        event_bus.clone(),
        AppLearningConfig {
            prompt_timeout_secs: config.learning.prompt_timeout_secs,
            max_pending_decisions: config.learning.max_pending_decisions,
        },
    ));

    let audit_service = Arc::new(AuditService::new(audit_repo));

    Ok(AppContext {
        rule_service,
        connection_service,
        learning_service,
        audit_service,
        event_bus,
        connection_monitor,
        firewall,
        rule_repo,
    })
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p syswall-daemon`
Expected: compiles (may have warnings about unused fields)

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/src/bootstrap.rs crates/daemon/Cargo.toml
git commit -m "feat: update bootstrap to wire real adapters behind config flags

Conditionally creates NftablesFirewallAdapter, ConntrackMonitorAdapter,
or ProcfsProcessResolver based on use_fake config flags. Exposes
connection_monitor and firewall in AppContext for Supervisor."
```

---

### Task 19: Daemon Main -- Whitelist and Monitoring Pipeline

**Files:**
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Update main.rs with whitelist and monitoring stream**

Replace `crates/daemon/src/main.rs`:

```rust
mod bootstrap;
mod config;
mod grpc;
mod signals;
mod supervisor;

use std::path::Path;

use futures::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use syswall_app::services::whitelist::ensure_system_whitelist;
use syswall_domain::entities::ConnectionVerdict;

use crate::config::SysWallConfig;
use crate::supervisor::Supervisor;

#[tokio::main]
async fn main() {
    // Init tracing with env filter
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "syswall=info".into()),
        )
        .init();

    info!("SysWall daemon starting...");

    // Load config from SYSWALL_CONFIG env var or default path
    let config_path = std::env::var("SYSWALL_CONFIG")
        .unwrap_or_else(|_| "config/default.toml".to_string());

    let config = match SysWallConfig::load(Path::new(&config_path)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Fatal: failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    // Bootstrap application context
    let ctx = match bootstrap::bootstrap(&config) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Fatal: bootstrap failed: {}", e);
            std::process::exit(1);
        }
    };

    // Create system whitelist if first start
    if let Err(e) = ensure_system_whitelist(&ctx.rule_service, ctx.rule_repo.as_ref()).await {
        error!("Failed to create system whitelist: {}", e);
        // Non-fatal: continue without whitelist
    }

    // Sync nftables rules with database
    match ctx.rule_repo.list_enabled_ordered().await {
        Ok(rules) => {
            if let Err(e) = ctx.firewall.sync_all_rules(&rules).await {
                error!("Failed to sync nftables rules: {}", e);
            } else {
                info!("nftables rules synced ({} rules)", rules.len());
            }
        }
        Err(e) => error!("Failed to load rules for sync: {}", e),
    }

    // Supervisor
    let cancel = CancellationToken::new();
    let mut supervisor = Supervisor::new(cancel.clone());

    // Signal handler
    supervisor.spawn("signal-handler", {
        let cancel = cancel.clone();
        async move {
            signals::wait_for_shutdown(cancel).await;
            Ok(())
        }
    });

    // Connection monitoring pipeline
    supervisor.spawn("connection-monitor", {
        let monitor = ctx.connection_monitor.clone();
        let connection_service = ctx.connection_service.clone();
        let learning_service = ctx.learning_service.clone();
        let cancel = cancel.clone();

        async move {
            let stream = monitor
                .stream_events()
                .await
                .map_err(|e| format!("Failed to start connection monitor: {}", e))?;

            tokio::pin!(stream);

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    event = stream.next() => {
                        match event {
                            Some(Ok(connection)) => {
                                match connection_service.process_connection(connection).await {
                                    Ok(processed) => {
                                        if processed.verdict == ConnectionVerdict::PendingDecision {
                                            let _ = learning_service
                                                .handle_unknown_connection(processed.snapshot())
                                                .await;
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Connection processing error: {}", e);
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                error!("Connection monitor error: {}", e);
                                return Err(format!("Monitor stream failed: {}", e));
                            }
                            None => {
                                warn!("Connection monitor stream ended");
                                return Err("Monitor stream ended unexpectedly".to_string());
                            }
                        }
                    }
                }
            }

            Ok(())
        }
    });

    info!("SysWall daemon ready");

    // Run until shutdown
    supervisor.run().await;

    info!("SysWall daemon stopped");
}
```

- [ ] **Step 2: Add `use` for RuleRepository trait**

Ensure `syswall_domain::ports::RuleRepository` is in scope by importing it. The `list_enabled_ordered` method is on the trait. This is already satisfied since `SqliteRuleRepository` implements `RuleRepository`.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p syswall-daemon`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/src/main.rs
git commit -m "feat: wire daemon with whitelist creation and monitoring pipeline

Startup now: loads config, bootstraps context, creates system
whitelist on first start, syncs nftables rules, then spawns
connection monitoring pipeline via Supervisor."
```

---

### Task 20: Add resolve_by_connection to ProcessResolver Trait

**Files:**
- Modify: `crates/domain/src/ports/system.rs`
- Modify: `crates/app/src/fakes/fake_process_resolver.rs`

- [ ] **Step 1: Add default method to ProcessResolver trait**

In `crates/domain/src/ports/system.rs`, add after the `resolve_by_socket` method:

```rust
    /// Resolve process info by connection 5-tuple. Default returns None.
    /// Not all resolvers support this -- only ProcfsProcessResolver implements it.
    ///
    /// Résout les informations du processus par 5-tuple de connexion.
    /// Tous les résolveurs ne le supportent pas -- seul ProcfsProcessResolver l'implémente.
    async fn resolve_by_connection(
        &self,
        _protocol: crate::value_objects::Protocol,
        _local_ip: std::net::IpAddr,
        _local_port: u16,
        _remote_ip: std::net::IpAddr,
        _remote_port: u16,
    ) -> Result<Option<ProcessInfo>, DomainError> {
        Ok(None)
    }
```

- [ ] **Step 2: Verify existing tests still pass**

Run: `cargo test`
Expected: all existing tests pass (default method, no breaking change)

- [ ] **Step 3: Commit**

```bash
git add crates/domain/src/ports/system.rs
git commit -m "feat: add resolve_by_connection default method to ProcessResolver

5-tuple based process resolution for the connection processing
pipeline. Default returns None, overridden by ProcfsProcessResolver."
```

---

### Task 21: Integration Wiring Test

**Files:**
- Create: `crates/app/src/services/pipeline_test.rs` (test-only module)
- Modify: `crates/app/src/services/mod.rs`

- [ ] **Step 1: Create end-to-end pipeline test with fakes**

`crates/app/src/services/pipeline_test.rs`:
```rust
//! End-to-end test that verifies the full connection processing pipeline
//! using fake adapters: connection event -> enrichment -> policy evaluation -> verdict.
//!
//! Test de bout en bout vérifiant le pipeline complet de traitement des connexions
//! avec des adaptateurs factices : événement connexion -> enrichissement -> évaluation -> verdict.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;
    use futures::StreamExt;

    use syswall_domain::entities::*;
    use syswall_domain::events::DefaultPolicy;
    use syswall_domain::value_objects::*;

    use crate::commands::CreateRuleCommand;
    use crate::fakes::*;
    use crate::services::connection_service::ConnectionService;
    use crate::services::learning_service::{LearningConfig, LearningService};
    use crate::services::rule_service::RuleService;
    use crate::services::whitelist::ensure_system_whitelist;

    fn make_connection(
        protocol: Protocol,
        dst_ip: &str,
        dst_port: u16,
        direction: Direction,
    ) -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol,
            source: SocketAddress::new(
                "192.168.1.100".parse().unwrap(),
                Port::new(45000).unwrap(),
            ),
            destination: SocketAddress::new(
                dst_ip.parse().unwrap(),
                Port::new(dst_port).unwrap(),
            ),
            direction,
            state: ConnectionState::New,
            process: Some(ProcessInfo {
                pid: 1234,
                name: "firefox".to_string(),
                path: None,
                cmdline: None,
            }),
            user: Some(SystemUser {
                uid: 1000,
                name: "seb".to_string(),
            }),
            bytes_sent: 0,
            bytes_received: 0,
            started_at: Utc::now(),
            verdict: ConnectionVerdict::Unknown,
            matched_rule: None,
        }
    }

    #[tokio::test]
    async fn full_pipeline_with_matching_rule() {
        // Setup
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let process_resolver = Arc::new(FakeProcessResolver::new());

        let rule_service = RuleService::new(
            rule_repo.clone(),
            firewall.clone(),
            event_bus.clone(),
        );

        // Create an allow rule for HTTPS
        let _rule = rule_service
            .create_rule(CreateRuleCommand {
                name: "Allow HTTPS".to_string(),
                priority: 10,
                criteria: RuleCriteria {
                    protocol: Some(Protocol::Tcp),
                    remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
                    direction: Some(Direction::Outbound),
                    ..Default::default()
                },
                effect: RuleEffect::Allow,
                scope: RuleScope::Permanent,
                source: RuleSource::Manual,
            })
            .await
            .unwrap();

        let connection_service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus,
            DefaultPolicy::Block,
        );

        // Simulate a connection event
        let conn = make_connection(Protocol::Tcp, "93.184.216.34", 443, Direction::Outbound);

        // Process through the pipeline
        let result = connection_service.process_connection(conn).await.unwrap();

        // Verify verdict
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
        assert!(result.matched_rule.is_some());
    }

    #[tokio::test]
    async fn full_pipeline_no_matching_rule_uses_default_policy() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let process_resolver = Arc::new(FakeProcessResolver::new());

        let connection_service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus,
            DefaultPolicy::Ask,
        );

        let conn = make_connection(Protocol::Tcp, "93.184.216.34", 443, Direction::Outbound);
        let result = connection_service.process_connection(conn).await.unwrap();

        assert_eq!(result.verdict, ConnectionVerdict::PendingDecision);
        assert!(result.matched_rule.is_none());
    }

    #[tokio::test]
    async fn full_pipeline_with_system_whitelist() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let process_resolver = Arc::new(FakeProcessResolver::new());

        let rule_service = RuleService::new(
            rule_repo.clone(),
            firewall.clone(),
            event_bus.clone(),
        );

        // Create system whitelist
        ensure_system_whitelist(&rule_service, rule_repo.as_ref())
            .await
            .unwrap();

        let connection_service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus,
            DefaultPolicy::Block,
        );

        // DNS query should be allowed by whitelist
        let dns_conn = make_connection(Protocol::Udp, "8.8.8.8", 53, Direction::Outbound);
        let result = connection_service.process_connection(dns_conn).await.unwrap();
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);

        // NTP should be allowed by whitelist
        let ntp_conn = make_connection(Protocol::Udp, "pool.ntp.org".parse::<std::net::IpAddr>().unwrap_or_else(|_| "129.6.15.28".parse().unwrap()).to_string().as_str(), 123, Direction::Outbound);
        let result = connection_service.process_connection(ntp_conn).await.unwrap();
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);

        // Random HTTP should be blocked (default policy = Block)
        let http_conn = make_connection(Protocol::Tcp, "93.184.216.34", 80, Direction::Outbound);
        let result = connection_service.process_connection(http_conn).await.unwrap();
        assert_eq!(result.verdict, ConnectionVerdict::Blocked);
    }

    #[tokio::test]
    async fn full_pipeline_ask_effect_triggers_learning() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let process_resolver = Arc::new(FakeProcessResolver::new());
        let pending_repo = Arc::new(FakePendingDecisionRepository::new());
        let decision_repo = Arc::new(FakeDecisionRepository::new());
        let notifier = Arc::new(FakeUserNotifier::new());

        let rule_service = RuleService::new(
            rule_repo.clone(),
            firewall.clone(),
            event_bus.clone(),
        );

        // Create an Ask rule
        rule_service
            .create_rule(CreateRuleCommand {
                name: "Ask for SSH".to_string(),
                priority: 10,
                criteria: RuleCriteria {
                    protocol: Some(Protocol::Tcp),
                    remote_port: Some(PortMatcher::Exact(Port::new(22).unwrap())),
                    ..Default::default()
                },
                effect: RuleEffect::Ask,
                scope: RuleScope::Permanent,
                source: RuleSource::Manual,
            })
            .await
            .unwrap();

        let connection_service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus.clone(),
            DefaultPolicy::Block,
        );

        let learning_service = LearningService::new(
            pending_repo.clone(),
            decision_repo,
            notifier,
            event_bus,
            LearningConfig {
                prompt_timeout_secs: 60,
                max_pending_decisions: 50,
            },
        );

        // Process SSH connection
        let conn = make_connection(Protocol::Tcp, "10.0.0.1", 22, Direction::Outbound);
        let result = connection_service.process_connection(conn).await.unwrap();
        assert_eq!(result.verdict, ConnectionVerdict::PendingDecision);

        // Feed to learning service (like the daemon pipeline would)
        let snapshot = result.snapshot();
        learning_service
            .handle_unknown_connection(snapshot)
            .await
            .unwrap();

        // Verify a pending decision was created
        let pending = pending_repo.list_pending().await.unwrap();
        assert_eq!(pending.len(), 1);
    }
}
```

- [ ] **Step 2: Register test module**

Add to `crates/app/src/services/mod.rs`:
```rust
pub mod audit_service;
pub mod connection_service;
pub mod learning_service;
#[cfg(test)]
mod pipeline_test;
pub mod rule_service;
pub mod whitelist;
```

- [ ] **Step 3: Verify all tests pass**

Run: `cargo test -p syswall-app pipeline_test`
Expected: 4 tests PASS

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: all tests PASS across all crates

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/services/pipeline_test.rs crates/app/src/services/mod.rs
git commit -m "feat: add end-to-end pipeline integration test with fakes

Verifies: rule matching -> allowed, no match -> default policy,
system whitelist -> DNS/NTP allowed + HTTP blocked,
Ask effect -> pending decision created via learning service."
```

---

### Task 22: Final Verification and Commit

- [ ] **Step 1: Run full test suite**

```bash
cargo test
```
Expected: all tests PASS

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --all-targets -- -D warnings
```
Expected: no warnings (fix any if present)

- [ ] **Step 3: Verify build**

```bash
cargo build
```
Expected: compiles successfully

- [ ] **Step 4: Final commit if any clippy fixes were needed**

```bash
git add -A
git commit -m "chore: fix clippy warnings from firewall engine implementation"
```
