# SysWall -- Firewall Engine Design Spec

**Date:** 2026-03-26
**Scope:** Sub-project 2 -- Firewall Engine (real nftables adapter, conntrack monitor, process resolver)
**Status:** Draft
**Depends on:** Sub-project 1 (Foundations) -- complete

---

## 1. Overview

Sub-project 1 delivered the full architecture scaffold with fake adapters (`FakeFirewallEngine`, `FakeConnectionMonitor`, `FakeProcessResolver`). This spec covers replacing all three fakes with real Linux system adapters, integrating them into the daemon, and adding a system whitelist for safe defaults.

After this sub-project, the daemon will:
- Apply and remove nftables rules in a dedicated `syswall` table
- Monitor real network connections via conntrack
- Resolve process information from /proc
- Process the full pipeline: conntrack event -> process resolution -> policy evaluation -> firewall verdict
- Bootstrap with safe system whitelist rules (DNS, DHCP, loopback, NTP)

This is sub-project 2 of 6:
1. Foundations (complete)
2. **Firewall Engine** (this spec)
3. Connection monitoring (UI integration, real-time stream to gRPC)
4. Auto-learning mode (decision prompts, debounce, rule generation)
5. Premium UI (dashboard, views, design system)
6. Audit & journal (persistence, search, export)

## 2. Architecture Context

### 2.1 Ports Already Defined

The domain ports from sub-project 1 are stable and unchanged. The three traits we implement:

```rust
// crates/domain/src/ports/system.rs

#[async_trait]
pub trait FirewallEngine: Send + Sync {
    async fn apply_rule(&self, rule: &Rule) -> Result<(), DomainError>;
    async fn remove_rule(&self, rule_id: &RuleId) -> Result<(), DomainError>;
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

### 2.2 Crate Placement

All three adapters live in `syswall-infra`:

```
crates/infra/src/
├── nftables/
│   ├── mod.rs              # re-exports
│   ├── adapter.rs          # NftablesFirewallAdapter
│   ├── command.rs          # NftCommand builder (typed nft invocation)
│   ├── parser.rs           # parse nft JSON output
│   └── types.rs            # NftRule, NftChain, NftTable (internal types)
├── conntrack/
│   ├── mod.rs              # re-exports
│   ├── adapter.rs          # ConntrackMonitorAdapter
│   ├── parser.rs           # parse conntrack events
│   └── types.rs            # ConntrackEvent, ConntrackEntry (internal types)
├── process/
│   ├── mod.rs              # re-exports
│   ├── resolver.rs         # ProcfsProcessResolver
│   ├── proc_parser.rs      # /proc file parsing
│   └── cache.rs            # LRU cache with TTL
├── persistence/            # (unchanged from sub-project 1)
├── event_bus/              # (unchanged from sub-project 1)
└── lib.rs
```

### 2.3 Dependency Additions

New dependencies for `syswall-infra`:

| Crate | Version | Purpose |
|---|---|---|
| `lru` | 0.12 | LRU cache for process resolver |
| `nix` | 0.29 | POSIX APIs: readlink for /proc/pid/exe, uid resolution |

No external nftables or conntrack library. Both tools are invoked via `std::process::Command` with JSON output parsing. This avoids binding to unstable C libraries and keeps the attack surface minimal.

**ADR: Why Command-based instead of library bindings?**
- `nftables` has no stable Rust binding. The `nft` CLI with `-j` (JSON) output is the officially supported machine-readable interface.
- `conntrack` similarly has no maintained Rust crate. The `conntrack` CLI in event mode (`-E`) with `-o xml` or parsed text is the most reliable interface.
- Using `std::process::Command` with typed argument builders (never `sh -c`) provides: injection safety, easy mocking in tests, no unsafe FFI, no ABI compatibility concerns.
- Trade-off: subprocess overhead (~2ms per nft call). Acceptable because rule changes are infrequent (user-initiated), and conntrack event streaming is a long-lived process.

---

## 3. NftablesFirewallAdapter

### 3.1 Responsibilities

Implements `FirewallEngine`. Manages a dedicated `syswall` nftables table with `input`, `output`, and `forward` chains. Translates domain `Rule` entities into nft commands.

### 3.2 Table and Chain Layout

On first `sync_all_rules()` or `apply_rule()`, the adapter ensures the table and chains exist:

```
table inet syswall {
    chain input {
        type filter hook input priority 0; policy accept;
        # SysWall rules inserted here
    }
    chain output {
        type filter hook output priority 0; policy accept;
    }
    chain forward {
        type filter hook forward priority 0; policy accept;
    }
}
```

**Policy is `accept` at the chain level.** SysWall rules explicitly allow or block individual connections. Default deny is NOT enforced at the nftables level -- it is handled by the PolicyEngine in the domain layer. This prevents the daemon from locking the machine out if it crashes: nftables chains with `accept` policy pass everything through when no SysWall rules are loaded.

**ADR: Why `inet` family?**
The `inet` family handles both IPv4 and IPv6 in a single table, reducing duplication. All domain types already support both address families.

### 3.3 Rule Translation

Domain `Rule` to nft rule mapping:

| Domain Field | nft Expression |
|---|---|
| `criteria.protocol = Tcp` | `meta l4proto tcp` |
| `criteria.protocol = Udp` | `meta l4proto udp` |
| `criteria.protocol = Icmp` | `meta l4proto icmp` |
| `criteria.remote_ip = Exact(ip)` | `ip daddr <ip>` (outbound) or `ip saddr <ip>` (inbound) |
| `criteria.remote_ip = Cidr{net, prefix}` | `ip daddr <net>/<prefix>` |
| `criteria.remote_port = Exact(port)` | `tcp dport <port>` or `udp dport <port>` |
| `criteria.remote_port = Range{start, end}` | `tcp dport <start>-<end>` |
| `criteria.local_port = Exact(port)` | `tcp sport <port>` |
| `criteria.direction = Inbound` | Rule placed in `input` chain |
| `criteria.direction = Outbound` | Rule placed in `output` chain |
| `criteria.direction = None` | Rules placed in both `input` and `output` chains |
| `criteria.user = Some(name)` | `meta skuid <uid>` (resolved to numeric UID at translation time) |
| `effect = Allow` | `accept` |
| `effect = Block` | `drop` |
| `effect = Observe` | `log prefix "syswall-observe: " accept` |
| `effect = Ask` | No nft rule generated (handled by PolicyEngine/LearningService in userspace) |

**Rules with `effect = Ask` are NOT translated to nftables.** The Ask effect triggers the auto-learning flow in userspace. Only Allow, Block, and Observe produce nft rules.

**Application matching (`criteria.application`) is NOT enforced in nftables.** nftables has no concept of application/process matching. Application-based filtering is enforced by the PolicyEngine in userspace. nftables rules only enforce network-level criteria (IP, port, protocol, direction, user). This is a fundamental architectural constraint: nftables operates at kernel level where process context is limited (only socket UID is available via `meta skuid`).

### 3.4 Rule Handle Tracking

Each nft rule gets a unique handle assigned by the kernel. The adapter maintains an in-memory mapping:

```rust
/// Maps domain RuleId to nftables rule handle(s).
/// A single domain Rule may produce multiple nft rules (e.g., both input and output chains).
struct HandleMap {
    handles: HashMap<RuleId, Vec<NftRuleHandle>>,
}

struct NftRuleHandle {
    chain: String,      // "input", "output", or "forward"
    handle: u64,        // nft rule handle number
}
```

This mapping is rebuilt on `sync_all_rules()` by listing current rules with `nft -j list table inet syswall` and correlating them with domain rules via a comment embedded in each nft rule:

```
nft add rule inet syswall output ... comment "syswall:<rule-uuid>"
```

The comment acts as a stable identifier for correlating nft rules with domain rules during sync.

### 3.5 NftCommand Builder

All nft invocations go through a typed builder that prevents injection:

```rust
/// Typed nft command builder. Never concatenates strings into shell commands.
pub struct NftCommand {
    args: Vec<String>,
    timeout: Duration,
    max_output_bytes: usize,
}

impl NftCommand {
    pub fn new() -> Self {
        Self {
            args: vec![],
            timeout: Duration::from_secs(5),
            max_output_bytes: 1_048_576, // 1 MB
        }
    }

    /// List the syswall table in JSON format.
    pub fn list_table(table: &str) -> Self {
        Self::new().arg("-j").arg("list").arg("table").arg("inet").arg(table)
    }

    /// Add a rule to a chain.
    pub fn add_rule(table: &str, chain: &str) -> Self {
        Self::new().arg("add").arg("rule").arg("inet").arg(table).arg(chain)
    }

    /// Delete a rule by handle.
    pub fn delete_rule(table: &str, chain: &str, handle: u64) -> Self {
        Self::new()
            .arg("delete").arg("rule").arg("inet")
            .arg(table).arg(chain)
            .arg("handle").arg(handle.to_string())
    }

    /// Create the syswall table if it does not exist.
    pub fn create_table(table: &str) -> Self {
        Self::new().arg("add").arg("table").arg("inet").arg(table)
    }

    /// Create a chain with the given hook and priority.
    pub fn create_chain(table: &str, chain: &str, hook: &str, priority: i32) -> Self {
        // Uses nft add chain syntax with type filter hook
        Self::new()
            .arg("add").arg("chain").arg("inet").arg(table).arg(chain)
            .arg(format!("{{ type filter hook {} priority {}; policy accept; }}", hook, priority))
    }

    /// Save the full ruleset for rollback.
    pub fn list_ruleset_json() -> Self {
        Self::new().arg("-j").arg("list").arg("ruleset")
    }

    fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Execute the command. Returns stdout on success.
    pub async fn execute(&self) -> Result<String, DomainError> {
        // Uses tokio::process::Command with:
        // - No shell (direct exec of /usr/sbin/nft)
        // - Timeout via tokio::time::timeout
        // - Bounded stdout/stderr capture
        // - Exit code validation
        todo!()
    }
}
```

**Security invariants:**
- `nft` is invoked directly via `Command::new("/usr/sbin/nft")`, never through a shell.
- All arguments are passed as separate `arg()` calls, never interpolated into a single string.
- IP addresses, ports, and protocol names are validated by domain value objects before reaching the adapter.
- User names are resolved to numeric UIDs via `/etc/passwd` (or `nix::unistd::User::from_name()`) before being passed to nft.
- The `comment` field containing the rule UUID is validated as a UUID string (alphanumeric + hyphens only).

### 3.6 apply_rule()

```
apply_rule(rule):
    1. If rule.effect == Ask → return Ok(()) (nothing to do in nftables)
    2. Ensure syswall table and chains exist (idempotent)
    3. Build nft expression from rule criteria
    4. Save current syswall table state for rollback
    5. Determine target chain(s) from rule.direction
    6. For each target chain:
       a. Execute: nft add rule inet syswall <chain> <expressions> <verdict> comment "syswall:<rule-id>"
       b. Parse handle from output
       c. Store in HandleMap
    7. On any failure: rollback by removing any rules that were added in this call
    8. Return Ok(())
```

### 3.7 remove_rule()

```
remove_rule(rule_id):
    1. Look up handles in HandleMap
    2. If not found → return Ok(()) (idempotent removal)
    3. For each handle:
       a. Execute: nft delete rule inet syswall <chain> handle <handle>
    4. Remove from HandleMap
    5. Return Ok(())
```

### 3.8 sync_all_rules()

Called at daemon startup and periodically to reconcile nftables state with the database.

```
sync_all_rules(rules):
    1. Save full syswall table state for rollback
    2. List current nft rules in syswall table: nft -j list table inet syswall
    3. Parse all rules, extract "syswall:<uuid>" comments
    4. Build sets:
       - nft_rules: set of RuleId currently in nftables
       - db_rules: set of RuleId from input (enabled, non-expired, non-Ask)
    5. Compute delta:
       - to_add = db_rules - nft_rules
       - to_remove = nft_rules - db_rules
       - to_update = db_rules ∩ nft_rules (where rule content has changed)
    6. Apply removals first (safe direction)
    7. Apply additions
    8. Apply updates (remove old, add new)
    9. Rebuild HandleMap from final state
    10. On any failure during steps 6-8:
        a. Attempt rollback to saved state
        b. If rollback also fails, log critical error and enter degraded mode
    11. Return Ok(())
```

### 3.9 get_status()

```
get_status():
    1. Run: nft -j list table inet syswall
    2. If table exists and has chains:
       - enabled = true
       - active_rules_count = number of rules with syswall: comments
       - nftables_synced = true (will be set to false if last sync failed)
    3. If table does not exist:
       - enabled = false
       - nftables_synced = false
    4. uptime_secs = daemon uptime (tracked via Instant::now() at construction)
    5. version = env!("CARGO_PKG_VERSION")
```

### 3.10 Rollback Safety

Before any modification, the adapter saves state:

```rust
struct RollbackState {
    /// JSON output of `nft -j list table inet syswall` before the operation.
    table_state: String,
    /// Timestamp of the snapshot.
    saved_at: Instant,
}
```

On failure:
1. Delete the syswall table entirely: `nft delete table inet syswall`
2. Restore from saved state: `nft -j -f -` (pipe the saved JSON ruleset back)
3. If restore fails, log a `Severity::Critical` system error and set `nftables_synced = false`

**Safety timer:** The adapter tracks the last successful sync time. If `sync_all_rules()` has not completed successfully for more than `rollback_timeout_secs` (default: 30s from config), the Supervisor should be notified to trigger a forced rollback. This prevents a broken ruleset from persisting.

### 3.11 Configuration

Uses existing `FirewallConfig` from daemon config:

```rust
pub struct FirewallConfig {
    pub default_policy: DefaultPolicyConfig,
    pub rollback_timeout_secs: u64,
    pub nftables_table_name: String,  // default: "syswall"
}
```

New config fields to add:

```rust
pub nft_binary_path: PathBuf,         // default: "/usr/sbin/nft"
pub nft_command_timeout_secs: u64,    // default: 5
pub nft_max_output_bytes: usize,      // default: 1_048_576 (1 MB)
```

### 3.12 Error Handling

| Situation | Behavior |
|---|---|
| `nft` binary not found | `DomainError::Infrastructure` at startup, daemon fails fast |
| `nft` command timeout (>5s) | Kill process, return `DomainError::Infrastructure` |
| `nft` returns non-zero exit | Parse stderr, return `DomainError::Infrastructure` with details |
| Permission denied | `DomainError::Infrastructure("CAP_NET_ADMIN required")` |
| Table does not exist on `apply_rule()` | Create it (idempotent setup) |
| Handle not found on `remove_rule()` | Return `Ok(())` (idempotent) |
| JSON parse failure | `DomainError::Infrastructure` with raw output snippet (bounded) |
| Rollback failure | `Severity::Critical` system error, set degraded mode flag |

---

## 4. ConntrackMonitorAdapter

### 4.1 Responsibilities

Implements `ConnectionMonitor`. Monitors real-time network connection events using the `conntrack` command-line tool in event mode.

### 4.2 stream_events()

Spawns `conntrack -E -o timestamp` as a long-lived child process and parses its stdout line by line into domain `Connection` entities.

```
stream_events():
    1. Spawn: conntrack -E -o timestamp -p tcp
       AND: conntrack -E -o timestamp -p udp
       (two separate processes, merged into one stream)
    2. Wrap stdout readers in async BufReader
    3. Parse each line into a ConntrackEvent
    4. Transform ConntrackEvent into domain Connection
    5. Return as ConnectionEventStream (Pin<Box<dyn Stream<...>>>)
```

**Why two processes?** The `-p` filter is applied at the kernel level, reducing noise. TCP and UDP are the primary protocols SysWall handles. ICMP can be added as a third stream if needed.

### 4.3 Conntrack Event Parsing

Raw conntrack output format (with `-o timestamp`):

```
[1711468800.123456]      [NEW] tcp      6 120 SYN_SENT src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 [UNREPLIED] src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000
[1711468800.234567]   [UPDATE] tcp      6 60  SYN_RECV src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000
[1711468800.345678]  [DESTROY] tcp      6 src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000
```

The parser extracts:

```rust
/// Raw conntrack event before domain transformation.
pub struct ConntrackEvent {
    pub timestamp: f64,
    pub event_type: ConntrackEventType,
    pub protocol: Protocol,
    pub proto_number: u8,
    pub state: Option<String>,        // SYN_SENT, ESTABLISHED, etc.
    pub src: IpAddr,
    pub dst: IpAddr,
    pub sport: u16,
    pub dport: u16,
    pub reply_src: Option<IpAddr>,
    pub reply_dst: Option<IpAddr>,
    pub reply_sport: Option<u16>,
    pub reply_dport: Option<u16>,
}

pub enum ConntrackEventType {
    New,
    Update,
    Destroy,
}
```

### 4.4 Domain Transformation

ConntrackEvent to domain Connection:

| ConntrackEvent field | Connection field |
|---|---|
| `event_type = New` | `state = ConnectionState::New` |
| `event_type = Update` + SYN_RECV | `state = ConnectionState::New` |
| `event_type = Update` + ESTABLISHED | `state = ConnectionState::Established` |
| `event_type = Update` + TIME_WAIT/CLOSE_WAIT | `state = ConnectionState::Closing` |
| `event_type = Destroy` | `state = ConnectionState::Closed` |
| `src` | `source.ip` |
| `sport` | `source.port` |
| `dst` | `destination.ip` |
| `dport` | `destination.port` |
| `protocol` | `protocol` |

**Direction detection:** Compare source IP against local interfaces. If `src` is a local IP, direction is `Outbound`; if `dst` is a local IP, direction is `Inbound`. Local IPs are detected once at adapter startup by reading network interfaces (via `nix::ifaddrs::getifaddrs()` or parsing `/proc/net/if_inet6` and `/proc/net/fib_trie`).

**Process info is NOT resolved here.** The connection is emitted with `process: None`. The daemon's connection processing pipeline (ConnectionService) will resolve process info via the ProcessResolver separately.

### 4.5 get_active_connections()

Dumps the current connection table:

```
get_active_connections():
    1. Run: conntrack -L -o extended
    2. Parse each line (same parser, different format)
    3. Transform into domain Connections
    4. Return Vec<Connection>
```

### 4.6 Backpressure Handling

The conntrack event stream can produce thousands of events per second under high network load. Backpressure strategy:

1. **Internal buffer:** The adapter maintains a bounded channel (`tokio::sync::mpsc` with capacity from `config.monitoring.conntrack_buffer_size`, default 4096) between the parser task and the stream consumer.
2. **Drop policy:** When the channel is full, new events are dropped with a `tracing::warn!` log. A counter tracks dropped events and is reported periodically (every 10 seconds).
3. **Filtering at source:** Only NEW events trigger the full connection processing pipeline (policy evaluation + process resolution). UPDATE and DESTROY events are used to update existing connection state (cheaper operation).
4. **Deduplication window:** Rapid duplicate events for the same 5-tuple within 100ms are collapsed.

### 4.7 Process Lifecycle

The `conntrack` child process requires careful lifecycle management:

- **Startup:** Spawned by `stream_events()`. If the command fails to start (binary not found, permission denied), return `DomainError::Infrastructure` immediately.
- **Monitoring:** A background task reads stderr. If the process exits unexpectedly, the stream yields a `DomainError` and terminates. The Supervisor detects the terminated stream and restarts it with exponential backoff.
- **Shutdown:** On cancellation (daemon shutdown), the child process is killed via `child.kill()`. The stream terminates cleanly.
- **Multiple calls to stream_events():** Each call spawns new processes. Previous streams remain active until dropped. The Supervisor should call this only once and manage the resulting stream.

### 4.8 Configuration

Uses existing `MonitoringConfig`:

```rust
pub struct MonitoringConfig {
    pub conntrack_buffer_size: usize,  // bounded channel capacity (default: 4096)
    pub process_cache_ttl_secs: u64,   // used by ProcessResolver, not here
    pub event_bus_capacity: usize,
}
```

New config fields to add:

```rust
pub conntrack_binary_path: PathBuf,       // default: "/usr/sbin/conntrack"
pub conntrack_protocols: Vec<String>,     // default: ["tcp", "udp"]
```

### 4.9 Error Handling

| Situation | Behavior |
|---|---|
| `conntrack` binary not found | `DomainError::Infrastructure` on `stream_events()` |
| Permission denied (no CAP_NET_ADMIN) | `DomainError::Infrastructure` with clear message |
| Process exits unexpectedly | Stream yields error, Supervisor restarts |
| Malformed output line | Skip line, log warning, continue |
| Buffer full (backpressure) | Drop event, increment counter, log periodically |
| `-L` command fails | Return `DomainError::Infrastructure` from `get_active_connections()` |

---

## 5. ProcfsProcessResolver

### 5.1 Responsibilities

Implements `ProcessResolver`. Reads `/proc` to resolve process information from PIDs and socket inodes. All operations are best-effort and return `Ok(None)` on failure.

### 5.2 resolve(pid)

```
resolve(pid):
    1. Check LRU cache by pid → return cached if fresh (within TTL)
    2. Read /proc/<pid>/exe → readlink to get executable path
    3. Read /proc/<pid>/cmdline → split on \0 to get command-line arguments
    4. Read /proc/<pid>/status → parse Name:, Uid: fields
    5. Construct ProcessInfo { pid, name, path, cmdline }
    6. Insert into LRU cache with current timestamp
    7. Return Ok(Some(info))
    8. On any error (process gone, permission denied): return Ok(None)
```

### 5.3 resolve_by_socket(inode)

Maps a socket inode to a PID, then resolves the PID:

```
resolve_by_socket(inode):
    1. Check LRU cache by inode → return cached if fresh
    2. Scan /proc/net/tcp and /proc/net/udp to find the row matching the inode
       → This gives us src:port dst:port but NOT the PID
    3. To find the PID: iterate /proc/[0-9]*/fd/*
       → For each fd, readlink → if target is "socket:[<inode>]", we found the PID
    4. Call resolve(pid) with the found PID
    5. Cache the inode → ProcessInfo mapping
    6. Return result
```

**Performance consideration:** Scanning `/proc/*/fd/*` is O(processes * fds), which can be expensive. Mitigations:
- The LRU cache with TTL prevents repeated scans for the same inode.
- The scan is done in `spawn_blocking` to avoid blocking the async runtime.
- A faster path: read `/proc/net/tcp` to get the UID for the socket, then narrow the `/proc/*/fd` scan to processes owned by that UID.

### 5.4 /proc Parsing

#### /proc/\<pid\>/exe
- `std::fs::read_link("/proc/<pid>/exe")` returns the executable path.
- May return "(deleted)" suffix if the binary was replaced -- strip it.
- Wrap in `ExecutablePath::new()` which validates it is absolute.

#### /proc/\<pid\>/cmdline
- Read as bytes, split on `\0`.
- First element is the command, rest are arguments.
- Join with spaces for the `cmdline` string field.
- Empty file means kernel thread or zombie -- return `None` for cmdline.

#### /proc/\<pid\>/status
- Line-oriented key-value format.
- Parse `Name:\t<name>` for the process name.
- Parse `Uid:\t<real> <effective> <saved> <fs>` for the user ID.
- Use `nix::unistd::User::from_uid()` to resolve UID to username for `SystemUser`.

#### /proc/net/tcp and /proc/net/udp
- Tabular format with hex-encoded addresses and ports.
- Column 10 (0-indexed) contains the inode number.
- Parse local_address (col 1), remote_address (col 2), state (col 3), inode (col 9).

### 5.5 LRU Cache

```rust
pub struct ProcessCache {
    /// PID -> (ProcessInfo, Instant)
    pid_cache: Mutex<LruCache<u32, CacheEntry>>,
    /// Socket inode -> (ProcessInfo, Instant)
    inode_cache: Mutex<LruCache<u64, CacheEntry>>,
    ttl: Duration,
}

struct CacheEntry {
    info: ProcessInfo,
    user: Option<SystemUser>,
    inserted_at: Instant,
}

impl ProcessCache {
    pub fn new(capacity: usize, ttl: Duration) -> Self;
    pub fn get_by_pid(&self, pid: u32) -> Option<(ProcessInfo, Option<SystemUser>)>;
    pub fn get_by_inode(&self, inode: u64) -> Option<(ProcessInfo, Option<SystemUser>)>;
    pub fn insert_pid(&self, pid: u32, info: ProcessInfo, user: Option<SystemUser>);
    pub fn insert_inode(&self, inode: u64, info: ProcessInfo, user: Option<SystemUser>);
}
```

- **Capacity:** 1024 entries per cache (configurable).
- **TTL:** From `config.monitoring.process_cache_ttl_secs` (default: 5 seconds).
- **Eviction:** LRU eviction when capacity is reached. Stale entries (past TTL) are evicted on access.
- **Thread safety:** `std::sync::Mutex` wrapping the LRU map. Lock contention is minimal because cache operations are fast (microseconds) and called from `spawn_blocking`.

### 5.6 Configuration

Uses existing `MonitoringConfig`:

```rust
pub struct MonitoringConfig {
    pub conntrack_buffer_size: usize,
    pub process_cache_ttl_secs: u64,   // LRU cache TTL (default: 5)
    pub event_bus_capacity: usize,
}
```

New config fields to add:

```rust
pub process_cache_capacity: usize,     // default: 1024
```

### 5.7 Error Handling

The process resolver NEVER returns errors to the caller for expected failures. It returns `Ok(None)`:

| Situation | Behavior |
|---|---|
| PID no longer exists | `Ok(None)` -- process terminated between event and resolution |
| Permission denied reading /proc | `Ok(None)` -- containerized or restricted process |
| /proc/\<pid\>/exe is "(deleted)" | Return info with path stripped of " (deleted)" |
| /proc/net/tcp parse failure | `Ok(None)` for that lookup, log at debug level |
| Socket inode not found in /proc | `Ok(None)` -- connection may have closed |
| /proc not mounted or inaccessible | `DomainError::Infrastructure` (fatal, startup check) |

---

## 6. Daemon Integration

### 6.1 Bootstrap Changes

The `bootstrap.rs` function currently wires fakes. It must be updated to conditionally use real adapters:

```rust
// Current (sub-project 1):
let firewall = Arc::new(FakeFirewallEngine::new());
let process_resolver = Arc::new(FakeProcessResolver::new());

// New (sub-project 2):
let firewall: Arc<dyn FirewallEngine> = if config.firewall.use_fake {
    Arc::new(FakeFirewallEngine::new())
} else {
    Arc::new(NftablesFirewallAdapter::new(&config.firewall)?)
};

let process_resolver: Arc<dyn ProcessResolver> = if config.monitoring.use_fake {
    Arc::new(FakeProcessResolver::new())
} else {
    Arc::new(ProcfsProcessResolver::new(&config.monitoring)?)
};

let connection_monitor: Arc<dyn ConnectionMonitor> = if config.monitoring.use_fake {
    Arc::new(FakeConnectionMonitor::new())
} else {
    Arc::new(ConntrackMonitorAdapter::new(&config.monitoring)?)
};
```

The `use_fake` flags default to `false` in production and `true` in test configurations. This allows tests to continue using fakes while the daemon uses real adapters.

**Alternative approach (feature flag):** Use `#[cfg(feature = "fake-adapters")]` compile-time selection. However, runtime config is preferred because:
- Integration tests need real adapters but may run in CI without capabilities.
- Developers can test the daemon locally without root by setting `use_fake = true`.
- No need to maintain multiple binary builds.

### 6.2 AppContext Changes

Add `ConnectionMonitor` to `AppContext` so the Supervisor can access it:

```rust
pub struct AppContext {
    pub rule_service: Arc<RuleService>,
    pub connection_service: Arc<ConnectionService>,
    pub learning_service: Arc<LearningService>,
    pub audit_service: Arc<AuditService>,
    pub event_bus: Arc<TokioBroadcastEventBus>,
    pub connection_monitor: Arc<dyn ConnectionMonitor>,  // NEW
    pub firewall: Arc<dyn FirewallEngine>,                // NEW (for sync_all_rules at startup)
}
```

### 6.3 Startup Sequence Updates

Insert steps between existing steps 5 and 6:

```
Existing:
  5. Instantiate services
  6. Resume pending decisions

New:
  5. Instantiate services
  5a. Verify system prerequisites (nft binary, conntrack binary, /proc accessible)
  5b. Create system whitelist rules if first start (see Section 7)
  6. Resume pending decisions
  7. Sync nftables (existing step, now uses real adapter)
  8. Start Supervisor with monitoring stream task (see 6.4)
```

### 6.4 Supervisor: Monitoring Stream Task

The Supervisor spawns a new task that runs the full connection processing pipeline:

```rust
supervisor.spawn("connection-monitor", {
    let monitor = ctx.connection_monitor.clone();
    let connection_service = ctx.connection_service.clone();
    let learning_service = ctx.learning_service.clone();
    let audit_service = ctx.audit_service.clone();
    let cancel = cancel.clone();

    async move {
        let stream = monitor.stream_events().await
            .map_err(|e| format!("Failed to start connection monitor: {}", e))?;

        tokio::pin!(stream);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                event = stream.next() => {
                    match event {
                        Some(Ok(connection)) => {
                            // Full processing pipeline
                            match connection_service.process_connection(connection).await {
                                Ok(processed) => {
                                    if processed.verdict == ConnectionVerdict::PendingDecision {
                                        let _ = learning_service
                                            .handle_unknown_connection(processed.snapshot())
                                            .await;
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Connection processing error: {}", e);
                                }
                            }
                        }
                        Some(Err(e)) => {
                            tracing::error!("Connection monitor error: {}", e);
                            // Stream error is terminal -- Supervisor will restart
                            return Err(format!("Monitor stream failed: {}", e));
                        }
                        None => {
                            // Stream ended
                            tracing::warn!("Connection monitor stream ended");
                            return Err("Monitor stream ended unexpectedly".to_string());
                        }
                    }
                }
            }
        }

        Ok(())
    }
});
```

### 6.5 ConnectionService Enhancement

The `process_connection()` method in `ConnectionService` currently has a placeholder for process resolution. Update it:

```rust
pub async fn process_connection(
    &self,
    mut connection: Connection,
) -> Result<Connection, DomainError> {
    // Best-effort process enrichment via socket inode or PID
    if connection.process.is_none() {
        // Try to resolve by examining /proc/net/* to find the socket owner
        // The conntrack event gives us src:sport dst:dport
        // We need to find the socket inode, then the owning PID
        if let Ok(Some(info)) = self.process_resolver
            .resolve_by_socket(/* inode from socket lookup */)
            .await
        {
            connection.process = Some(info);
        }
    }

    // Load rules and evaluate
    let rules = self.rule_repo.list_enabled_ordered().await?;
    let evaluation = PolicyEngine::evaluate(&connection, &rules, self.default_policy);

    connection.verdict = evaluation.verdict;
    connection.matched_rule = evaluation.matched_rule_id;

    // Publish events...
    Ok(connection)
}
```

**Socket inode resolution challenge:** Conntrack events provide the 5-tuple (src IP, src port, dst IP, dst port, protocol) but NOT the socket inode. To find the inode:
1. Parse `/proc/net/tcp` (or `/proc/net/udp`) to find the row matching the 5-tuple.
2. Extract the inode from that row.
3. Pass to `resolve_by_socket(inode)`.

This multi-step lookup will be encapsulated in a new method on `ProcfsProcessResolver`:

```rust
pub async fn resolve_by_connection(
    &self,
    protocol: Protocol,
    local_ip: IpAddr,
    local_port: u16,
    remote_ip: IpAddr,
    remote_port: u16,
) -> Result<Option<(ProcessInfo, Option<SystemUser>)>, DomainError>
```

This combines the `/proc/net/*` lookup and the `/proc/*/fd/*` scan into a single operation, allowing the cache to work efficiently at the 5-tuple level.

To expose this to the domain layer cleanly, add an optional method to the `ProcessResolver` trait with a default implementation:

```rust
#[async_trait]
pub trait ProcessResolver: Send + Sync {
    async fn resolve(&self, pid: u32) -> Result<Option<ProcessInfo>, DomainError>;
    async fn resolve_by_socket(&self, inode: u64) -> Result<Option<ProcessInfo>, DomainError>;

    /// Resolve by connection 5-tuple. Default returns None (not all resolvers support this).
    async fn resolve_by_connection(
        &self,
        _protocol: Protocol,
        _local_ip: IpAddr,
        _local_port: u16,
        _remote_ip: IpAddr,
        _remote_port: u16,
    ) -> Result<Option<ProcessInfo>, DomainError> {
        Ok(None)
    }
}
```

---

## 7. System Whitelist

### 7.1 Purpose

On first start, create default system rules that guarantee basic network connectivity. Without these, the firewall could block DNS resolution, DHCP, or loopback traffic, effectively breaking the machine.

### 7.2 Default Rules

| Name | Direction | Protocol | Port(s) | Effect | Rationale |
|---|---|---|---|---|---|
| Allow DNS (UDP) | Both | UDP | remote 53 | Allow | DNS resolution is essential |
| Allow DNS (TCP) | Both | TCP | remote 53 | Allow | DNS over TCP for large responses |
| Allow DHCP Client | Outbound | UDP | local 68, remote 67 | Allow | DHCP lease renewal |
| Allow DHCP Server | Inbound | UDP | local 67, remote 68 | Allow | DHCP responses |
| Allow Loopback | Both | Any | Any | Allow | Loopback traffic (127.0.0.0/8, ::1) |
| Allow NTP | Outbound | UDP | remote 123 | Allow | Time synchronization |
| Allow SysWall Daemon | Both | Any | Any | Allow | The daemon's own connections (matched by UID) |

### 7.3 Rule Properties

All system whitelist rules share:
- `source: RuleSource::System`
- `priority: RulePriority::system()` (value 0 -- highest priority)
- `scope: RuleScope::Permanent`
- `enabled: true`

### 7.4 First Start Detection

Check the database for existing rules with `source = System`. If none exist, this is a first start and the whitelist is created:

```rust
pub async fn ensure_system_whitelist(
    rule_service: &RuleService,
    rule_repo: &dyn RuleRepository,
) -> Result<(), DomainError> {
    let existing = rule_repo.find_all(
        &RuleFilters { source: Some(RuleSource::System), ..Default::default() },
        &Pagination { offset: 0, limit: 1 },
    ).await?;

    if !existing.is_empty() {
        tracing::info!("System whitelist already exists ({} rules)", existing.len());
        return Ok(());
    }

    tracing::info!("Creating system whitelist rules...");

    let whitelist = vec![
        create_system_rule("Allow DNS (UDP)", RuleCriteria {
            protocol: Some(Protocol::Udp),
            remote_port: Some(PortMatcher::Exact(Port::new(53).unwrap())),
            ..Default::default()
        }),
        create_system_rule("Allow DNS (TCP)", RuleCriteria {
            protocol: Some(Protocol::Tcp),
            remote_port: Some(PortMatcher::Exact(Port::new(53).unwrap())),
            ..Default::default()
        }),
        create_system_rule("Allow DHCP Client", RuleCriteria {
            protocol: Some(Protocol::Udp),
            local_port: Some(PortMatcher::Exact(Port::new(68).unwrap())),
            remote_port: Some(PortMatcher::Exact(Port::new(67).unwrap())),
            direction: Some(Direction::Outbound),
            ..Default::default()
        }),
        create_system_rule("Allow DHCP Server Response", RuleCriteria {
            protocol: Some(Protocol::Udp),
            local_port: Some(PortMatcher::Exact(Port::new(67).unwrap())),
            remote_port: Some(PortMatcher::Exact(Port::new(68).unwrap())),
            direction: Some(Direction::Inbound),
            ..Default::default()
        }),
        create_system_rule("Allow Loopback", RuleCriteria {
            remote_ip: Some(IpMatcher::Cidr {
                network: "127.0.0.0".parse().unwrap(),
                prefix_len: 8,
            }),
            ..Default::default()
        }),
        create_system_rule("Allow NTP", RuleCriteria {
            protocol: Some(Protocol::Udp),
            remote_port: Some(PortMatcher::Exact(Port::new(123).unwrap())),
            direction: Some(Direction::Outbound),
            ..Default::default()
        }),
    ];

    for cmd in whitelist {
        rule_service.create_rule(cmd).await?;
    }

    tracing::info!("System whitelist created successfully");
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
```

### 7.5 Loopback Handling

The loopback rule needs special treatment because loopback connections are internal (src and dst are both 127.x.x.x). In nftables, this maps to:

```
nft add rule inet syswall input ip saddr 127.0.0.0/8 accept comment "syswall:<uuid>"
nft add rule inet syswall output ip daddr 127.0.0.0/8 accept comment "syswall:<uuid>"
```

The IPv6 loopback (::1) should also be covered:

```
nft add rule inet syswall input ip6 saddr ::1 accept comment "syswall:<uuid>"
nft add rule inet syswall output ip6 daddr ::1 accept comment "syswall:<uuid>"
```

The adapter should detect loopback rules (by IP range 127.0.0.0/8 or ::1) and generate the correct nft syntax.

---

## 8. Configuration Updates

### 8.1 New Config Fields

Add to `config/default.toml`:

```toml
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
```

### 8.2 Config Struct Updates

Update `FirewallConfig` and `MonitoringConfig` in `crates/daemon/src/config.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct FirewallConfig {
    pub default_policy: DefaultPolicyConfig,
    pub rollback_timeout_secs: u64,
    pub nftables_table_name: String,
    #[serde(default = "default_nft_path")]
    pub nft_binary_path: PathBuf,
    #[serde(default = "default_nft_timeout")]
    pub nft_command_timeout_secs: u64,
    #[serde(default = "default_nft_max_output")]
    pub nft_max_output_bytes: usize,
    #[serde(default)]
    pub use_fake: bool,
}

#[derive(Debug, Deserialize)]
pub struct MonitoringConfig {
    pub conntrack_buffer_size: usize,
    pub process_cache_ttl_secs: u64,
    #[serde(default = "default_cache_capacity")]
    pub process_cache_capacity: usize,
    pub event_bus_capacity: usize,
    #[serde(default = "default_conntrack_path")]
    pub conntrack_binary_path: PathBuf,
    #[serde(default = "default_conntrack_protocols")]
    pub conntrack_protocols: Vec<String>,
    #[serde(default)]
    pub use_fake: bool,
}
```

All new fields have `#[serde(default)]` so existing config files continue to work without modification.

---

## 9. Testing Strategy

### 9.1 Unit Tests: NftCommand Builder

Test the command building logic without executing any nft commands:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_table_produces_correct_args() {
        let cmd = NftCommand::list_table("syswall");
        assert_eq!(cmd.args, vec!["-j", "list", "table", "inet", "syswall"]);
    }

    #[test]
    fn add_rule_produces_correct_args() {
        let cmd = NftCommand::add_rule("syswall", "output")
            .arg("tcp").arg("dport").arg("443")
            .arg("accept")
            .arg("comment").arg("\"syswall:abc-123\"");
        assert!(cmd.args.contains(&"output".to_string()));
        assert!(cmd.args.contains(&"443".to_string()));
    }

    #[test]
    fn delete_rule_includes_handle() {
        let cmd = NftCommand::delete_rule("syswall", "input", 42);
        assert!(cmd.args.contains(&"handle".to_string()));
        assert!(cmd.args.contains(&"42".to_string()));
    }
}
```

### 9.2 Unit Tests: Rule Translation

Test the mapping from domain `Rule` to nft expressions:

```rust
#[test]
fn tcp_outbound_port_443_translates_correctly() {
    let rule = Rule {
        criteria: RuleCriteria {
            protocol: Some(Protocol::Tcp),
            remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
            direction: Some(Direction::Outbound),
            ..Default::default()
        },
        effect: RuleEffect::Allow,
        ..test_rule()
    };
    let exprs = translate_rule(&rule);
    assert_eq!(exprs.chain, "output");
    assert!(exprs.args.contains(&"tcp".to_string()));
    assert!(exprs.args.contains(&"dport".to_string()));
    assert!(exprs.args.contains(&"443".to_string()));
    assert!(exprs.args.contains(&"accept".to_string()));
}

#[test]
fn ask_effect_produces_no_nft_rule() {
    let rule = Rule {
        effect: RuleEffect::Ask,
        ..test_rule()
    };
    let result = translate_rule(&rule);
    assert!(result.is_none());
}

#[test]
fn cidr_remote_ip_translates_correctly() {
    let rule = Rule {
        criteria: RuleCriteria {
            remote_ip: Some(IpMatcher::Cidr {
                network: "10.0.0.0".parse().unwrap(),
                prefix_len: 8,
            }),
            direction: Some(Direction::Outbound),
            ..Default::default()
        },
        effect: RuleEffect::Block,
        ..test_rule()
    };
    let exprs = translate_rule(&rule);
    assert!(exprs.args.contains(&"10.0.0.0/8".to_string()));
    assert!(exprs.args.contains(&"drop".to_string()));
}

#[test]
fn port_range_translates_correctly() {
    let rule = Rule {
        criteria: RuleCriteria {
            remote_port: Some(PortMatcher::Range {
                start: Port::new(8000).unwrap(),
                end: Port::new(9000).unwrap(),
            }),
            protocol: Some(Protocol::Tcp),
            ..Default::default()
        },
        effect: RuleEffect::Allow,
        ..test_rule()
    };
    let exprs = translate_rule(&rule);
    assert!(exprs.args.contains(&"8000-9000".to_string()));
}

#[test]
fn no_direction_produces_rules_for_both_chains() {
    let rule = Rule {
        criteria: RuleCriteria {
            direction: None,
            ..Default::default()
        },
        effect: RuleEffect::Allow,
        ..test_rule()
    };
    let chains = get_target_chains(&rule);
    assert_eq!(chains, vec!["input", "output"]);
}

#[test]
fn user_criteria_resolved_to_uid() {
    let rule = Rule {
        criteria: RuleCriteria {
            user: Some("seb".to_string()),
            ..Default::default()
        },
        effect: RuleEffect::Allow,
        ..test_rule()
    };
    // Would need a mock for user resolution in unit tests
    let exprs = translate_rule_with_uid(&rule, 1000);
    assert!(exprs.args.contains(&"skuid".to_string()));
    assert!(exprs.args.contains(&"1000".to_string()));
}
```

### 9.3 Unit Tests: Conntrack Event Parsing

```rust
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
}

#[test]
fn parse_destroy_event() {
    let line = "[1711468800.345678]  [DESTROY] tcp      6 src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000";
    let event = parse_conntrack_line(line).unwrap();
    assert_eq!(event.event_type, ConntrackEventType::Destroy);
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
    assert_eq!(event.dport, 53);
}

#[test]
fn malformed_line_returns_none() {
    let line = "garbage data that is not conntrack output";
    assert!(parse_conntrack_line(line).is_none());
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
}
```

### 9.4 Unit Tests: Procfs Parsing

Use fixture files or test strings:

```rust
#[test]
fn parse_proc_net_tcp() {
    let content = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 0100007F:0050 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 12345 1 0000000000000000 100 0 0 10 0\n";
    let entries = parse_proc_net_tcp(content);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].local_ip, "127.0.0.1".parse::<IpAddr>().unwrap());
    assert_eq!(entries[0].local_port, 80);
    assert_eq!(entries[0].inode, 12345);
}

#[test]
fn parse_proc_status_name() {
    let content = "Name:\tfirefox\nUmask:\t0022\nState:\tS (sleeping)\nTgid:\t1234\nNgid:\t0\nPid:\t1234\nPPid:\t1000\nUid:\t1000\t1000\t1000\t1000\nGid:\t1000\t1000\t1000\t1000\n";
    let info = parse_proc_status(content);
    assert_eq!(info.name, "firefox");
    assert_eq!(info.uid, 1000);
}

#[test]
fn parse_proc_cmdline() {
    let bytes = b"firefox\0--no-remote\0https://example.com\0";
    let cmdline = parse_cmdline(bytes);
    assert_eq!(cmdline, "firefox --no-remote https://example.com");
}

#[test]
fn empty_cmdline_returns_none() {
    let bytes = b"";
    let result = parse_cmdline_opt(bytes);
    assert!(result.is_none());
}

#[test]
fn parse_proc_net_tcp_hex_addresses() {
    // 0100007F = 127.0.0.1 (little-endian)
    let ip = parse_hex_ip("0100007F");
    assert_eq!(ip, "127.0.0.1".parse::<IpAddr>().unwrap());
}

#[test]
fn parse_proc_net_tcp6() {
    let content = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 00000000000000000000000001000000:0050 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 12345 1 0000000000000000 100 0 0 10 0\n";
    let entries = parse_proc_net_tcp6(content);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].local_ip, "::1".parse::<IpAddr>().unwrap());
}
```

### 9.5 Unit Tests: LRU Cache

```rust
#[test]
fn cache_returns_fresh_entry() {
    let cache = ProcessCache::new(10, Duration::from_secs(5));
    let info = ProcessInfo { pid: 1, name: "test".into(), path: None, cmdline: None };
    cache.insert_pid(1, info.clone(), None);
    assert_eq!(cache.get_by_pid(1).unwrap().0.name, "test");
}

#[test]
fn cache_evicts_stale_entry() {
    let cache = ProcessCache::new(10, Duration::from_millis(1));
    let info = ProcessInfo { pid: 1, name: "test".into(), path: None, cmdline: None };
    cache.insert_pid(1, info, None);
    std::thread::sleep(Duration::from_millis(10));
    assert!(cache.get_by_pid(1).is_none());
}

#[test]
fn cache_respects_capacity() {
    let cache = ProcessCache::new(2, Duration::from_secs(60));
    cache.insert_pid(1, make_info(1), None);
    cache.insert_pid(2, make_info(2), None);
    cache.insert_pid(3, make_info(3), None); // evicts PID 1
    assert!(cache.get_by_pid(1).is_none());
    assert!(cache.get_by_pid(2).is_some());
    assert!(cache.get_by_pid(3).is_some());
}

#[test]
fn inode_cache_independent_from_pid_cache() {
    let cache = ProcessCache::new(10, Duration::from_secs(5));
    let info = make_info(1);
    cache.insert_inode(99, info.clone(), None);
    assert!(cache.get_by_pid(1).is_none());
    assert!(cache.get_by_inode(99).is_some());
}
```

### 9.6 Unit Tests: Nft JSON Output Parsing

```rust
#[test]
fn parse_nft_list_table_json() {
    let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}, {"table": {"family": "inet", "name": "syswall", "handle": 1}}, {"chain": {"family": "inet", "table": "syswall", "name": "output", "handle": 2, "type": "filter", "hook": "output", "prio": 0, "policy": "accept"}}, {"rule": {"family": "inet", "table": "syswall", "chain": "output", "handle": 5, "comment": "syswall:550e8400-e29b-41d4-a716-446655440000", "expr": [{"match": {"op": "==", "left": {"meta": {"key": "l4proto"}}, "right": "tcp"}}, {"match": {"op": "==", "left": {"payload": {"protocol": "tcp", "field": "dport"}}, "right": 443}}, {"accept": null}]}}]}"#;
    let rules = parse_nft_table_rules(json).unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].handle, 5);
    assert_eq!(rules[0].chain, "output");
    assert_eq!(rules[0].comment, Some("syswall:550e8400-e29b-41d4-a716-446655440000".to_string()));
}

#[test]
fn parse_empty_table() {
    let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}, {"table": {"family": "inet", "name": "syswall", "handle": 1}}]}"#;
    let rules = parse_nft_table_rules(json).unwrap();
    assert!(rules.is_empty());
}

#[test]
fn parse_table_not_found() {
    let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}]}"#;
    let result = parse_nft_table_rules(json);
    assert!(result.is_err() || result.unwrap().is_empty());
}
```

### 9.7 Unit Tests: Domain Transformation

```rust
#[test]
fn conntrack_new_event_becomes_new_connection() {
    let event = ConntrackEvent {
        event_type: ConntrackEventType::New,
        protocol: Protocol::Tcp,
        src: "192.168.1.100".parse().unwrap(),
        dst: "93.184.216.34".parse().unwrap(),
        sport: 45000,
        dport: 443,
        state: Some("SYN_SENT".into()),
        ..Default::default()
    };
    let local_ips = vec!["192.168.1.100".parse().unwrap()];
    let conn = conntrack_to_connection(event, &local_ips).unwrap();
    assert_eq!(conn.state, ConnectionState::New);
    assert_eq!(conn.direction, Direction::Outbound);
    assert_eq!(conn.verdict, ConnectionVerdict::Unknown);
    assert!(conn.process.is_none());
}

#[test]
fn direction_detected_from_local_ips() {
    let local_ips: Vec<IpAddr> = vec!["192.168.1.100".parse().unwrap()];

    // Outbound: src is local
    let event = make_event("192.168.1.100", "8.8.8.8");
    let conn = conntrack_to_connection(event, &local_ips).unwrap();
    assert_eq!(conn.direction, Direction::Outbound);

    // Inbound: dst is local
    let event = make_event("8.8.8.8", "192.168.1.100");
    let conn = conntrack_to_connection(event, &local_ips).unwrap();
    assert_eq!(conn.direction, Direction::Inbound);
}
```

### 9.8 Integration Tests

Guarded by `#[cfg(feature = "integration")]` -- require real Linux capabilities:

```rust
/// Requires: CAP_NET_ADMIN
#[cfg(feature = "integration")]
mod nftables_integration {
    #[tokio::test]
    async fn create_and_delete_table() {
        let adapter = NftablesFirewallAdapter::new(&test_config()).unwrap();
        // Uses a randomized table name to avoid conflicts
        let status = adapter.get_status().await.unwrap();
        // Cleanup: remove test table
    }

    #[tokio::test]
    async fn apply_and_remove_rule() {
        let adapter = NftablesFirewallAdapter::new(&test_config()).unwrap();
        let rule = make_test_rule(RuleEffect::Block);
        adapter.apply_rule(&rule).await.unwrap();
        // Verify rule exists via nft list
        adapter.remove_rule(&rule.id).await.unwrap();
        // Verify rule is gone
    }

    #[tokio::test]
    async fn sync_adds_missing_rules() {
        let adapter = NftablesFirewallAdapter::new(&test_config()).unwrap();
        let rules = vec![make_test_rule(RuleEffect::Allow)];
        adapter.sync_all_rules(&rules).await.unwrap();
        // Verify rules were added
    }

    #[tokio::test]
    async fn sync_removes_stale_rules() {
        let adapter = NftablesFirewallAdapter::new(&test_config()).unwrap();
        // Add rule manually, then sync with empty list
        adapter.sync_all_rules(&[]).await.unwrap();
        // Verify stale rule was removed
    }
}

/// Requires: CAP_NET_ADMIN
#[cfg(feature = "integration")]
mod conntrack_integration {
    #[tokio::test]
    async fn stream_receives_events() {
        let adapter = ConntrackMonitorAdapter::new(&test_config()).unwrap();
        let stream = adapter.stream_events().await.unwrap();
        // Generate traffic (e.g., DNS query), expect to see event within 5s
        // This is inherently timing-dependent, use timeout
    }

    #[tokio::test]
    async fn get_active_connections_returns_entries() {
        let adapter = ConntrackMonitorAdapter::new(&test_config()).unwrap();
        let connections = adapter.get_active_connections().await.unwrap();
        // On a running system, there should be at least one connection
    }
}

/// No special capabilities needed -- just /proc access
#[cfg(feature = "integration")]
mod procfs_integration {
    #[tokio::test]
    async fn resolve_own_pid() {
        let resolver = ProcfsProcessResolver::new(&test_config()).unwrap();
        let pid = std::process::id();
        let info = resolver.resolve(pid).await.unwrap();
        assert!(info.is_some());
        let info = info.unwrap();
        // The test binary name should contain "syswall" or the test runner name
        assert!(!info.name.is_empty());
    }

    #[tokio::test]
    async fn resolve_nonexistent_pid_returns_none() {
        let resolver = ProcfsProcessResolver::new(&test_config()).unwrap();
        let info = resolver.resolve(u32::MAX).await.unwrap();
        assert!(info.is_none());
    }

    #[tokio::test]
    async fn resolve_by_socket_finds_listening_process() {
        // Start a TCP listener, find its inode in /proc/net/tcp, resolve it
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let resolver = ProcfsProcessResolver::new(&test_config()).unwrap();
        // Find socket inode from /proc/net/tcp
        // Then resolve it
    }
}
```

### 9.9 Test Commands

```bash
# Unit tests only (no capabilities needed)
cargo test

# Include integration tests (requires CAP_NET_ADMIN or root)
cargo test --features integration

# Run a specific adapter's tests
cargo test -p syswall-infra nftables
cargo test -p syswall-infra conntrack
cargo test -p syswall-infra process
```

---

## 10. Security Considerations

### 10.1 Privilege Separation

The daemon runs as root (required for `CAP_NET_ADMIN` and `/proc` access). The UI runs as the regular user. Communication is via gRPC over Unix socket with `SO_PEERCRED` verification (defined in sub-project 1).

### 10.2 Command Injection Prevention

- **nft commands:** Built via typed `NftCommand` builder. All values come from validated domain types (`Port`, `IpAddr`, `Protocol`, `RuleId`). No string interpolation or shell invocation.
- **conntrack commands:** Static argument lists. No user-provided data in command arguments.
- **Rule comments:** Contain only UUID strings (validated format: `[0-9a-f-]{36}`).

### 10.3 Resource Limits

| Resource | Limit | Rationale |
|---|---|---|
| nft command timeout | 5 seconds | Prevent hung nft processes |
| nft stdout capture | 1 MB | Prevent memory exhaustion from malicious/huge output |
| conntrack buffer | 4096 events | Backpressure under high load |
| Process cache | 1024 entries | Bounded memory for LRU cache |
| /proc file reads | 64 KB per file | Prevent reading unreasonably large /proc entries |

### 10.4 Fail-Safe Design

- nftables chain policy is `accept` -- if the daemon crashes, traffic flows normally.
- `sync_all_rules()` removes stale rules before adding new ones (safe direction).
- Rollback on any nft modification failure.
- System whitelist ensures DNS, DHCP, loopback, NTP always work.
- Process resolution failures never block policy evaluation.

### 10.5 Capability Requirements

The daemon needs:
- `CAP_NET_ADMIN` -- required for nftables and conntrack.
- `CAP_NET_RAW` -- required for conntrack event monitoring.
- Read access to `/proc` -- required for process resolution.

These are already declared in the systemd unit file from sub-project 1:
```ini
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW
```

---

## 11. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| nft CLI output format changes between versions | Low | High | Pin to JSON output (`-j`), which is the stable machine-readable format. Validate output structure at startup. |
| conntrack event flood under high traffic | Medium | Medium | Bounded buffer, drop policy, filtering at source (NEW events only for full pipeline). |
| Process gone before /proc read (race) | High | Low | Return `Ok(None)`, already handled by best-effort design. |
| Daemon crash leaves partial nft rules | Low | Medium | Rollback state saved before every modification. `sync_all_rules()` at startup reconciles. |
| Permission denied despite CAP_NET_ADMIN | Low | High | Startup check verifies capabilities. Clear error message. |
| LRU cache memory growth | Low | Low | Fixed capacity, bounded by design. |
| nft and conntrack binaries not installed | Medium | High | Startup check with clear error message: "nftables package required". |
| IPv6 conntrack events | Medium | Low | Parser handles both v4 and v6. Integration tests cover both. |

---

## 12. Implementation Order

Recommended implementation sequence within this sub-project:

1. **ProcfsProcessResolver** (simplest, no external dependencies beyond /proc)
   - /proc parsing functions with unit tests
   - LRU cache with unit tests
   - Adapter implementation
   - Integration tests

2. **NftablesFirewallAdapter** (most critical, needs careful testing)
   - NftCommand builder with unit tests
   - nft JSON output parser with unit tests
   - Rule translation (domain Rule -> nft expressions) with unit tests
   - Handle tracking
   - apply_rule / remove_rule / sync_all_rules
   - Rollback mechanism
   - Integration tests

3. **ConntrackMonitorAdapter** (depends on understanding from #1 and #2)
   - Conntrack event parser with unit tests
   - Domain transformation with unit tests
   - Async stream implementation
   - Backpressure handling
   - Integration tests

4. **System whitelist** (depends on #2 for actual nft application)
   - Whitelist rule definitions
   - First-start detection
   - Unit tests

5. **Daemon integration** (wires everything together)
   - Bootstrap updates
   - Supervisor monitoring stream task
   - ConnectionService enhancement
   - Config updates
   - End-to-end smoke test

---

## 13. Success Criteria

This sub-project is complete when:

- [ ] `NftablesFirewallAdapter` passes all unit tests and creates/removes real nft rules in integration tests
- [ ] `ConntrackMonitorAdapter` receives real conntrack events and transforms them into domain Connections
- [ ] `ProcfsProcessResolver` resolves the test process's own PID and socket
- [ ] The daemon starts with real adapters, creates the syswall nftables table, and applies system whitelist rules
- [ ] The full pipeline works: a new connection triggers conntrack event -> process resolution -> policy evaluation -> event published
- [ ] Fakes remain available for unit/integration tests that do not need real system access
- [ ] All existing tests continue to pass
- [ ] `cargo test` runs unit tests without root
- [ ] `cargo test --features integration` runs full integration tests with root
