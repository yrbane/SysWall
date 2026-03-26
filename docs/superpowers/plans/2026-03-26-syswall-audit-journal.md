# SysWall Audit & Journal Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enhance the audit subsystem so domain events are automatically recorded via a background listener with batch writes, the audit repository supports full SQL filtering/search/date ranges/statistics, old events are automatically rotated, dashboard stats are served via gRPC, and audit logs can be exported as JSON.

**Architecture:** The daemon gets two new supervisor tasks: `audit-listener` (subscribes to EventBus, buffers events, batch-writes) and `audit-cleanup` (hourly purge of events older than `journal_retention_days`). The `AuditRepository` trait gains `append_batch()`, `delete_before()`, `get_stats()`. The `SqliteAuditRepository` implements dynamic SQL WHERE clause building from `AuditFilters`. Three new gRPC RPCs are added: `QueryAuditLog`, `GetDashboardStats`, `ExportAuditLog`.

**Tech Stack:** Rust, SQLite (rusqlite), tokio (broadcast, intervals, select), serde_json, tonic/prost (gRPC), chrono

**Spec:** `docs/superpowers/specs/2026-03-26-syswall-audit-journal-design.md`

---

## File Map

### crates/domain/src/entities/
| File | Responsibility |
|---|---|
| `audit.rs` | Add `AuditStats` struct |

### crates/domain/src/ports/
| File | Responsibility |
|---|---|
| `repositories.rs` | Extend `AuditRepository` trait with `append_batch()`, `delete_before()`, `get_stats()` |

### crates/app/src/services/
| File | Responsibility |
|---|---|
| `audit_service.rs` | Add `BufferedAuditWriter`, batch/flush methods, `get_stats()`, `delete_before()`, `export_events()`, extend DomainEvent coverage |

### crates/app/src/fakes/
| File | Responsibility |
|---|---|
| `fake_audit_repository.rs` | Implement `append_batch()`, `delete_before()`, `get_stats()` |

### crates/infra/src/persistence/
| File | Responsibility |
|---|---|
| `audit_repository.rs` | Implement SQL filtering in `query()`/`count()`, batch append, delete_before, get_stats |

### crates/daemon/src/
| File | Responsibility |
|---|---|
| `main.rs` | Add `audit-listener` and `audit-cleanup` supervisor tasks |

### crates/daemon/src/grpc/
| File | Responsibility |
|---|---|
| `control_service.rs` | Add `audit_service` field, implement `QueryAuditLog`, `GetDashboardStats`, `ExportAuditLog` |
| `converters.rs` | Add `audit_event_to_proto`, `proto_to_audit_filters`, `audit_stats_to_proto` converters |

### proto/
| File | Responsibility |
|---|---|
| `syswall.proto` | Add 3 RPCs and 7 messages for audit functionality |

---

### Task 1: Proto Schema -- Add Audit RPCs and Messages

**Files:**
- Modify: `proto/syswall.proto`

- [ ] **Step 1: Add audit-related messages to syswall.proto**

Add after the existing messages at the end of the file:

```protobuf
// --- Audit Messages ---

message AuditLogRequest {
    string severity = 1;
    string category = 2;
    string search = 3;
    string from = 4;
    string to = 5;
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
    string from = 1;
    string to = 2;
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
    string format = 6;
}

message ExportAuditLogResponse {
    bytes data = 1;
    string content_type = 2;
}
```

- [ ] **Step 2: Add three RPCs to the SysWallControl service**

Add these RPCs inside the `service SysWallControl { ... }` block:

```protobuf
    rpc QueryAuditLog(AuditLogRequest) returns (AuditLogResponse);
    rpc GetDashboardStats(DashboardStatsRequest) returns (DashboardStatsResponse);
    rpc ExportAuditLog(ExportAuditLogRequest) returns (ExportAuditLogResponse);
```

- [ ] **Step 3: Verify proto compiles**

```bash
cd /home/seb/Dev/SysWall && cargo check -p syswall-proto
```

---

### Task 2: Domain Layer -- AuditStats Entity and Repository Trait Extensions

**Files:**
- Modify: `crates/domain/src/entities/audit.rs`
- Modify: `crates/domain/src/ports/repositories.rs`

- [ ] **Step 1: Add AuditStats struct to audit.rs**

Add after the `AuditEvent` impl block, before `#[cfg(test)]`:

```rust
/// Aggregated audit statistics for a time range.
/// Statistiques d'audit agrégées pour une plage temporelle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    /// Total number of events in the time range.
    /// Nombre total d'événements dans la plage temporelle.
    pub total: u64,
    /// Event counts grouped by category (key: category string, value: count).
    /// Nombre d'événements par catégorie (clé : chaîne de catégorie, valeur : nombre).
    pub by_category: HashMap<String, u64>,
    /// Event counts grouped by severity (key: severity string, value: count).
    /// Nombre d'événements par sévérité (clé : chaîne de sévérité, valeur : nombre).
    pub by_severity: HashMap<String, u64>,
}
```

Make sure `AuditStats` is re-exported from `crates/domain/src/entities/mod.rs`.

- [ ] **Step 2: Extend AuditRepository trait with new methods**

In `crates/domain/src/ports/repositories.rs`, add to the `AuditRepository` trait:

```rust
    /// Append multiple events in a single batch (transactional).
    /// Ajoute plusieurs événements en un seul lot (transactionnel).
    async fn append_batch(&self, events: &[AuditEvent]) -> Result<(), DomainError>;

    /// Delete all events with timestamp before the given cutoff. Returns count deleted.
    /// Supprime tous les événements antérieurs au seuil donné. Retourne le nombre supprimé.
    async fn delete_before(&self, before: chrono::DateTime<chrono::Utc>) -> Result<u64, DomainError>;

    /// Get aggregated statistics for events in the given time range.
    /// Obtient les statistiques agrégées pour les événements dans la plage temporelle donnée.
    async fn get_stats(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<AuditStats, DomainError>;
```

Add `AuditStats` to the imports at the top of the file.

- [ ] **Step 3: Verify domain crate compiles (expect errors from unimplemented trait methods -- that's OK)**

```bash
cd /home/seb/Dev/SysWall && cargo check -p syswall-domain
```

---

### Task 3: Fake Audit Repository -- Implement New Trait Methods

**Files:**
- Modify: `crates/app/src/fakes/fake_audit_repository.rs`

- [ ] **Step 1: Implement append_batch**

```rust
    async fn append_batch(&self, events: &[AuditEvent]) -> Result<(), DomainError> {
        let mut store = self.events.lock().unwrap();
        for event in events {
            store.push(event.clone());
        }
        Ok(())
    }
```

- [ ] **Step 2: Implement delete_before**

```rust
    async fn delete_before(&self, before: chrono::DateTime<chrono::Utc>) -> Result<u64, DomainError> {
        let mut store = self.events.lock().unwrap();
        let before_count = store.len();
        store.retain(|e| e.timestamp >= before);
        Ok((before_count - store.len()) as u64)
    }
```

- [ ] **Step 3: Implement get_stats**

```rust
    async fn get_stats(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<AuditStats, DomainError> {
        let store = self.events.lock().unwrap();
        let filtered: Vec<_> = store.iter()
            .filter(|e| e.timestamp >= from && e.timestamp <= to)
            .collect();

        let total = filtered.len() as u64;
        let mut by_category = std::collections::HashMap::new();
        let mut by_severity = std::collections::HashMap::new();

        for event in &filtered {
            let cat_key = serde_json::to_string(&event.category)
                .unwrap_or_default().trim_matches('"').to_string();
            *by_category.entry(cat_key).or_insert(0u64) += 1;

            let sev_key = serde_json::to_string(&event.severity)
                .unwrap_or_default().trim_matches('"').to_string();
            *by_severity.entry(sev_key).or_insert(0u64) += 1;
        }

        Ok(AuditStats { total, by_category, by_severity })
    }
```

- [ ] **Step 4: Verify app crate compiles**

```bash
cd /home/seb/Dev/SysWall && cargo check -p syswall-app
```

---

### Task 4: SQLite Audit Repository -- Implement Filtering, Batch, Delete, Stats

**Files:**
- Modify: `crates/infra/src/persistence/audit_repository.rs`

- [ ] **Step 1: Rewrite query() to apply AuditFilters in SQL**

Replace the existing `query()` implementation. Build a dynamic WHERE clause:

```rust
async fn query(
    &self,
    filters: &AuditFilters,
    pagination: &Pagination,
) -> Result<Vec<AuditEvent>, DomainError> {
    let filters = filters.clone();
    let offset = pagination.offset;
    let limit = pagination.limit;
    let db = self.db.clone();

    tokio::task::spawn_blocking(move || {
        db.with_writer(|conn| {
            let (sql, params) = Self::build_filtered_query(
                "SELECT id, timestamp, severity, category, description, metadata_json FROM audit_events",
                &filters,
                Some(limit),
                Some(offset),
            );

            let mut stmt = conn.prepare(&sql)
                .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter()
                .map(|p| p.as_ref() as &dyn rusqlite::types::ToSql)
                .collect();

            let events = stmt
                .query_map(param_refs.as_slice(), Self::row_to_audit_event)
                .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                .filter_map(|r| r.ok())
                .collect();

            Ok(events)
        })
    })
    .await
    .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
}
```

Add a private helper `build_filtered_query()` that constructs SQL and params:

```rust
fn build_filtered_query(
    base_sql: &str,
    filters: &AuditFilters,
    limit: Option<u64>,
    offset: Option<u64>,
) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut sql = format!("{} WHERE 1=1", base_sql);
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref severity) = filters.severity {
        sql.push_str(&format!(" AND severity = ?{}", params.len() + 1));
        params.push(Box::new(
            serde_json::to_string(severity).unwrap().trim_matches('"').to_string(),
        ));
    }
    if let Some(ref category) = filters.category {
        sql.push_str(&format!(" AND category = ?{}", params.len() + 1));
        params.push(Box::new(
            serde_json::to_string(category).unwrap().trim_matches('"').to_string(),
        ));
    }
    if let Some(ref search) = filters.search {
        sql.push_str(&format!(" AND description LIKE ?{}", params.len() + 1));
        params.push(Box::new(format!("%{}%", search)));
    }
    if let Some(ref from) = filters.from {
        sql.push_str(&format!(" AND timestamp >= ?{}", params.len() + 1));
        params.push(Box::new(from.to_rfc3339()));
    }
    if let Some(ref to) = filters.to {
        sql.push_str(&format!(" AND timestamp <= ?{}", params.len() + 1));
        params.push(Box::new(to.to_rfc3339()));
    }

    sql.push_str(" ORDER BY timestamp DESC");

    if let Some(limit) = limit {
        sql.push_str(&format!(" LIMIT ?{}", params.len() + 1));
        params.push(Box::new(limit as i64));
        if let Some(offset) = offset {
            sql.push_str(&format!(" OFFSET ?{}", params.len() + 1));
            params.push(Box::new(offset as i64));
        }
    }

    (sql, params)
}
```

- [ ] **Step 2: Rewrite count() to apply AuditFilters in SQL**

Same WHERE clause logic as query(), but with `SELECT COUNT(*)`:

```rust
async fn count(&self, filters: &AuditFilters) -> Result<u64, DomainError> {
    let filters = filters.clone();
    let db = self.db.clone();

    tokio::task::spawn_blocking(move || {
        db.with_writer(|conn| {
            let (sql, params) = Self::build_filtered_query(
                "SELECT COUNT(*) FROM audit_events",
                &filters,
                None,
                None,
            );

            // Remove ORDER BY for count query
            let sql = sql.replace(" ORDER BY timestamp DESC", "");

            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter()
                .map(|p| p.as_ref() as &dyn rusqlite::types::ToSql)
                .collect();

            let count: i64 = conn
                .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
                .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

            Ok(count as u64)
        })
    })
    .await
    .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
}
```

- [ ] **Step 3: Implement append_batch()**

```rust
async fn append_batch(&self, events: &[AuditEvent]) -> Result<(), DomainError> {
    if events.is_empty() {
        return Ok(());
    }
    let events = events.to_vec();
    let db = self.db.clone();

    tokio::task::spawn_blocking(move || {
        db.with_writer(|conn| {
            let tx = conn.unchecked_transaction()
                .map_err(|e| DomainError::Infrastructure(format!("Failed to start transaction: {}", e)))?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT INTO audit_events (id, timestamp, severity, category, description, metadata_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                ).map_err(|e| DomainError::Infrastructure(format!("Failed to prepare statement: {}", e)))?;

                for event in &events {
                    stmt.execute(rusqlite::params![
                        event.id.as_uuid().to_string(),
                        event.timestamp.to_rfc3339(),
                        serde_json::to_string(&event.severity).unwrap().trim_matches('"'),
                        serde_json::to_string(&event.category).unwrap().trim_matches('"'),
                        event.description,
                        serde_json::to_string(&event.metadata).unwrap(),
                    ]).map_err(|e| DomainError::Infrastructure(format!("Failed to insert audit event: {}", e)))?;
                }
            }
            tx.commit()
                .map_err(|e| DomainError::Infrastructure(format!("Failed to commit batch: {}", e)))?;
            Ok(())
        })
    })
    .await
    .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
}
```

- [ ] **Step 4: Implement delete_before()**

```rust
async fn delete_before(&self, before: chrono::DateTime<chrono::Utc>) -> Result<u64, DomainError> {
    let db = self.db.clone();
    tokio::task::spawn_blocking(move || {
        db.with_writer(|conn| {
            let deleted = conn.execute(
                "DELETE FROM audit_events WHERE timestamp < ?1",
                rusqlite::params![before.to_rfc3339()],
            ).map_err(|e| DomainError::Infrastructure(format!("Failed to delete old audit events: {}", e)))?;
            Ok(deleted as u64)
        })
    })
    .await
    .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
}
```

- [ ] **Step 5: Implement get_stats()**

```rust
async fn get_stats(
    &self,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
) -> Result<AuditStats, DomainError> {
    let db = self.db.clone();
    tokio::task::spawn_blocking(move || {
        db.with_writer(|conn| {
            let from_str = from.to_rfc3339();
            let to_str = to.to_rfc3339();

            let total: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_events WHERE timestamp >= ?1 AND timestamp <= ?2",
                rusqlite::params![&from_str, &to_str],
                |row| row.get(0),
            ).map_err(|e| DomainError::Infrastructure(e.to_string()))?;

            let mut by_category = std::collections::HashMap::new();
            {
                let mut stmt = conn.prepare(
                    "SELECT category, COUNT(*) FROM audit_events WHERE timestamp >= ?1 AND timestamp <= ?2 GROUP BY category"
                ).map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                let rows = stmt.query_map(
                    rusqlite::params![&from_str, &to_str],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                ).map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                for row in rows.flatten() {
                    by_category.insert(row.0, row.1 as u64);
                }
            }

            let mut by_severity = std::collections::HashMap::new();
            {
                let mut stmt = conn.prepare(
                    "SELECT severity, COUNT(*) FROM audit_events WHERE timestamp >= ?1 AND timestamp <= ?2 GROUP BY severity"
                ).map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                let rows = stmt.query_map(
                    rusqlite::params![&from_str, &to_str],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                ).map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                for row in rows.flatten() {
                    by_severity.insert(row.0, row.1 as u64);
                }
            }

            Ok(AuditStats { total: total as u64, by_category, by_severity })
        })
    })
    .await
    .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
}
```

- [ ] **Step 6: Add tests for filtered queries**

Add tests to the existing `#[cfg(test)] mod tests` in `audit_repository.rs`:

```rust
#[tokio::test]
async fn query_filters_by_severity() {
    let repo = setup().await;
    repo.append(&AuditEvent::new(Severity::Info, EventCategory::Rule, "info event")).await.unwrap();
    repo.append(&AuditEvent::new(Severity::Error, EventCategory::System, "error event")).await.unwrap();

    let filters = AuditFilters { severity: Some(Severity::Error), ..Default::default() };
    let results = repo.query(&filters, &Pagination::default()).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].description, "error event");
}

#[tokio::test]
async fn query_filters_by_category() { /* similar pattern */ }

#[tokio::test]
async fn query_filters_by_search() { /* LIKE matching */ }

#[tokio::test]
async fn query_filters_by_date_range() { /* from/to timestamps */ }

#[tokio::test]
async fn count_respects_filters() { /* count with severity filter */ }

#[tokio::test]
async fn batch_append_writes_all_events() { /* append_batch with multiple events */ }

#[tokio::test]
async fn batch_append_empty_succeeds() { /* append_batch with empty slice */ }

#[tokio::test]
async fn delete_before_removes_old_events() { /* insert, delete, verify */ }

#[tokio::test]
async fn get_stats_returns_correct_counts() { /* insert known events, verify stats */ }
```

- [ ] **Step 7: Verify infra crate compiles and tests pass**

```bash
cd /home/seb/Dev/SysWall && cargo test -p syswall-infra -- audit
```

---

### Task 5: AuditService Enhancement -- Batch Buffer, Export, Stats, Extended Coverage

**Files:**
- Modify: `crates/app/src/services/audit_service.rs`

- [ ] **Step 1: Add BufferedAuditWriter struct**

Add before the `AuditService` struct:

```rust
use std::sync::Mutex;
use syswall_domain::entities::AuditStats;

/// Buffers audit events and flushes them in batches to the repository.
/// Met en tampon les événements d'audit et les écrit par lots dans le dépôt.
pub struct BufferedAuditWriter {
    repo: Arc<dyn AuditRepository>,
    buffer: Mutex<Vec<AuditEvent>>,
    batch_size: usize,
}

impl BufferedAuditWriter {
    pub fn new(repo: Arc<dyn AuditRepository>, batch_size: usize) -> Self {
        Self {
            repo,
            buffer: Mutex::new(Vec::with_capacity(batch_size)),
            batch_size,
        }
    }

    /// Buffer an event. If the buffer reaches batch_size, flush automatically.
    /// Met en tampon un événement. Si le tampon atteint batch_size, écrit automatiquement.
    pub async fn buffer_event(&self, event: AuditEvent) -> Result<bool, DomainError> {
        let should_flush = {
            let mut buf = self.buffer.lock().map_err(|e| {
                DomainError::Infrastructure(format!("Buffer lock poisoned: {}", e))
            })?;
            buf.push(event);
            buf.len() >= self.batch_size
        };

        if should_flush {
            self.flush().await?;
        }

        Ok(should_flush)
    }

    /// Flush all buffered events to the repository.
    /// Écrit tous les événements en tampon dans le dépôt.
    pub async fn flush(&self) -> Result<(), DomainError> {
        let events = {
            let mut buf = self.buffer.lock().map_err(|e| {
                DomainError::Infrastructure(format!("Buffer lock poisoned: {}", e))
            })?;
            std::mem::take(&mut *buf)
        };

        if !events.is_empty() {
            self.repo.append_batch(&events).await?;
        }

        Ok(())
    }
}
```

- [ ] **Step 2: Extend DomainEvent coverage in record_event()**

Update the `record_event()` match to handle all variants:

```rust
pub async fn record_event(&self, event: &DomainEvent) -> Result<(), DomainError> {
    let audit_event = match event {
        DomainEvent::ConnectionDetected(conn) => AuditEvent::new(
            Severity::Debug,
            EventCategory::Connection,
            format!("Connection detected: {} -> {}", conn.source, conn.destination),
        ),
        DomainEvent::ConnectionClosed(id) => AuditEvent::new(
            Severity::Debug,
            EventCategory::Connection,
            format!("Connection closed: {:?}", id),
        ),
        DomainEvent::RuleCreated(rule) => AuditEvent::new(
            Severity::Info,
            EventCategory::Rule,
            format!("Rule created: {}", rule.name),
        ).with_metadata("rule_id", rule.id.as_uuid().to_string()),
        DomainEvent::RuleUpdated(rule) => AuditEvent::new(
            Severity::Info,
            EventCategory::Rule,
            format!("Rule updated: {}", rule.name),
        ).with_metadata("rule_id", rule.id.as_uuid().to_string()),
        DomainEvent::RuleDeleted(id) => AuditEvent::new(
            Severity::Info,
            EventCategory::Rule,
            format!("Rule deleted: {:?}", id),
        ),
        DomainEvent::RuleMatched { connection_id, rule_id, verdict } => AuditEvent::new(
            Severity::Debug,
            EventCategory::Rule,
            format!("Rule {:?} matched connection {:?}: {:?}", rule_id, connection_id, verdict),
        ),
        DomainEvent::DecisionRequired(pd) => AuditEvent::new(
            Severity::Info,
            EventCategory::Decision,
            format!("Decision required for {} -> {}",
                pd.connection_snapshot.process_name.as_deref().unwrap_or("unknown"),
                pd.connection_snapshot.destination),
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
        DomainEvent::FirewallStatusChanged(status) => AuditEvent::new(
            Severity::Info,
            EventCategory::System,
            format!("Firewall status changed: enabled={}", status.enabled),
        ),
        DomainEvent::SystemError { message, severity } => {
            AuditEvent::new(*severity, EventCategory::System, message.clone())
        }
        _ => return Ok(()),
    };

    self.audit_repo.append(&audit_event).await
}
```

- [ ] **Step 3: Add get_stats() method**

```rust
/// Get aggregated statistics for a time range.
/// Obtient les statistiques agrégées pour une plage temporelle.
pub async fn get_stats(
    &self,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
) -> Result<AuditStats, DomainError> {
    self.audit_repo.get_stats(from, to).await
}
```

- [ ] **Step 4: Add delete_before() method**

```rust
/// Delete events older than the given timestamp. Returns count of deleted events.
/// Supprime les événements antérieurs au seuil donné. Retourne le nombre supprimé.
pub async fn delete_before(
    &self,
    before: chrono::DateTime<chrono::Utc>,
) -> Result<u64, DomainError> {
    self.audit_repo.delete_before(before).await
}
```

- [ ] **Step 5: Add export_events() method**

```rust
/// Supported export formats.
/// Formats d'export supportés.
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Json,
}

/// Export audit events matching the given filters as bytes in the specified format.
/// Exporte les événements d'audit correspondant aux filtres donnés sous forme d'octets.
pub async fn export_events(
    &self,
    filters: &AuditFilters,
    format: ExportFormat,
) -> Result<Vec<u8>, DomainError> {
    let pagination = Pagination { offset: 0, limit: 100_000 };
    let events = self.audit_repo.query(filters, &pagination).await?;

    match format {
        ExportFormat::Json => {
            serde_json::to_vec_pretty(&events)
                .map_err(|e| DomainError::Infrastructure(format!("JSON serialization failed: {}", e)))
        }
    }
}
```

- [ ] **Step 6: Add new unit tests**

Add tests for the new methods and extended event coverage:

```rust
#[tokio::test]
async fn records_rule_updated_event() { /* verify RuleUpdated is now handled */ }

#[tokio::test]
async fn records_rule_matched_event() { /* verify RuleMatched produces Debug/Rule audit event */ }

#[tokio::test]
async fn records_decision_required_event() { /* verify DecisionRequired is now handled */ }

#[tokio::test]
async fn records_firewall_status_changed_event() { /* verify FirewallStatusChanged is now handled */ }

#[tokio::test]
async fn buffered_writer_flushes_at_threshold() {
    let repo = Arc::new(FakeAuditRepository::new());
    let writer = BufferedAuditWriter::new(repo.clone(), 3);

    let e1 = AuditEvent::new(Severity::Info, EventCategory::Rule, "Event 1");
    let e2 = AuditEvent::new(Severity::Info, EventCategory::Rule, "Event 2");
    let e3 = AuditEvent::new(Severity::Info, EventCategory::Rule, "Event 3");

    assert!(!writer.buffer_event(e1).await.unwrap()); // not flushed
    assert!(!writer.buffer_event(e2).await.unwrap()); // not flushed
    assert!(writer.buffer_event(e3).await.unwrap());  // flushed at threshold

    let events = repo.events.lock().unwrap();
    assert_eq!(events.len(), 3);
}

#[tokio::test]
async fn buffered_writer_manual_flush() {
    let repo = Arc::new(FakeAuditRepository::new());
    let writer = BufferedAuditWriter::new(repo.clone(), 100);

    writer.buffer_event(AuditEvent::new(Severity::Info, EventCategory::Rule, "Event 1")).await.unwrap();
    writer.buffer_event(AuditEvent::new(Severity::Info, EventCategory::Rule, "Event 2")).await.unwrap();

    assert_eq!(repo.events.lock().unwrap().len(), 0); // not flushed yet
    writer.flush().await.unwrap();
    assert_eq!(repo.events.lock().unwrap().len(), 2); // flushed
}
```

- [ ] **Step 7: Verify app crate compiles and tests pass**

```bash
cd /home/seb/Dev/SysWall && cargo test -p syswall-app -- audit
```

---

### Task 6: gRPC Converters -- Add Audit Converters

**Files:**
- Modify: `crates/daemon/src/grpc/converters.rs`

- [ ] **Step 1: Add audit event converter**

```rust
use syswall_domain::entities::{AuditEvent, AuditStats, EventCategory, Severity};
use syswall_proto::syswall::{
    AuditEventMessage, AuditLogRequest, DashboardStatsResponse,
};

/// Convert a domain AuditEvent to a proto AuditEventMessage.
/// Convertit un AuditEvent du domaine en AuditEventMessage proto.
pub fn audit_event_to_proto(event: &AuditEvent) -> AuditEventMessage {
    AuditEventMessage {
        id: event.id.as_uuid().to_string(),
        timestamp: event.timestamp.to_rfc3339(),
        severity: serde_json::to_string(&event.severity).unwrap_or_default().trim_matches('"').to_string(),
        category: serde_json::to_string(&event.category).unwrap_or_default().trim_matches('"').to_string(),
        description: event.description.clone(),
        metadata_json: serde_json::to_string(&event.metadata).unwrap_or_default(),
    }
}
```

- [ ] **Step 2: Add proto-to-filters converter**

```rust
use syswall_domain::ports::AuditFilters;

/// Convert an AuditLogRequest into domain AuditFilters.
/// Convertit une AuditLogRequest en AuditFilters du domaine.
pub fn proto_to_audit_filters(req: &AuditLogRequest) -> Result<AuditFilters, tonic::Status> {
    let severity = if req.severity.is_empty() {
        None
    } else {
        Some(parse_severity(&req.severity)?)
    };

    let category = if req.category.is_empty() {
        None
    } else {
        Some(parse_event_category(&req.category)?)
    };

    let search = if req.search.is_empty() { None } else { Some(req.search.clone()) };

    let from = if req.from.is_empty() {
        None
    } else {
        Some(chrono::DateTime::parse_from_rfc3339(&req.from)
            .map_err(|e| tonic::Status::invalid_argument(format!("Invalid 'from' timestamp: {}", e)))?
            .with_timezone(&chrono::Utc))
    };

    let to = if req.to.is_empty() {
        None
    } else {
        Some(chrono::DateTime::parse_from_rfc3339(&req.to)
            .map_err(|e| tonic::Status::invalid_argument(format!("Invalid 'to' timestamp: {}", e)))?
            .with_timezone(&chrono::Utc))
    };

    Ok(AuditFilters { severity, category, search, from, to })
}
```

- [ ] **Step 3: Add severity and category string parsers**

```rust
fn parse_severity(s: &str) -> Result<Severity, tonic::Status> {
    match s {
        "Debug" => Ok(Severity::Debug),
        "Info" => Ok(Severity::Info),
        "Warning" => Ok(Severity::Warning),
        "Error" => Ok(Severity::Error),
        "Critical" => Ok(Severity::Critical),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown severity: '{}'. Expected: Debug, Info, Warning, Error, Critical", s
        ))),
    }
}

fn parse_event_category(s: &str) -> Result<EventCategory, tonic::Status> {
    match s {
        "Connection" => Ok(EventCategory::Connection),
        "Rule" => Ok(EventCategory::Rule),
        "Decision" => Ok(EventCategory::Decision),
        "System" => Ok(EventCategory::System),
        "Config" => Ok(EventCategory::Config),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown category: '{}'. Expected: Connection, Rule, Decision, System, Config", s
        ))),
    }
}
```

- [ ] **Step 4: Add stats converter**

```rust
/// Convert AuditStats to a proto DashboardStatsResponse.
/// Convertit les AuditStats en DashboardStatsResponse proto.
pub fn audit_stats_to_proto(stats: &AuditStats) -> DashboardStatsResponse {
    DashboardStatsResponse {
        total_events: stats.total,
        by_category: stats.by_category.clone(),
        by_severity: stats.by_severity.clone(),
    }
}
```

- [ ] **Step 5: Add unit tests for new converters**

```rust
#[test]
fn audit_event_to_proto_all_fields() {
    let event = AuditEvent::new(Severity::Warning, EventCategory::Rule, "test event")
        .with_metadata("key", "value");
    let msg = audit_event_to_proto(&event);
    assert_eq!(msg.severity, "Warning");
    assert_eq!(msg.category, "Rule");
    assert_eq!(msg.description, "test event");
    assert!(msg.metadata_json.contains("key"));
}

#[test]
fn parse_severity_all_values() {
    assert_eq!(parse_severity("Debug").unwrap(), Severity::Debug);
    assert_eq!(parse_severity("Info").unwrap(), Severity::Info);
    assert_eq!(parse_severity("Warning").unwrap(), Severity::Warning);
    assert_eq!(parse_severity("Error").unwrap(), Severity::Error);
    assert_eq!(parse_severity("Critical").unwrap(), Severity::Critical);
    assert!(parse_severity("bad").is_err());
}

#[test]
fn parse_event_category_all_values() {
    assert_eq!(parse_event_category("Connection").unwrap(), EventCategory::Connection);
    assert_eq!(parse_event_category("Rule").unwrap(), EventCategory::Rule);
    assert_eq!(parse_event_category("Decision").unwrap(), EventCategory::Decision);
    assert_eq!(parse_event_category("System").unwrap(), EventCategory::System);
    assert_eq!(parse_event_category("Config").unwrap(), EventCategory::Config);
    assert!(parse_event_category("bad").is_err());
}
```

- [ ] **Step 6: Verify daemon crate compiles**

```bash
cd /home/seb/Dev/SysWall && cargo check -p syswall-daemon
```

---

### Task 7: gRPC Control Service -- Add Audit RPCs

**Files:**
- Modify: `crates/daemon/src/grpc/control_service.rs`

- [ ] **Step 1: Add audit_service field to SysWallControlService**

Update the struct and constructor:

```rust
use syswall_app::services::audit_service::AuditService;

pub struct SysWallControlService {
    rule_service: Arc<RuleService>,
    learning_service: Arc<LearningService>,
    firewall: Arc<dyn FirewallEngine>,
    audit_service: Arc<AuditService>,
}

impl SysWallControlService {
    pub fn new(
        rule_service: Arc<RuleService>,
        learning_service: Arc<LearningService>,
        firewall: Arc<dyn FirewallEngine>,
        audit_service: Arc<AuditService>,
    ) -> Self {
        Self { rule_service, learning_service, firewall, audit_service }
    }
}
```

- [ ] **Step 2: Add audit-related proto imports**

Update the import block to include the new proto types:

```rust
use syswall_proto::syswall::{
    // ... existing imports ...
    AuditLogRequest, AuditLogResponse, DashboardStatsRequest, DashboardStatsResponse,
    ExportAuditLogRequest, ExportAuditLogResponse,
};

use super::converters::{
    // ... existing imports ...
    audit_event_to_proto, proto_to_audit_filters, audit_stats_to_proto,
};
```

- [ ] **Step 3: Implement query_audit_log RPC**

```rust
async fn query_audit_log(
    &self,
    request: Request<AuditLogRequest>,
) -> Result<Response<AuditLogResponse>, Status> {
    let req = request.into_inner();
    let filters = proto_to_audit_filters(&req)?;
    let pagination = Pagination {
        offset: req.offset,
        limit: if req.limit == 0 { 50 } else { req.limit },
    };

    let (events, total_count) = tokio::try_join!(
        self.audit_service.query_events(&filters, &pagination),
        self.audit_service.count_events(&filters),
    ).map_err(domain_error_to_status)?;

    let event_messages = events.iter().map(audit_event_to_proto).collect();

    Ok(Response::new(AuditLogResponse {
        events: event_messages,
        total_count,
    }))
}
```

- [ ] **Step 4: Implement get_dashboard_stats RPC**

```rust
async fn get_dashboard_stats(
    &self,
    request: Request<DashboardStatsRequest>,
) -> Result<Response<DashboardStatsResponse>, Status> {
    let req = request.into_inner();

    let from = if req.from.is_empty() {
        chrono::Utc::now() - chrono::Duration::hours(1)
    } else {
        chrono::DateTime::parse_from_rfc3339(&req.from)
            .map_err(|e| Status::invalid_argument(format!("Invalid 'from' timestamp: {}", e)))?
            .with_timezone(&chrono::Utc)
    };

    let to = if req.to.is_empty() {
        chrono::Utc::now()
    } else {
        chrono::DateTime::parse_from_rfc3339(&req.to)
            .map_err(|e| Status::invalid_argument(format!("Invalid 'to' timestamp: {}", e)))?
            .with_timezone(&chrono::Utc)
    };

    let stats = self.audit_service
        .get_stats(from, to)
        .await
        .map_err(domain_error_to_status)?;

    Ok(Response::new(audit_stats_to_proto(&stats)))
}
```

- [ ] **Step 5: Implement export_audit_log RPC**

```rust
use syswall_app::services::audit_service::ExportFormat;

async fn export_audit_log(
    &self,
    request: Request<ExportAuditLogRequest>,
) -> Result<Response<ExportAuditLogResponse>, Status> {
    let req = request.into_inner();

    let filters = proto_to_audit_filters(&AuditLogRequest {
        severity: req.severity,
        category: req.category,
        search: req.search,
        from: req.from,
        to: req.to,
        offset: 0,
        limit: 0,
    })?;

    let format = match req.format.as_str() {
        "json" | "" => ExportFormat::Json,
        _ => return Err(Status::invalid_argument(format!(
            "Unsupported export format: '{}'. Expected: json", req.format
        ))),
    };

    let data = self.audit_service
        .export_events(&filters, format)
        .await
        .map_err(domain_error_to_status)?;

    Ok(Response::new(ExportAuditLogResponse {
        data,
        content_type: "application/json".to_string(),
    }))
}
```

- [ ] **Step 6: Verify daemon crate compiles**

```bash
cd /home/seb/Dev/SysWall && cargo check -p syswall-daemon
```

---

### Task 8: Daemon Integration -- Supervisor Tasks and Wiring

**Files:**
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Update SysWallControlService construction to pass audit_service**

In the gRPC server task setup, change:

```rust
let control_service = SysWallControlService::new(
    ctx.rule_service.clone(),
    ctx.learning_service.clone(),
    ctx.firewall.clone(),
    ctx.audit_service.clone(),  // ADD THIS
);
```

- [ ] **Step 2: Add audit-listener supervisor task**

Add after the connection-monitor task and before the gRPC server task:

```rust
// Audit event listener -- subscribes to EventBus and batch-writes to audit repository
supervisor.spawn("audit-listener", {
    let audit_service = ctx.audit_service.clone();
    let event_bus = ctx.event_bus.clone();
    let batch_size = config.database.audit_batch_size;
    let flush_interval_secs = config.database.audit_flush_interval_secs;
    let cancel = cancel.clone();

    async move {
        use syswall_app::services::audit_service::BufferedAuditWriter;
        use syswall_domain::ports::EventBus;

        let writer = BufferedAuditWriter::new(
            // We need the audit_repo here, but AuditService wraps it.
            // Solution: AuditService exposes a method to create a buffered writer,
            // or we pass the repo separately. For simplicity, use AuditService's
            // record_event with manual batching via BufferedAuditWriter.
            // Actually, BufferedAuditWriter needs Arc<dyn AuditRepository>.
            // We'll need to make this available from AppContext.
        );

        // Alternative approach: use AuditService directly with its own buffering
        let mut receiver = event_bus.subscribe();
        let mut flush_interval = tokio::time::interval(
            std::time::Duration::from_secs(flush_interval_secs)
        );

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    // Flush remaining buffered events on shutdown
                    break;
                }
                _ = flush_interval.tick() => {
                    // Timer-based flush (handled by BufferedAuditWriter)
                }
                result = receiver.recv() => {
                    match result {
                        Ok(event) => {
                            if let Err(e) = audit_service.record_event(&event).await {
                                warn!("Audit listener: failed to record event: {}", e);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Audit listener lagged, missed {} events", n);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            info!("Audit listener: event bus closed");
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }
});
```

**Important implementation note:** The simplest initial approach is to use `audit_service.record_event()` directly in the listener (single writes). The `BufferedAuditWriter` can be integrated in a follow-up refinement by:
1. Adding `audit_repo` to `AppContext` (or adding a `create_buffered_writer(batch_size)` method to `AuditService`)
2. Replacing direct `record_event()` calls with `writer.buffer_event()` + periodic `writer.flush()`

For the initial implementation, use direct writes to keep the listener simple and correct. Add buffering as a refinement step.

**Refined approach with buffering:**

To properly integrate `BufferedAuditWriter`, modify `AuditService` to hold an optional writer:

```rust
impl AuditService {
    /// Create a BufferedAuditWriter using this service's repository.
    pub fn create_buffered_writer(&self, batch_size: usize) -> BufferedAuditWriter {
        BufferedAuditWriter::new(self.audit_repo.clone(), batch_size)
    }
}
```

Then in the listener:

```rust
let writer = audit_service.create_buffered_writer(batch_size);

loop {
    tokio::select! {
        _ = cancel.cancelled() => {
            if let Err(e) = writer.flush().await {
                error!("Audit listener: flush on shutdown failed: {}", e);
            }
            break;
        }
        _ = flush_interval.tick() => {
            if let Err(e) = writer.flush().await {
                warn!("Audit listener: periodic flush failed: {}", e);
            }
        }
        result = receiver.recv() => {
            match result {
                Ok(event) => {
                    let audit_event = audit_service.convert_event(&event);
                    if let Some(audit_event) = audit_event {
                        if let Err(e) = writer.buffer_event(audit_event).await {
                            warn!("Audit listener: buffer failed: {}", e);
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Audit listener lagged, missed {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}
```

This requires extracting the conversion logic from `record_event()` into a separate `convert_event()` method that returns `Option<AuditEvent>`.

- [ ] **Step 3: Add convert_event() method to AuditService**

Add to `crates/app/src/services/audit_service.rs`:

```rust
/// Convert a domain event to an audit event, if applicable. Returns None for events
/// that should not be recorded.
/// Convertit un événement du domaine en événement d'audit, le cas échéant.
pub fn convert_event(&self, event: &DomainEvent) -> Option<AuditEvent> {
    // Same match logic as record_event(), but returns Option<AuditEvent>
    // instead of persisting directly
}
```

Refactor `record_event()` to call `convert_event()` internally.

- [ ] **Step 4: Add audit-cleanup supervisor task**

Add after the audit-listener task:

```rust
// Audit cleanup -- periodic purge of old events
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
                    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
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

- [ ] **Step 5: Add required imports to main.rs**

```rust
use syswall_domain::ports::EventBus;
```

- [ ] **Step 6: Verify full workspace compiles**

```bash
cd /home/seb/Dev/SysWall && cargo check
```

---

### Task 9: Full Test Suite

**Files:**
- Existing test modules in modified files

- [ ] **Step 1: Run all unit tests**

```bash
cd /home/seb/Dev/SysWall && cargo test -- audit
```

- [ ] **Step 2: Run all workspace tests**

```bash
cd /home/seb/Dev/SysWall && cargo test
```

- [ ] **Step 3: Verify no warnings**

```bash
cd /home/seb/Dev/SysWall && cargo clippy --all-targets
```

---

## Summary

| Task | Description | Files Modified |
|---|---|---|
| 1 | Proto schema: 3 RPCs + 7 messages | `proto/syswall.proto` |
| 2 | Domain: AuditStats entity + trait extensions | `entities/audit.rs`, `ports/repositories.rs` |
| 3 | Fake repo: implement new trait methods | `fakes/fake_audit_repository.rs` |
| 4 | SQLite repo: filtering, batch, delete, stats | `persistence/audit_repository.rs` |
| 5 | AuditService: buffer, export, stats, coverage | `services/audit_service.rs` |
| 6 | gRPC converters: audit converters | `grpc/converters.rs` |
| 7 | gRPC control: 3 new RPCs | `grpc/control_service.rs` |
| 8 | Daemon: listener + cleanup tasks + wiring | `main.rs` |
| 9 | Tests: full suite | All test modules |

**Total: 9 tasks, 37 steps**
