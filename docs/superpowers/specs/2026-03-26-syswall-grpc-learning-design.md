# SysWall -- gRPC Services + Auto-Learning Integration Design Spec

**Date:** 2026-03-26
**Scope:** Sub-project 3 -- gRPC Services + Auto-Learning Integration
**Status:** Draft
**Depends on:** Sub-project 1 (Foundations) and Sub-project 2 (Firewall Engine) -- both complete

---

## 1. Overview

Sub-projects 1 and 2 delivered the full architecture: domain model, application services, SQLite persistence, real nftables/conntrack/procfs adapters, daemon with supervisor and monitoring pipeline, and proto definitions with generated code. The daemon has a monitoring pipeline that processes connections and feeds PendingDecision verdicts to the LearningService. However, the gRPC handlers are empty stubs and the daemon cannot communicate with the UI.

This spec covers bringing the daemon from "has proto definitions" to "fully functional gRPC server that the UI can connect to." After this sub-project:

- The UI can connect over Unix socket and call all SysWallControl RPCs (status, rules CRUD, pending decisions, decision response)
- The UI receives real-time DomainEvent streaming via SysWallEvents/SubscribeEvents
- The full auto-learning loop works end-to-end: connection detected -> policy evaluation -> PendingDecision created -> gRPC event streamed to UI -> user responds via gRPC -> rule created -> nftables updated
- Overdue pending decisions are expired periodically by the supervisor

This is sub-project 3 of 6:
1. Foundations (complete)
2. Firewall Engine (complete)
3. **gRPC Services + Auto-Learning Integration** (this spec)
4. Premium UI (dashboard, views, design system)
5. Audit & journal (persistence, search, export)
6. Polish & packaging

## 2. Architecture Context

### 2.1 Existing Components

The following are already implemented and stable:

**Domain layer (`syswall-domain`):**
- All entities: `Rule`, `Connection`, `PendingDecision`, `Decision`, `AuditEvent`
- All value objects: `Port`, `Protocol`, `Direction`, `SocketAddress`, `ExecutablePath`, `RulePriority`
- `PolicyEngine` with full rule matching (application, IP, port, protocol, direction, CIDR)
- `DomainEvent` enum with all variants (connection, rule, decision, status, error events)
- All port traits: `RuleRepository`, `PendingDecisionRepository`, `DecisionRepository`, `AuditRepository`, `FirewallEngine`, `ConnectionMonitor`, `ProcessResolver`, `EventBus`, `UserNotifier`

**Application layer (`syswall-app`):**
- `RuleService`: create/delete/toggle/list rules with firewall sync and event publishing
- `LearningService`: handle_unknown_connection (creates PendingDecision, deduplicates, publishes DecisionRequired event), resolve_decision, expire_overdue, get_pending_decisions
- `ConnectionService`: process_connection (enrichment + PolicyEngine evaluation + event publishing)
- `AuditService`: record_event (DomainEvent -> AuditEvent persistence)
- `CreateRuleCommand`, `RespondToDecisionCommand` command types
- All fake implementations for testing

**Infrastructure layer (`syswall-infra`):**
- SQLite repositories for all entities (with WAL mode, migrations)
- `TokioBroadcastEventBus` (tokio broadcast channel wrapping `EventBus` trait)
- `NftablesFirewallAdapter`, `ConntrackMonitorAdapter`, `ProcfsProcessResolver`

**Daemon (`syswall-daemon`):**
- `SysWallConfig` with all configuration sections including `daemon.socket_path`
- `bootstrap()` wiring all dependencies into `AppContext`
- `Supervisor` with `JoinSet`-based task management and `CancellationToken`
- Signal handler (SIGTERM/SIGINT)
- Connection monitoring pipeline that calls `LearningService::handle_unknown_connection` on PendingDecision verdicts

**Proto (`syswall-proto`):**
- `syswall.proto` with `SysWallControl` (7 RPCs) and `SysWallEvents` (1 streaming RPC) fully defined
- Generated Rust code with server traits `SysWallControl` and `SysWallEvents`
- All message types generated: `RuleMessage`, `PendingDecisionMessage`, `DomainEventMessage`, etc.

### 2.2 What's Missing (This Spec)

| Gap | File | Description |
|---|---|---|
| gRPC control handler | `grpc/control_service.rs` | Implement all 7 `SysWallControl` RPCs |
| gRPC event handler | `grpc/event_service.rs` | Implement `SubscribeEvents` server-side streaming |
| Proto converters | `grpc/converters.rs` | Bidirectional conversion between domain types and proto messages |
| gRPC server setup | `grpc/server.rs` | tonic server on Unix socket with socket permissions |
| gRPC module wiring | `grpc/mod.rs` | Re-export all gRPC modules |
| Daemon integration | `main.rs` | Start gRPC server task in supervisor |
| Daemon integration | `bootstrap.rs` | Expose event_bus for gRPC event streaming |
| Periodic expiration | `main.rs` | Supervisor task calling `LearningService::expire_overdue()` every 30s |
| Daemon dependencies | `Cargo.toml` | Add `serde_json`, `uuid`, `nix` to daemon crate |

## 3. gRPC Control Service

### 3.1 Service Structure

The control service holds `Arc` references to the application services it delegates to:

```rust
pub struct SysWallControlService {
    rule_service: Arc<RuleService>,
    learning_service: Arc<LearningService>,
    firewall: Arc<dyn FirewallEngine>,
}
```

It implements the generated `SysWallControl` trait from `syswall_proto::syswall::sys_wall_control_server::SysWallControl`.

### 3.2 RPC Implementations

#### GetStatus

```
rpc GetStatus(Empty) returns (StatusResponse);
```

- Calls `self.firewall.get_status().await`
- Converts `FirewallStatus` -> `StatusResponse` (direct field mapping)
- On error: returns `tonic::Status::internal()`

#### ListRules

```
rpc ListRules(RuleFiltersRequest) returns (RuleListResponse);
```

- Extracts `offset` and `limit` from request, builds `Pagination`
- Calls `self.rule_service.list_rules(&RuleFilters::default(), &pagination).await`
- Converts each `Rule` -> `RuleMessage` using converter
- Returns `RuleListResponse { rules }`

#### CreateRule

```
rpc CreateRule(CreateRuleRequest) returns (RuleResponse);
```

- Converts `CreateRuleRequest` -> `CreateRuleCommand` via converter (parses JSON fields)
- Calls `self.rule_service.create_rule(cmd).await`
- Converts resulting `Rule` -> `RuleMessage`
- Returns `RuleResponse { rule }`
- On validation error: returns `tonic::Status::invalid_argument()`
- On infra error: returns `tonic::Status::internal()`

#### DeleteRule

```
rpc DeleteRule(RuleIdRequest) returns (Empty);
```

- Parses `id` string as UUID, constructs `RuleId`
- Calls `self.rule_service.delete_rule(&rule_id).await`
- On NotFound: returns `tonic::Status::not_found()`
- On NotPermitted: returns `tonic::Status::permission_denied()`

#### ToggleRule

```
rpc ToggleRule(ToggleRuleRequest) returns (RuleResponse);
```

- Parses `id` as UUID, extracts `enabled` bool
- Calls `self.rule_service.toggle_rule(&rule_id, enabled).await`
- Converts resulting `Rule` -> `RuleMessage`

#### RespondToDecision

```
rpc RespondToDecision(DecisionResponseRequest) returns (DecisionAck);
```

- Converts `DecisionResponseRequest` -> `RespondToDecisionCommand` via converter
- Calls `self.learning_service.resolve_decision(cmd).await`
- Returns `DecisionAck { decision_id: decision.id.as_uuid().to_string() }`

#### ListPendingDecisions

```
rpc ListPendingDecisions(Empty) returns (PendingDecisionListResponse);
```

- Calls `self.learning_service.get_pending_decisions().await`
- Converts each `PendingDecision` -> `PendingDecisionMessage` via converter
- Returns `PendingDecisionListResponse { decisions }`

### 3.3 Error Mapping

All RPC handlers map `DomainError` variants to appropriate gRPC status codes:

| DomainError | gRPC Status |
|---|---|
| `Validation(_)` | `InvalidArgument` |
| `NotFound(_)` | `NotFound` |
| `AlreadyExists(_)` | `AlreadyExists` |
| `Infrastructure(_)` | `Internal` |
| `NotPermitted(_)` | `PermissionDenied` |

A helper function `domain_error_to_status(e: DomainError) -> tonic::Status` centralizes this mapping.

## 4. gRPC Event Service

### 4.1 Service Structure

```rust
pub struct SysWallEventService {
    event_bus: Arc<TokioBroadcastEventBus>,
}
```

### 4.2 SubscribeEvents Implementation

```
rpc SubscribeEvents(SubscribeRequest) returns (stream DomainEventMessage);
```

**Flow:**

1. Call `self.event_bus.subscribe()` to get a `broadcast::Receiver<DomainEvent>`
2. Wrap the receiver in a `tokio_stream::wrappers::BroadcastStream`
3. Map each `DomainEvent` to `DomainEventMessage` using converter
4. Filter out `Lagged` errors (log them, continue streaming)
5. Return the stream as the response

**DomainEvent -> DomainEventMessage conversion:**

Each `DomainEvent` variant is serialized as:
- `event_type`: a discriminant string (e.g., `"connection_detected"`, `"rule_created"`, `"decision_required"`)
- `payload_json`: the variant's inner data serialized as JSON using serde_json
- `timestamp`: current UTC timestamp in RFC 3339 format

This approach keeps the proto schema stable (no oneof explosion) while supporting all current and future event variants.

### 4.3 Backpressure and Lag

The `broadcast::Receiver` can lag if the subscriber is slow. When a `Lagged(n)` error occurs:
- Log a warning with the number of missed events
- Continue streaming (do not disconnect the client)
- The client may miss events but will receive all subsequent ones

## 5. Proto <-> Domain Converters

### 5.1 Module Structure

All converters live in `crates/daemon/src/grpc/converters.rs`. They are pure functions (no I/O, no async).

### 5.2 Rule Conversions

#### Rule -> RuleMessage

```rust
pub fn rule_to_proto(rule: &Rule) -> RuleMessage
```

| Domain field | Proto field | Conversion |
|---|---|---|
| `id` | `id` | `rule.id.as_uuid().to_string()` |
| `name` | `name` | Direct |
| `priority` | `priority` | `rule.priority.value()` |
| `enabled` | `enabled` | Direct |
| `criteria` | `criteria_json` | `serde_json::to_string(&rule.criteria)` |
| `effect` | `effect` | Match to string: `"allow"`, `"block"`, `"ask"`, `"observe"` |
| `scope` | `scope_json` | `serde_json::to_string(&rule.scope)` |
| `source` | `source` | Match to string: `"manual"`, `"auto_learning"`, `"import"`, `"system"` |
| `created_at` | `created_at` | `rule.created_at.to_rfc3339()` |
| `updated_at` | `updated_at` | `rule.updated_at.to_rfc3339()` |

#### CreateRuleRequest -> CreateRuleCommand

```rust
pub fn proto_to_create_rule_cmd(req: &CreateRuleRequest) -> Result<CreateRuleCommand, tonic::Status>
```

| Proto field | Domain field | Conversion |
|---|---|---|
| `name` | `name` | Direct (validate non-empty) |
| `priority` | `priority` | Direct |
| `criteria_json` | `criteria` | `serde_json::from_str::<RuleCriteria>()` |
| `effect` | `effect` | Parse string to `RuleEffect` enum |
| `scope_json` | `scope` | `serde_json::from_str::<RuleScope>()` |
| `source` | `source` | Parse string to `RuleSource` enum |

Parsing failures return `tonic::Status::invalid_argument()` with descriptive message.

### 5.3 PendingDecision Conversions

#### PendingDecision -> PendingDecisionMessage

```rust
pub fn pending_decision_to_proto(pd: &PendingDecision) -> PendingDecisionMessage
```

| Domain field | Proto field | Conversion |
|---|---|---|
| `id` | `id` | `pd.id.as_uuid().to_string()` |
| `connection_snapshot` | `snapshot_json` | `serde_json::to_string(&pd.connection_snapshot)` |
| `requested_at` | `requested_at` | `pd.requested_at.to_rfc3339()` |
| `expires_at` | `expires_at` | `pd.expires_at.to_rfc3339()` |
| `status` | `status` | Match to string: `"pending"`, `"resolved"`, `"expired"`, `"cancelled"` |

#### DecisionResponseRequest -> RespondToDecisionCommand

```rust
pub fn proto_to_respond_cmd(req: &DecisionResponseRequest) -> Result<RespondToDecisionCommand, tonic::Status>
```

| Proto field | Domain field | Conversion |
|---|---|---|
| `pending_decision_id` | `pending_decision_id` | Parse UUID, wrap in `PendingDecisionId::from_uuid()` |
| `action` | `action` | Parse string to `DecisionAction` enum |
| `granularity` | `granularity` | Parse string to `DecisionGranularity` enum |

### 5.4 DomainEvent Conversion

#### DomainEvent -> DomainEventMessage

```rust
pub fn domain_event_to_proto(event: &DomainEvent) -> DomainEventMessage
```

Maps the enum variant to:
- `event_type`: snake_case discriminant string
- `payload_json`: serde_json serialization of inner data
- `timestamp`: `Utc::now().to_rfc3339()`

Event type strings:
| Variant | `event_type` |
|---|---|
| `ConnectionDetected(_)` | `"connection_detected"` |
| `ConnectionUpdated { .. }` | `"connection_updated"` |
| `ConnectionClosed(_)` | `"connection_closed"` |
| `RuleCreated(_)` | `"rule_created"` |
| `RuleUpdated(_)` | `"rule_updated"` |
| `RuleDeleted(_)` | `"rule_deleted"` |
| `RuleMatched { .. }` | `"rule_matched"` |
| `DecisionRequired(_)` | `"decision_required"` |
| `DecisionResolved(_)` | `"decision_resolved"` |
| `DecisionExpired(_)` | `"decision_expired"` |
| `FirewallStatusChanged(_)` | `"firewall_status_changed"` |
| `SystemError { .. }` | `"system_error"` |

### 5.5 String <-> Enum Parsing Helpers

Small helper functions for parsing effect, source, action, granularity, and status strings:

```rust
fn parse_rule_effect(s: &str) -> Result<RuleEffect, tonic::Status>
fn parse_rule_source(s: &str) -> Result<RuleSource, tonic::Status>
fn parse_decision_action(s: &str) -> Result<DecisionAction, tonic::Status>
fn parse_decision_granularity(s: &str) -> Result<DecisionGranularity, tonic::Status>
```

Each returns `tonic::Status::invalid_argument()` on unrecognized values.

## 6. gRPC Server Setup

### 6.1 Unix Socket Transport

The gRPC server listens on a Unix domain socket. The socket path comes from `config.daemon.socket_path` (default: `/var/run/syswall/syswall.sock`).

**Implementation approach:**

tonic supports custom I/O transports. We use `tokio::net::UnixListener` and feed accepted connections to the tonic server via `hyper`'s connection handling:

```rust
pub async fn start_grpc_server(
    socket_path: PathBuf,
    control_service: SysWallControlService,
    event_service: SysWallEventService,
    cancel: CancellationToken,
) -> Result<(), String>
```

Steps:
1. Remove existing socket file if present
2. Create parent directory if needed
3. Bind `UnixListener` to the path
4. Set socket permissions to `0660` (owner + group read/write)
5. Set socket group to `syswall` (if the group exists, otherwise warn and continue)
6. Build tonic `Router` with both services
7. Accept connections in a loop, stopping on cancellation

### 6.2 Socket Permissions

The socket uses permissions `0660` with group `syswall`:
- The daemon runs as root and owns the socket
- Users in the `syswall` group can connect (the Tauri UI runs as the user)
- Other users cannot access the socket

Group resolution uses `nix::unistd::Group::from_name("syswall")`. If the group doesn't exist, the daemon logs a warning and uses the default group (this allows development without creating the group).

### 6.3 Daemon Integration

The gRPC server is started as a supervisor task:

```rust
supervisor.spawn("grpc-server", {
    let cancel = cancel.clone();
    async move {
        start_grpc_server(
            config.daemon.socket_path.clone(),
            control_service,
            event_service,
            cancel,
        ).await
    }
});
```

The `AppContext` needs to be extended to expose the services needed for constructing the gRPC handlers.

## 7. Auto-Learning Flow Completion

### 7.1 End-to-End Flow

The auto-learning flow is already nearly complete. The missing piece is the gRPC transport:

```
conntrack event
    |
    v
ConnectionService::process_connection()
    |
    v
PolicyEngine::evaluate() -> PendingDecision verdict
    |
    v
LearningService::handle_unknown_connection()
    |
    v
PendingDecision persisted + DecisionRequired event published
    |
    v
EventBus -> gRPC SubscribeEvents stream -> UI  [NEW: this spec]
    |
    v
UI displays decision prompt to user
    |
    v
User responds via gRPC RespondToDecision RPC    [NEW: this spec]
    |
    v
LearningService::resolve_decision()
    |
    v
Decision persisted + DecisionResolved event published
```

The monitoring pipeline in `main.rs` already calls `learning_service.handle_unknown_connection()` when the verdict is `PendingDecision`. The gRPC event stream will deliver the `DecisionRequired` event to connected UI clients.

### 7.2 Rule Creation from Decisions

Currently, `LearningService::resolve_decision()` creates a `Decision` record but does not automatically create a firewall rule. The comment in the code says "Rule creation from decisions will be fully wired in sub-project 4." This remains true for this spec -- automatic rule generation from decisions (based on action + granularity) is deferred.

However, the `RespondToDecision` RPC is fully functional: it resolves the pending decision, persists the decision record, and publishes the `DecisionResolved` event.

## 8. Periodic Expiration

### 8.1 Expiration Task

The supervisor spawns a periodic task that calls `LearningService::expire_overdue()` every 30 seconds:

```rust
supervisor.spawn("decision-expiry", {
    let learning_service = ctx.learning_service.clone();
    let cancel = cancel.clone();
    async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    match learning_service.expire_overdue().await {
                        Ok(expired) if !expired.is_empty() => {
                            info!("Expired {} overdue pending decisions", expired.len());
                        }
                        Err(e) => warn!("Decision expiry error: {}", e),
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
});
```

This ensures that pending decisions that are not responded to within `config.learning.prompt_timeout_secs` are automatically expired, publishing `DecisionExpired` events for each one.

## 9. Dependency Changes

### 9.1 Daemon Cargo.toml

The daemon crate needs additional dependencies:

| Dependency | Purpose |
|---|---|
| `serde_json` (workspace) | Serialize/deserialize JSON fields in proto converters |
| `uuid` (workspace) | Parse UUID strings in gRPC requests |
| `nix` (workspace) | Set socket permissions and group ownership |
| `tokio-stream` (workspace) | `BroadcastStream` wrapper for event streaming |
| `hyper` + `hyper-util` + `tower` | Required for tonic Unix socket transport |

### 9.2 Workspace Cargo.toml

Add new workspace dependencies if not already present:

| Dependency | Version |
|---|---|
| `hyper` | `1` |
| `hyper-util` | `0.1` |
| `tower` | `0.5` |
| `http` | `1` |

## 10. Testing Strategy

### 10.1 Unit Tests

**Converters** (`grpc/converters.rs`):
- Round-trip test: `Rule` -> `RuleMessage` -> verify all fields
- Round-trip test: `PendingDecision` -> `PendingDecisionMessage` -> verify all fields
- `CreateRuleRequest` with valid JSON -> `CreateRuleCommand` succeeds
- `CreateRuleRequest` with invalid JSON -> returns `InvalidArgument`
- `DecisionResponseRequest` with valid fields -> `RespondToDecisionCommand` succeeds
- `DecisionResponseRequest` with invalid UUID -> returns `InvalidArgument`
- `DomainEvent` variant -> `DomainEventMessage` with correct event_type and parseable payload_json
- Enum string parsers: valid and invalid inputs for effect, source, action, granularity

**Error mapping**:
- Each `DomainError` variant maps to the correct gRPC status code

### 10.2 Integration Tests

**gRPC control service** (using fake backends):
- `GetStatus` returns valid status
- `CreateRule` -> `ListRules` -> verify rule appears
- `CreateRule` -> `DeleteRule` -> `ListRules` -> verify rule gone
- `CreateRule` -> `ToggleRule(false)` -> verify disabled
- `ListPendingDecisions` returns empty when no decisions
- `RespondToDecision` with invalid ID returns NotFound

**gRPC event service**:
- Subscribe -> publish event to EventBus -> verify event received on stream
- Multiple subscribers receive the same event

### 10.3 What's NOT Tested Here

- Real Unix socket binding (requires root or specific permissions)
- Socket group ownership (requires syswall group to exist)
- Full end-to-end with real nftables/conntrack (covered by sub-project 2 tests)

## 11. File Summary

### New Files

| File | Purpose |
|---|---|
| `crates/daemon/src/grpc/control_service.rs` | SysWallControl trait implementation |
| `crates/daemon/src/grpc/event_service.rs` | SysWallEvents trait implementation |
| `crates/daemon/src/grpc/converters.rs` | Proto <-> domain type conversions |
| `crates/daemon/src/grpc/server.rs` | Unix socket gRPC server setup |

### Modified Files

| File | Changes |
|---|---|
| `crates/daemon/src/grpc/mod.rs` | Replace empty stub with module declarations and re-exports |
| `crates/daemon/src/main.rs` | Add gRPC server and decision-expiry supervisor tasks |
| `crates/daemon/src/bootstrap.rs` | No changes needed (AppContext already exposes all required services) |
| `crates/daemon/Cargo.toml` | Add serde_json, uuid, nix, tokio-stream, hyper, hyper-util, tower, http |
| `Cargo.toml` (workspace) | Add hyper, hyper-util, tower, http workspace dependencies |
