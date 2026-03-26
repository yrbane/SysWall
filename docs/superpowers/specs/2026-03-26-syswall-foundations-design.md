# SysWall — Foundations Design Spec

**Date:** 2026-03-26
**Scope:** Sub-project 1 — Foundations (scaffolding, architecture, core types, infrastructure)
**Status:** Draft (v2 — revised after external review)

---

## 1. Overview

SysWall is a Linux desktop firewall built with Rust + Tauri. This spec covers the foundational layer: project structure, domain model, ports & adapters, service layer, daemon lifecycle, gRPC communication, UI scaffold, testing strategy, and configuration.

This is sub-project 1 of 6:
1. **Foundations** (this spec)
2. Firewall engine (nftables adapter, rule matching)
3. Connection monitoring (conntrack, process resolution, real-time stream)
4. Auto-learning mode (decision prompts, debounce, rule generation)
5. Premium UI (dashboard, views, design system)
6. Audit & journal (persistence, search, export)

## 2. Technology Stack

| Component | Choice | Rationale |
|---|---|---|
| Backend language | Rust | Performance, safety, Linux ecosystem |
| Desktop framework | Tauri | Lightweight, Rust backend, web frontend |
| Frontend | Svelte + TypeScript | Small bundle, reactive by default, great for real-time dashboards |
| Persistence | SQLite via rusqlite | Single file, no server, full SQL, direct bindings |
| Privileged service | Systemd daemon | Runs as root, isolated from UI, auto-restart, hardened |
| UI ↔ Daemon comm | gRPC (tonic + prost) over Unix socket | Bidirectional streaming for real-time events, strict proto contract |
| Internal event bus | tokio broadcast wrapped in trait | Already in stack via tonic, multi-consumer, backpressure |
| Structured logging | tracing + tracing-subscriber | De facto Rust standard, spans, async-native |
| System targets | nftables, conntrack | Modern Linux firewall and connection tracking |

## 3. Workspace Structure

Monorepo Cargo workspace with 6 crates. The crate dependency graph enforces architectural boundaries at compile time.

```
syswall/
├── Cargo.toml                    # workspace root
├── proto/
│   └── syswall.proto             # gRPC definitions
├── crates/
│   ├── domain/                   # syswall-domain
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── entities/         # Connection, Rule, Decision, PendingDecision...
│   │       ├── value_objects/    # Port, Protocol, Direction, RulePriority...
│   │       ├── services/         # PolicyEngine, RuleMatcher (pure domain logic)
│   │       ├── ports/            # traits: RuleRepository, ConnectionMonitor, FirewallEngine...
│   │       ├── events/           # DomainEvent enum and variants
│   │       └── errors/           # DomainError, RuleError...
│   ├── app/                      # syswall-app
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── services/         # RuleService, ConnectionService, LearningService...
│   │       └── commands/         # CreateRule, DeleteRule, RespondToDecision...
│   ├── infra/                    # syswall-infra
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── nftables/         # NftablesFirewallAdapter
│   │       ├── conntrack/        # ConntrackMonitorAdapter
│   │       ├── persistence/      # SqliteRuleRepository, SqliteAuditLog...
│   │       ├── process/          # ProcfsProcessResolver
│   │       └── event_bus/        # TokioBroadcastEventBus
│   ├── proto/                    # syswall-proto
│   │   └── src/
│   │       ├── lib.rs
│   │       └── build.rs          # tonic-build for gRPC codegen
│   ├── daemon/                   # syswall-daemon
│   │   └── src/
│   │       ├── main.rs           # entrypoint
│   │       ├── bootstrap.rs      # DI wiring, startup sequence
│   │       ├── supervisor.rs     # task lifecycle, cancellation, restart
│   │       ├── grpc/             # gRPC service implementations + converters
│   │       ├── config.rs         # configuration loading
│   │       └── signals.rs        # SIGTERM/SIGINT handling
│   └── ui/                       # syswall-ui (Tauri app)
│       ├── src-tauri/
│       │   ├── src/
│       │   │   ├── main.rs
│       │   │   ├── commands/     # Tauri commands (thin wrappers)
│       │   │   └── grpc_client.rs
│       │   └── Cargo.toml
│       ├── src/                  # Svelte frontend
│       │   ├── App.svelte
│       │   ├── lib/
│       │   └── routes/
│       ├── package.json
│       └── vite.config.ts
├── config/
│   └── default.toml              # default configuration
└── tests/
    └── integration/              # cross-crate integration tests
```

### Crate Dependency Graph

```
domain → (no SysWall deps, only serde/chrono/uuid/async-trait)
app → domain
infra → domain, app
proto → (independent, generates gRPC types)
daemon → app, infra, proto
ui (Tauri Rust) → proto only
```

Key constraint: **UI never depends on domain or infra directly.** All communication goes through gRPC. Total separation.

## 4. Domain Model

All types live in `syswall-domain`. Zero infrastructure dependencies.

### 4.1 Connection

A network connection observed by the system.

| Field | Type | Description |
|---|---|---|
| id | ConnectionId (UUID newtype) | Unique identifier |
| protocol | Protocol | TCP, UDP, ICMP, Other |
| source | SocketAddress | IP + port |
| destination | SocketAddress | IP + port |
| direction | Direction | Inbound, Outbound |
| state | ConnectionState | New, Established, Related, Closing, Closed |
| process | Option\<ProcessInfo\> | Resolved process — best effort, may be absent (see 8.6) |
| user | Option\<SystemUser\> | System user owning the process |
| bytes_sent | u64 | Traffic volume sent |
| bytes_received | u64 | Traffic volume received |
| started_at | DateTime\<Utc\> | Connection start time |
| verdict | ConnectionVerdict | Unknown, PendingDecision, Allowed, Blocked, Ignored |
| matched_rule | Option\<RuleId\> | Rule that determined the verdict |

### 4.2 Rule

A firewall rule.

| Field | Type | Description |
|---|---|---|
| id | RuleId (UUID newtype) | Unique identifier |
| name | String | Human-readable name |
| priority | RulePriority (u32 newtype, validated) | Lower = higher priority |
| enabled | bool | Whether the rule is active |
| criteria | RuleCriteria | What the rule matches |
| effect | RuleEffect | Allow, Block, Ask, Observe |
| scope | RuleScope | Permanent, Temporary { expires_at } |
| created_at | DateTime\<Utc\> | Creation timestamp |
| updated_at | DateTime\<Utc\> | Last modification |
| source | RuleSource | Manual, AutoLearning, Import, System |

### 4.3 RuleCriteria (Specification Pattern)

All fields are `Option`. `None` = match anything. All present criteria must match (AND logic).

| Field | Type | Description |
|---|---|---|
| application | Option\<AppMatcher\> | Match by name, path, or binary hash |
| user | Option\<String\> | System user |
| remote_ip | Option\<IpMatcher\> | Exact IP, CIDR, or range |
| remote_port | Option\<PortMatcher\> | Exact port or range |
| local_port | Option\<PortMatcher\> | Exact port or range |
| protocol | Option\<Protocol\> | TCP, UDP, ICMP, Other |
| direction | Option\<Direction\> | Inbound, Outbound |
| schedule | Option\<Schedule\> | Time-based restrictions |

### 4.4 PendingDecision

A decision request waiting for user response. Core entity for async auto-learning flow.

| Field | Type | Description |
|---|---|---|
| id | PendingDecisionId (UUID newtype) | Unique identifier |
| connection_snapshot | ConnectionSnapshot | Connection state at request time |
| requested_at | DateTime\<Utc\> | When the decision was requested |
| expires_at | DateTime\<Utc\> | Timeout deadline |
| deduplication_key | String | Hash of (app + remote_ip + remote_port + protocol) |
| status | PendingDecisionStatus | Pending, Resolved, Expired, Cancelled |

```rust
enum PendingDecisionStatus {
    Pending,
    Resolved,
    Expired,
    Cancelled,
}
```

### 4.5 Decision

A resolved auto-learning decision.

| Field | Type | Description |
|---|---|---|
| id | DecisionId (UUID newtype) | Unique identifier |
| pending_decision_id | PendingDecisionId | Link to the original request |
| connection_snapshot | ConnectionSnapshot | Connection state at decision time |
| action | DecisionAction | AllowOnce, BlockOnce, AlwaysAllow, AlwaysBlock, CreateRule, Ignore |
| granularity | DecisionGranularity | AppOnly, AppAndIp, AppAndPort, AppAndDomain, etc. |
| decided_at | DateTime\<Utc\> | When the user decided |
| generated_rule | Option\<RuleId\> | Rule created from this decision, if any |

### 4.6 AuditEvent

A journal entry.

| Field | Type | Description |
|---|---|---|
| id | EventId (UUID newtype) | Unique identifier |
| timestamp | DateTime\<Utc\> | Event time |
| severity | Severity | Debug, Info, Warning, Error, Critical |
| category | EventCategory | Connection, Rule, Decision, System, Config |
| description | String | Human-readable description |
| metadata | HashMap\<String, String\> | Structured context data |

### 4.7 Key Value Objects

Newtypes with validation for domain-critical values:

```rust
/// Port number (1-65535). Rejects 0.
struct Port(u16);

/// Rule priority. Lower = higher priority. System rules use 0.
struct RulePriority(u32);

/// Validated executable path (must be absolute).
struct ExecutablePath(PathBuf);
```

Primitive wrappers (`u64` for bytes, `String` for username) remain unwrapped — they carry no domain invariant that justifies a newtype.

### 4.8 RuleEffect vs ConnectionVerdict

Separate concepts that must not be conflated:

```rust
/// What a rule DOES when it matches (configuration)
enum RuleEffect {
    Allow,
    Block,
    Ask,      // triggers auto-learning prompt
    Observe,  // log only, don't enforce
}

/// The OUTCOME for a connection (runtime state)
enum ConnectionVerdict {
    Unknown,          // not yet evaluated
    PendingDecision,  // waiting for user response
    Allowed,
    Blocked,
    Ignored,
}
```

## 5. Domain Services

Pure business logic living in `syswall-domain/src/services/`. No I/O, no infrastructure.

### 5.1 PolicyEngine

The core evaluation logic. Takes rules and a connection, returns an evaluation result.

```rust
struct PolicyEvaluation {
    verdict: ConnectionVerdict,
    matched_rule_id: Option<RuleId>,
    reason: EvaluationReason,
}

enum EvaluationReason {
    MatchedRule { rule_id: RuleId, effect: RuleEffect },
    NoMatchingRule,
    PendingUserDecision,
    DefaultPolicyApplied { policy: DefaultPolicy },
    TemporaryBypass,
}
```

- `evaluate(connection, rules) → PolicyEvaluation` — walks rules by priority, returns first match
- `matches(criteria, connection) → bool` — Specification pattern, pure function
- `explain(connection, rules) → Vec<MatchExplanation>` — debug/audit: why each rule did or didn't match

PolicyEngine takes no ports. It receives rules as data, not via repository. The caller (application service) loads the rules and passes them in.

## 6. Ports (Traits)

The hexagonal boundaries. Domain declares what it needs, infrastructure implements.

**All ports are `#[async_trait]`.** Even for sync-native implementations (rusqlite, procfs). Adapters manage `spawn_blocking` internally. This ensures uniform signatures across all services, stable interfaces when implementations change, and simpler testing.

### 6.1 Primary Ports (called by application services)

```rust
#[async_trait]
trait RuleRepository: Send + Sync {
    async fn save(&self, rule: &Rule) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: &RuleId) -> Result<Option<Rule>, DomainError>;
    async fn find_all(&self, filters: &RuleFilters, pagination: &Pagination) -> Result<Vec<Rule>, DomainError>;
    async fn delete(&self, id: &RuleId) -> Result<(), DomainError>;
    async fn list_enabled_ordered(&self) -> Result<Vec<Rule>, DomainError>;
}

#[async_trait]
trait AuditRepository: Send + Sync {
    async fn append(&self, event: &AuditEvent) -> Result<(), DomainError>;
    async fn query(&self, filters: &AuditFilters, pagination: &Pagination) -> Result<Vec<AuditEvent>, DomainError>;
    async fn count(&self, filters: &AuditFilters) -> Result<u64, DomainError>;
}

#[async_trait]
trait DecisionRepository: Send + Sync {
    async fn save(&self, decision: &Decision) -> Result<(), DomainError>;
    async fn find_by_connection_pattern(&self, criteria: &RuleCriteria) -> Result<Option<Decision>, DomainError>;
}

#[async_trait]
trait PendingDecisionRepository: Send + Sync {
    async fn create(&self, request: &PendingDecision) -> Result<(), DomainError>;
    async fn list_pending(&self) -> Result<Vec<PendingDecision>, DomainError>;
    async fn resolve(&self, id: &PendingDecisionId) -> Result<(), DomainError>;
    async fn expire_overdue(&self) -> Result<Vec<PendingDecision>, DomainError>;
    async fn find_by_dedup_key(&self, key: &str) -> Result<Option<PendingDecision>, DomainError>;
}

#[async_trait]
trait FirewallEngine: Send + Sync {
    async fn apply_rule(&self, rule: &Rule) -> Result<(), DomainError>;
    async fn remove_rule(&self, rule_id: &RuleId) -> Result<(), DomainError>;
    async fn sync_all_rules(&self, rules: &[Rule]) -> Result<(), DomainError>;
    async fn get_status(&self) -> Result<FirewallStatus, DomainError>;
}

#[async_trait]
trait ConnectionMonitor: Send + Sync {
    async fn stream_events(&self) -> Result<ConnectionEventStream, DomainError>;
    async fn get_active_connections(&self) -> Result<Vec<Connection>, DomainError>;
}

#[async_trait]
trait ProcessResolver: Send + Sync {
    async fn resolve(&self, pid: u32) -> Result<Option<ProcessInfo>, DomainError>;
    async fn resolve_by_socket(&self, inode: u64) -> Result<Option<ProcessInfo>, DomainError>;
}
```

Note: `RuleRepository` no longer has `find_matching()`. Rule matching is the PolicyEngine's job. The repository provides `list_enabled_ordered()` and the application service passes the rules to PolicyEngine for evaluation.

Note: `ConnectionMonitor` no longer has `start()`. It provides `stream_events()` which returns a stream. Lifecycle (start, stop, restart on error) is managed by the daemon's Supervisor, not the port.

### 6.2 Secondary Ports (push events into the system)

```rust
#[async_trait]
trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent) -> Result<(), DomainError>;
    fn subscribe(&self) -> EventReceiver;
}

#[async_trait]
trait UserNotifier: Send + Sync {
    async fn notify_decision_required(&self, request: &PendingDecision) -> Result<(), DomainError>;
    async fn notify(&self, notification: &Notification) -> Result<(), DomainError>;
}
```

Note: `UserNotifier` is now **non-blocking**. It notifies the UI that a decision is needed but does NOT wait for a response. The response comes back asynchronously via `RespondToDecision` gRPC call → `LearningService::resolve_decision()`.

### 6.3 Supporting Types

```rust
/// Snapshot of connection state at a point in time (for Decision records)
struct ConnectionSnapshot {
    protocol: Protocol,
    source: SocketAddress,
    destination: SocketAddress,
    direction: Direction,
    process_name: Option<String>,
    process_path: Option<ExecutablePath>,
    user: Option<String>,
}

/// All domain events flowing through the EventBus
enum DomainEvent {
    ConnectionDetected(Connection),
    ConnectionUpdated { id: ConnectionId, state: ConnectionState },
    ConnectionClosed(ConnectionId),
    RuleCreated(Rule),
    RuleUpdated(Rule),
    RuleDeleted(RuleId),
    RuleMatched { connection_id: ConnectionId, rule_id: RuleId, verdict: ConnectionVerdict },
    DecisionRequired(PendingDecision),
    DecisionResolved(Decision),
    DecisionExpired(PendingDecisionId),
    FirewallStatusChanged(FirewallStatus),
    SystemError { message: String, severity: Severity },
}

/// Overall firewall status
struct FirewallStatus {
    enabled: bool,
    active_rules_count: u32,
    nftables_synced: bool,
    uptime: Duration,
    version: String,
}

/// Pagination parameters for list queries
struct Pagination {
    offset: u64,
    limit: u64,
}

/// Notification sent to the UI (non-blocking, informational)
struct Notification {
    title: String,
    message: String,
    severity: Severity,
}
```

### 6.4 EventBus Scope

The EventBus is for **real-time notification**, not durability. It uses `tokio::broadcast` which is fire-and-forget — if no subscriber is listening, the event is lost. This is intentional:

- **Volatile events** (connection detected, stats update) → EventBus only
- **Durable state** (pending decisions, audit log, rules) → persisted to SQLite via repositories

The AuditService listens to the EventBus and persists events worth keeping. But the bus itself is not a persistence mechanism.

### 6.5 Design Principles

- All traits return `Result<T, DomainError>` — no infrastructure types leak
- `ConnectionEventStream` is `Pin<Box<dyn Stream<Item = Result<Connection, DomainError>> + Send>>` — async, abstract, and error-aware
- Domain has no knowledge of nftables, conntrack, or SQLite
- Each trait has a single responsibility

## 7. Application Services

The `syswall-app` crate orchestrates ports to implement business logic. Dependencies injected via constructors.

### 7.1 RuleService (CRUD only)

- `create_rule(cmd) → Result<Rule>` — validate, persist, sync to firewall, publish RuleCreated
- `update_rule(cmd) → Result<Rule>` — with nftables reconciliation
- `delete_rule(id) → Result<()>` — remove from nftables, delete from DB, publish RuleDeleted
- `toggle_rule(id, enabled) → Result<()>` — activate/deactivate
- `list_rules(filters, pagination) → Result<Vec<Rule>>`
- `import_rules(rules) → Result<ImportReport>`
- `export_rules(filters) → Result<Vec<Rule>>`
- **Validation:** rejects overly broad rules (e.g., allow all/all permanent) unless explicitly confirmed
- **Dependencies:** RuleRepository, FirewallEngine, EventBus

### 7.2 ConnectionService

- `start_monitoring()` is NOT here — lifecycle is the Supervisor's job
- `get_active_connections(filters) → Result<Vec<Connection>>`
- `get_connection_detail(id) → Result<Connection>`
- `process_connection(raw_connection) → Result<Connection>` — enriches with process info, evaluates policy, publishes events
- Internally: loads rules via `RuleRepository::list_enabled_ordered()`, passes to `PolicyEngine::evaluate()`
- **Dependencies:** ProcessResolver, RuleRepository, PolicyEngine, EventBus

### 7.3 LearningService (async, non-blocking)

- `handle_unknown_connection(connection) → Result<()>` — when PolicyEngine returns `NoMatchingRule` and default policy is `Ask`:
  1. Computes deduplication key
  2. Checks `PendingDecisionRepository` for existing pending with same key → skip if found (debounce)
  3. Creates `PendingDecision` with expiration
  4. Persists to `PendingDecisionRepository`
  5. Publishes `DecisionRequired` event
  6. Calls `UserNotifier::notify_decision_required()` (non-blocking)
  7. Returns immediately — does NOT wait for user response
- `resolve_decision(pending_id, user_response) → Result<()>` — called when user responds:
  1. Loads PendingDecision, validates it's still Pending
  2. Creates Decision record
  3. If AlwaysAllow/AlwaysBlock/CreateRule → creates Rule via RuleService
  4. If AllowOnce/BlockOnce → applies temporary verdict
  5. Marks PendingDecision as Resolved
  6. Publishes `DecisionResolved` event
- `expire_overdue() → Result<()>` — periodic task (called by Supervisor):
  1. Finds expired PendingDecisions
  2. Applies `default_timeout_action` from config
  3. Marks as Expired, publishes `DecisionExpired`
- `get_pending_decisions() → Result<Vec<PendingDecision>>`
- **Dependencies:** PendingDecisionRepository, DecisionRepository, RuleService, UserNotifier, EventBus

### 7.4 AuditService

- Listens to EventBus, persists all relevant events
- `query_events(filters, pagination) → Result<PaginatedResult<AuditEvent>>`
- `get_stats(time_range) → Result<AuditStats>` — counters for dashboard
- **Dependencies:** AuditRepository, EventBus

### 7.5 Main Flow (fully async)

```
Conntrack → ConnectionMonitor.stream_events()
  → Supervisor receives stream, spawns processing task
    → ConnectionService.process_connection() enriches (process, user)
      → PolicyEngine.evaluate(connection, rules)
        → Match found → ConnectionVerdict applied, AuditEvent emitted
        → No match + policy=Ask → LearningService.handle_unknown_connection()
          → PendingDecision created and persisted
          → UserNotifier.notify_decision_required() (non-blocking)
          → UI receives event via gRPC stream
            → User decides later → RespondToDecision gRPC call
              → LearningService.resolve_decision()
                → Rule created if needed → FirewallEngine applies
        → No match + policy=Block/Allow → DefaultPolicy applied directly
```

### 7.6 Policy Fallback Matrix

Explicit behavior for every edge case:

| Situation | Behavior |
|---|---|
| No rule match + learning enabled (policy=Ask) | Create PendingDecision, notify UI |
| No rule match + learning disabled (policy=Block) | Block connection immediately |
| No rule match + learning disabled (policy=Allow) | Allow connection immediately |
| PendingDecision timeout expires | Apply `default_timeout_action` from config |
| Pending queue full (`max_pending_decisions`) | Apply `default_timeout_action`, log warning |
| UI not connected | PendingDecision persisted, timeout will apply if no response |
| Internal error during evaluation | Fail closed (block), log error |
| Daemon restarts with pending decisions | Load from DB, expire overdue, resume |

## 8. gRPC Communication

Contract between daemon and UI. Defined in `proto/syswall.proto`.

### 8.1 SysWallControl (request/response)

| RPC | Request | Response | Description |
|---|---|---|---|
| GetStatus | Empty | StatusResponse | Firewall state, version, uptime |
| ListConnections | ConnectionFilters | ConnectionList | Active connections |
| GetConnection | ConnectionId | ConnectionDetail | Single connection detail |
| CreateRule | CreateRuleRequest | RuleResponse | Create a new rule |
| UpdateRule | UpdateRuleRequest | RuleResponse | Modify existing rule |
| DeleteRule | RuleId | Empty | Remove a rule |
| ToggleRule | ToggleRequest | RuleResponse | Enable/disable rule |
| ListRules | RuleFilters | RuleList | List all rules |
| QueryAuditLog | AuditFilters | AuditEventList | Query journal |
| GetDashboardStats | TimeRange | DashboardStats | Dashboard metrics |
| RespondToDecision | DecisionResponse | DecisionAck | Answer learning prompt |
| ListPendingDecisions | Empty | PendingDecisionList | Get current pending decisions |
| ImportRules | ImportRequest | ImportReport | Import rules |
| ExportRules | ExportFilters | RuleList | Export rules |

### 8.2 SysWallEvents (server-side streaming)

| RPC | Request | Stream Response | Description |
|---|---|---|---|
| SubscribeConnections | ConnectionFilters | ConnectionEvent | New, updated, closed connections |
| SubscribeAlerts | AlertFilters | AlertEvent | Decisions required, rule triggers, errors |
| SubscribeStats | Empty | StatsSnapshot | Aggregated metrics every N seconds |

### 8.3 Design Decisions

- Streams map directly to the internal EventBus, filtered and transformed
- Proto messages are DTOs — proto ↔ domain conversion happens in `daemon/grpc/` (adapter pattern)
- UI only knows proto types, never domain types
- Socket: `unix:///var/run/syswall/syswall.sock` with restrictive permissions

### 8.4 Socket Security

- Permissions: `0660`, owner `root:syswall`
- User added to `syswall` group to connect UI to daemon
- **Daemon-side:** verifies peer credentials via `SO_PEERCRED` on each connection — extracts `uid`, `gid`, `pid` and rejects unauthorized clients even if filesystem permissions are misconfigured (defense in depth)
- **UI-side:** verifies socket is owned by root before connecting (protection against fake daemon)

## 9. Infrastructure Adapters

Concrete implementations of domain ports.

### 9.1 NftablesFirewallAdapter (implements FirewallEngine)

- Uses `nft` command via `std::process::Command` — no `sh -c`, no shell concatenation, arguments passed programmatically
- All command arguments are typed and sanitized
- Systematic timeout on command execution (5s default)
- Stdout/stderr capture bounded (prevent memory exhaustion)
- Exit codes mapped to `DomainError` variants
- Manages a dedicated `syswall` table with `input`, `output`, `forward` chains
- `apply_rule()` translates domain Rule into nft command
- `sync_all_rules()` diffs current nftables state against DB rules, applies deltas
- **Rollback safety:** Before each modification, saves current state (`nft list ruleset`). On failure, automatic restore. 30-second safety timer: if daemon doesn't confirm rules work, automatic rollback (prevents network lockout)

### 9.2 ConntrackMonitorAdapter (implements ConnectionMonitor)

- Uses `conntrack-rs` or parses `conntrack -E` (event mode) for real-time events
- Transforms raw conntrack events into domain Connection
- Backpressure: if event bus is saturated, internal buffer drops oldest events + warning log

### 9.3 SqliteRuleRepository (implements RuleRepository)

- Schema: `rules` table, `criteria` serialized as JSON in TEXT column
- Indexes on `priority`, `enabled`, `source`
- `list_enabled_ordered()` returns enabled rules sorted by priority — matching is done by PolicyEngine in the domain layer, not in SQL

### 9.4 SqliteAuditRepository (implements AuditRepository)

- Table `audit_events` with indexes on `timestamp`, `severity`, `category`
- Automatic rotation: purge events older than 30 days (configurable)
- Batch writes: buffer of 100 events or flush every 2 seconds for performance

### 9.5 SqlitePendingDecisionRepository (implements PendingDecisionRepository)

- Table `pending_decisions` with indexes on `status`, `expires_at`, `deduplication_key`
- Enables: debounce lookup by dedup key, expiration sweep, persistence across daemon restarts

### 9.6 SqliteDecisionRepository (implements DecisionRepository)

- Table `decisions` with connection patterns for fast lookup

### 9.7 SQLite Concurrency Strategy

- **WAL mode** enabled at connection open — allows concurrent readers with a single writer
- **Single serialized writer** — all writes go through one connection (no write contention)
- **Multiple reader connections** for queries — non-blocking reads
- **`busy_timeout(5000)`** — 5-second retry on lock contention before returning error
- **Batch writes for audit** — buffer events and write in a single transaction
- **Migrations** run at boot in a transaction, fail-fast if schema is inconsistent
- All async adapter methods use `spawn_blocking` to avoid blocking the tokio runtime

### 9.8 ProcfsProcessResolver (implements ProcessResolver)

- Reads `/proc/<pid>/exe`, `/proc/<pid>/cmdline`, `/proc/<pid>/status`
- Socket → PID resolution via `/proc/net/tcp`, `/proc/net/udp` or netlink
- LRU cache with 5-second TTL
- **Best effort:** Process resolution is opportunistic. A connection may remain partially enriched (no process info) due to: PID already terminated, race condition between network event and /proc read, permission/namespace restrictions, containerized processes. Missing process info must never block policy evaluation or firewall enforcement.

### 9.9 TokioBroadcastEventBus (implements EventBus)

- Wrapper around `tokio::sync::broadcast::channel`
- Configurable capacity (default: 4096 events)
- Warning log when a subscriber lags

## 10. Daemon Bootstrap, Supervision & Lifecycle

### 10.1 Supervisor

The daemon includes a `Supervisor` module responsible for:

- **Task management:** Spawns and tracks async tasks (monitoring stream, expiration sweeper, gRPC server)
- **Cancellation:** Uses `tokio_util::CancellationToken` — propagated to all tasks for coordinated shutdown
- **Stream recovery:** If the conntrack stream errors, the Supervisor logs, waits with exponential backoff, and restarts the stream (recoverable error). If the stream fails N times in M seconds, it enters degraded mode and alerts.
- **Error classification:**
  - *Recoverable:* stream interruption, transient nft failure, isolated gRPC error → retry/restart
  - *Degraded:* nftables sync impossible, conntrack unavailable → continue with reduced functionality, alert
  - *Fatal:* DB corrupted, config invalid, startup failure → shutdown with clear error

### 10.2 Startup Sequence

1. **Parse config** — load `config/default.toml`, merge with `/etc/syswall/config.toml`, merge with `SYSWALL_*` env vars
2. **Init tracing** — configure subscribers (stdout + rotating file in `/var/log/syswall/`)
3. **Init persistence** — open SQLite DB (`/var/lib/syswall/syswall.db`), enable WAL, run migrations
4. **Instantiate adapters** — create concrete port implementations
5. **Instantiate services** — inject adapters into application services
6. **Resume pending decisions** — load from DB, expire overdue, apply timeout actions
7. **Sync nftables** — RuleService loads all rules from DB, calls `FirewallEngine::sync_all_rules()` to reconcile with actual nftables state
8. **Start Supervisor** — spawns: monitoring stream task, pending decision expiration sweeper (periodic), gRPC server
9. **Ready** — log "SysWall daemon ready", notify systemd via `sd_notify(READY=1)`

### 10.3 Graceful Shutdown

- Captures `SIGTERM`/`SIGINT` via `signals.rs`
- Triggers CancellationToken — all tasks begin shutdown
- Stops gRPC server (no new connections, drain existing)
- Flushes audit buffer to SQLite
- Closes conntrack stream
- **nftables rules remain in place** — firewall continues protecting even if daemon stops. Deliberate security choice.
- Log "SysWall daemon stopped"

### 10.4 Systemd Unit

```ini
[Unit]
Description=SysWall Firewall Daemon
After=network.target

[Service]
Type=notify
ExecStart=/usr/bin/syswall-daemon
User=root
Restart=on-failure
RestartSec=5s
WatchdogSec=30s
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/syswall /var/log/syswall /var/run/syswall

[Install]
WantedBy=multi-user.target
```

### 10.5 Filesystem Paths

| Path | Purpose |
|---|---|
| `/etc/syswall/config.toml` | Configuration |
| `/var/lib/syswall/syswall.db` | SQLite database |
| `/var/run/syswall/syswall.sock` | gRPC Unix socket |
| `/var/log/syswall/` | Log files |

### 10.6 System Whitelist

On first start, automatic system rules are created:
- DNS (port 53), DHCP (ports 67-68), loopback, NTP (port 123), the daemon itself
- Source: `System`, priority: 0
- **Not deletable** from UI but **can be disabled** (user retains control, but can't accidentally remove them)
- Guarantees basic connectivity is never broken by default

## 11. Tauri UI Architecture

### 11.1 Rust Side (src-tauri/)

Minimal Rust crate:
- `grpc_client.rs` — maintains gRPC connection to daemon Unix socket
- `commands/` — Tauri commands wrapping gRPC client. Each command is a thin wrapper: deserialize Svelte args, call gRPC, return result. No business logic.
- `streams.rs` — subscribes to gRPC streams and pushes to frontend via `app.emit()` (Tauri events)

### 11.2 Tauri Commands

`get_status`, `get_dashboard_stats`, `list_connections`, `get_connection`, `list_rules`, `create_rule`, `update_rule`, `delete_rule`, `toggle_rule`, `query_audit_log`, `respond_to_decision`, `list_pending_decisions`, `import_rules`, `export_rules`

### 11.3 Svelte Frontend Structure

```
src/
├── App.svelte              # main layout, router, event subscriptions
├── lib/
│   ├── api/                # typed wrappers around invoke()
│   │   ├── connections.ts
│   │   ├── rules.ts
│   │   ├── audit.ts
│   │   └── dashboard.ts
│   ├── stores/             # Svelte stores (reactive)
│   │   ├── connections.ts  # fed by real-time stream
│   │   ├── rules.ts
│   │   ├── dashboard.ts
│   │   └── alerts.ts       # pending decision queue
│   ├── components/         # reusable components
│   │   ├── ui/             # design system (Badge, Card, Table, Chart...)
│   │   ├── connections/    # ConnectionRow, ConnectionDetail...
│   │   ├── rules/          # RuleForm, RuleRow, RuleCriteria...
│   │   ├── dashboard/      # StatCard, TrafficChart, TopApps...
│   │   └── learning/       # DecisionPrompt, DecisionHistory...
│   ├── types/              # TypeScript types mirroring proto
│   └── i18n/               # localization
│       └── fr.ts
├── routes/
│   ├── Dashboard.svelte
│   ├── Connections.svelte
│   ├── Rules.svelte
│   ├── Audit.svelte
│   └── Settings.svelte
└── app.css                 # design tokens, global theme
```

### 11.4 Real-Time Data Flow

```
Daemon gRPC stream → Tauri (Rust) receives
  → app.emit("connection-event", data)
    → Svelte listens via listen("connection-event")
      → updates connection store
        → components re-render automatically
```

### 11.5 Auto-Learning UI Flow

- `SubscribeAlerts` stream pushes `DecisionRequired` with PendingDecision info
- `alerts` store accumulates pending decisions
- On app start, also calls `ListPendingDecisions` to load any decisions that were pending before the UI connected
- `DecisionPrompt` component displays as overlay (not a system window — stays in Tauri app)
- User responds → `respond_to_decision` sends response to daemon via gRPC
- Visual debounce: multiple rapid decisions stack in a queue with counter
- Expiration countdown shown per pending decision

### 11.6 No Business Logic in UI

The frontend only:
1. Calls Tauri commands
2. Listens to events
3. Displays data
4. Collects user input

## 12. Testing Strategy

### 12.1 syswall-domain — Pure Unit Tests

- Entities: invariant validation (port out of range, invalid CIDR, negative priority...)
- Value objects: Port rejects 0, RulePriority construction, ExecutablePath must be absolute
- **PolicyEngine — exhaustive tests:**
  - Exact IP, CIDR, range match
  - Application match by name, path, hash
  - Port exact, range match
  - Criteria combinations (AND logic)
  - Edge cases: empty criteria = match all, all criteria None = match all
  - Priority: first matching rule wins
  - `explain()` returns correct match reasons
- No mocks needed — everything in memory, no I/O

### 12.2 syswall-app — Unit Tests with Port Mocks

- Each service tested with fake port implementations
- `FakeRuleRepository` — in-memory HashMap
- `FakeFirewallEngine` — records calls, verifies correct rules applied/removed
- `FakePendingDecisionRepository` — in-memory with expiration tracking
- `FakeEventBus` — collects emitted events for assertions
- `FakeUserNotifier` — records notifications sent (non-blocking)
- Key scenarios:
  - `PolicyEngine::evaluate()` with 0 rules → applies default policy
  - `PolicyEngine::evaluate()` with multiple rules → highest priority wins
  - `LearningService::handle_unknown_connection()` — creates PendingDecision, does not block
  - `LearningService::resolve_decision()` — creates rule, resolves pending
  - `LearningService` debounce — same dedup key → no duplicate pending
  - `LearningService::expire_overdue()` — applies timeout action
  - `AuditService` — every event persisted

### 12.3 Contract Tests for Repositories

Shared test suite reusable across fake and SQLite implementations:

- save then find_by_id → same data
- delete then find_by_id → None
- list returns correct order
- invalid data rejected at domain boundary (before reaching repo)

Both `FakeRuleRepository` and `SqliteRuleRepository` run the same contract suite.

### 12.4 syswall-infra — Integration Tests

- `SqliteRuleRepository`: tested with real SQLite in-memory (`:memory:`) — uses contract tests
- `SqlitePendingDecisionRepository`: expiration, dedup key lookup, status transitions
- `NftablesFirewallAdapter`: contract tests with real nftables — requires `CAP_NET_ADMIN`, CI only with `#[cfg(feature = "integration")]`
- `ConntrackMonitorAdapter`: same, requires capabilities
- `ProcfsProcessResolver`: testable on any Linux (reads /proc)
- `TokioBroadcastEventBus`: multi-subscriber, backpressure, lagged tests

### 12.5 syswall-daemon — Integration Tests

- Starts real gRPC server on temporary Unix socket with injected fakes
- Tests contract: gRPC call → correct response
- Tests streams: subscribe → receive events
- **Recovery tests:**
  - DB absent at boot → clear error, fail-fast
  - Socket already present → clean up and rebind
  - nft sync failure → rollback, enter degraded mode
  - Shutdown during audit flush → no data corruption
  - Restart with pending decisions → resume and expire overdue

### 12.6 syswall-ui — Frontend Tests

- Svelte components: unit tests with `@testing-library/svelte`
- Stores: reactivity tests with mocked data
- No E2E tests in foundations (later sub-project)

### 12.7 Test Commands

- `cargo test` — all unit tests + SQLite integration
- `cargo test --features integration` — includes nftables/conntrack tests (CI only)
- `npm test` in `crates/ui/` — frontend tests

## 13. Configuration

### 13.1 Format

TOML — readable, standard in Rust with `serde` + `toml` crate.

### 13.2 Hierarchy (merge by priority)

1. Hardcoded defaults in code (fail-safe)
2. `/etc/syswall/config.toml` (system)
3. `SYSWALL_*` environment variables (override)

### 13.3 Structure

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
default_policy = "ask"          # ask | allow | block
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
default_timeout_action = "block"  # block | allow | ignore
max_pending_decisions = 50
overflow_action = "block"         # applied when queue is full

[ui]
locale = "fr"
theme = "dark"
refresh_interval_ms = 1000
```

### 13.4 Validation

Config is deserialized into a typed Rust struct via `serde`. Fail-fast at startup — if a value is invalid, daemon refuses to start with a clear error message.

### 13.5 Versioning

`config_version = 1` field enables future migrations if the schema changes.
