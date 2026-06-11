# Phase 1 — Fuzzing + Property Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Construire le filet de sécurité du sous-projet ① : property tests sur les invariants du `PolicyEngine` et cibles cargo-fuzz sur les parsers qui survivront à la migration eBPF (config TOML, JSON critères/scope, converters gRPC), avec job CI fuzz-smoke.

**Architecture:** proptest en dev-dependency du crate `domain` (tests d'intégration `crates/domain/tests/`). Deux crates fuzz hors workspace (`crates/domain/fuzz`, `crates/daemon/fuzz`) via cargo-fuzz/libFuzzer (nightly). Le daemon gagne un `src/lib.rs` pour exposer `config` et `grpc::converters` aux cibles fuzz.

**Tech Stack:** proptest 1.x, cargo-fuzz 0.12+/libfuzzer-sys 0.4, arbitrary 1.x (derive), toolchain nightly (fuzz uniquement), GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-06-11-robustesse-performance-design.md` (section Phase 1). Les phases 2-4 auront leurs propres plans.

**Note TDD :** les property tests ciblent du code *existant* — ce sont des tests de caractérisation. Le cycle rouge→vert ne s'applique pas : on écrit la propriété, on l'exécute ; si elle échoue, on a trouvé un vrai bug qu'on corrige (et le test devient la non-régression). Chaque échec de propriété doit être investigué, jamais contourné en affaiblissant la propriété sans justification écrite.

**Environnement :** rustup avec `stable` + `nightly` installés, `cargo-fuzz` déjà présent (`~/.cargo/bin/cargo-fuzz`). Les commandes fuzz utilisent `cargo +nightly fuzz`.

---

## File Structure

| Fichier | Responsabilité |
|---|---|
| `Cargo.toml` (racine) | + `proptest` dans `[workspace.dependencies]`, + exclusions `crates/domain/fuzz`, `crates/daemon/fuzz` |
| `crates/domain/Cargo.toml` | + dev-dependency `proptest` |
| `crates/domain/tests/policy_engine_proptest.rs` | Stratégies proptest + propriétés du PolicyEngine (un seul fichier : stratégies et propriétés changent ensemble) |
| `crates/domain/fuzz/` | Crate cargo-fuzz : cible `fuzz_rule_criteria_json` |
| `crates/daemon/src/lib.rs` | **Créé** : expose `config`, `grpc`, etc. (le binaire `main.rs` consomme la lib) |
| `crates/daemon/src/main.rs` | Modifié : supprime les `mod`, importe depuis `syswall_daemon::` |
| `crates/daemon/fuzz/` | Crate cargo-fuzz : cibles `fuzz_config_toml`, `fuzz_create_rule_cmd` |
| `.github/workflows/ci.yml` | + job `fuzz-smoke` (nightly, 60 s/cible) |
| `CHANGELOG.md` | Section V0.3 : entrées fuzzing/proptest |

---

### Task 1: proptest en dépendance + première propriété (politique par défaut)

**Files:**
- Modify: `Cargo.toml` (racine, `[workspace.dependencies]`)
- Modify: `crates/domain/Cargo.toml` (`[dev-dependencies]`)
- Create: `crates/domain/tests/policy_engine_proptest.rs`

- [ ] **Step 1: Ajouter proptest aux dépendances workspace**

Dans `Cargo.toml` racine, section `[workspace.dependencies]`, ajouter après `etherparse = "0.20"` :

```toml
proptest = "1"
```

Dans `crates/domain/Cargo.toml`, section `[dev-dependencies]` :

```toml
[dev-dependencies]
tokio = { workspace = true }
proptest = { workspace = true }
```

- [ ] **Step 2: Écrire le fichier de stratégies + première propriété**

Créer `crates/domain/tests/policy_engine_proptest.rs` :

```rust
//! Property tests du PolicyEngine : invariants vérifiés sur entrées arbitraires.
//! PolicyEngine property tests: invariants checked against arbitrary inputs.

use chrono::{Duration, Utc};
use proptest::prelude::*;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;

use syswall_domain::entities::*;
use syswall_domain::events::DefaultPolicy;
use syswall_domain::services::PolicyEngine;
use syswall_domain::value_objects::*;

// --- Stratégies / Strategies ---

fn arb_ip() -> impl Strategy<Value = IpAddr> {
    prop_oneof![
        any::<[u8; 4]>().prop_map(|b| IpAddr::V4(Ipv4Addr::from(b))),
        any::<[u8; 16]>().prop_map(|b| IpAddr::V6(Ipv6Addr::from(b))),
    ]
}

fn arb_port() -> impl Strategy<Value = Port> {
    (1u16..=u16::MAX).prop_map(|p| Port::new(p).expect("port non nul par construction"))
}

fn arb_protocol() -> impl Strategy<Value = Protocol> {
    prop_oneof![
        Just(Protocol::Tcp),
        Just(Protocol::Udp),
        Just(Protocol::Icmp),
        any::<u8>().prop_map(Protocol::Other),
    ]
}

fn arb_direction() -> impl Strategy<Value = Direction> {
    prop_oneof![Just(Direction::Inbound), Just(Direction::Outbound)]
}

fn arb_socket_address() -> impl Strategy<Value = SocketAddress> {
    (arb_ip(), arb_port()).prop_map(|(ip, port)| SocketAddress::new(ip, port))
}

fn arb_process() -> impl Strategy<Value = Option<ProcessInfo>> {
    proptest::option::of("[a-z]{1,12}".prop_map(|name| ProcessInfo {
        pid: 1234,
        name,
        path: Some(
            ExecutablePath::new(PathBuf::from("/usr/bin/app")).expect("chemin absolu valide"),
        ),
        cmdline: None,
        icon: None,
    }))
}

fn arb_connection() -> impl Strategy<Value = Connection> {
    (
        arb_protocol(),
        arb_socket_address(),
        arb_socket_address(),
        arb_direction(),
        arb_process(),
    )
        .prop_map(|(protocol, source, destination, direction, process)| Connection {
            id: ConnectionId::new(),
            protocol,
            source,
            destination,
            direction,
            state: ConnectionState::New,
            process,
            user: None,
            bytes_sent: 0,
            bytes_received: 0,
            started_at: Utc::now(),
            verdict: ConnectionVerdict::Unknown,
            matched_rule: None,
            remote_hostname: None,
        })
}

fn arb_policy() -> impl Strategy<Value = DefaultPolicy> {
    prop_oneof![
        Just(DefaultPolicy::Ask),
        Just(DefaultPolicy::Allow),
        Just(DefaultPolicy::Block),
    ]
}

// --- Propriétés / Properties ---

proptest! {
    /// Sans règle, le verdict découle uniquement de la politique par défaut.
    /// With no rules, the verdict derives solely from the default policy.
    #[test]
    fn empty_rules_apply_default_policy(conn in arb_connection(), policy in arb_policy()) {
        let eval = PolicyEngine::evaluate(&conn, &[], policy);
        let expected = match policy {
            DefaultPolicy::Ask => ConnectionVerdict::PendingDecision,
            DefaultPolicy::Allow => ConnectionVerdict::Allowed,
            DefaultPolicy::Block => ConnectionVerdict::Blocked,
        };
        prop_assert_eq!(eval.verdict, expected);
        prop_assert!(eval.matched_rule_id.is_none());
    }
}
```

- [ ] **Step 3: Exécuter et vérifier que la propriété passe**

Run: `cargo test -p syswall-domain --test policy_engine_proptest`
Expected: `1 passed` (256 cas générés par défaut). Si échec : investiguer le contre-exemple minimal affiché par proptest — c'est un bug réel.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/domain/Cargo.toml crates/domain/tests/policy_engine_proptest.rs
git commit -m "test(domain): proptest + première propriété PolicyEngine (politique par défaut)"
```

---

### Task 2: Propriétés de cohérence du matching

**Files:**
- Modify: `crates/domain/tests/policy_engine_proptest.rs`

- [ ] **Step 1: Ajouter les stratégies règles**

Ajouter dans la section stratégies (après `arb_policy`) :

```rust
fn arb_ip_matcher() -> impl Strategy<Value = IpMatcher> {
    prop_oneof![
        arb_ip().prop_map(IpMatcher::Exact),
        (arb_ip(), 0u8..=128).prop_map(|(network, prefix_len)| IpMatcher::Cidr {
            network,
            prefix_len,
        }),
        (arb_ip(), arb_ip()).prop_map(|(start, end)| IpMatcher::Range { start, end }),
    ]
}

fn arb_port_matcher() -> impl Strategy<Value = PortMatcher> {
    prop_oneof![
        arb_port().prop_map(PortMatcher::Exact),
        (arb_port(), arb_port()).prop_map(|(a, b)| {
            let (start, end) = if a.value() <= b.value() { (a, b) } else { (b, a) };
            PortMatcher::Range { start, end }
        }),
    ]
}

fn arb_criteria() -> impl Strategy<Value = RuleCriteria> {
    (
        proptest::option::of(arb_ip_matcher()),
        proptest::option::of(arb_port_matcher()),
        proptest::option::of(arb_protocol()),
        proptest::option::of(arb_direction()),
    )
        .prop_map(|(remote_ip, remote_port, protocol, direction)| RuleCriteria {
            application: None,
            user: None,
            remote_ip,
            remote_port,
            local_port: None,
            protocol,
            direction,
            schedule: None,
        })
}

fn arb_effect() -> impl Strategy<Value = RuleEffect> {
    prop_oneof![
        Just(RuleEffect::Allow),
        Just(RuleEffect::Block),
        Just(RuleEffect::Ask),
        Just(RuleEffect::Observe),
    ]
}

fn arb_rule() -> impl Strategy<Value = Rule> {
    (0u32..1000, any::<bool>(), arb_criteria(), arb_effect(), any::<bool>()).prop_map(
        |(priority, enabled, criteria, effect, expired)| Rule {
            id: RuleId::new(),
            name: "prop rule".to_string(),
            priority: RulePriority::new(priority),
            enabled,
            criteria,
            effect,
            scope: if expired {
                RuleScope::Temporary {
                    expires_at: Utc::now() - Duration::hours(1),
                }
            } else {
                RuleScope::Permanent
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        },
    )
}
```

- [ ] **Step 2: Ajouter les propriétés de cohérence**

Ajouter dans le bloc `proptest!` :

```rust
    /// evaluate ne panique jamais, quelles que soient les entrées.
    /// evaluate never panics, whatever the inputs.
    #[test]
    fn evaluate_never_panics(
        conn in arb_connection(),
        mut rules in proptest::collection::vec(arb_rule(), 0..20),
        policy in arb_policy(),
    ) {
        rules.sort_by_key(|r| r.priority);
        let _ = PolicyEngine::evaluate(&conn, &rules, policy);
    }

    /// Une règle matchée est toujours active, non expirée, et matche réellement.
    /// A matched rule is always enabled, not expired, and actually matches.
    #[test]
    fn matched_rule_is_enabled_not_expired_and_matches(
        conn in arb_connection(),
        mut rules in proptest::collection::vec(arb_rule(), 0..20),
        policy in arb_policy(),
    ) {
        rules.sort_by_key(|r| r.priority);
        let eval = PolicyEngine::evaluate(&conn, &rules, policy);
        if let Some(id) = eval.matched_rule_id {
            let rule = rules.iter().find(|r| r.id == id)
                .expect("matched_rule_id référence une règle de la liste");
            prop_assert!(rule.enabled);
            prop_assert!(!rule.is_expired());
            prop_assert!(PolicyEngine::matches(&rule.criteria, &conn));
        }
    }
```

- [ ] **Step 3: Exécuter**

Run: `cargo test -p syswall-domain --test policy_engine_proptest`
Expected: `3 passed`. Tout échec = bug réel à corriger avant de poursuivre (corriger le moteur, pas la propriété).

- [ ] **Step 4: Commit**

```bash
git add crates/domain/tests/policy_engine_proptest.rs
git commit -m "test(domain): propriétés de cohérence du matching (no-panic, règle matchée valide)"
```

---

### Task 3: Propriétés first-match-wins et familles IP

**Files:**
- Modify: `crates/domain/tests/policy_engine_proptest.rs`

- [ ] **Step 1: Ajouter une stratégie de règles décisives (Allow/Block uniquement)**

`RuleEffect::Observe` et `Ask` ont une sémantique propre ; la propriété d'ordre se vérifie sur les effets décisifs :

```rust
fn arb_decisive_rule() -> impl Strategy<Value = Rule> {
    (
        0u32..1000,
        arb_criteria(),
        prop_oneof![Just(RuleEffect::Allow), Just(RuleEffect::Block)],
    )
        .prop_map(|(priority, criteria, effect)| Rule {
            id: RuleId::new(),
            name: "decisive rule".to_string(),
            priority: RulePriority::new(priority),
            enabled: true,
            criteria,
            effect,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        })
}
```

- [ ] **Step 2: Ajouter les propriétés**

Dans le bloc `proptest!` :

```rust
    /// La première règle décisive (triée par priorité) qui matche gagne.
    /// The first matching decisive rule (sorted by priority) wins.
    #[test]
    fn first_matching_decisive_rule_wins(
        conn in arb_connection(),
        mut rules in proptest::collection::vec(arb_decisive_rule(), 0..20),
    ) {
        rules.sort_by_key(|r| r.priority);
        let expected = rules
            .iter()
            .find(|r| PolicyEngine::matches(&r.criteria, &conn))
            .map(|r| r.id);
        let eval = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Block);
        prop_assert_eq!(eval.matched_rule_id, expected);
    }

    /// Un matcher IP d'une famille ne matche jamais une IP de l'autre famille.
    /// An IP matcher of one family never matches an IP of the other family.
    #[test]
    fn ip_family_mismatch_never_matches(v4 in any::<[u8; 4]>(), v6 in any::<[u8; 16]>()) {
        let matcher_v4 = IpMatcher::Exact(IpAddr::V4(Ipv4Addr::from(v4)));
        prop_assert!(!PolicyEngine::matches(
            &RuleCriteria {
                remote_ip: Some(matcher_v4),
                ..RuleCriteria::default()
            },
            &connection_to(IpAddr::V6(Ipv6Addr::from(v6))),
        ));
        let matcher_v6 = IpMatcher::Cidr {
            network: IpAddr::V6(Ipv6Addr::from(v6)),
            prefix_len: 64,
        };
        prop_assert!(!PolicyEngine::matches(
            &RuleCriteria {
                remote_ip: Some(matcher_v6),
                ..RuleCriteria::default()
            },
            &connection_to(IpAddr::V4(Ipv4Addr::from(v4))),
        ));
    }

    /// Les bornes d'un PortMatcher::Range sont inclusives.
    /// PortMatcher::Range bounds are inclusive.
    #[test]
    fn port_range_bounds_inclusive(start in 1u16..=u16::MAX, end in 1u16..=u16::MAX) {
        let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
        let matcher = PortMatcher::Range {
            start: Port::new(lo).expect("port non nul"),
            end: Port::new(hi).expect("port non nul"),
        };
        let criteria = RuleCriteria {
            remote_port: Some(matcher),
            ..RuleCriteria::default()
        };
        prop_assert!(PolicyEngine::matches(&criteria, &connection_to_port(lo)));
        prop_assert!(PolicyEngine::matches(&criteria, &connection_to_port(hi)));
    }
```

Et les helpers (hors bloc `proptest!`, après les stratégies) :

```rust
/// Connexion sortante fixe vers l'IP donnée (port 443).
/// Fixed outbound connection to the given IP (port 443).
fn connection_to(ip: IpAddr) -> Connection {
    Connection {
        id: ConnectionId::new(),
        protocol: Protocol::Tcp,
        source: SocketAddress::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)),
            Port::new(45000).expect("port non nul"),
        ),
        destination: SocketAddress::new(ip, Port::new(443).expect("port non nul")),
        direction: Direction::Outbound,
        state: ConnectionState::New,
        process: None,
        user: None,
        bytes_sent: 0,
        bytes_received: 0,
        started_at: Utc::now(),
        verdict: ConnectionVerdict::Unknown,
        matched_rule: None,
        remote_hostname: None,
    }
}

/// Connexion sortante fixe vers le port distant donné.
/// Fixed outbound connection to the given remote port.
fn connection_to_port(port: u16) -> Connection {
    let mut conn = connection_to(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)));
    conn.destination.port = Port::new(port).expect("port non nul");
    conn
}
```

- [ ] **Step 3: Exécuter**

Run: `cargo test -p syswall-domain --test policy_engine_proptest`
Expected: `6 passed`. Si `first_matching_decisive_rule_wins` échoue, vérifier d'abord si le contre-exemple révèle un écart entre `matches` public et la boucle interne d'`evaluate` (bug réel) avant tout ajustement.

- [ ] **Step 4: Vérifier la suite complète + clippy**

Run: `cargo test -p syswall-domain && cargo clippy -p syswall-domain --all-targets -- -D warnings`
Expected: tout passe.

- [ ] **Step 5: Commit**

```bash
git add crates/domain/tests/policy_engine_proptest.rs
git commit -m "test(domain): propriétés first-match-wins, familles IP et bornes de ports"
```

---

### Task 4: lib.rs daemon (exposition pour fuzz)

**Files:**
- Create: `crates/daemon/src/lib.rs`
- Modify: `crates/daemon/src/main.rs`
- Modify (si nécessaire): `crates/daemon/src/grpc/mod.rs`

- [ ] **Step 1: Créer la lib**

Créer `crates/daemon/src/lib.rs` :

```rust
//! Bibliothèque du daemon SysWall : exposée pour les tests d'intégration et le fuzzing.
//! SysWall daemon library: exposed for integration tests and fuzzing.

#![cfg_attr(not(test), deny(clippy::unwrap_used))]

pub mod bootstrap;
pub mod config;
pub mod grpc;
pub mod signals;
pub mod startup_error;
pub mod supervisor;
pub mod watchdog;
```

- [ ] **Step 2: Adapter main.rs**

Dans `crates/daemon/src/main.rs` : supprimer toutes les déclarations `mod xxx;` et l'attribut `#![cfg_attr(not(test), deny(clippy::unwrap_used))]` s'il est dupliqué, remplacer les `use crate::xxx` par `use syswall_daemon::xxx`. Le corps de `main`/`init_tracing` ne change pas.

- [ ] **Step 3: Vérifier la visibilité des converters**

Run: `grep -n 'mod converters' crates/daemon/src/grpc/mod.rs && grep -n 'pub use\|pub fn proto_to_create_rule_cmd' crates/daemon/src/grpc/converters/mod.rs crates/daemon/src/grpc/converters/rule.rs`

Si `converters` n'est pas `pub mod` dans `grpc/mod.rs`, le passer en `pub mod converters;`. Si `proto_to_create_rule_cmd` n'est pas réexporté par `converters/mod.rs`, ajouter dans `converters/mod.rs` :

```rust
pub use rule::proto_to_create_rule_cmd;
```

- [ ] **Step 4: Compiler et tester**

Run: `cargo build -p syswall-daemon && cargo test -p syswall-daemon && cargo clippy -p syswall-daemon --all-targets -- -D warnings`
Expected: build OK, tests OK, zéro warning. Les `pub` de la lib peuvent rendre certains `#[allow(dead_code)]` superflus — les retirer si clippy le signale.

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/lib.rs crates/daemon/src/main.rs crates/daemon/src/grpc/mod.rs crates/daemon/src/grpc/converters/mod.rs
git commit -m "refactor(daemon): expose une lib (config, grpc) pour les tests et le fuzzing"
```

---

### Task 5: Crate fuzz domain (JSON critères/scope)

**Files:**
- Create: `crates/domain/fuzz/Cargo.toml`
- Create: `crates/domain/fuzz/fuzz_targets/fuzz_rule_criteria_json.rs`
- Create: `crates/domain/fuzz/.gitignore`
- Modify: `Cargo.toml` (racine, `exclude`)

- [ ] **Step 1: Exclure les crates fuzz du workspace**

Dans `Cargo.toml` racine :

```toml
exclude = [
    "crates/ebpf-prog",
    "crates/domain/fuzz",
    "crates/daemon/fuzz",
]
```

- [ ] **Step 2: Créer le crate fuzz**

`crates/domain/fuzz/Cargo.toml` :

```toml
[package]
name = "syswall-domain-fuzz"
version = "0.0.0"
publish = false
edition = "2021"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
serde_json = "1"
syswall-domain = { path = ".." }

[[bin]]
name = "fuzz_rule_criteria_json"
path = "fuzz_targets/fuzz_rule_criteria_json.rs"
test = false
doc = false
bench = false

[profile.release]
debug = 1
```

`crates/domain/fuzz/.gitignore` :

```
target/
corpus/
artifacts/
coverage/
```

`crates/domain/fuzz/fuzz_targets/fuzz_rule_criteria_json.rs` :

```rust
//! Fuzz du parsing JSON des critères et scopes de règles (entrées gRPC non fiables).
//! Fuzzing of rule criteria/scope JSON parsing (untrusted gRPC inputs).

#![no_main]

use libfuzzer_sys::fuzz_target;
use syswall_domain::entities::{Rule, RuleCriteria, RuleScope};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<RuleCriteria>(s);
        let _ = serde_json::from_str::<RuleScope>(s);
        let _ = serde_json::from_str::<Rule>(s);
    }
});
```

- [ ] **Step 3: Seeds de corpus**

```bash
mkdir -p crates/domain/fuzz/seeds/fuzz_rule_criteria_json
cat > crates/domain/fuzz/seeds/fuzz_rule_criteria_json/criteria.json <<'EOF'
{"application":{"ByName":"firefox"},"user":null,"remote_ip":{"Cidr":{"network":"10.0.0.0","prefix_len":8}},"remote_port":{"Exact":443},"local_port":null,"protocol":"Tcp","direction":"Outbound","schedule":null}
EOF
cat > crates/domain/fuzz/seeds/fuzz_rule_criteria_json/scope.json <<'EOF'
{"Temporary":{"expires_at":"2026-06-11T12:00:00Z"}}
EOF
```

(Le dossier `seeds/` est versionné, contrairement à `corpus/` qui est généré.)

- [ ] **Step 4: Lancer 60 secondes de fuzz**

Run: `cd crates/domain && cargo +nightly fuzz run fuzz_rule_criteria_json seeds/fuzz_rule_criteria_json -- -max_total_time=60`
Expected: `Done` sans crash. Un crash produit un fichier dans `artifacts/` : le reproduire avec `cargo +nightly fuzz run fuzz_rule_criteria_json artifacts/<fichier>`, corriger le bug dans le code de production, puis relancer.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/domain/fuzz
git commit -m "test(domain): cible cargo-fuzz sur le parsing JSON criteria/scope/rule"
```

---

### Task 6: Crate fuzz daemon (config TOML + CreateRuleRequest)

**Files:**
- Create: `crates/daemon/fuzz/Cargo.toml`
- Create: `crates/daemon/fuzz/fuzz_targets/fuzz_config_toml.rs`
- Create: `crates/daemon/fuzz/fuzz_targets/fuzz_create_rule_cmd.rs`
- Create: `crates/daemon/fuzz/.gitignore`

- [ ] **Step 1: Créer le crate fuzz**

`crates/daemon/fuzz/Cargo.toml` :

```toml
[package]
name = "syswall-daemon-fuzz"
version = "0.0.0"
publish = false
edition = "2021"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
arbitrary = { version = "1", features = ["derive"] }
syswall-daemon = { path = ".." }
syswall-proto = { path = "../../proto" }

[[bin]]
name = "fuzz_config_toml"
path = "fuzz_targets/fuzz_config_toml.rs"
test = false
doc = false
bench = false

[[bin]]
name = "fuzz_create_rule_cmd"
path = "fuzz_targets/fuzz_create_rule_cmd.rs"
test = false
doc = false
bench = false

[profile.release]
debug = 1
```

`crates/daemon/fuzz/.gitignore` : identique à celui du domain (`target/`, `corpus/`, `artifacts/`, `coverage/`).

- [ ] **Step 2: Cible config TOML**

`crates/daemon/fuzz/fuzz_targets/fuzz_config_toml.rs` :

```rust
//! Fuzz du parsing de la configuration TOML du daemon.
//! Fuzzing of the daemon TOML configuration parsing.

#![no_main]

use libfuzzer_sys::fuzz_target;
use syswall_daemon::config::SysWallConfig;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = SysWallConfig::from_toml(s);
    }
});
```

- [ ] **Step 3: Cible converter CreateRuleRequest**

`crates/daemon/fuzz/fuzz_targets/fuzz_create_rule_cmd.rs` :

```rust
//! Fuzz du converter gRPC CreateRuleRequest -> CreateRuleCommand (entrée non fiable).
//! Fuzzing of the gRPC CreateRuleRequest -> CreateRuleCommand converter (untrusted input).

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use syswall_daemon::grpc::converters::proto_to_create_rule_cmd;
use syswall_proto::syswall::CreateRuleRequest;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    name: String,
    priority: u32,
    criteria_json: String,
    scope_json: String,
    effect: String,
    source: String,
}

fuzz_target!(|input: FuzzInput| {
    let req = CreateRuleRequest {
        name: input.name,
        priority: input.priority,
        criteria_json: input.criteria_json,
        scope_json: input.scope_json,
        effect: input.effect,
        source: input.source,
        ..Default::default()
    };
    let _ = proto_to_create_rule_cmd(&req);
});
```

Note : si `CreateRuleRequest` n'a pas exactement ces champs, ajuster aux champs réels du message proto (vérifier avec `grep -n 'message CreateRuleRequest' -A 15 crates/proto/proto/*.proto`) — le `..Default::default()` couvre les champs supplémentaires. Si les six champs couvrent tout le message, retirer `..Default::default()` (clippy `needless_update`).

- [ ] **Step 4: Seed config**

```bash
mkdir -p crates/daemon/fuzz/seeds/fuzz_config_toml
cp system/config.toml crates/daemon/fuzz/seeds/fuzz_config_toml/config.toml 2>/dev/null \
  || cp /etc/syswall/config.toml crates/daemon/fuzz/seeds/fuzz_config_toml/config.toml 2>/dev/null \
  || true
ls crates/daemon/fuzz/seeds/fuzz_config_toml/
```

Si aucun config.toml exemple n'existe dans le repo, créer le seed à partir du bloc TOML de la section Configuration du README.

- [ ] **Step 5: Lancer 60 secondes par cible**

Run:
```bash
cd crates/daemon
cargo +nightly fuzz run fuzz_config_toml seeds/fuzz_config_toml -- -max_total_time=60
cargo +nightly fuzz run fuzz_create_rule_cmd -- -max_total_time=60
```
Expected: `Done` sans crash pour les deux cibles. Même procédure qu'en Task 5 en cas de crash.

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/fuzz
git commit -m "test(daemon): cibles cargo-fuzz config TOML et converter CreateRuleRequest"
```

---

### Task 7: Job CI fuzz-smoke + documentation

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Ajouter le job CI**

Dans `.github/workflows/ci.yml`, après le job `nfqueue-smoke` :

```yaml
  fuzz-smoke:
    name: Fuzz smoke (60s par cible / per target)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - uses: Swatinem/rust-cache@v2
      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz --locked
      - name: Fuzz domain (criteria/scope/rule JSON)
        working-directory: crates/domain
        run: cargo +nightly fuzz run fuzz_rule_criteria_json seeds/fuzz_rule_criteria_json -- -max_total_time=60
      - name: Fuzz daemon (config TOML)
        working-directory: crates/daemon
        run: cargo +nightly fuzz run fuzz_config_toml seeds/fuzz_config_toml -- -max_total_time=60
      - name: Fuzz daemon (CreateRuleRequest)
        working-directory: crates/daemon
        run: cargo +nightly fuzz run fuzz_create_rule_cmd -- -max_total_time=60
```

- [ ] **Step 2: CHANGELOG**

Dans `CHANGELOG.md`, section `## [0.3.0]` → `### Added / Ajoute`, ajouter :

```markdown
- **Property tests PolicyEngine** : 6 invariants vérifiés par proptest (politique par défaut, no-panic, cohérence de la règle matchée, first-match-wins, isolation des familles IP, bornes de ports inclusives) dans `crates/domain/tests/policy_engine_proptest.rs`.
- **Fuzzing cargo-fuzz** : 3 cibles libFuzzer sur les surfaces d'entrée non fiables — JSON criteria/scope/rule (`crates/domain/fuzz`), config TOML et converter gRPC `CreateRuleRequest` (`crates/daemon/fuzz`). Job CI `fuzz-smoke` (60 s/cible, nightly).
- **Lib daemon** : `crates/daemon/src/lib.rs` expose `config` et `grpc` pour les tests d'intégration et le fuzzing (le binaire reste inchangé).
```

- [ ] **Step 3: Vérification finale complète**

Run: `cargo test --workspace && cargo clippy --workspace --exclude ui --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: tout passe.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml CHANGELOG.md
git commit -m "ci: job fuzz-smoke (3 cibles, 60s) + changelog fuzzing/proptest"
```

---

## Self-review (fait à la rédaction)

- **Couverture spec Phase 1** : proptest PolicyEngine ✓ (Tasks 1-3), fuzz config TOML ✓ (Task 6), fuzz converters gRPC ✓ (Task 6), fuzz JSON domain ✓ (Task 5), job CI 60 s/cible ✓ (Task 7), corpus versionné ✓ (seeds/). Le parser NFQUEUE n'est volontairement pas fuzzé (condamné par la Phase 3).
- **Types vérifiés contre le code réel** : `Rule`, `RuleCriteria`, `Connection`, `Port::new`, `RulePriority` (Ord), `PolicyEngine::{evaluate, matches}`, `SysWallConfig::from_toml` — signatures relevées dans le code source le 2026-06-11.
- **Point d'attention** : la propriété `first_matching_decisive_rule_wins` suppose que `matches` (public) et la boucle d'`evaluate` sont cohérents ; un échec signale un vrai bug, pas un défaut du test.
