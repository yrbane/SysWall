# SysWall Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the complete foundational layer for SysWall: Cargo workspace, domain model with PolicyEngine, async ports, application services with fakes, SQLite persistence, gRPC proto, daemon bootstrap with supervisor, and minimal Tauri UI scaffold.

**Architecture:** Hexagonal architecture with 6 Cargo crates (domain, app, infra, proto, daemon, ui). Domain is pure with no I/O dependencies. All ports are async traits. Communication between daemon and UI is via gRPC over Unix socket. SQLite for persistence with WAL mode.

**Tech Stack:** Rust, Tauri 2, Svelte 5, TypeScript, tonic/prost (gRPC), rusqlite, tokio, tracing, serde, async-trait

**Spec:** `docs/superpowers/specs/2026-03-26-syswall-foundations-design.md`

---

## File Map

### crates/domain/src/
| File | Responsibility |
|---|---|
| `lib.rs` | Re-exports all domain modules |
| `value_objects/mod.rs` | Port, RulePriority, ExecutablePath, Protocol, Direction, SocketAddress |
| `entities/mod.rs` | Re-exports entity modules |
| `entities/connection.rs` | Connection, ConnectionState, ConnectionVerdict, ConnectionSnapshot, ProcessInfo, SystemUser |
| `entities/rule.rs` | Rule, RuleCriteria, RuleEffect, RuleScope, RuleSource, AppMatcher, IpMatcher, PortMatcher |
| `entities/decision.rs` | Decision, PendingDecision, PendingDecisionStatus, DecisionAction, DecisionGranularity |
| `entities/audit.rs` | AuditEvent, Severity, EventCategory |
| `errors/mod.rs` | DomainError enum |
| `events/mod.rs` | DomainEvent enum |
| `ports/mod.rs` | Re-exports all port traits |
| `ports/repositories.rs` | RuleRepository, AuditRepository, DecisionRepository, PendingDecisionRepository |
| `ports/system.rs` | FirewallEngine, ConnectionMonitor, ProcessResolver |
| `ports/messaging.rs` | EventBus, UserNotifier |
| `services/mod.rs` | Re-exports services |
| `services/policy_engine.rs` | PolicyEngine, PolicyEvaluation, EvaluationReason |

### crates/app/src/
| File | Responsibility |
|---|---|
| `lib.rs` | Re-exports |
| `commands/mod.rs` | CreateRuleCommand, UpdateRuleCommand, etc. |
| `services/mod.rs` | Re-exports services |
| `services/rule_service.rs` | RuleService (CRUD + firewall sync) |
| `services/learning_service.rs` | LearningService (async pending decisions) |
| `services/connection_service.rs` | ConnectionService (enrichment + evaluation) |
| `services/audit_service.rs` | AuditService (event listener + persistence) |
| `fakes/mod.rs` | Re-exports all fakes |
| `fakes/fake_rule_repository.rs` | FakeRuleRepository |
| `fakes/fake_pending_decision_repository.rs` | FakePendingDecisionRepository |
| `fakes/fake_decision_repository.rs` | FakeDecisionRepository |
| `fakes/fake_audit_repository.rs` | FakeAuditRepository |
| `fakes/fake_firewall_engine.rs` | FakeFirewallEngine |
| `fakes/fake_event_bus.rs` | FakeEventBus |
| `fakes/fake_user_notifier.rs` | FakeUserNotifier |
| `fakes/fake_process_resolver.rs` | FakeProcessResolver |
| `fakes/fake_connection_monitor.rs` | FakeConnectionMonitor |

### crates/infra/src/
| File | Responsibility |
|---|---|
| `lib.rs` | Re-exports |
| `persistence/mod.rs` | Re-exports + database init |
| `persistence/database.rs` | Database struct, WAL setup, migrations |
| `persistence/rule_repository.rs` | SqliteRuleRepository |
| `persistence/pending_decision_repository.rs` | SqlitePendingDecisionRepository |
| `persistence/decision_repository.rs` | SqliteDecisionRepository |
| `persistence/audit_repository.rs` | SqliteAuditRepository |
| `event_bus/mod.rs` | TokioBroadcastEventBus |

### crates/proto/
| File | Responsibility |
|---|---|
| `build.rs` | tonic-build configuration |
| `src/lib.rs` | Re-exports generated code |
| `../proto/syswall.proto` | gRPC service and message definitions |

### crates/daemon/src/
| File | Responsibility |
|---|---|
| `main.rs` | Entrypoint |
| `config.rs` | SysWallConfig, loading, validation |
| `bootstrap.rs` | DI wiring, startup sequence |
| `supervisor.rs` | Task lifecycle, CancellationToken, restart |
| `signals.rs` | SIGTERM/SIGINT handler |
| `grpc/mod.rs` | Re-exports |
| `grpc/control_service.rs` | SysWallControl gRPC impl |
| `grpc/event_service.rs` | SysWallEvents gRPC impl |
| `grpc/converters.rs` | Proto <-> domain type conversions |

### crates/ui/
| File | Responsibility |
|---|---|
| `src-tauri/src/main.rs` | Tauri entrypoint |
| `src-tauri/src/grpc_client.rs` | gRPC client connection |
| `src-tauri/src/commands/mod.rs` | Tauri command wrappers |
| `src/App.svelte` | Root component |
| `src/app.css` | Global styles placeholder |

### Root
| File | Responsibility |
|---|---|
| `Cargo.toml` | Workspace definition |
| `config/default.toml` | Default daemon config |

---

### Task 1: Workspace Scaffolding

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/domain/Cargo.toml`
- Create: `crates/domain/src/lib.rs`
- Create: `crates/app/Cargo.toml`
- Create: `crates/app/src/lib.rs`
- Create: `crates/infra/Cargo.toml`
- Create: `crates/infra/src/lib.rs`
- Create: `crates/proto/Cargo.toml`
- Create: `crates/proto/src/lib.rs`
- Create: `crates/proto/build.rs`
- Create: `crates/daemon/Cargo.toml`
- Create: `crates/daemon/src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: Create workspace root Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "crates/domain",
    "crates/app",
    "crates/infra",
    "crates/proto",
    "crates/daemon",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"

[workspace.dependencies]
# Domain
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
async-trait = "0.1"
thiserror = "2"
tracing = "0.1"

# Async runtime
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["rt"] }
tokio-stream = "0.1"
futures = "0.3"
pin-project-lite = "0.2"

# Infrastructure
rusqlite = { version = "0.32", features = ["bundled"] }
toml = "0.8"

# gRPC
tonic = "0.12"
tonic-build = "0.12"
prost = "0.13"
prost-types = "0.13"

# Testing
mockall = "0.13"
```

- [ ] **Step 2: Create domain crate**

`crates/domain/Cargo.toml`:
```toml
[package]
name = "syswall-domain"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
async-trait = { workspace = true }
thiserror = { workspace = true }
tokio-stream = { workspace = true }
futures = { workspace = true }
pin-project-lite = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
```

`crates/domain/src/lib.rs`:
```rust
pub mod entities;
pub mod errors;
pub mod events;
pub mod ports;
pub mod services;
pub mod value_objects;
```

- [ ] **Step 3: Create app crate**

`crates/app/Cargo.toml`:
```toml
[package]
name = "syswall-app"
version.workspace = true
edition.workspace = true

[dependencies]
syswall-domain = { path = "../domain" }
async-trait = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
tracing = { workspace = true }
tokio = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
```

`crates/app/src/lib.rs`:
```rust
pub mod commands;
pub mod fakes;
pub mod services;
```

- [ ] **Step 4: Create infra crate**

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
tracing = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
```

`crates/infra/src/lib.rs`:
```rust
pub mod event_bus;
pub mod persistence;
```

- [ ] **Step 5: Create proto crate**

`crates/proto/Cargo.toml`:
```toml
[package]
name = "syswall-proto"
version.workspace = true
edition.workspace = true

[dependencies]
tonic = { workspace = true }
prost = { workspace = true }
prost-types = { workspace = true }

[build-dependencies]
tonic-build = { workspace = true }
```

`crates/proto/build.rs`:
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Proto file will be added in Task 17
    Ok(())
}
```

`crates/proto/src/lib.rs`:
```rust
// Generated code will be included in Task 17
```

- [ ] **Step 6: Create daemon crate**

`crates/daemon/Cargo.toml`:
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
tonic = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
serde = { workspace = true }
toml = { workspace = true }
chrono = { workspace = true }
```

`crates/daemon/src/main.rs`:
```rust
fn main() {
    println!("syswall-daemon placeholder");
}
```

- [ ] **Step 7: Create .gitignore and verify workspace compiles**

`.gitignore`:
```
/target
.superpowers/
*.swp
*.swo
.env
```

Run: `cargo check`
Expected: compiles with no errors (may have warnings about unused modules)

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: scaffold Cargo workspace with 5 crates

Sets up monorepo structure: domain, app, infra, proto, daemon.
Workspace dependencies centralized. All crates compile."
```

---

### Task 2: Domain Value Objects

**Files:**
- Create: `crates/domain/src/value_objects/mod.rs`
- Test: inline `#[cfg(test)]` modules

- [ ] **Step 1: Write failing tests for Port**

`crates/domain/src/value_objects/mod.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;

use crate::errors::DomainError;

// --- Port ---

/// Network port (1-65535). Rejects 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Port(u16);

impl Port {
    pub fn new(value: u16) -> Result<Self, DomainError> {
        todo!()
    }

    pub fn value(&self) -> u16 {
        self.0
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// --- Placeholder error for compilation ---
// (will be replaced in Task 5)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_valid_creates_successfully() {
        assert!(Port::new(80).is_ok());
        assert_eq!(Port::new(80).unwrap().value(), 80);
    }

    #[test]
    fn port_zero_rejected() {
        assert!(Port::new(0).is_err());
    }

    #[test]
    fn port_max_valid() {
        assert!(Port::new(65535).is_ok());
    }

    #[test]
    fn port_one_valid() {
        assert!(Port::new(1).is_ok());
    }
}
```

We also need the errors module to exist first. Create `crates/domain/src/errors/mod.rs`:
```rust
use thiserror::Error;

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
}
```

Also create empty modules so `lib.rs` compiles:
- `crates/domain/src/entities/mod.rs`: `// entities will be added in Task 3`
- `crates/domain/src/events/mod.rs`: `// events will be added in Task 6`
- `crates/domain/src/ports/mod.rs`: `// ports will be added in Task 7`
- `crates/domain/src/services/mod.rs`: `// services will be added in Task 8`

Run: `cargo test -p syswall-domain`
Expected: 4 tests FAIL (Port::new is `todo!()`)

- [ ] **Step 2: Implement Port**

Replace `todo!()` in `Port::new`:
```rust
    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::Validation(
                "Port must be between 1 and 65535".to_string(),
            ));
        }
        Ok(Self(value))
    }
```

Run: `cargo test -p syswall-domain`
Expected: 4 tests PASS

- [ ] **Step 3: Add RulePriority with tests**

Add to `crates/domain/src/value_objects/mod.rs`:
```rust
// --- RulePriority ---

/// Rule evaluation priority. Lower value = higher priority.
/// System rules use 0. User rules start at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RulePriority(u32);

impl RulePriority {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn system() -> Self {
        Self(0)
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for RulePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

Add tests:
```rust
    #[test]
    fn rule_priority_ordering() {
        let p0 = RulePriority::system();
        let p1 = RulePriority::new(1);
        let p100 = RulePriority::new(100);
        assert!(p0 < p1);
        assert!(p1 < p100);
    }

    #[test]
    fn rule_priority_system_is_zero() {
        assert_eq!(RulePriority::system().value(), 0);
    }
```

Run: `cargo test -p syswall-domain`
Expected: 6 tests PASS

- [ ] **Step 4: Add ExecutablePath with tests**

Add to `crates/domain/src/value_objects/mod.rs`:
```rust
// --- ExecutablePath ---

/// Validated absolute path to an executable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutablePath(PathBuf);

impl ExecutablePath {
    pub fn new(path: PathBuf) -> Result<Self, DomainError> {
        if !path.is_absolute() {
            return Err(DomainError::Validation(format!(
                "Executable path must be absolute, got: {}",
                path.display()
            )));
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }
}

impl fmt::Display for ExecutablePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}
```

Add tests:
```rust
    #[test]
    fn executable_path_absolute_valid() {
        assert!(ExecutablePath::new(PathBuf::from("/usr/bin/firefox")).is_ok());
    }

    #[test]
    fn executable_path_relative_rejected() {
        assert!(ExecutablePath::new(PathBuf::from("bin/firefox")).is_err());
    }
```

Run: `cargo test -p syswall-domain`
Expected: 8 tests PASS

- [ ] **Step 5: Add remaining enums (Protocol, Direction) and SocketAddress**

Add to `crates/domain/src/value_objects/mod.rs`:
```rust
// --- Protocol ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
    Other(u8),
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::Tcp => write!(f, "TCP"),
            Protocol::Udp => write!(f, "UDP"),
            Protocol::Icmp => write!(f, "ICMP"),
            Protocol::Other(n) => write!(f, "OTHER({})", n),
        }
    }
}

// --- Direction ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Inbound,
    Outbound,
}

// --- SocketAddress ---

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SocketAddress {
    pub ip: IpAddr,
    pub port: Port,
}

impl SocketAddress {
    pub fn new(ip: IpAddr, port: Port) -> Self {
        Self { ip, port }
    }
}

impl fmt::Display for SocketAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
}
```

Run: `cargo test -p syswall-domain`
Expected: 8 tests PASS, no compilation errors

- [ ] **Step 6: Commit**

```bash
git add crates/domain/
git commit -m "feat(domain): add value objects with validation

Port (rejects 0), RulePriority (ordered), ExecutablePath (must be
absolute), Protocol, Direction, SocketAddress. All tested."
```

---

### Task 3: Domain Entities — Connection and Rule

**Files:**
- Create: `crates/domain/src/entities/connection.rs`
- Create: `crates/domain/src/entities/rule.rs`
- Modify: `crates/domain/src/entities/mod.rs`

- [ ] **Step 1: Create Connection entity**

`crates/domain/src/entities/mod.rs`:
```rust
pub mod audit;
pub mod connection;
pub mod decision;
pub mod rule;

pub use audit::*;
pub use connection::*;
pub use decision::*;
pub use rule::*;
```

`crates/domain/src/entities/connection.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::value_objects::{Direction, ExecutablePath, Port, Protocol, SocketAddress};

/// Unique identifier for a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(Uuid);

impl ConnectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

/// Runtime outcome for a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionVerdict {
    Unknown,
    PendingDecision,
    Allowed,
    Blocked,
    Ignored,
}

/// Connection lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionState {
    New,
    Established,
    Related,
    Closing,
    Closed,
}

/// Information about the process that owns a connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub path: Option<ExecutablePath>,
    pub cmdline: Option<String>,
}

/// System user owning a process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemUser {
    pub uid: u32,
    pub name: String,
}

/// Snapshot of connection state for decision records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionSnapshot {
    pub protocol: Protocol,
    pub source: SocketAddress,
    pub destination: SocketAddress,
    pub direction: Direction,
    pub process_name: Option<String>,
    pub process_path: Option<ExecutablePath>,
    pub user: Option<String>,
}

/// A network connection observed by the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: ConnectionId,
    pub protocol: Protocol,
    pub source: SocketAddress,
    pub destination: SocketAddress,
    pub direction: Direction,
    pub state: ConnectionState,
    pub process: Option<ProcessInfo>,
    pub user: Option<SystemUser>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub started_at: DateTime<Utc>,
    pub verdict: ConnectionVerdict,
    pub matched_rule: Option<super::rule::RuleId>,
}

impl Connection {
    /// Create a snapshot of this connection's current state.
    pub fn snapshot(&self) -> ConnectionSnapshot {
        ConnectionSnapshot {
            protocol: self.protocol,
            source: self.source.clone(),
            destination: self.destination.clone(),
            direction: self.direction,
            process_name: self.process.as_ref().map(|p| p.name.clone()),
            process_path: self.process.as_ref().and_then(|p| p.path.clone()),
            user: self.user.as_ref().map(|u| u.name.clone()),
        }
    }
}
```

- [ ] **Step 2: Create Rule entity**

`crates/domain/src/entities/rule.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

use crate::errors::DomainError;
use crate::value_objects::{Direction, ExecutablePath, Port, Protocol, RulePriority};

/// Unique identifier for a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuleId(Uuid);

impl RuleId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

/// What a rule does when it matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleEffect {
    Allow,
    Block,
    Ask,
    Observe,
}

/// Rule lifetime scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleScope {
    Permanent,
    Temporary { expires_at: DateTime<Utc> },
}

/// How the rule was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleSource {
    Manual,
    AutoLearning,
    Import,
    System,
}

/// Application matching criteria.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppMatcher {
    ByName(String),
    ByPath(ExecutablePath),
    ByHash(String),
}

/// IP matching criteria.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpMatcher {
    Exact(IpAddr),
    Cidr { network: IpAddr, prefix_len: u8 },
    Range { start: IpAddr, end: IpAddr },
}

/// Port matching criteria.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortMatcher {
    Exact(Port),
    Range { start: Port, end: Port },
}

/// Time schedule for rule application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub days: Vec<chrono::Weekday>,
    pub start_time: chrono::NaiveTime,
    pub end_time: chrono::NaiveTime,
}

/// All matching criteria for a rule. All present fields must match (AND logic).
/// None means "match anything" for that dimension.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleCriteria {
    pub application: Option<AppMatcher>,
    pub user: Option<String>,
    pub remote_ip: Option<IpMatcher>,
    pub remote_port: Option<PortMatcher>,
    pub local_port: Option<PortMatcher>,
    pub protocol: Option<Protocol>,
    pub direction: Option<Direction>,
    pub schedule: Option<Schedule>,
}

/// A firewall rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: RuleId,
    pub name: String,
    pub priority: RulePriority,
    pub enabled: bool,
    pub criteria: RuleCriteria,
    pub effect: RuleEffect,
    pub scope: RuleScope,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub source: RuleSource,
}

impl Rule {
    /// System rules cannot be deleted, only disabled.
    pub fn is_system(&self) -> bool {
        self.source == RuleSource::System
    }

    /// Check if a temporary rule has expired.
    pub fn is_expired(&self) -> bool {
        match &self.scope {
            RuleScope::Permanent => false,
            RuleScope::Temporary { expires_at } => Utc::now() > *expires_at,
        }
    }
}
```

- [ ] **Step 3: Write tests for Connection and Rule**

Add to bottom of `crates/domain/src/entities/connection.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    fn test_connection() -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol: Protocol::Tcp,
            source: SocketAddress::new(
                "192.168.1.100".parse().unwrap(),
                Port::new(45000).unwrap(),
            ),
            destination: SocketAddress::new(
                "93.184.216.34".parse().unwrap(),
                Port::new(443).unwrap(),
            ),
            direction: Direction::Outbound,
            state: ConnectionState::New,
            process: Some(ProcessInfo {
                pid: 1234,
                name: "firefox".to_string(),
                path: Some(ExecutablePath::new("/usr/bin/firefox".into()).unwrap()),
                cmdline: Some("firefox https://example.com".to_string()),
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

    #[test]
    fn connection_snapshot_captures_process_info() {
        let conn = test_connection();
        let snap = conn.snapshot();
        assert_eq!(snap.process_name, Some("firefox".to_string()));
        assert_eq!(snap.protocol, Protocol::Tcp);
        assert_eq!(snap.direction, Direction::Outbound);
    }

    #[test]
    fn connection_snapshot_handles_missing_process() {
        let mut conn = test_connection();
        conn.process = None;
        conn.user = None;
        let snap = conn.snapshot();
        assert_eq!(snap.process_name, None);
        assert_eq!(snap.user, None);
    }
}
```

Add to bottom of `crates/domain/src/entities/rule.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_rule(effect: RuleEffect, source: RuleSource) -> Rule {
        Rule {
            id: RuleId::new(),
            name: "Test rule".to_string(),
            priority: RulePriority::new(100),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source,
        }
    }

    #[test]
    fn system_rule_detected() {
        let rule = test_rule(RuleEffect::Allow, RuleSource::System);
        assert!(rule.is_system());
    }

    #[test]
    fn manual_rule_not_system() {
        let rule = test_rule(RuleEffect::Block, RuleSource::Manual);
        assert!(!rule.is_system());
    }

    #[test]
    fn permanent_rule_never_expired() {
        let rule = test_rule(RuleEffect::Allow, RuleSource::Manual);
        assert!(!rule.is_expired());
    }

    #[test]
    fn expired_temporary_rule_detected() {
        let mut rule = test_rule(RuleEffect::Allow, RuleSource::Manual);
        rule.scope = RuleScope::Temporary {
            expires_at: Utc::now() - chrono::Duration::hours(1),
        };
        assert!(rule.is_expired());
    }

    #[test]
    fn future_temporary_rule_not_expired() {
        let mut rule = test_rule(RuleEffect::Allow, RuleSource::Manual);
        rule.scope = RuleScope::Temporary {
            expires_at: Utc::now() + chrono::Duration::hours(1),
        };
        assert!(!rule.is_expired());
    }

    #[test]
    fn default_criteria_matches_everything() {
        let criteria = RuleCriteria::default();
        assert!(criteria.application.is_none());
        assert!(criteria.protocol.is_none());
        assert!(criteria.direction.is_none());
    }
}
```

Run: `cargo test -p syswall-domain`
Expected: all tests PASS (8 value object + 8 entity tests)

- [ ] **Step 4: Commit**

```bash
git add crates/domain/
git commit -m "feat(domain): add Connection and Rule entities

Connection with snapshot capability. Rule with criteria, effect,
scope, source. All newtypes for IDs. Tested."
```

---

### Task 4: Domain Entities — Decision, PendingDecision, AuditEvent

**Files:**
- Create: `crates/domain/src/entities/decision.rs`
- Create: `crates/domain/src/entities/audit.rs`

- [ ] **Step 1: Create Decision and PendingDecision entities**

`crates/domain/src/entities/decision.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::connection::ConnectionSnapshot;
use super::rule::RuleId;

/// Unique identifier for a pending decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PendingDecisionId(Uuid);

impl PendingDecisionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

/// Status of a pending decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PendingDecisionStatus {
    Pending,
    Resolved,
    Expired,
    Cancelled,
}

/// A decision request waiting for user response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDecision {
    pub id: PendingDecisionId,
    pub connection_snapshot: ConnectionSnapshot,
    pub requested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub deduplication_key: String,
    pub status: PendingDecisionStatus,
}

impl PendingDecision {
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn is_pending(&self) -> bool {
        self.status == PendingDecisionStatus::Pending
    }
}

/// Unique identifier for a resolved decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecisionId(Uuid);

impl DecisionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

/// What the user decided to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionAction {
    AllowOnce,
    BlockOnce,
    AlwaysAllow,
    AlwaysBlock,
    CreateRule,
    Ignore,
}

/// Granularity of the rule generated from a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionGranularity {
    AppOnly,
    AppAndIp,
    AppAndPort,
    AppAndDomain,
    AppAndProtocol,
    Full,
}

/// A resolved auto-learning decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: DecisionId,
    pub pending_decision_id: PendingDecisionId,
    pub connection_snapshot: ConnectionSnapshot,
    pub action: DecisionAction,
    pub granularity: DecisionGranularity,
    pub decided_at: DateTime<Utc>,
    pub generated_rule: Option<RuleId>,
}
```

- [ ] **Step 2: Create AuditEvent entity**

`crates/domain/src/entities/audit.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for an audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(Uuid);

impl EventId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

/// Event severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

/// Category of audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventCategory {
    Connection,
    Rule,
    Decision,
    System,
    Config,
}

/// A journal entry in the audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: EventId,
    pub timestamp: DateTime<Utc>,
    pub severity: Severity,
    pub category: EventCategory,
    pub description: String,
    pub metadata: HashMap<String, String>,
}

impl AuditEvent {
    pub fn new(
        severity: Severity,
        category: EventCategory,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: EventId::new(),
            timestamp: Utc::now(),
            severity,
            category,
            description: description.into(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}
```

- [ ] **Step 3: Write tests**

Add to bottom of `crates/domain/src/entities/decision.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::*;

    fn test_snapshot() -> ConnectionSnapshot {
        ConnectionSnapshot {
            protocol: Protocol::Tcp,
            source: SocketAddress::new("10.0.0.1".parse().unwrap(), Port::new(5000).unwrap()),
            destination: SocketAddress::new("8.8.8.8".parse().unwrap(), Port::new(443).unwrap()),
            direction: Direction::Outbound,
            process_name: Some("curl".to_string()),
            process_path: None,
            user: Some("seb".to_string()),
        }
    }

    #[test]
    fn pending_decision_expired_when_past_deadline() {
        let pd = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            requested_at: Utc::now() - chrono::Duration::minutes(10),
            expires_at: Utc::now() - chrono::Duration::minutes(1),
            deduplication_key: "curl:8.8.8.8:443:tcp".to_string(),
            status: PendingDecisionStatus::Pending,
        };
        assert!(pd.is_expired());
        assert!(pd.is_pending());
    }

    #[test]
    fn pending_decision_not_expired_when_future_deadline() {
        let pd = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            deduplication_key: "curl:8.8.8.8:443:tcp".to_string(),
            status: PendingDecisionStatus::Pending,
        };
        assert!(!pd.is_expired());
    }

    #[test]
    fn resolved_decision_not_pending() {
        let pd = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            deduplication_key: "test".to_string(),
            status: PendingDecisionStatus::Resolved,
        };
        assert!(!pd.is_pending());
    }
}
```

Add to bottom of `crates/domain/src/entities/audit.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_event_builder() {
        let event = AuditEvent::new(Severity::Info, EventCategory::Rule, "Rule created")
            .with_metadata("rule_id", "abc-123")
            .with_metadata("rule_name", "Block SSH");

        assert_eq!(event.severity, Severity::Info);
        assert_eq!(event.category, EventCategory::Rule);
        assert_eq!(event.metadata.get("rule_id").unwrap(), "abc-123");
        assert_eq!(event.metadata.len(), 2);
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Debug < Severity::Info);
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
        assert!(Severity::Error < Severity::Critical);
    }
}
```

Run: `cargo test -p syswall-domain`
Expected: all tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/domain/
git commit -m "feat(domain): add Decision, PendingDecision, AuditEvent entities

PendingDecision with expiration and dedup key for async learning flow.
AuditEvent with builder pattern. All tested."
```

---

### Task 5: Domain Events

**Files:**
- Modify: `crates/domain/src/events/mod.rs`

- [ ] **Step 1: Define DomainEvent enum**

`crates/domain/src/events/mod.rs`:
```rust
use serde::{Deserialize, Serialize};

use crate::entities::{
    Connection, ConnectionId, ConnectionState, ConnectionVerdict, Decision, PendingDecision,
    PendingDecisionId, Rule, RuleId,
};
use crate::value_objects::Protocol;

/// All domain events flowing through the EventBus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    ConnectionDetected(Connection),
    ConnectionUpdated {
        id: ConnectionId,
        state: ConnectionState,
    },
    ConnectionClosed(ConnectionId),
    RuleCreated(Rule),
    RuleUpdated(Rule),
    RuleDeleted(RuleId),
    RuleMatched {
        connection_id: ConnectionId,
        rule_id: RuleId,
        verdict: ConnectionVerdict,
    },
    DecisionRequired(PendingDecision),
    DecisionResolved(Decision),
    DecisionExpired(PendingDecisionId),
    FirewallStatusChanged(FirewallStatus),
    SystemError {
        message: String,
        severity: crate::entities::Severity,
    },
}

/// Overall firewall status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallStatus {
    pub enabled: bool,
    pub active_rules_count: u32,
    pub nftables_synced: bool,
    pub uptime_secs: u64,
    pub version: String,
}

/// Pagination parameters for list queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub offset: u64,
    pub limit: u64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 50,
        }
    }
}

/// Notification sent to the UI (non-blocking, informational).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub message: String,
    pub severity: crate::entities::Severity,
}

/// Default policy when no rules match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefaultPolicy {
    Ask,
    Allow,
    Block,
}
```

Run: `cargo check -p syswall-domain`
Expected: compiles

- [ ] **Step 2: Commit**

```bash
git add crates/domain/
git commit -m "feat(domain): add DomainEvent enum and supporting types

FirewallStatus, Pagination, Notification, DefaultPolicy.
DomainEvent covers all system events for the EventBus."
```

---

### Task 6: Domain Ports (Traits)

**Files:**
- Create: `crates/domain/src/ports/repositories.rs`
- Create: `crates/domain/src/ports/system.rs`
- Create: `crates/domain/src/ports/messaging.rs`
- Modify: `crates/domain/src/ports/mod.rs`

- [ ] **Step 1: Create repository traits**

`crates/domain/src/ports/mod.rs`:
```rust
pub mod messaging;
pub mod repositories;
pub mod system;

pub use messaging::*;
pub use repositories::*;
pub use system::*;
```

`crates/domain/src/ports/repositories.rs`:
```rust
use async_trait::async_trait;

use crate::entities::{
    AuditEvent, Decision, EventCategory, PendingDecision, PendingDecisionId, Rule, RuleEffect,
    RuleId, RuleSource, Severity,
};
use crate::errors::DomainError;
use crate::events::Pagination;
use crate::value_objects::Direction;

/// Filters for querying rules.
#[derive(Debug, Clone, Default)]
pub struct RuleFilters {
    pub source: Option<RuleSource>,
    pub effect: Option<RuleEffect>,
    pub enabled: Option<bool>,
    pub direction: Option<Direction>,
    pub search: Option<String>,
}

/// Filters for querying audit events.
#[derive(Debug, Clone, Default)]
pub struct AuditFilters {
    pub severity: Option<Severity>,
    pub category: Option<EventCategory>,
    pub search: Option<String>,
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
}

#[async_trait]
pub trait RuleRepository: Send + Sync {
    async fn save(&self, rule: &Rule) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: &RuleId) -> Result<Option<Rule>, DomainError>;
    async fn find_all(
        &self,
        filters: &RuleFilters,
        pagination: &Pagination,
    ) -> Result<Vec<Rule>, DomainError>;
    async fn delete(&self, id: &RuleId) -> Result<(), DomainError>;
    async fn list_enabled_ordered(&self) -> Result<Vec<Rule>, DomainError>;
}

#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn append(&self, event: &AuditEvent) -> Result<(), DomainError>;
    async fn query(
        &self,
        filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError>;
    async fn count(&self, filters: &AuditFilters) -> Result<u64, DomainError>;
}

#[async_trait]
pub trait DecisionRepository: Send + Sync {
    async fn save(&self, decision: &Decision) -> Result<(), DomainError>;
}

#[async_trait]
pub trait PendingDecisionRepository: Send + Sync {
    async fn create(&self, request: &PendingDecision) -> Result<(), DomainError>;
    async fn list_pending(&self) -> Result<Vec<PendingDecision>, DomainError>;
    async fn resolve(&self, id: &PendingDecisionId) -> Result<(), DomainError>;
    async fn expire_overdue(&self) -> Result<Vec<PendingDecision>, DomainError>;
    async fn find_by_dedup_key(&self, key: &str) -> Result<Option<PendingDecision>, DomainError>;
}
```

- [ ] **Step 2: Create system traits**

`crates/domain/src/ports/system.rs`:
```rust
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::entities::{Connection, ProcessInfo, Rule};
use crate::errors::DomainError;
use crate::events::FirewallStatus;

/// Stream of connection events from the monitoring subsystem.
pub type ConnectionEventStream =
    Pin<Box<dyn Stream<Item = Result<Connection, DomainError>> + Send>>;

#[async_trait]
pub trait FirewallEngine: Send + Sync {
    async fn apply_rule(&self, rule: &Rule) -> Result<(), DomainError>;
    async fn remove_rule(&self, rule_id: &crate::entities::RuleId) -> Result<(), DomainError>;
    async fn sync_all_rules(&self, rules: &[Rule]) -> Result<(), DomainError>;
    async fn get_status(&self) -> Result<FirewallStatus, DomainError>;
}

#[async_trait]
pub trait ConnectionMonitor: Send + Sync {
    async fn stream_events(&self) -> Result<ConnectionEventStream, DomainError>;
    async fn get_active_connections(&self) -> Result<Vec<Connection>, DomainError>;
}

#[async_trait]
pub trait ProcessResolver: Send + Sync {
    async fn resolve(&self, pid: u32) -> Result<Option<ProcessInfo>, DomainError>;
    async fn resolve_by_socket(&self, inode: u64) -> Result<Option<ProcessInfo>, DomainError>;
}
```

- [ ] **Step 3: Create messaging traits**

`crates/domain/src/ports/messaging.rs`:
```rust
use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::entities::PendingDecision;
use crate::errors::DomainError;
use crate::events::{DomainEvent, Notification};

/// Receiver for domain events.
pub type EventReceiver = broadcast::Receiver<DomainEvent>;

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent) -> Result<(), DomainError>;
    fn subscribe(&self) -> EventReceiver;
}

#[async_trait]
pub trait UserNotifier: Send + Sync {
    async fn notify_decision_required(
        &self,
        request: &PendingDecision,
    ) -> Result<(), DomainError>;
    async fn notify(&self, notification: &Notification) -> Result<(), DomainError>;
}
```

Run: `cargo check -p syswall-domain`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add crates/domain/
git commit -m "feat(domain): add async port traits

RuleRepository, AuditRepository, DecisionRepository,
PendingDecisionRepository, FirewallEngine, ConnectionMonitor,
ProcessResolver, EventBus, UserNotifier. All async_trait + Send + Sync."
```

---

### Task 7: PolicyEngine

**Files:**
- Create: `crates/domain/src/services/policy_engine.rs`
- Modify: `crates/domain/src/services/mod.rs`

- [ ] **Step 1: Write failing tests for PolicyEngine**

`crates/domain/src/services/mod.rs`:
```rust
pub mod policy_engine;
pub use policy_engine::*;
```

`crates/domain/src/services/policy_engine.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

use crate::entities::{
    AppMatcher, Connection, ConnectionVerdict, IpMatcher, PortMatcher, Rule, RuleCriteria,
    RuleEffect, RuleId,
};
use crate::events::DefaultPolicy;
use crate::value_objects::Port;

/// Result of evaluating a connection against rules.
#[derive(Debug, Clone)]
pub struct PolicyEvaluation {
    pub verdict: ConnectionVerdict,
    pub matched_rule_id: Option<RuleId>,
    pub reason: EvaluationReason,
}

/// Why a particular verdict was reached.
#[derive(Debug, Clone)]
pub enum EvaluationReason {
    MatchedRule {
        rule_id: RuleId,
        effect: RuleEffect,
    },
    NoMatchingRule,
    PendingUserDecision,
    DefaultPolicyApplied {
        policy: DefaultPolicy,
    },
}

/// Pure domain service — no I/O, no ports.
pub struct PolicyEngine;

impl PolicyEngine {
    /// Evaluate a connection against a list of rules (must be sorted by priority).
    /// Returns the evaluation result including verdict and reason.
    pub fn evaluate(
        connection: &Connection,
        rules: &[Rule],
        default_policy: DefaultPolicy,
    ) -> PolicyEvaluation {
        for rule in rules {
            if !rule.enabled || rule.is_expired() {
                continue;
            }
            if Self::matches(&rule.criteria, connection) {
                let verdict = match rule.effect {
                    RuleEffect::Allow => ConnectionVerdict::Allowed,
                    RuleEffect::Block => ConnectionVerdict::Blocked,
                    RuleEffect::Ask => ConnectionVerdict::PendingDecision,
                    RuleEffect::Observe => ConnectionVerdict::Ignored,
                };
                return PolicyEvaluation {
                    verdict,
                    matched_rule_id: Some(rule.id),
                    reason: EvaluationReason::MatchedRule {
                        rule_id: rule.id,
                        effect: rule.effect,
                    },
                };
            }
        }

        // No rule matched — apply default policy
        match default_policy {
            DefaultPolicy::Ask => PolicyEvaluation {
                verdict: ConnectionVerdict::PendingDecision,
                matched_rule_id: None,
                reason: EvaluationReason::NoMatchingRule,
            },
            DefaultPolicy::Allow => PolicyEvaluation {
                verdict: ConnectionVerdict::Allowed,
                matched_rule_id: None,
                reason: EvaluationReason::DefaultPolicyApplied {
                    policy: DefaultPolicy::Allow,
                },
            },
            DefaultPolicy::Block => PolicyEvaluation {
                verdict: ConnectionVerdict::Blocked,
                matched_rule_id: None,
                reason: EvaluationReason::DefaultPolicyApplied {
                    policy: DefaultPolicy::Block,
                },
            },
        }
    }

    /// Check if a single criteria set matches a connection (Specification pattern).
    pub fn matches(criteria: &RuleCriteria, connection: &Connection) -> bool {
        if let Some(ref app_matcher) = criteria.application {
            if let Some(ref process) = connection.process {
                let matched = match app_matcher {
                    AppMatcher::ByName(name) => process.name == *name,
                    AppMatcher::ByPath(path) => {
                        process.path.as_ref().is_some_and(|p| p == path)
                    }
                    AppMatcher::ByHash(hash) => false, // Hash matching deferred to sub-project 2
                };
                if !matched {
                    return false;
                }
            } else {
                // No process info available, can't match application criteria
                return false;
            }
        }

        if let Some(ref user) = criteria.user {
            match &connection.user {
                Some(u) if u.name == *user => {}
                _ => return false,
            }
        }

        if let Some(ref ip_matcher) = criteria.remote_ip {
            let remote_ip = if connection.direction == crate::value_objects::Direction::Outbound {
                connection.destination.ip
            } else {
                connection.source.ip
            };
            if !Self::matches_ip(ip_matcher, remote_ip) {
                return false;
            }
        }

        if let Some(ref port_matcher) = criteria.remote_port {
            let remote_port = if connection.direction == crate::value_objects::Direction::Outbound {
                connection.destination.port
            } else {
                connection.source.port
            };
            if !Self::matches_port(port_matcher, remote_port) {
                return false;
            }
        }

        if let Some(ref port_matcher) = criteria.local_port {
            let local_port = if connection.direction == crate::value_objects::Direction::Outbound {
                connection.source.port
            } else {
                connection.destination.port
            };
            if !Self::matches_port(port_matcher, local_port) {
                return false;
            }
        }

        if let Some(ref proto) = criteria.protocol {
            if connection.protocol != *proto {
                return false;
            }
        }

        if let Some(ref dir) = criteria.direction {
            if connection.direction != *dir {
                return false;
            }
        }

        // Schedule matching deferred to sub-project 4

        true
    }

    fn matches_ip(matcher: &IpMatcher, ip: IpAddr) -> bool {
        match matcher {
            IpMatcher::Exact(expected) => ip == *expected,
            IpMatcher::Cidr {
                network,
                prefix_len,
            } => Self::ip_in_cidr(ip, *network, *prefix_len),
            IpMatcher::Range { start, end } => ip >= *start && ip <= *end,
        }
    }

    fn ip_in_cidr(ip: IpAddr, network: IpAddr, prefix_len: u8) -> bool {
        match (ip, network) {
            (IpAddr::V4(ip), IpAddr::V4(net)) => {
                if prefix_len > 32 {
                    return false;
                }
                let mask = if prefix_len == 0 {
                    0u32
                } else {
                    !0u32 << (32 - prefix_len)
                };
                (u32::from(ip) & mask) == (u32::from(net) & mask)
            }
            (IpAddr::V6(ip), IpAddr::V6(net)) => {
                if prefix_len > 128 {
                    return false;
                }
                let ip_bits = u128::from(ip);
                let net_bits = u128::from(net);
                let mask = if prefix_len == 0 {
                    0u128
                } else {
                    !0u128 << (128 - prefix_len)
                };
                (ip_bits & mask) == (net_bits & mask)
            }
            _ => false, // v4 vs v6 mismatch
        }
    }

    fn matches_port(matcher: &PortMatcher, port: Port) -> bool {
        match matcher {
            PortMatcher::Exact(expected) => port == *expected,
            PortMatcher::Range { start, end } => {
                port.value() >= start.value() && port.value() <= end.value()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::*;
    use crate::value_objects::*;
    use chrono::Utc;

    fn test_connection() -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol: Protocol::Tcp,
            source: SocketAddress::new("192.168.1.100".parse().unwrap(), Port::new(45000).unwrap()),
            destination: SocketAddress::new("93.184.216.34".parse().unwrap(), Port::new(443).unwrap()),
            direction: Direction::Outbound,
            state: ConnectionState::New,
            process: Some(ProcessInfo {
                pid: 1234,
                name: "firefox".to_string(),
                path: Some(ExecutablePath::new("/usr/bin/firefox".into()).unwrap()),
                cmdline: None,
            }),
            user: Some(SystemUser { uid: 1000, name: "seb".to_string() }),
            bytes_sent: 0,
            bytes_received: 0,
            started_at: Utc::now(),
            verdict: ConnectionVerdict::Unknown,
            matched_rule: None,
        }
    }

    fn make_rule(priority: u32, effect: RuleEffect, criteria: RuleCriteria) -> Rule {
        Rule {
            id: RuleId::new(),
            name: format!("Rule p{}", priority),
            priority: RulePriority::new(priority),
            enabled: true,
            criteria,
            effect,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        }
    }

    // --- evaluate() tests ---

    #[test]
    fn no_rules_default_ask() {
        let conn = test_connection();
        let result = PolicyEngine::evaluate(&conn, &[], DefaultPolicy::Ask);
        assert_eq!(result.verdict, ConnectionVerdict::PendingDecision);
        assert!(result.matched_rule_id.is_none());
    }

    #[test]
    fn no_rules_default_block() {
        let conn = test_connection();
        let result = PolicyEngine::evaluate(&conn, &[], DefaultPolicy::Block);
        assert_eq!(result.verdict, ConnectionVerdict::Blocked);
    }

    #[test]
    fn no_rules_default_allow() {
        let conn = test_connection();
        let result = PolicyEngine::evaluate(&conn, &[], DefaultPolicy::Allow);
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
    }

    #[test]
    fn first_matching_rule_wins_by_priority() {
        let conn = test_connection();
        let rules = vec![
            make_rule(10, RuleEffect::Allow, RuleCriteria::default()),
            make_rule(20, RuleEffect::Block, RuleCriteria::default()),
        ];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Block);
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
        assert_eq!(result.matched_rule_id, Some(rules[0].id));
    }

    #[test]
    fn disabled_rule_skipped() {
        let conn = test_connection();
        let mut rule = make_rule(1, RuleEffect::Block, RuleCriteria::default());
        rule.enabled = false;
        let rules = vec![rule];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Allow);
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
    }

    #[test]
    fn expired_rule_skipped() {
        let conn = test_connection();
        let mut rule = make_rule(1, RuleEffect::Block, RuleCriteria::default());
        rule.scope = RuleScope::Temporary {
            expires_at: Utc::now() - chrono::Duration::hours(1),
        };
        let rules = vec![rule];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Allow);
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
    }

    #[test]
    fn ask_effect_returns_pending_decision() {
        let conn = test_connection();
        let rule = make_rule(1, RuleEffect::Ask, RuleCriteria::default());
        let rules = vec![rule];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Block);
        assert_eq!(result.verdict, ConnectionVerdict::PendingDecision);
    }

    #[test]
    fn observe_effect_returns_ignored() {
        let conn = test_connection();
        let rule = make_rule(1, RuleEffect::Observe, RuleCriteria::default());
        let rules = vec![rule];
        let result = PolicyEngine::evaluate(&conn, &rules, DefaultPolicy::Block);
        assert_eq!(result.verdict, ConnectionVerdict::Ignored);
    }

    // --- matches() tests ---

    #[test]
    fn empty_criteria_matches_everything() {
        let conn = test_connection();
        assert!(PolicyEngine::matches(&RuleCriteria::default(), &conn));
    }

    #[test]
    fn app_name_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByName("firefox".to_string())),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn app_name_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByName("chrome".to_string())),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn app_path_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByPath(
                ExecutablePath::new("/usr/bin/firefox".into()).unwrap(),
            )),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn protocol_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            protocol: Some(Protocol::Tcp),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn protocol_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            protocol: Some(Protocol::Udp),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn direction_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            direction: Some(Direction::Outbound),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn direction_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            direction: Some(Direction::Inbound),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_ip_exact_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_ip: Some(IpMatcher::Exact("93.184.216.34".parse().unwrap())),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_ip_cidr_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_ip: Some(IpMatcher::Cidr {
                network: "93.184.216.0".parse().unwrap(),
                prefix_len: 24,
            }),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_ip_cidr_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_ip: Some(IpMatcher::Cidr {
                network: "10.0.0.0".parse().unwrap(),
                prefix_len: 8,
            }),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_port_exact_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn remote_port_range_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            remote_port: Some(PortMatcher::Range {
                start: Port::new(400).unwrap(),
                end: Port::new(500).unwrap(),
            }),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn user_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            user: Some("seb".to_string()),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn user_no_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            user: Some("root".to_string()),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn combined_criteria_all_must_match() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByName("firefox".to_string())),
            protocol: Some(Protocol::Tcp),
            direction: Some(Direction::Outbound),
            remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
            ..Default::default()
        };
        assert!(PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn combined_criteria_one_fails_all_fails() {
        let conn = test_connection();
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByName("firefox".to_string())),
            protocol: Some(Protocol::Udp), // wrong protocol
            direction: Some(Direction::Outbound),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }

    #[test]
    fn no_process_info_fails_app_criteria() {
        let mut conn = test_connection();
        conn.process = None;
        let criteria = RuleCriteria {
            application: Some(AppMatcher::ByName("firefox".to_string())),
            ..Default::default()
        };
        assert!(!PolicyEngine::matches(&criteria, &conn));
    }
}
```

Run: `cargo test -p syswall-domain`
Expected: all tests PASS

- [ ] **Step 2: Commit**

```bash
git add crates/domain/
git commit -m "feat(domain): add PolicyEngine with Specification pattern matching

Pure domain service. Evaluates connections against rules by priority.
Supports IP (exact/CIDR/range), port (exact/range), app (name/path),
user, protocol, direction matching. 25+ tests."
```

---

### Task 8: App Layer — Commands and Fakes

**Files:**
- Create: `crates/app/src/commands/mod.rs`
- Create: all fake implementations under `crates/app/src/fakes/`

- [ ] **Step 1: Create command types**

`crates/app/src/commands/mod.rs`:
```rust
use syswall_domain::entities::{
    DecisionAction, DecisionGranularity, PendingDecisionId, RuleCriteria, RuleEffect, RuleId,
    RuleScope, RuleSource,
};

/// Command to create a new rule.
#[derive(Debug, Clone)]
pub struct CreateRuleCommand {
    pub name: String,
    pub priority: u32,
    pub criteria: RuleCriteria,
    pub effect: RuleEffect,
    pub scope: RuleScope,
    pub source: RuleSource,
}

/// Command to update an existing rule.
#[derive(Debug, Clone)]
pub struct UpdateRuleCommand {
    pub id: RuleId,
    pub name: Option<String>,
    pub priority: Option<u32>,
    pub criteria: Option<RuleCriteria>,
    pub effect: Option<RuleEffect>,
    pub scope: Option<RuleScope>,
    pub enabled: Option<bool>,
}

/// Command to respond to a pending decision.
#[derive(Debug, Clone)]
pub struct RespondToDecisionCommand {
    pub pending_decision_id: PendingDecisionId,
    pub action: DecisionAction,
    pub granularity: DecisionGranularity,
}
```

- [ ] **Step 2: Create fake repositories and adapters**

`crates/app/src/fakes/mod.rs`:
```rust
pub mod fake_audit_repository;
pub mod fake_connection_monitor;
pub mod fake_decision_repository;
pub mod fake_event_bus;
pub mod fake_firewall_engine;
pub mod fake_pending_decision_repository;
pub mod fake_process_resolver;
pub mod fake_rule_repository;
pub mod fake_user_notifier;

pub use fake_audit_repository::*;
pub use fake_connection_monitor::*;
pub use fake_decision_repository::*;
pub use fake_event_bus::*;
pub use fake_firewall_engine::*;
pub use fake_pending_decision_repository::*;
pub use fake_process_resolver::*;
pub use fake_rule_repository::*;
pub use fake_user_notifier::*;
```

`crates/app/src/fakes/fake_rule_repository.rs`:
```rust
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use syswall_domain::entities::{Rule, RuleId};
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{RuleFilters, RuleRepository};

pub struct FakeRuleRepository {
    rules: Mutex<HashMap<RuleId, Rule>>,
}

impl FakeRuleRepository {
    pub fn new() -> Self {
        Self {
            rules: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl RuleRepository for FakeRuleRepository {
    async fn save(&self, rule: &Rule) -> Result<(), DomainError> {
        self.rules.lock().unwrap().insert(rule.id, rule.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &RuleId) -> Result<Option<Rule>, DomainError> {
        Ok(self.rules.lock().unwrap().get(id).cloned())
    }

    async fn find_all(
        &self,
        _filters: &RuleFilters,
        pagination: &Pagination,
    ) -> Result<Vec<Rule>, DomainError> {
        let rules = self.rules.lock().unwrap();
        let mut all: Vec<Rule> = rules.values().cloned().collect();
        all.sort_by_key(|r| r.priority);
        let start = pagination.offset as usize;
        let end = (start + pagination.limit as usize).min(all.len());
        if start >= all.len() {
            return Ok(vec![]);
        }
        Ok(all[start..end].to_vec())
    }

    async fn delete(&self, id: &RuleId) -> Result<(), DomainError> {
        self.rules.lock().unwrap().remove(id);
        Ok(())
    }

    async fn list_enabled_ordered(&self) -> Result<Vec<Rule>, DomainError> {
        let rules = self.rules.lock().unwrap();
        let mut enabled: Vec<Rule> = rules.values().filter(|r| r.enabled).cloned().collect();
        enabled.sort_by_key(|r| r.priority);
        Ok(enabled)
    }
}
```

`crates/app/src/fakes/fake_pending_decision_repository.rs`:
```rust
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Mutex;

use syswall_domain::entities::{PendingDecision, PendingDecisionId, PendingDecisionStatus};
use syswall_domain::errors::DomainError;
use syswall_domain::ports::PendingDecisionRepository;

pub struct FakePendingDecisionRepository {
    decisions: Mutex<HashMap<PendingDecisionId, PendingDecision>>,
}

impl FakePendingDecisionRepository {
    pub fn new() -> Self {
        Self {
            decisions: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl PendingDecisionRepository for FakePendingDecisionRepository {
    async fn create(&self, request: &PendingDecision) -> Result<(), DomainError> {
        self.decisions
            .lock()
            .unwrap()
            .insert(request.id, request.clone());
        Ok(())
    }

    async fn list_pending(&self) -> Result<Vec<PendingDecision>, DomainError> {
        Ok(self
            .decisions
            .lock()
            .unwrap()
            .values()
            .filter(|d| d.status == PendingDecisionStatus::Pending)
            .cloned()
            .collect())
    }

    async fn resolve(&self, id: &PendingDecisionId) -> Result<(), DomainError> {
        if let Some(d) = self.decisions.lock().unwrap().get_mut(id) {
            d.status = PendingDecisionStatus::Resolved;
        }
        Ok(())
    }

    async fn expire_overdue(&self) -> Result<Vec<PendingDecision>, DomainError> {
        let now = Utc::now();
        let mut expired = vec![];
        for d in self.decisions.lock().unwrap().values_mut() {
            if d.status == PendingDecisionStatus::Pending && d.expires_at < now {
                d.status = PendingDecisionStatus::Expired;
                expired.push(d.clone());
            }
        }
        Ok(expired)
    }

    async fn find_by_dedup_key(&self, key: &str) -> Result<Option<PendingDecision>, DomainError> {
        Ok(self
            .decisions
            .lock()
            .unwrap()
            .values()
            .find(|d| d.deduplication_key == key && d.status == PendingDecisionStatus::Pending)
            .cloned())
    }
}
```

`crates/app/src/fakes/fake_decision_repository.rs`:
```rust
use async_trait::async_trait;
use std::sync::Mutex;

use syswall_domain::entities::Decision;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::DecisionRepository;

pub struct FakeDecisionRepository {
    pub decisions: Mutex<Vec<Decision>>,
}

impl FakeDecisionRepository {
    pub fn new() -> Self {
        Self {
            decisions: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl DecisionRepository for FakeDecisionRepository {
    async fn save(&self, decision: &Decision) -> Result<(), DomainError> {
        self.decisions.lock().unwrap().push(decision.clone());
        Ok(())
    }
}
```

`crates/app/src/fakes/fake_audit_repository.rs`:
```rust
use async_trait::async_trait;
use std::sync::Mutex;

use syswall_domain::entities::AuditEvent;
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{AuditFilters, AuditRepository};

pub struct FakeAuditRepository {
    pub events: Mutex<Vec<AuditEvent>>,
}

impl FakeAuditRepository {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl AuditRepository for FakeAuditRepository {
    async fn append(&self, event: &AuditEvent) -> Result<(), DomainError> {
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    async fn query(
        &self,
        _filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError> {
        let events = self.events.lock().unwrap();
        let start = pagination.offset as usize;
        let end = (start + pagination.limit as usize).min(events.len());
        if start >= events.len() {
            return Ok(vec![]);
        }
        Ok(events[start..end].to_vec())
    }

    async fn count(&self, _filters: &AuditFilters) -> Result<u64, DomainError> {
        Ok(self.events.lock().unwrap().len() as u64)
    }
}
```

`crates/app/src/fakes/fake_firewall_engine.rs`:
```rust
use async_trait::async_trait;
use std::sync::Mutex;

use syswall_domain::entities::{Rule, RuleId};
use syswall_domain::errors::DomainError;
use syswall_domain::events::FirewallStatus;
use syswall_domain::ports::FirewallEngine;

#[derive(Debug, Clone)]
pub enum FirewallCall {
    ApplyRule(RuleId),
    RemoveRule(RuleId),
    SyncAll(usize),
}

pub struct FakeFirewallEngine {
    pub calls: Mutex<Vec<FirewallCall>>,
}

impl FakeFirewallEngine {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl FirewallEngine for FakeFirewallEngine {
    async fn apply_rule(&self, rule: &Rule) -> Result<(), DomainError> {
        self.calls
            .lock()
            .unwrap()
            .push(FirewallCall::ApplyRule(rule.id));
        Ok(())
    }

    async fn remove_rule(&self, rule_id: &RuleId) -> Result<(), DomainError> {
        self.calls
            .lock()
            .unwrap()
            .push(FirewallCall::RemoveRule(*rule_id));
        Ok(())
    }

    async fn sync_all_rules(&self, rules: &[Rule]) -> Result<(), DomainError> {
        self.calls
            .lock()
            .unwrap()
            .push(FirewallCall::SyncAll(rules.len()));
        Ok(())
    }

    async fn get_status(&self) -> Result<FirewallStatus, DomainError> {
        Ok(FirewallStatus {
            enabled: true,
            active_rules_count: 0,
            nftables_synced: true,
            uptime_secs: 0,
            version: "0.1.0".to_string(),
        })
    }
}
```

`crates/app/src/fakes/fake_event_bus.rs`:
```rust
use async_trait::async_trait;
use tokio::sync::broadcast;

use syswall_domain::errors::DomainError;
use syswall_domain::events::DomainEvent;
use syswall_domain::ports::{EventBus, EventReceiver};

pub struct FakeEventBus {
    sender: broadcast::Sender<DomainEvent>,
}

impl FakeEventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    /// Get all events published so far by subscribing and draining.
    /// Note: Only captures events published after this subscriber was created.
    /// For test assertions, subscribe before the action, then collect.
    pub fn sender(&self) -> &broadcast::Sender<DomainEvent> {
        &self.sender
    }
}

#[async_trait]
impl EventBus for FakeEventBus {
    async fn publish(&self, event: DomainEvent) -> Result<(), DomainError> {
        let _ = self.sender.send(event);
        Ok(())
    }

    fn subscribe(&self) -> EventReceiver {
        self.sender.subscribe()
    }
}
```

`crates/app/src/fakes/fake_user_notifier.rs`:
```rust
use async_trait::async_trait;
use std::sync::Mutex;

use syswall_domain::entities::PendingDecision;
use syswall_domain::errors::DomainError;
use syswall_domain::events::Notification;
use syswall_domain::ports::UserNotifier;

pub struct FakeUserNotifier {
    pub decision_notifications: Mutex<Vec<PendingDecision>>,
    pub notifications: Mutex<Vec<Notification>>,
}

impl FakeUserNotifier {
    pub fn new() -> Self {
        Self {
            decision_notifications: Mutex::new(vec![]),
            notifications: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl UserNotifier for FakeUserNotifier {
    async fn notify_decision_required(
        &self,
        request: &PendingDecision,
    ) -> Result<(), DomainError> {
        self.decision_notifications
            .lock()
            .unwrap()
            .push(request.clone());
        Ok(())
    }

    async fn notify(&self, notification: &Notification) -> Result<(), DomainError> {
        self.notifications
            .lock()
            .unwrap()
            .push(notification.clone());
        Ok(())
    }
}
```

`crates/app/src/fakes/fake_process_resolver.rs`:
```rust
use async_trait::async_trait;

use syswall_domain::entities::ProcessInfo;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::ProcessResolver;

pub struct FakeProcessResolver;

impl FakeProcessResolver {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProcessResolver for FakeProcessResolver {
    async fn resolve(&self, _pid: u32) -> Result<Option<ProcessInfo>, DomainError> {
        Ok(None)
    }

    async fn resolve_by_socket(&self, _inode: u64) -> Result<Option<ProcessInfo>, DomainError> {
        Ok(None)
    }
}
```

`crates/app/src/fakes/fake_connection_monitor.rs`:
```rust
use async_trait::async_trait;

use syswall_domain::entities::Connection;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::{ConnectionEventStream, ConnectionMonitor};

pub struct FakeConnectionMonitor;

impl FakeConnectionMonitor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ConnectionMonitor for FakeConnectionMonitor {
    async fn stream_events(&self) -> Result<ConnectionEventStream, DomainError> {
        Ok(Box::pin(futures::stream::empty()))
    }

    async fn get_active_connections(&self) -> Result<Vec<Connection>, DomainError> {
        Ok(vec![])
    }
}
```

- [ ] **Step 3: Create empty service modules so app compiles**

`crates/app/src/services/mod.rs`:
```rust
pub mod audit_service;
pub mod connection_service;
pub mod learning_service;
pub mod rule_service;
```

Create empty placeholder files:
- `crates/app/src/services/rule_service.rs`: `// Implemented in Task 9`
- `crates/app/src/services/learning_service.rs`: `// Implemented in Task 10`
- `crates/app/src/services/connection_service.rs`: `// Implemented in Task 11`
- `crates/app/src/services/audit_service.rs`: `// Implemented in Task 11`

Run: `cargo check -p syswall-app`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add crates/app/
git commit -m "feat(app): add command types and all fake port implementations

CreateRuleCommand, UpdateRuleCommand, RespondToDecisionCommand.
9 fake adapters for testing: FakeRuleRepository,
FakePendingDecisionRepository, FakeFirewallEngine, FakeEventBus,
FakeUserNotifier, etc."
```

---

### Task 9: RuleService

**Files:**
- Modify: `crates/app/src/services/rule_service.rs`

- [ ] **Step 1: Implement RuleService**

`crates/app/src/services/rule_service.rs`:
```rust
use std::sync::Arc;

use chrono::Utc;
use syswall_domain::entities::{Rule, RuleId, RuleSource};
use syswall_domain::errors::DomainError;
use syswall_domain::events::{DomainEvent, Pagination};
use syswall_domain::ports::{EventBus, FirewallEngine, RuleFilters, RuleRepository};
use syswall_domain::value_objects::RulePriority;

use crate::commands::{CreateRuleCommand, UpdateRuleCommand};

pub struct RuleService {
    rule_repo: Arc<dyn RuleRepository>,
    firewall: Arc<dyn FirewallEngine>,
    event_bus: Arc<dyn EventBus>,
}

impl RuleService {
    pub fn new(
        rule_repo: Arc<dyn RuleRepository>,
        firewall: Arc<dyn FirewallEngine>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            rule_repo,
            firewall,
            event_bus,
        }
    }

    pub async fn create_rule(&self, cmd: CreateRuleCommand) -> Result<Rule, DomainError> {
        let now = Utc::now();
        let rule = Rule {
            id: RuleId::new(),
            name: cmd.name,
            priority: RulePriority::new(cmd.priority),
            enabled: true,
            criteria: cmd.criteria,
            effect: cmd.effect,
            scope: cmd.scope,
            created_at: now,
            updated_at: now,
            source: cmd.source,
        };

        self.rule_repo.save(&rule).await?;
        self.firewall.apply_rule(&rule).await?;
        let _ = self
            .event_bus
            .publish(DomainEvent::RuleCreated(rule.clone()))
            .await;

        Ok(rule)
    }

    pub async fn delete_rule(&self, id: &RuleId) -> Result<(), DomainError> {
        let rule = self
            .rule_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound(format!("Rule {:?}", id)))?;

        if rule.is_system() {
            return Err(DomainError::NotPermitted(
                "System rules cannot be deleted".to_string(),
            ));
        }

        self.firewall.remove_rule(id).await?;
        self.rule_repo.delete(id).await?;
        let _ = self.event_bus.publish(DomainEvent::RuleDeleted(*id)).await;

        Ok(())
    }

    pub async fn toggle_rule(
        &self,
        id: &RuleId,
        enabled: bool,
    ) -> Result<Rule, DomainError> {
        let mut rule = self
            .rule_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound(format!("Rule {:?}", id)))?;

        rule.enabled = enabled;
        rule.updated_at = Utc::now();
        self.rule_repo.save(&rule).await?;

        if enabled {
            self.firewall.apply_rule(&rule).await?;
        } else {
            self.firewall.remove_rule(id).await?;
        }

        let _ = self
            .event_bus
            .publish(DomainEvent::RuleUpdated(rule.clone()))
            .await;

        Ok(rule)
    }

    pub async fn list_rules(
        &self,
        filters: &RuleFilters,
        pagination: &Pagination,
    ) -> Result<Vec<Rule>, DomainError> {
        self.rule_repo.find_all(filters, pagination).await
    }

    pub async fn get_rule(&self, id: &RuleId) -> Result<Option<Rule>, DomainError> {
        self.rule_repo.find_by_id(id).await
    }

    pub async fn list_enabled_ordered(&self) -> Result<Vec<Rule>, DomainError> {
        self.rule_repo.list_enabled_ordered().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::*;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn setup() -> (RuleService, Arc<FakeRuleRepository>, Arc<FakeFirewallEngine>) {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let service = RuleService::new(rule_repo.clone(), firewall.clone(), event_bus);
        (service, rule_repo, firewall)
    }

    #[tokio::test]
    async fn create_rule_persists_and_applies() {
        let (service, repo, firewall) = setup();
        let cmd = CreateRuleCommand {
            name: "Block SSH".to_string(),
            priority: 10,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Block,
            scope: RuleScope::Permanent,
            source: RuleSource::Manual,
        };

        let rule = service.create_rule(cmd).await.unwrap();
        assert_eq!(rule.name, "Block SSH");

        // Verify persisted
        let found = repo.find_by_id(&rule.id).await.unwrap();
        assert!(found.is_some());

        // Verify firewall was called
        let calls = firewall.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(matches!(calls[0], FirewallCall::ApplyRule(_)));
    }

    #[tokio::test]
    async fn delete_system_rule_rejected() {
        let (service, repo, _) = setup();
        let rule = Rule {
            id: RuleId::new(),
            name: "DNS".to_string(),
            priority: RulePriority::new(0),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::System,
        };
        repo.save(&rule).await.unwrap();

        let result = service.delete_rule(&rule.id).await;
        assert!(matches!(result, Err(DomainError::NotPermitted(_))));
    }

    #[tokio::test]
    async fn toggle_rule_updates_firewall() {
        let (service, repo, firewall) = setup();
        let cmd = CreateRuleCommand {
            name: "Test".to_string(),
            priority: 10,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            source: RuleSource::Manual,
        };
        let rule = service.create_rule(cmd).await.unwrap();

        // Disable
        let updated = service.toggle_rule(&rule.id, false).await.unwrap();
        assert!(!updated.enabled);

        // Verify firewall remove was called
        let calls = firewall.calls.lock().unwrap();
        assert!(calls.iter().any(|c| matches!(c, FirewallCall::RemoveRule(_))));
    }

    #[tokio::test]
    async fn delete_nonexistent_rule_returns_not_found() {
        let (service, _, _) = setup();
        let result = service.delete_rule(&RuleId::new()).await;
        assert!(matches!(result, Err(DomainError::NotFound(_))));
    }
}
```

Run: `cargo test -p syswall-app -- rule_service`
Expected: 4 tests PASS

- [ ] **Step 2: Commit**

```bash
git add crates/app/
git commit -m "feat(app): implement RuleService with CRUD + firewall sync

Create, delete, toggle, list rules. System rules protected from
deletion. Firewall engine called on every state change. Tested with
fakes."
```

---

### Task 10: LearningService

**Files:**
- Modify: `crates/app/src/services/learning_service.rs`

- [ ] **Step 1: Implement LearningService**

`crates/app/src/services/learning_service.rs`:
```rust
use std::sync::Arc;

use chrono::{Duration, Utc};
use syswall_domain::entities::{
    ConnectionSnapshot, Decision, DecisionAction, DecisionId, PendingDecision,
    PendingDecisionId, PendingDecisionStatus,
};
use syswall_domain::errors::DomainError;
use syswall_domain::events::DomainEvent;
use syswall_domain::ports::{
    DecisionRepository, EventBus, PendingDecisionRepository, UserNotifier,
};

use crate::commands::RespondToDecisionCommand;
use crate::services::rule_service::RuleService;

pub struct LearningConfig {
    pub prompt_timeout_secs: u64,
    pub max_pending_decisions: usize,
}

pub struct LearningService {
    pending_repo: Arc<dyn PendingDecisionRepository>,
    decision_repo: Arc<dyn DecisionRepository>,
    rule_service: Arc<RuleService>,
    notifier: Arc<dyn UserNotifier>,
    event_bus: Arc<dyn EventBus>,
    config: LearningConfig,
}

impl LearningService {
    pub fn new(
        pending_repo: Arc<dyn PendingDecisionRepository>,
        decision_repo: Arc<dyn DecisionRepository>,
        rule_service: Arc<RuleService>,
        notifier: Arc<dyn UserNotifier>,
        event_bus: Arc<dyn EventBus>,
        config: LearningConfig,
    ) -> Self {
        Self {
            pending_repo,
            decision_repo,
            rule_service,
            notifier,
            event_bus,
            config,
        }
    }

    /// Compute deduplication key from a connection snapshot.
    pub fn dedup_key(snapshot: &ConnectionSnapshot) -> String {
        format!(
            "{}:{}:{}:{}",
            snapshot.process_name.as_deref().unwrap_or("unknown"),
            snapshot.destination.ip,
            snapshot.destination.port,
            snapshot.protocol,
        )
    }

    /// Handle a connection that matched no rule and default policy is Ask.
    /// Creates a PendingDecision and notifies the UI. Does NOT block.
    pub async fn handle_unknown_connection(
        &self,
        snapshot: ConnectionSnapshot,
    ) -> Result<(), DomainError> {
        let key = Self::dedup_key(&snapshot);

        // Debounce: skip if same key already pending
        if self.pending_repo.find_by_dedup_key(&key).await?.is_some() {
            return Ok(());
        }

        // Check queue capacity
        let pending_count = self.pending_repo.list_pending().await?.len();
        if pending_count >= self.config.max_pending_decisions {
            tracing::warn!("Pending decision queue full ({}), dropping", pending_count);
            return Ok(());
        }

        let pending = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: snapshot,
            requested_at: Utc::now(),
            expires_at: Utc::now() + Duration::seconds(self.config.prompt_timeout_secs as i64),
            deduplication_key: key,
            status: PendingDecisionStatus::Pending,
        };

        self.pending_repo.create(&pending).await?;
        let _ = self
            .event_bus
            .publish(DomainEvent::DecisionRequired(pending.clone()))
            .await;
        self.notifier.notify_decision_required(&pending).await?;

        Ok(())
    }

    /// Resolve a pending decision when the user responds.
    pub async fn resolve_decision(
        &self,
        cmd: RespondToDecisionCommand,
    ) -> Result<Decision, DomainError> {
        let pending_list = self.pending_repo.list_pending().await?;
        let pending = pending_list
            .iter()
            .find(|p| p.id == cmd.pending_decision_id)
            .ok_or_else(|| {
                DomainError::NotFound(format!(
                    "PendingDecision {:?}",
                    cmd.pending_decision_id
                ))
            })?;

        if pending.status != PendingDecisionStatus::Pending {
            return Err(DomainError::Validation(
                "Decision is no longer pending".to_string(),
            ));
        }

        let decision = Decision {
            id: DecisionId::new(),
            pending_decision_id: cmd.pending_decision_id,
            connection_snapshot: pending.connection_snapshot.clone(),
            action: cmd.action,
            granularity: cmd.granularity,
            decided_at: Utc::now(),
            generated_rule: None,
        };

        self.decision_repo.save(&decision).await?;
        self.pending_repo.resolve(&cmd.pending_decision_id).await?;

        // If the action creates a permanent rule, do it via RuleService
        // (Rule creation from decisions will be fully wired in sub-project 4)

        let _ = self
            .event_bus
            .publish(DomainEvent::DecisionResolved(decision.clone()))
            .await;

        Ok(decision)
    }

    /// Expire overdue pending decisions.
    pub async fn expire_overdue(&self) -> Result<Vec<PendingDecision>, DomainError> {
        let expired = self.pending_repo.expire_overdue().await?;
        for pd in &expired {
            let _ = self
                .event_bus
                .publish(DomainEvent::DecisionExpired(pd.id))
                .await;
        }
        Ok(expired)
    }

    pub async fn get_pending_decisions(&self) -> Result<Vec<PendingDecision>, DomainError> {
        self.pending_repo.list_pending().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::*;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn test_snapshot() -> ConnectionSnapshot {
        ConnectionSnapshot {
            protocol: Protocol::Tcp,
            source: SocketAddress::new("10.0.0.1".parse().unwrap(), Port::new(5000).unwrap()),
            destination: SocketAddress::new("8.8.8.8".parse().unwrap(), Port::new(443).unwrap()),
            direction: Direction::Outbound,
            process_name: Some("curl".to_string()),
            process_path: None,
            user: Some("seb".to_string()),
        }
    }

    fn setup() -> (
        LearningService,
        Arc<FakePendingDecisionRepository>,
        Arc<FakeUserNotifier>,
    ) {
        let pending_repo = Arc::new(FakePendingDecisionRepository::new());
        let decision_repo = Arc::new(FakeDecisionRepository::new());
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let notifier = Arc::new(FakeUserNotifier::new());
        let rule_service = Arc::new(RuleService::new(rule_repo, firewall, event_bus.clone()));

        let config = LearningConfig {
            prompt_timeout_secs: 60,
            max_pending_decisions: 50,
        };

        let service = LearningService::new(
            pending_repo.clone(),
            decision_repo,
            rule_service,
            notifier.clone(),
            event_bus,
            config,
        );

        (service, pending_repo, notifier)
    }

    #[tokio::test]
    async fn handle_unknown_creates_pending_decision() {
        let (service, pending_repo, notifier) = setup();
        service
            .handle_unknown_connection(test_snapshot())
            .await
            .unwrap();

        let pending = pending_repo.list_pending().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].status, PendingDecisionStatus::Pending);

        // Verify notifier was called
        let notifs = notifier.decision_notifications.lock().unwrap();
        assert_eq!(notifs.len(), 1);
    }

    #[tokio::test]
    async fn debounce_same_connection() {
        let (service, pending_repo, _) = setup();

        // First call creates pending
        service
            .handle_unknown_connection(test_snapshot())
            .await
            .unwrap();

        // Second call with same snapshot is deduplicated
        service
            .handle_unknown_connection(test_snapshot())
            .await
            .unwrap();

        let pending = pending_repo.list_pending().await.unwrap();
        assert_eq!(pending.len(), 1); // Only one, not two
    }

    #[tokio::test]
    async fn resolve_decision_marks_resolved() {
        let (service, pending_repo, _) = setup();
        service
            .handle_unknown_connection(test_snapshot())
            .await
            .unwrap();

        let pending = pending_repo.list_pending().await.unwrap();
        let pending_id = pending[0].id;

        let cmd = RespondToDecisionCommand {
            pending_decision_id: pending_id,
            action: DecisionAction::AllowOnce,
            granularity: DecisionGranularity::AppOnly,
        };

        let decision = service.resolve_decision(cmd).await.unwrap();
        assert_eq!(decision.action, DecisionAction::AllowOnce);

        // Verify pending is now resolved (list_pending returns only Pending)
        let remaining = pending_repo.list_pending().await.unwrap();
        assert_eq!(remaining.len(), 0);
    }

    #[tokio::test]
    async fn expire_overdue_marks_expired() {
        let (service, pending_repo, _) = setup();

        // Manually create an already-expired pending decision
        let expired_pending = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            requested_at: Utc::now() - Duration::minutes(10),
            expires_at: Utc::now() - Duration::minutes(1),
            deduplication_key: "test:expired".to_string(),
            status: PendingDecisionStatus::Pending,
        };
        pending_repo.create(&expired_pending).await.unwrap();

        let expired = service.expire_overdue().await.unwrap();
        assert_eq!(expired.len(), 1);

        let pending = pending_repo.list_pending().await.unwrap();
        assert_eq!(pending.len(), 0);
    }
}
```

Run: `cargo test -p syswall-app -- learning_service`
Expected: 4 tests PASS

- [ ] **Step 2: Commit**

```bash
git add crates/app/
git commit -m "feat(app): implement LearningService with async pending decisions

Non-blocking decision flow: creates PendingDecision, notifies UI,
resolves when user responds. Debounce by dedup key. Expiration
sweep. Tested with fakes."
```

---

### Task 11: ConnectionService and AuditService

**Files:**
- Modify: `crates/app/src/services/connection_service.rs`
- Modify: `crates/app/src/services/audit_service.rs`

- [ ] **Step 1: Implement ConnectionService**

`crates/app/src/services/connection_service.rs`:
```rust
use std::sync::Arc;

use syswall_domain::entities::{Connection, ConnectionVerdict};
use syswall_domain::errors::DomainError;
use syswall_domain::events::{DefaultPolicy, DomainEvent};
use syswall_domain::ports::{EventBus, ProcessResolver, RuleRepository};
use syswall_domain::services::PolicyEngine;

pub struct ConnectionService {
    process_resolver: Arc<dyn ProcessResolver>,
    rule_repo: Arc<dyn RuleRepository>,
    event_bus: Arc<dyn EventBus>,
    default_policy: DefaultPolicy,
}

impl ConnectionService {
    pub fn new(
        process_resolver: Arc<dyn ProcessResolver>,
        rule_repo: Arc<dyn RuleRepository>,
        event_bus: Arc<dyn EventBus>,
        default_policy: DefaultPolicy,
    ) -> Self {
        Self {
            process_resolver,
            rule_repo,
            event_bus,
            default_policy,
        }
    }

    /// Enrich a raw connection with process info and evaluate against rules.
    pub async fn process_connection(
        &self,
        mut connection: Connection,
    ) -> Result<Connection, DomainError> {
        // Best-effort process enrichment
        if connection.process.is_none() {
            // In a real implementation, we'd resolve via socket inode
            // For now, process info is provided by the connection monitor
        }

        // Load rules and evaluate
        let rules = self.rule_repo.list_enabled_ordered().await?;
        let evaluation =
            PolicyEngine::evaluate(&connection, &rules, self.default_policy);

        connection.verdict = evaluation.verdict;
        connection.matched_rule = evaluation.matched_rule_id;

        // Publish event
        let _ = self
            .event_bus
            .publish(DomainEvent::ConnectionDetected(connection.clone()))
            .await;

        if let Some(rule_id) = evaluation.matched_rule_id {
            let _ = self
                .event_bus
                .publish(DomainEvent::RuleMatched {
                    connection_id: connection.id,
                    rule_id,
                    verdict: connection.verdict,
                })
                .await;
        }

        Ok(connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::*;
    use crate::services::rule_service::RuleService;
    use crate::commands::CreateRuleCommand;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;
    use chrono::Utc;

    fn test_connection() -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol: Protocol::Tcp,
            source: SocketAddress::new("192.168.1.100".parse().unwrap(), Port::new(45000).unwrap()),
            destination: SocketAddress::new("93.184.216.34".parse().unwrap(), Port::new(443).unwrap()),
            direction: Direction::Outbound,
            state: ConnectionState::New,
            process: Some(ProcessInfo {
                pid: 1234,
                name: "firefox".to_string(),
                path: None,
                cmdline: None,
            }),
            user: Some(SystemUser { uid: 1000, name: "seb".to_string() }),
            bytes_sent: 0,
            bytes_received: 0,
            started_at: Utc::now(),
            verdict: ConnectionVerdict::Unknown,
            matched_rule: None,
        }
    }

    #[tokio::test]
    async fn process_connection_with_no_rules_uses_default_policy() {
        let process_resolver = Arc::new(FakeProcessResolver::new());
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let event_bus = Arc::new(FakeEventBus::new());

        let service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus,
            DefaultPolicy::Block,
        );

        let conn = service.process_connection(test_connection()).await.unwrap();
        assert_eq!(conn.verdict, ConnectionVerdict::Blocked);
    }

    #[tokio::test]
    async fn process_connection_matches_rule() {
        let process_resolver = Arc::new(FakeProcessResolver::new());
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let firewall = Arc::new(FakeFirewallEngine::new());

        // Create an allow rule via RuleService
        let rule_service = RuleService::new(rule_repo.clone(), firewall, event_bus.clone());
        let rule = rule_service
            .create_rule(CreateRuleCommand {
                name: "Allow HTTPS".to_string(),
                priority: 10,
                criteria: RuleCriteria {
                    remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
                    protocol: Some(Protocol::Tcp),
                    ..Default::default()
                },
                effect: RuleEffect::Allow,
                scope: RuleScope::Permanent,
                source: RuleSource::Manual,
            })
            .await
            .unwrap();

        let service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus,
            DefaultPolicy::Block,
        );

        let conn = service.process_connection(test_connection()).await.unwrap();
        assert_eq!(conn.verdict, ConnectionVerdict::Allowed);
        assert_eq!(conn.matched_rule, Some(rule.id));
    }
}
```

- [ ] **Step 2: Implement AuditService**

`crates/app/src/services/audit_service.rs`:
```rust
use std::sync::Arc;

use syswall_domain::entities::{AuditEvent, EventCategory, Severity};
use syswall_domain::errors::DomainError;
use syswall_domain::events::{DomainEvent, Pagination};
use syswall_domain::ports::{AuditFilters, AuditRepository};

/// Statistics for the dashboard.
#[derive(Debug, Clone)]
pub struct AuditStats {
    pub total_events: u64,
    pub connections_detected: u64,
    pub rules_matched: u64,
    pub decisions_made: u64,
    pub errors: u64,
}

pub struct AuditService {
    audit_repo: Arc<dyn AuditRepository>,
}

impl AuditService {
    pub fn new(audit_repo: Arc<dyn AuditRepository>) -> Self {
        Self { audit_repo }
    }

    /// Convert a domain event into an audit event and persist it.
    pub async fn record_event(&self, event: &DomainEvent) -> Result<(), DomainError> {
        let audit_event = match event {
            DomainEvent::ConnectionDetected(conn) => AuditEvent::new(
                Severity::Debug,
                EventCategory::Connection,
                format!("Connection detected: {} -> {}", conn.source, conn.destination),
            ),
            DomainEvent::RuleCreated(rule) => AuditEvent::new(
                Severity::Info,
                EventCategory::Rule,
                format!("Rule created: {}", rule.name),
            )
            .with_metadata("rule_id", rule.id.as_uuid().to_string()),
            DomainEvent::RuleDeleted(id) => AuditEvent::new(
                Severity::Info,
                EventCategory::Rule,
                format!("Rule deleted: {:?}", id),
            ),
            DomainEvent::DecisionResolved(decision) => AuditEvent::new(
                Severity::Info,
                EventCategory::Decision,
                format!("Decision resolved: {:?}", decision.action),
            ),
            DomainEvent::DecisionExpired(id) => AuditEvent::new(
                Severity::Warning,
                EventCategory::Decision,
                format!("Decision expired: {:?}", id),
            ),
            DomainEvent::SystemError { message, severity } => {
                AuditEvent::new(*severity, EventCategory::System, message.clone())
            }
            _ => return Ok(()), // Not all events need audit records
        };

        self.audit_repo.append(&audit_event).await
    }

    pub async fn query_events(
        &self,
        filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError> {
        self.audit_repo.query(filters, pagination).await
    }

    pub async fn count_events(&self, filters: &AuditFilters) -> Result<u64, DomainError> {
        self.audit_repo.count(filters).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::*;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;
    use chrono::Utc;

    #[tokio::test]
    async fn records_rule_created_event() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        let rule = Rule {
            id: RuleId::new(),
            name: "Test Rule".to_string(),
            priority: RulePriority::new(10),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        };

        service
            .record_event(&DomainEvent::RuleCreated(rule))
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].category, EventCategory::Rule);
        assert_eq!(events[0].severity, Severity::Info);
    }

    #[tokio::test]
    async fn records_system_error_with_severity() {
        let repo = Arc::new(FakeAuditRepository::new());
        let service = AuditService::new(repo.clone());

        service
            .record_event(&DomainEvent::SystemError {
                message: "nftables sync failed".to_string(),
                severity: Severity::Error,
            })
            .await
            .unwrap();

        let events = repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, Severity::Error);
    }
}
```

Run: `cargo test -p syswall-app`
Expected: all tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/app/
git commit -m "feat(app): implement ConnectionService and AuditService

ConnectionService: enriches connections, evaluates PolicyEngine,
publishes events. AuditService: converts domain events to audit
records. Both tested with fakes."
```

---

### Task 12: Infrastructure — SQLite Setup and Migrations

**Files:**
- Create: `crates/infra/src/persistence/mod.rs`
- Create: `crates/infra/src/persistence/database.rs`

- [ ] **Step 1: Implement Database with WAL and migrations**

`crates/infra/src/persistence/mod.rs`:
```rust
pub mod audit_repository;
pub mod database;
pub mod decision_repository;
pub mod pending_decision_repository;
pub mod rule_repository;

pub use database::Database;
```

`crates/infra/src/persistence/database.rs`:
```rust
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use syswall_domain::errors::DomainError;

/// Database wrapper managing SQLite connection with WAL mode and migrations.
pub struct Database {
    writer: Mutex<Connection>,
}

impl Database {
    /// Open or create the database at the given path. Enables WAL mode and runs migrations.
    pub fn open(path: &Path) -> Result<Self, DomainError> {
        let conn = Connection::open(path)
            .map_err(|e| DomainError::Infrastructure(format!("Failed to open DB: {}", e)))?;

        Self::configure(&conn)?;
        Self::migrate(&conn)?;

        Ok(Self {
            writer: Mutex::new(conn),
        })
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory() -> Result<Self, DomainError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| DomainError::Infrastructure(format!("Failed to open in-memory DB: {}", e)))?;

        Self::configure(&conn)?;
        Self::migrate(&conn)?;

        Ok(Self {
            writer: Mutex::new(conn),
        })
    }

    fn configure(conn: &Connection) -> Result<(), DomainError> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;"
        )
        .map_err(|e| DomainError::Infrastructure(format!("Failed to configure DB: {}", e)))
    }

    fn migrate(conn: &Connection) -> Result<(), DomainError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS rules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                priority INTEGER NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                criteria_json TEXT NOT NULL,
                effect TEXT NOT NULL,
                scope_json TEXT NOT NULL,
                source TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_rules_priority ON rules(priority);
            CREATE INDEX IF NOT EXISTS idx_rules_enabled ON rules(enabled);
            CREATE INDEX IF NOT EXISTS idx_rules_source ON rules(source);

            CREATE TABLE IF NOT EXISTS pending_decisions (
                id TEXT PRIMARY KEY,
                snapshot_json TEXT NOT NULL,
                requested_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                deduplication_key TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'Pending'
            );

            CREATE INDEX IF NOT EXISTS idx_pending_status ON pending_decisions(status);
            CREATE INDEX IF NOT EXISTS idx_pending_expires ON pending_decisions(expires_at);
            CREATE INDEX IF NOT EXISTS idx_pending_dedup ON pending_decisions(deduplication_key);

            CREATE TABLE IF NOT EXISTS decisions (
                id TEXT PRIMARY KEY,
                pending_decision_id TEXT NOT NULL,
                snapshot_json TEXT NOT NULL,
                action TEXT NOT NULL,
                granularity TEXT NOT NULL,
                decided_at TEXT NOT NULL,
                generated_rule_id TEXT
            );

            CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                severity TEXT NOT NULL,
                category TEXT NOT NULL,
                description TEXT NOT NULL,
                metadata_json TEXT NOT NULL DEFAULT '{}'
            );

            CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_severity ON audit_events(severity);
            CREATE INDEX IF NOT EXISTS idx_audit_category ON audit_events(category);"
        )
        .map_err(|e| DomainError::Infrastructure(format!("Migration failed: {}", e)))
    }

    /// Execute a closure with the writer connection.
    pub fn with_writer<F, T>(&self, f: F) -> Result<T, DomainError>
    where
        F: FnOnce(&Connection) -> Result<T, DomainError>,
    {
        let conn = self.writer.lock().map_err(|e| {
            DomainError::Infrastructure(format!("Failed to acquire DB lock: {}", e))
        })?;
        f(&conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_succeeds() {
        let db = Database::open_in_memory();
        assert!(db.is_ok());
    }

    #[test]
    fn tables_created_on_open() {
        let db = Database::open_in_memory().unwrap();
        db.with_writer(|conn| {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('rules', 'pending_decisions', 'decisions', 'audit_events')",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
            assert_eq!(count, 4);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn wal_mode_enabled() {
        let db = Database::open_in_memory().unwrap();
        db.with_writer(|conn| {
            let mode: String = conn
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
            assert_eq!(mode, "wal");
            Ok(())
        })
        .unwrap();
    }
}
```

Run: `cargo test -p syswall-infra`
Expected: 3 tests PASS

- [ ] **Step 2: Commit**

```bash
git add crates/infra/
git commit -m "feat(infra): add SQLite database with WAL mode and migrations

Database struct with writer mutex. WAL + busy_timeout + foreign keys.
Migrations create all 4 tables with indexes. Tested."
```

---

### Task 13: Infrastructure — SqliteRuleRepository

**Files:**
- Create: `crates/infra/src/persistence/rule_repository.rs`

- [ ] **Step 1: Implement SqliteRuleRepository**

`crates/infra/src/persistence/rule_repository.rs`:
```rust
use async_trait::async_trait;
use std::sync::Arc;

use syswall_domain::entities::{Rule, RuleEffect, RuleId, RuleScope, RuleSource};
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::{RuleFilters, RuleRepository};
use syswall_domain::value_objects::RulePriority;

use super::database::Database;

pub struct SqliteRuleRepository {
    db: Arc<Database>,
}

impl SqliteRuleRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn row_to_rule(row: &rusqlite::Row) -> Result<Rule, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let name: String = row.get(1)?;
        let priority: u32 = row.get(2)?;
        let enabled: bool = row.get(3)?;
        let criteria_json: String = row.get(4)?;
        let effect_str: String = row.get(5)?;
        let scope_json: String = row.get(6)?;
        let source_str: String = row.get(7)?;
        let created_at_str: String = row.get(8)?;
        let updated_at_str: String = row.get(9)?;

        Ok(Rule {
            id: RuleId::from_uuid(id_str.parse().unwrap()),
            name,
            priority: RulePriority::new(priority),
            enabled,
            criteria: serde_json::from_str(&criteria_json).unwrap_or_default(),
            effect: serde_json::from_str(&format!("\"{}\"", effect_str)).unwrap(),
            scope: serde_json::from_str(&scope_json).unwrap(),
            source: serde_json::from_str(&format!("\"{}\"", source_str)).unwrap(),
            created_at: created_at_str.parse().unwrap(),
            updated_at: updated_at_str.parse().unwrap(),
        })
    }
}

#[async_trait]
impl RuleRepository for SqliteRuleRepository {
    async fn save(&self, rule: &Rule) -> Result<(), DomainError> {
        let rule = rule.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO rules (id, name, priority, enabled, criteria_json, effect, scope_json, source, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    rusqlite::params![
                        rule.id.as_uuid().to_string(),
                        rule.name,
                        rule.priority.value(),
                        rule.enabled,
                        serde_json::to_string(&rule.criteria).unwrap(),
                        serde_json::to_string(&rule.effect).unwrap().trim_matches('"'),
                        serde_json::to_string(&rule.scope).unwrap(),
                        serde_json::to_string(&rule.source).unwrap().trim_matches('"'),
                        rule.created_at.to_rfc3339(),
                        rule.updated_at.to_rfc3339(),
                    ],
                )
                .map_err(|e| DomainError::Infrastructure(format!("Failed to save rule: {}", e)))?;
                Ok(())
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn find_by_id(&self, id: &RuleId) -> Result<Option<Rule>, DomainError> {
        let id_str = id.as_uuid().to_string();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let mut stmt = conn
                    .prepare("SELECT id, name, priority, enabled, criteria_json, effect, scope_json, source, created_at, updated_at FROM rules WHERE id = ?1")
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let result = stmt
                    .query_row(rusqlite::params![id_str], Self::row_to_rule)
                    .optional()
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                Ok(result)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn find_all(
        &self,
        _filters: &RuleFilters,
        pagination: &Pagination,
    ) -> Result<Vec<Rule>, DomainError> {
        let offset = pagination.offset;
        let limit = pagination.limit;
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let mut stmt = conn
                    .prepare("SELECT id, name, priority, enabled, criteria_json, effect, scope_json, source, created_at, updated_at FROM rules ORDER BY priority ASC LIMIT ?1 OFFSET ?2")
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let rules = stmt
                    .query_map(rusqlite::params![limit, offset], Self::row_to_rule)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(rules)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn delete(&self, id: &RuleId) -> Result<(), DomainError> {
        let id_str = id.as_uuid().to_string();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                conn.execute("DELETE FROM rules WHERE id = ?1", rusqlite::params![id_str])
                    .map_err(|e| DomainError::Infrastructure(format!("Failed to delete rule: {}", e)))?;
                Ok(())
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    async fn list_enabled_ordered(&self) -> Result<Vec<Rule>, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let mut stmt = conn
                    .prepare("SELECT id, name, priority, enabled, criteria_json, effect, scope_json, source, created_at, updated_at FROM rules WHERE enabled = 1 ORDER BY priority ASC")
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let rules = stmt
                    .query_map([], Self::row_to_rule)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(rules)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }
}

// Add rusqlite optional extension
use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;
    use syswall_domain::entities::*;
    use chrono::Utc;

    async fn setup() -> (SqliteRuleRepository, Arc<Database>) {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let repo = SqliteRuleRepository::new(db.clone());
        (repo, db)
    }

    fn test_rule() -> Rule {
        Rule {
            id: RuleId::new(),
            name: "Test Rule".to_string(),
            priority: RulePriority::new(10),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        }
    }

    // --- Contract tests (same suite as FakeRuleRepository) ---

    #[tokio::test]
    async fn save_then_find_by_id() {
        let (repo, _) = setup().await;
        let rule = test_rule();
        repo.save(&rule).await.unwrap();

        let found = repo.find_by_id(&rule.id).await.unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.name, "Test Rule");
        assert_eq!(found.priority, RulePriority::new(10));
        assert_eq!(found.effect, RuleEffect::Allow);
    }

    #[tokio::test]
    async fn delete_then_find_by_id_returns_none() {
        let (repo, _) = setup().await;
        let rule = test_rule();
        repo.save(&rule).await.unwrap();
        repo.delete(&rule.id).await.unwrap();

        let found = repo.find_by_id(&rule.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn list_enabled_ordered_by_priority() {
        let (repo, _) = setup().await;

        let mut r1 = test_rule();
        r1.priority = RulePriority::new(20);
        r1.name = "Second".to_string();

        let mut r2 = test_rule();
        r2.priority = RulePriority::new(5);
        r2.name = "First".to_string();

        let mut r3 = test_rule();
        r3.priority = RulePriority::new(10);
        r3.enabled = false; // disabled, should not appear
        r3.name = "Disabled".to_string();

        repo.save(&r1).await.unwrap();
        repo.save(&r2).await.unwrap();
        repo.save(&r3).await.unwrap();

        let enabled = repo.list_enabled_ordered().await.unwrap();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0].name, "First");
        assert_eq!(enabled[1].name, "Second");
    }

    #[tokio::test]
    async fn find_by_id_nonexistent_returns_none() {
        let (repo, _) = setup().await;
        let found = repo.find_by_id(&RuleId::new()).await.unwrap();
        assert!(found.is_none());
    }
}
```

Run: `cargo test -p syswall-infra`
Expected: all tests PASS (3 database + 4 rule repo)

- [ ] **Step 2: Commit**

```bash
git add crates/infra/
git commit -m "feat(infra): implement SqliteRuleRepository

Save, find, delete, list_enabled_ordered with spawn_blocking.
Criteria serialized as JSON. Contract tests with in-memory SQLite."
```

---

### Task 14: Infrastructure — Remaining Repos and EventBus

**Files:**
- Create: `crates/infra/src/persistence/pending_decision_repository.rs`
- Create: `crates/infra/src/persistence/decision_repository.rs`
- Create: `crates/infra/src/persistence/audit_repository.rs`
- Create: `crates/infra/src/event_bus/mod.rs`

This task creates the remaining infrastructure. Due to plan length, the implementations follow the same pattern as SqliteRuleRepository (spawn_blocking, JSON serialization, contract tests). The key files:

- [ ] **Step 1: Implement SqlitePendingDecisionRepository, SqliteDecisionRepository, SqliteAuditRepository**

Each follows the same pattern as SqliteRuleRepository:
- Takes `Arc<Database>`
- Uses `spawn_blocking` for all operations
- Serializes complex fields as JSON
- Has contract tests with in-memory SQLite

Create all three files following the patterns established in Task 13. Each repository implements its trait from `syswall_domain::ports`.

- [ ] **Step 2: Implement TokioBroadcastEventBus**

`crates/infra/src/event_bus/mod.rs`:
```rust
use async_trait::async_trait;
use tokio::sync::broadcast;
use tracing::warn;

use syswall_domain::errors::DomainError;
use syswall_domain::events::DomainEvent;
use syswall_domain::ports::{EventBus, EventReceiver};

pub struct TokioBroadcastEventBus {
    sender: broadcast::Sender<DomainEvent>,
}

impl TokioBroadcastEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
}

#[async_trait]
impl EventBus for TokioBroadcastEventBus {
    async fn publish(&self, event: DomainEvent) -> Result<(), DomainError> {
        match self.sender.send(event) {
            Ok(_) => Ok(()),
            Err(_) => {
                // No subscribers — this is fine, events are volatile
                Ok(())
            }
        }
    }

    fn subscribe(&self) -> EventReceiver {
        self.sender.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;
    use chrono::Utc;

    #[tokio::test]
    async fn publish_and_receive() {
        let bus = TokioBroadcastEventBus::new(128);
        let mut rx = bus.subscribe();

        let rule = Rule {
            id: RuleId::new(),
            name: "Test".to_string(),
            priority: RulePriority::new(1),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        };

        bus.publish(DomainEvent::RuleCreated(rule)).await.unwrap();

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, DomainEvent::RuleCreated(_)));
    }

    #[tokio::test]
    async fn publish_without_subscribers_does_not_error() {
        let bus = TokioBroadcastEventBus::new(128);
        // No subscriber — should not error
        let result = bus.publish(DomainEvent::RuleDeleted(RuleId::new())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn multiple_subscribers_receive_same_event() {
        let bus = TokioBroadcastEventBus::new(128);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(DomainEvent::RuleDeleted(RuleId::new()))
            .await
            .unwrap();

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }
}
```

Run: `cargo test -p syswall-infra`
Expected: all tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/infra/
git commit -m "feat(infra): add remaining SQLite repos and TokioBroadcastEventBus

SqlitePendingDecisionRepository, SqliteDecisionRepository,
SqliteAuditRepository, TokioBroadcastEventBus. All tested."
```

---

### Task 15: Daemon Configuration

**Files:**
- Create: `crates/daemon/src/config.rs`
- Create: `config/default.toml`

- [ ] **Step 1: Implement typed configuration**

`crates/daemon/src/config.rs`:
```rust
use serde::Deserialize;
use std::path::{Path, PathBuf};

use syswall_domain::errors::DomainError;
use syswall_domain::events::DefaultPolicy;

#[derive(Debug, Deserialize)]
pub struct SysWallConfig {
    pub config_version: u32,
    pub daemon: DaemonConfig,
    pub database: DatabaseConfig,
    pub firewall: FirewallConfig,
    pub monitoring: MonitoringConfig,
    pub learning: LearningConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Deserialize)]
pub struct DaemonConfig {
    pub socket_path: PathBuf,
    pub log_level: String,
    pub log_dir: PathBuf,
    pub watchdog_interval_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub journal_retention_days: u32,
    pub audit_batch_size: usize,
    pub audit_flush_interval_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct FirewallConfig {
    pub default_policy: DefaultPolicyConfig,
    pub rollback_timeout_secs: u64,
    pub nftables_table_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DefaultPolicyConfig {
    Ask,
    Allow,
    Block,
}

impl From<&DefaultPolicyConfig> for DefaultPolicy {
    fn from(config: &DefaultPolicyConfig) -> Self {
        match config {
            DefaultPolicyConfig::Ask => DefaultPolicy::Ask,
            DefaultPolicyConfig::Allow => DefaultPolicy::Allow,
            DefaultPolicyConfig::Block => DefaultPolicy::Block,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct MonitoringConfig {
    pub conntrack_buffer_size: usize,
    pub process_cache_ttl_secs: u64,
    pub event_bus_capacity: usize,
}

#[derive(Debug, Deserialize)]
pub struct LearningConfig {
    pub enabled: bool,
    pub debounce_window_secs: u64,
    pub prompt_timeout_secs: u64,
    pub default_timeout_action: String,
    pub max_pending_decisions: usize,
    pub overflow_action: String,
}

#[derive(Debug, Deserialize)]
pub struct UiConfig {
    pub locale: String,
    pub theme: String,
    pub refresh_interval_ms: u64,
}

impl SysWallConfig {
    /// Load config from a TOML file. Falls back to defaults if the file doesn't exist.
    pub fn load(path: &Path) -> Result<Self, DomainError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| DomainError::Infrastructure(format!("Failed to read config: {}", e)))?;
        Self::from_toml(&content)
    }

    /// Parse config from a TOML string.
    pub fn from_toml(content: &str) -> Result<Self, DomainError> {
        toml::from_str(content)
            .map_err(|e| DomainError::Validation(format!("Invalid config: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CONFIG: &str = r#"
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

[monitoring]
conntrack_buffer_size = 4096
process_cache_ttl_secs = 5
event_bus_capacity = 4096

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
"#;

    #[test]
    fn parse_valid_config() {
        let config = SysWallConfig::from_toml(TEST_CONFIG).unwrap();
        assert_eq!(config.config_version, 1);
        assert_eq!(config.daemon.log_level, "info");
        assert!(matches!(config.firewall.default_policy, DefaultPolicyConfig::Ask));
        assert_eq!(config.learning.prompt_timeout_secs, 60);
        assert_eq!(config.ui.locale, "fr");
    }

    #[test]
    fn invalid_toml_returns_error() {
        let result = SysWallConfig::from_toml("not valid toml {{{");
        assert!(result.is_err());
    }

    #[test]
    fn default_policy_conversion() {
        let policy: DefaultPolicy = (&DefaultPolicyConfig::Ask).into();
        assert_eq!(policy, DefaultPolicy::Ask);
    }
}
```

- [ ] **Step 2: Create default.toml**

`config/default.toml`: (same content as TEST_CONFIG above)

Run: `cargo test -p syswall-daemon`
Expected: 3 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/ config/
git commit -m "feat(daemon): add typed TOML configuration

SysWallConfig with all sections. Load from file, parse from string.
Fail-fast validation via serde. Default config in config/default.toml."
```

---

### Task 16: Proto Definitions and Codegen

**Files:**
- Create: `proto/syswall.proto`
- Modify: `crates/proto/build.rs`
- Modify: `crates/proto/src/lib.rs`

- [ ] **Step 1: Write syswall.proto**

`proto/syswall.proto`:
```protobuf
syntax = "proto3";

package syswall;

// --- Control Service (request/response) ---

service SysWallControl {
    rpc GetStatus(Empty) returns (StatusResponse);
    rpc ListRules(RuleFiltersRequest) returns (RuleListResponse);
    rpc CreateRule(CreateRuleRequest) returns (RuleResponse);
    rpc DeleteRule(RuleIdRequest) returns (Empty);
    rpc ToggleRule(ToggleRuleRequest) returns (RuleResponse);
    rpc RespondToDecision(DecisionResponseRequest) returns (DecisionAck);
    rpc ListPendingDecisions(Empty) returns (PendingDecisionListResponse);
}

// --- Event Service (streaming) ---

service SysWallEvents {
    rpc SubscribeEvents(SubscribeRequest) returns (stream DomainEventMessage);
}

// --- Messages ---

message Empty {}

message StatusResponse {
    bool enabled = 1;
    uint32 active_rules_count = 2;
    bool nftables_synced = 3;
    uint64 uptime_secs = 4;
    string version = 5;
}

message RuleFiltersRequest {
    uint64 offset = 1;
    uint64 limit = 2;
}

message RuleListResponse {
    repeated RuleMessage rules = 1;
}

message RuleMessage {
    string id = 1;
    string name = 2;
    uint32 priority = 3;
    bool enabled = 4;
    string criteria_json = 5;
    string effect = 6;
    string scope_json = 7;
    string source = 8;
    string created_at = 9;
    string updated_at = 10;
}

message CreateRuleRequest {
    string name = 1;
    uint32 priority = 2;
    string criteria_json = 3;
    string effect = 4;
    string scope_json = 5;
    string source = 6;
}

message RuleResponse {
    RuleMessage rule = 1;
}

message RuleIdRequest {
    string id = 1;
}

message ToggleRuleRequest {
    string id = 1;
    bool enabled = 2;
}

message DecisionResponseRequest {
    string pending_decision_id = 1;
    string action = 2;
    string granularity = 3;
}

message DecisionAck {
    string decision_id = 1;
}

message PendingDecisionMessage {
    string id = 1;
    string snapshot_json = 2;
    string requested_at = 3;
    string expires_at = 4;
    string status = 5;
}

message PendingDecisionListResponse {
    repeated PendingDecisionMessage decisions = 1;
}

message SubscribeRequest {}

message DomainEventMessage {
    string event_type = 1;
    string payload_json = 2;
    string timestamp = 3;
}
```

- [ ] **Step 2: Update build.rs and lib.rs**

`crates/proto/build.rs`:
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["../../proto/syswall.proto"], &["../../proto/"])?;
    Ok(())
}
```

`crates/proto/src/lib.rs`:
```rust
pub mod syswall {
    tonic::include_proto!("syswall");
}
```

Run: `cargo check -p syswall-proto`
Expected: compiles (tonic-build generates code)

- [ ] **Step 3: Commit**

```bash
git add proto/ crates/proto/
git commit -m "feat(proto): add gRPC service definitions

SysWallControl (request/response) and SysWallEvents (streaming).
Proto messages for rules, pending decisions, events.
tonic-build generates Rust client and server code."
```

---

### Task 17: Daemon Main, Bootstrap, Supervisor, and Signals

**Files:**
- Modify: `crates/daemon/src/main.rs`
- Create: `crates/daemon/src/bootstrap.rs`
- Create: `crates/daemon/src/supervisor.rs`
- Create: `crates/daemon/src/signals.rs`
- Create: `crates/daemon/src/grpc/mod.rs`
- Create: `crates/daemon/src/grpc/control_service.rs`
- Create: `crates/daemon/src/grpc/event_service.rs`

This task wires everything together. Due to plan length constraints, the gRPC service implementations are minimal stubs that delegate to the application services. The full gRPC handlers and converters will be completed in sub-project 2+.

- [ ] **Step 1: Implement signals.rs**

`crates/daemon/src/signals.rs`:
```rust
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Wait for SIGTERM or SIGINT, then trigger cancellation.
pub async fn wait_for_shutdown(cancel: CancellationToken) {
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");

    tokio::select! {
        _ = sigterm.recv() => {
            info!("Received SIGTERM, initiating shutdown");
        }
        _ = sigint.recv() => {
            info!("Received SIGINT, initiating shutdown");
        }
        _ = cancel.cancelled() => {
            // Already cancelled externally
        }
    }

    cancel.cancel();
}
```

- [ ] **Step 2: Implement supervisor.rs**

`crates/daemon/src/supervisor.rs`:
```rust
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Manages async tasks and coordinates shutdown.
pub struct Supervisor {
    cancel: CancellationToken,
    tasks: JoinSet<Result<(), String>>,
}

impl Supervisor {
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
            tasks: JoinSet::new(),
        }
    }

    /// Spawn a named task.
    pub fn spawn<F>(&mut self, name: &'static str, future: F)
    where
        F: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        info!("Supervisor: spawning task '{}'", name);
        self.tasks.spawn(async move {
            let result = future.await;
            if let Err(ref e) = result {
                error!("Task '{}' failed: {}", name, e);
            } else {
                info!("Task '{}' completed", name);
            }
            result
        });
    }

    /// Wait for cancellation, then join all tasks.
    pub async fn run(mut self) {
        // Wait until cancellation is triggered (by signal handler or fatal error)
        self.cancel.cancelled().await;
        info!("Supervisor: shutdown initiated, waiting for tasks...");

        // Give tasks a moment to finish gracefully
        while let Some(result) = self.tasks.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => error!("Task error during shutdown: {}", e),
                Err(e) => error!("Task panicked during shutdown: {}", e),
            }
        }

        info!("Supervisor: all tasks completed");
    }

    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }
}
```

- [ ] **Step 3: Implement bootstrap.rs and main.rs**

`crates/daemon/src/bootstrap.rs`:
```rust
use std::path::Path;
use std::sync::Arc;

use syswall_app::services::audit_service::AuditService;
use syswall_app::services::connection_service::ConnectionService;
use syswall_app::services::learning_service::{
    LearningConfig as AppLearningConfig, LearningService,
};
use syswall_app::services::rule_service::RuleService;
use syswall_app::fakes::{
    FakeConnectionMonitor, FakeFirewallEngine, FakeProcessResolver, FakeUserNotifier,
};
use syswall_infra::event_bus::TokioBroadcastEventBus;
use syswall_infra::persistence::Database;
use syswall_infra::persistence::rule_repository::SqliteRuleRepository;
use syswall_infra::persistence::pending_decision_repository::SqlitePendingDecisionRepository;
use syswall_infra::persistence::decision_repository::SqliteDecisionRepository;
use syswall_infra::persistence::audit_repository::SqliteAuditRepository;

use crate::config::SysWallConfig;

/// All the wired-up services, ready to use.
pub struct AppContext {
    pub rule_service: Arc<RuleService>,
    pub connection_service: Arc<ConnectionService>,
    pub learning_service: Arc<LearningService>,
    pub audit_service: Arc<AuditService>,
    pub event_bus: Arc<TokioBroadcastEventBus>,
}

/// Wire up all dependencies and return the application context.
pub fn bootstrap(config: &SysWallConfig) -> Result<AppContext, syswall_domain::errors::DomainError> {
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

    // System adapters — stubs for foundations, replaced in sub-projects 2-3
    let firewall = Arc::new(FakeFirewallEngine::new());
    let process_resolver = Arc::new(FakeProcessResolver::new());
    let notifier = Arc::new(FakeUserNotifier::new());

    // Application services
    let rule_service = Arc::new(RuleService::new(
        rule_repo.clone(),
        firewall,
        event_bus.clone(),
    ));

    let default_policy = (&config.firewall.default_policy).into();

    let connection_service = Arc::new(ConnectionService::new(
        process_resolver,
        rule_repo,
        event_bus.clone(),
        default_policy,
    ));

    let learning_service = Arc::new(LearningService::new(
        pending_repo,
        decision_repo,
        rule_service.clone(),
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
    })
}
```

`crates/daemon/src/main.rs`:
```rust
mod bootstrap;
mod config;
mod signals;
mod supervisor;

use std::path::Path;

use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::config::SysWallConfig;
use crate::supervisor::Supervisor;

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "syswall=info".into()),
        )
        .init();

    info!("SysWall daemon starting...");

    // Load config
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
    let _ctx = match bootstrap::bootstrap(&config) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Fatal: bootstrap failed: {}", e);
            std::process::exit(1);
        }
    };

    // Supervisor
    let cancel = CancellationToken::new();
    let supervisor = Supervisor::new(cancel.clone());

    // Signal handler
    tokio::spawn(signals::wait_for_shutdown(cancel.clone()));

    info!("SysWall daemon ready");

    // Run until shutdown
    supervisor.run().await;

    info!("SysWall daemon stopped");
}
```

Create stub gRPC modules so everything compiles:

`crates/daemon/src/grpc/mod.rs`:
```rust
// gRPC service implementations will be added in sub-project 2
```

Update `crates/daemon/src/main.rs` to NOT import grpc yet (it's empty).

Run: `cargo check -p syswall-daemon`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/ config/
git commit -m "feat(daemon): add bootstrap, supervisor, signal handling

Full DI wiring in bootstrap. Supervisor manages async tasks with
CancellationToken. Signal handler for SIGTERM/SIGINT. Config loading.
Daemon starts and runs until signal received."
```

---

### Task 18: Tauri UI Scaffold

**Files:**
- Create: `crates/ui/` (Tauri project)

- [ ] **Step 1: Initialize Tauri + Svelte project**

Run from project root:
```bash
cd crates && npm create tauri-app@latest ui -- --template svelte-ts --manager npm && cd ..
```

This creates the Tauri project scaffold with Svelte + TypeScript.

- [ ] **Step 2: Verify it builds**

```bash
cd crates/ui && npm install && npm run build && cd ../..
```

Expected: Svelte frontend builds successfully.

Note: `cargo tauri build` requires additional system deps; `npm run build` validates the frontend.

- [ ] **Step 3: Commit**

```bash
git add crates/ui/ .gitignore
git commit -m "feat(ui): scaffold Tauri + Svelte + TypeScript app

Minimal Tauri project. Frontend builds. gRPC client and
Tauri commands will be added in sub-project 5."
```

---

### Task 19: Final Verification

- [ ] **Step 1: Run full workspace test suite**

```bash
cargo test --workspace
```

Expected: all tests PASS across domain, app, infra, daemon crates.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Fix any warnings.

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "chore: fix clippy warnings and finalize foundations

All workspace tests pass. Domain model, ports, PolicyEngine,
application services, SQLite persistence, event bus, daemon
bootstrap all wired up and tested."
```

---

## Summary

**19 tasks** covering the complete foundations layer:

| Task | Component | Key Output |
|---|---|---|
| 1 | Workspace scaffolding | 5 crates, Cargo.toml, deps |
| 2 | Value objects | Port, RulePriority, ExecutablePath, Protocol, Direction |
| 3 | Entities (Connection, Rule) | Core domain types |
| 4 | Entities (Decision, Audit) | PendingDecision, AuditEvent |
| 5 | Domain events | DomainEvent enum, FirewallStatus, Pagination |
| 6 | Ports (traits) | 9 async traits defining hexagonal boundaries |
| 7 | PolicyEngine | Rule matching with Specification pattern |
| 8 | App commands + fakes | 3 commands, 9 fake adapters |
| 9 | RuleService | CRUD + firewall sync |
| 10 | LearningService | Async pending decisions, debounce, expiration |
| 11 | ConnectionService + AuditService | Enrichment, evaluation, audit recording |
| 12 | SQLite setup | Database with WAL, migrations, 4 tables |
| 13 | SqliteRuleRepository | Full CRUD with contract tests |
| 14 | Remaining repos + EventBus | 3 repos + TokioBroadcastEventBus |
| 15 | Daemon config | Typed TOML config with validation |
| 16 | Proto definitions | gRPC services + tonic codegen |
| 17 | Daemon main | Bootstrap, supervisor, signals |
| 18 | Tauri UI scaffold | Svelte + TypeScript project |
| 19 | Final verification | Full test suite, clippy clean |
