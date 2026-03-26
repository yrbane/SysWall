# SysWall -- Audit & Journal Enhancement Design Spec

**Date:** 2026-03-26
**Scope:** Sub-project 5 -- Audit & Journal Enhancement
**Status:** Draft
**Depends on:** Sub-project 1 (Foundations), Sub-project 2 (Firewall Engine), Sub-project 3 (gRPC Services + Auto-Learning) -- all complete

---

## 1. Overview

Sub-projects 1 through 3 delivered the full SysWall architecture: domain model, application services, SQLite persistence, real nftables/conntrack/procfs adapters, a supervisor-managed daemon with connection monitoring pipeline, gRPC control and event services, and the auto-learning loop. Sub-project 4 delivered the premium UI.

The `AuditService` exists and can convert `DomainEvent` instances into `AuditEvent` records, but it has critical gaps:

1. **No automatic listener**: `record_event()` exists but nothing calls it. Events flow through the `EventBus` without being recorded.
2. **No filtering in SQL**: The `SqliteAuditRepository::query()` ignores `AuditFilters` entirely -- it just paginates all rows.
3. **No retention management**: Events accumulate forever with no rotation or cleanup.
4. **No statistics**: No aggregation queries for dashboards.
5. **No export**: No way to export audit logs.
6. **No batch writes**: Each event is written individually, which is inefficient under high event throughput.

After this sub-project:

- The daemon automatically records all relevant domain events to the audit journal via a background listener task
- Events are buffered and batch-written (configurable buffer size and flush interval from `config.database`)
- The audit repository supports full-text search, date range filtering, severity/category filtering, and paginated results with total count
- Old events are automatically purged based on `journal_retention_days`
- Dashboard statistics are available via `AuditService::get_stats()`
- The `QueryAuditLog` and `GetDashboardStats` gRPC RPCs return real data
- Audit events can be exported as JSON

This is sub-project 5 of 6:
1. Foundations (complete)
2. Firewall Engine (complete)
3. gRPC Services + Auto-Learning Integration (complete)
4. Premium UI (complete)
5. **Audit & Journal Enhancement** (this spec)
6. Polish & packaging

## 2. Architecture Context

### 2.1 Existing Components

**AuditService** (`crates/app/src/services/audit_service.rs`):
- `record_event(&DomainEvent)` -- converts to `AuditEvent` and calls `audit_repo.append()`
- `query_events(&AuditFilters, &Pagination)` -- delegates to `audit_repo.query()`
- `count_events(&AuditFilters)` -- delegates to `audit_repo.count()`
- Handles: `ConnectionDetected`, `RuleCreated`, `RuleDeleted`, `DecisionResolved`, `DecisionExpired`, `SystemError`
- Silently ignores: `RuleUpdated`, `ConnectionUpdated`, `ConnectionClosed`, `RuleMatched`, `DecisionRequired`, `FirewallStatusChanged`

**SqliteAuditRepository** (`crates/infra/src/persistence/audit_repository.rs`):
- `append()` -- single-row INSERT
- `query()` -- ignores `_filters`, just does `SELECT ... ORDER BY timestamp DESC LIMIT ? OFFSET ?`
- `count()` -- ignores `_filters`, just does `SELECT COUNT(*)`

**AuditFilters** (`crates/domain/src/ports/repositories.rs`):
- Already defined with `severity`, `category`, `search`, `from`, `to` fields
- But nobody implements filtering on them

**AuditRepository trait** (`crates/domain/src/ports/repositories.rs`):
- `append()`, `query()`, `count()` -- need extension for batch writes, deletion, and stats

**Database schema** (`crates/infra/src/persistence/database.rs`):
- `audit_events` table with columns: `id`, `timestamp`, `severity`, `category`, `description`, `metadata_json`
- Existing indices on: `timestamp`, `severity`, `category`

**Config** (`crates/daemon/src/config.rs`):
- `database.journal_retention_days: u32` -- already defined (default 30)
- `database.audit_batch_size: usize` -- already defined (default 100)
- `database.audit_flush_interval_secs: u64` -- already defined (default 2)

**EventBus** (`crates/infra/src/event_bus/mod.rs`):
- `TokioBroadcastEventBus` with `publish()` and `subscribe()` returning `broadcast::Receiver<DomainEvent>`

**Supervisor** (`crates/daemon/src/supervisor.rs`):
- `spawn(name, future)` -- adds named task to JoinSet
- Already manages: signal-handler, connection-monitor, grpc-server, decision-expiry

**gRPC** (`crates/daemon/src/grpc/`):
- `SysWallControlService` -- has 7 RPCs, none audit-related
- `SysWallEventService` -- streams `DomainEvent` instances
- Proto defines `SysWallControl` and `SysWallEvents` services

### 2.2 What's Missing (This Spec)

| Gap | Location | Description |
|---|---|---|
| Audit event listener | `daemon/src/main.rs` | Background task subscribing to EventBus, feeding events to AuditService |
| Batch write buffer | `app/src/services/audit_service.rs` | Buffer events, flush on size threshold or timer |
| SQL filtering | `infra/src/persistence/audit_repository.rs` | Apply AuditFilters in SQL WHERE clause |
| Full-text search | `infra/src/persistence/database.rs` | LIKE-based search on description field |
| Batch append | `domain/src/ports/repositories.rs` | `append_batch(&[AuditEvent])` method on trait |
| Delete old events | `domain/src/ports/repositories.rs` | `delete_before(DateTime)` method on trait |
| Statistics query | `domain/src/ports/repositories.rs` | `get_stats(from, to)` method on trait |
| AuditStats entity | `domain/src/entities/audit.rs` | Struct for aggregated statistics |
| Retention cleanup | `daemon/src/main.rs` | Periodic supervisor task purging old events |
| Dashboard stats RPC | `proto/syswall.proto` | `GetDashboardStats` RPC and messages |
| Audit log query RPC | `proto/syswall.proto` | `QueryAuditLog` RPC and messages |
| Export RPC | `proto/syswall.proto` | `ExportAuditLog` RPC and messages |
| Export service method | `app/src/services/audit_service.rs` | `export_events(filters, format) -> Vec<u8>` |
| gRPC handler updates | `daemon/src/grpc/control_service.rs` | Implement audit-related RPCs |
| Fake repo updates | `app/src/fakes/fake_audit_repository.rs` | Implement new trait methods |

## 3. Audit Event Listener

### 3.1 Background Task

The daemon spawns an `audit-listener` task in the Supervisor. This task:

1. Calls `event_bus.subscribe()` to get a `broadcast::Receiver<DomainEvent>`
2. Loops, receiving events from the broadcast channel
3. Feeds each event to the `AuditService` for recording
4. Respects the `CancellationToken` for graceful shutdown

```
EventBus (broadcast)
    |
    v
audit-listener task (Supervisor)
    |
    v
AuditService::record_event()
    |
    v
AuditRepository::append() or batch buffer
```

### 3.2 Batch Write Buffer

To avoid one SQLite write per event (which could be hundreds per second during connection storms), the `AuditService` gains an internal buffer:

**Architecture decision:** The buffering lives inside a new `BufferedAuditWriter` struct that wraps the `AuditRepository`. The `AuditService` uses this writer instead of calling `append()` directly from the listener path.

```rust
pub struct BufferedAuditWriter {
    repo: Arc<dyn AuditRepository>,
    buffer: Mutex<Vec<AuditEvent>>,
    batch_size: usize,
}
```

**Flush triggers:**
- Buffer reaches `batch_size` events (from `config.database.audit_batch_size`, default 100)
- A periodic flush timer fires every `audit_flush_interval_secs` (default 2 seconds)

**Flush timer:** The `audit-listener` task uses `tokio::select!` to listen for both new events AND a periodic timer. When the timer fires, it calls `flush()` even if the buffer is below threshold.

```rust
loop {
    tokio::select! {
        _ = cancel.cancelled() => {
            writer.flush().await?;
            break;
        }
        _ = flush_interval.tick() => {
            writer.flush().await?;
        }
        result = receiver.recv() => {
            match result {
                Ok(event) => {
                    writer.buffer_event(&event).await?;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Audit listener lagged, missed {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}
```

**Graceful shutdown:** On cancellation, the task flushes remaining buffered events before exiting.

### 3.3 AuditService Changes

The `AuditService` gets a new method for the listener path:

```rust
impl AuditService {
    /// Buffer a domain event for batch writing.
    pub async fn buffer_event(&self, event: &DomainEvent) -> Result<(), DomainError> { ... }

    /// Flush all buffered events to the repository.
    pub async fn flush(&self) -> Result<(), DomainError> { ... }
}
```

The existing `record_event()` remains for synchronous single-event recording (used in tests and direct calls). The buffer methods are used by the daemon listener task.

### 3.4 Extended DomainEvent Coverage

The `record_event()` method is extended to handle more event types:

| DomainEvent | Severity | Category | Description |
|---|---|---|---|
| `ConnectionDetected` | Debug | Connection | "Connection detected: {source} -> {dest}" |
| `ConnectionClosed` | Debug | Connection | "Connection closed: {id}" |
| `RuleCreated` | Info | Rule | "Rule created: {name}" |
| `RuleUpdated` | Info | Rule | "Rule updated: {name}" |
| `RuleDeleted` | Info | Rule | "Rule deleted: {id}" |
| `RuleMatched` | Debug | Rule | "Rule {rule_id} matched connection {conn_id}: {verdict}" |
| `DecisionRequired` | Info | Decision | "Decision required for {process} -> {dest}" |
| `DecisionResolved` | Info | Decision | "Decision resolved: {action}" |
| `DecisionExpired` | Warning | Decision | "Decision expired: {id}" |
| `FirewallStatusChanged` | Info | System | "Firewall status changed: enabled={enabled}" |
| `SystemError` | (from event) | System | (from event) |

Previously skipped events (`RuleUpdated`, `ConnectionClosed`, `RuleMatched`, `DecisionRequired`, `FirewallStatusChanged`) are now recorded.

## 4. Enhanced Audit Repository

### 4.1 AuditRepository Trait Extensions

The `AuditRepository` trait gains new methods:

```rust
#[async_trait]
pub trait AuditRepository: Send + Sync {
    // Existing
    async fn append(&self, event: &AuditEvent) -> Result<(), DomainError>;
    async fn query(&self, filters: &AuditFilters, pagination: &Pagination) -> Result<Vec<AuditEvent>, DomainError>;
    async fn count(&self, filters: &AuditFilters) -> Result<u64, DomainError>;

    // New
    async fn append_batch(&self, events: &[AuditEvent]) -> Result<(), DomainError>;
    async fn delete_before(&self, before: chrono::DateTime<chrono::Utc>) -> Result<u64, DomainError>;
    async fn get_stats(&self, from: chrono::DateTime<chrono::Utc>, to: chrono::DateTime<chrono::Utc>) -> Result<AuditStats, DomainError>;
}
```

### 4.2 SQL Filtering Implementation

The `SqliteAuditRepository::query()` method is rewritten to build a dynamic WHERE clause from `AuditFilters`:

```sql
SELECT id, timestamp, severity, category, description, metadata_json
FROM audit_events
WHERE 1=1
  AND (?1 IS NULL OR severity = ?1)
  AND (?2 IS NULL OR category = ?2)
  AND (?3 IS NULL OR description LIKE '%' || ?3 || '%')
  AND (?4 IS NULL OR timestamp >= ?4)
  AND (?5 IS NULL OR timestamp <= ?5)
ORDER BY timestamp DESC
LIMIT ?6 OFFSET ?7
```

**Implementation approach:** Build the SQL string dynamically and collect params into a `Vec` to avoid binding NULL parameters for unused filters. This is cleaner than binding NULLs and using `?N IS NULL OR` patterns in SQLite.

```rust
async fn query(&self, filters: &AuditFilters, pagination: &Pagination) -> Result<Vec<AuditEvent>, DomainError> {
    let filters = filters.clone();
    let offset = pagination.offset;
    let limit = pagination.limit;
    let db = self.db.clone();

    tokio::task::spawn_blocking(move || {
        db.with_writer(|conn| {
            let mut sql = String::from(
                "SELECT id, timestamp, severity, category, description, metadata_json FROM audit_events WHERE 1=1"
            );
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

            if let Some(ref severity) = filters.severity {
                sql.push_str(" AND severity = ?");
                params.push(Box::new(serde_json::to_string(severity).unwrap().trim_matches('"').to_string()));
            }
            if let Some(ref category) = filters.category {
                sql.push_str(" AND category = ?");
                params.push(Box::new(serde_json::to_string(category).unwrap().trim_matches('"').to_string()));
            }
            if let Some(ref search) = filters.search {
                sql.push_str(" AND description LIKE ?");
                params.push(Box::new(format!("%{}%", search)));
            }
            if let Some(ref from) = filters.from {
                sql.push_str(" AND timestamp >= ?");
                params.push(Box::new(from.to_rfc3339()));
            }
            if let Some(ref to) = filters.to {
                sql.push_str(" AND timestamp <= ?");
                params.push(Box::new(to.to_rfc3339()));
            }

            sql.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");
            params.push(Box::new(limit as i64));
            params.push(Box::new(offset as i64));

            // Execute with dynamic params...
        })
    }).await?
}
```

The `count()` method follows the same pattern but returns `SELECT COUNT(*)` with the same WHERE clause (without LIMIT/OFFSET).

### 4.3 Batch Append

```rust
async fn append_batch(&self, events: &[AuditEvent]) -> Result<(), DomainError> {
    let events = events.to_vec();
    let db = self.db.clone();
    tokio::task::spawn_blocking(move || {
        db.with_writer(|conn| {
            let tx = conn.unchecked_transaction()?;
            let mut stmt = tx.prepare_cached(
                "INSERT INTO audit_events (id, timestamp, severity, category, description, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            )?;
            for event in &events {
                stmt.execute(rusqlite::params![
                    event.id.as_uuid().to_string(),
                    event.timestamp.to_rfc3339(),
                    /* severity, category, description, metadata_json */
                ])?;
            }
            tx.commit()?;
            Ok(())
        })
    }).await?
}
```

Using a transaction ensures atomicity and improves performance (SQLite commits once instead of per-row).

### 4.4 Delete Before (Rotation)

```rust
async fn delete_before(&self, before: DateTime<Utc>) -> Result<u64, DomainError> {
    let db = self.db.clone();
    tokio::task::spawn_blocking(move || {
        db.with_writer(|conn| {
            let deleted = conn.execute(
                "DELETE FROM audit_events WHERE timestamp < ?1",
                rusqlite::params![before.to_rfc3339()],
            )?;
            Ok(deleted as u64)
        })
    }).await?
}
```

### 4.5 Statistics Query

```rust
async fn get_stats(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<AuditStats, DomainError> {
    let db = self.db.clone();
    tokio::task::spawn_blocking(move || {
        db.with_writer(|conn| {
            // Total count in range
            let total: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_events WHERE timestamp >= ?1 AND timestamp <= ?2",
                params![from.to_rfc3339(), to.to_rfc3339()],
                |row| row.get(0),
            )?;

            // Counts per category
            let mut stmt = conn.prepare(
                "SELECT category, COUNT(*) FROM audit_events
                 WHERE timestamp >= ?1 AND timestamp <= ?2
                 GROUP BY category"
            )?;
            let by_category: HashMap<String, u64> = stmt.query_map(
                params![from.to_rfc3339(), to.to_rfc3339()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64)),
            )?.filter_map(|r| r.ok()).collect();

            // Counts per severity
            let mut stmt = conn.prepare(
                "SELECT severity, COUNT(*) FROM audit_events
                 WHERE timestamp >= ?1 AND timestamp <= ?2
                 GROUP BY severity"
            )?;
            let by_severity: HashMap<String, u64> = stmt.query_map(
                params![from.to_rfc3339(), to.to_rfc3339()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64)),
            )?.filter_map(|r| r.ok()).collect();

            Ok(AuditStats {
                total: total as u64,
                by_category,
                by_severity,
            })
        })
    }).await?
}
```

### 4.6 Schema Migration

Add a new index for full-text-like searching (speeds up LIKE queries when the pattern has a leading wildcard):

```sql
-- No new FTS table needed; LIKE '%search%' is sufficient for the expected data volume.
-- The existing idx_audit_timestamp index supports date range filtering efficiently.
```

No schema changes are required. The existing `audit_events` table and indices are sufficient. SQLite's LIKE operator on the `description` column is adequate for the expected audit log volume (thousands to low millions of rows). If performance becomes an issue, FTS5 can be added later.

## 5. AuditStats Entity

### 5.1 Definition

```rust
// crates/domain/src/entities/audit.rs

/// Aggregated audit statistics for a time range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    /// Total number of events in the time range.
    pub total: u64,
    /// Event counts grouped by category (key: category string, value: count).
    pub by_category: HashMap<String, u64>,
    /// Event counts grouped by severity (key: severity string, value: count).
    pub by_severity: HashMap<String, u64>,
}
```

### 5.2 AuditService Stats Method

```rust
impl AuditService {
    /// Get aggregated statistics for a time range.
    pub async fn get_stats(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<AuditStats, DomainError> {
        self.audit_repo.get_stats(from, to).await
    }
}
```

## 6. Retention Cleanup

### 6.1 Periodic Cleanup Task

The daemon spawns an `audit-cleanup` task in the Supervisor:

```rust
supervisor.spawn("audit-cleanup", {
    let audit_service = ctx.audit_service.clone();
    let retention_days = config.database.journal_retention_days;
    let cancel = cancel.clone();

    async move {
        let cleanup_interval = Duration::from_secs(3600); // Run hourly
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(cleanup_interval) => {
                    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
                    match audit_service.delete_before(cutoff).await {
                        Ok(deleted) if deleted > 0 => {
                            info!("Audit cleanup: purged {} events older than {} days", deleted, retention_days);
                        }
                        Err(e) => warn!("Audit cleanup error: {}", e),
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
});
```

### 6.2 AuditService Delete Method

```rust
impl AuditService {
    /// Delete events older than the given timestamp. Returns count of deleted events.
    pub async fn delete_before(&self, before: DateTime<Utc>) -> Result<u64, DomainError> {
        self.audit_repo.delete_before(before).await
    }
}
```

The cleanup runs every hour. With `journal_retention_days = 30`, events older than 30 days are purged.

## 7. Export Support

### 7.1 Export Format

Only JSON is supported initially. The export produces a JSON array of audit events:

```json
[
    {
        "id": "...",
        "timestamp": "2026-03-26T10:00:00Z",
        "severity": "Info",
        "category": "Rule",
        "description": "Rule created: Block SSH",
        "metadata": { "rule_id": "..." }
    },
    ...
]
```

### 7.2 AuditService Export Method

```rust
/// Supported export formats.
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Json,
}

impl AuditService {
    /// Export audit events matching the given filters as bytes in the specified format.
    pub async fn export_events(
        &self,
        filters: &AuditFilters,
        format: ExportFormat,
    ) -> Result<Vec<u8>, DomainError> {
        // Query all matching events (no pagination limit for export)
        let pagination = Pagination { offset: 0, limit: 100_000 };
        let events = self.audit_repo.query(filters, &pagination).await?;

        match format {
            ExportFormat::Json => {
                serde_json::to_vec_pretty(&events)
                    .map_err(|e| DomainError::Infrastructure(format!("JSON serialization failed: {}", e)))
            }
        }
    }
}
```

The export has a hard limit of 100,000 events to prevent unbounded memory usage. The caller can use filters to narrow the scope.

## 8. gRPC Integration

### 8.1 Proto Changes

Add three RPCs to the `SysWallControl` service and corresponding messages:

```protobuf
service SysWallControl {
    // ... existing RPCs ...
    rpc QueryAuditLog(AuditLogRequest) returns (AuditLogResponse);
    rpc GetDashboardStats(DashboardStatsRequest) returns (DashboardStatsResponse);
    rpc ExportAuditLog(ExportAuditLogRequest) returns (ExportAuditLogResponse);
}

message AuditLogRequest {
    string severity = 1;         // optional: "Debug", "Info", "Warning", "Error", "Critical"
    string category = 2;         // optional: "Connection", "Rule", "Decision", "System", "Config"
    string search = 3;           // optional: full-text search on description
    string from = 4;             // optional: RFC 3339 timestamp
    string to = 5;               // optional: RFC 3339 timestamp
    uint64 offset = 6;
    uint64 limit = 7;
}

message AuditLogResponse {
    repeated AuditEventMessage events = 1;
    uint64 total_count = 2;
}

message AuditEventMessage {
    string id = 1;
    string timestamp = 2;
    string severity = 3;
    string category = 4;
    string description = 5;
    string metadata_json = 6;
}

message DashboardStatsRequest {
    string from = 1;             // RFC 3339 timestamp
    string to = 2;               // RFC 3339 timestamp
}

message DashboardStatsResponse {
    uint64 total_events = 1;
    map<string, uint64> by_category = 2;
    map<string, uint64> by_severity = 3;
}

message ExportAuditLogRequest {
    string severity = 1;
    string category = 2;
    string search = 3;
    string from = 4;
    string to = 5;
    string format = 6;           // "json"
}

message ExportAuditLogResponse {
    bytes data = 1;
    string content_type = 2;     // "application/json"
}
```

### 8.2 Control Service Extensions

The `SysWallControlService` gains an `Arc<AuditService>` field and implements the three new RPCs.

```rust
pub struct SysWallControlService {
    rule_service: Arc<RuleService>,
    learning_service: Arc<LearningService>,
    firewall: Arc<dyn FirewallEngine>,
    audit_service: Arc<AuditService>,  // NEW
}
```

#### QueryAuditLog

1. Parse `AuditLogRequest` into `AuditFilters` and `Pagination`
2. Call `audit_service.query_events()` and `audit_service.count_events()` in parallel
3. Convert `Vec<AuditEvent>` to `Vec<AuditEventMessage>`
4. Return `AuditLogResponse { events, total_count }`

#### GetDashboardStats

1. Parse `from` and `to` timestamps from request
2. Call `audit_service.get_stats(from, to)`
3. Convert `AuditStats` to `DashboardStatsResponse`

#### ExportAuditLog

1. Parse request into `AuditFilters` and `ExportFormat`
2. Call `audit_service.export_events(filters, format)`
3. Return `ExportAuditLogResponse { data, content_type }`

### 8.3 Converters

New converter functions in `grpc/converters.rs`:

```rust
pub fn audit_event_to_proto(event: &AuditEvent) -> AuditEventMessage { ... }
pub fn proto_to_audit_filters(req: &AuditLogRequest) -> AuditFilters { ... }
pub fn audit_stats_to_proto(stats: &AuditStats) -> DashboardStatsResponse { ... }
```

### 8.4 Event Service: Audit Event Streaming

The existing `SubscribeEvents` RPC already streams all `DomainEvent` variants. No changes needed -- audit events are domain events first, and the UI can filter client-side.

## 9. Dependency Changes

### 9.1 No New External Dependencies

All required crates are already in the workspace:
- `chrono` (for date calculations)
- `serde_json` (for export serialization)
- `tokio` (for intervals, select, spawn_blocking)

### 9.2 Proto Regeneration

After modifying `proto/syswall.proto`, the generated code in `syswall-proto` must be regenerated. The build script handles this automatically.

## 10. Testing Strategy

### 10.1 Unit Tests

**BufferedAuditWriter** (`app/src/services/audit_service.rs`):
- Buffer events below threshold: verify no flush occurs
- Buffer events at threshold: verify flush writes batch
- Flush on timer: buffer some events, call flush, verify all written
- Graceful shutdown: buffer events, signal cancellation, verify all flushed

**AuditService**:
- `record_event()` for all DomainEvent variants: verify correct severity, category, description
- `get_stats()` returns correct aggregation
- `export_events()` returns valid JSON

### 10.2 Integration Tests (with Real SQLite)

**SqliteAuditRepository filtering**:
- Insert events with different severities, query with severity filter: verify only matching returned
- Insert events with different categories, query with category filter: verify only matching returned
- Insert events, query with search term: verify LIKE matching works
- Insert events across time range, query with from/to: verify date range works
- Combine multiple filters: verify AND logic
- Pagination with filters: verify correct offset/limit behavior
- Count with filters: verify count matches filtered set

**Batch append**:
- `append_batch()` with 0 events: succeeds, no rows added
- `append_batch()` with 100 events: all 100 present in queries
- `append_batch()` atomicity: if format is consistent, all rows inserted in one transaction

**Delete before**:
- Insert events at different timestamps, delete before cutoff: verify old events gone, recent events remain
- Delete before future timestamp: all events deleted
- Delete before past timestamp: no events deleted

**Statistics**:
- Insert known events, verify `get_stats()` counts match expected values per category and severity
- Empty time range: returns zero counts

### 10.3 Daemon Integration Tests

**Audit listener** (using fake EventBus and repo):
- Publish events to EventBus, verify they appear in AuditRepository after flush
- Publish burst of events exceeding batch_size, verify batch write occurs
- Cancel token triggers: verify remaining buffer is flushed

### 10.4 gRPC Tests

- `QueryAuditLog` with various filter combinations
- `GetDashboardStats` returns valid stats
- `ExportAuditLog` returns valid JSON bytes

## 11. File Summary

### New Files

| File | Purpose |
|---|---|
| (none) | All changes are modifications to existing files |

### Modified Files

| File | Changes |
|---|---|
| `proto/syswall.proto` | Add `QueryAuditLog`, `GetDashboardStats`, `ExportAuditLog` RPCs and messages |
| `crates/domain/src/entities/audit.rs` | Add `AuditStats` struct |
| `crates/domain/src/ports/repositories.rs` | Add `append_batch()`, `delete_before()`, `get_stats()` to `AuditRepository` trait |
| `crates/app/src/services/audit_service.rs` | Add `BufferedAuditWriter`, batch/flush methods, `get_stats()`, `delete_before()`, `export_events()`, extended DomainEvent coverage |
| `crates/app/src/fakes/fake_audit_repository.rs` | Implement new trait methods |
| `crates/infra/src/persistence/audit_repository.rs` | Implement SQL filtering, batch append, delete_before, get_stats |
| `crates/daemon/src/main.rs` | Add `audit-listener` and `audit-cleanup` supervisor tasks |
| `crates/daemon/src/grpc/control_service.rs` | Add `audit_service` field, implement 3 new RPCs |
| `crates/daemon/src/grpc/converters.rs` | Add audit event converters |
| `crates/daemon/src/bootstrap.rs` | No changes needed (audit_service already in AppContext) |
