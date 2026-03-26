# SysWall Premium UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the complete frontend UI for SysWall: design system, Tauri gRPC client bridge, TypeScript types/stores/API, sidebar navigation layout, and six views (Dashboard, Connections, Rules, Auto-learning, Audit, Settings). All user-facing text in French.

**Architecture:** The Tauri Rust backend connects to the daemon via gRPC (Unix socket), exposes Tauri commands for request/response and emits Tauri events for streaming. The Svelte 5 frontend uses reactive stores fed by these events, with typed API wrappers around `invoke()`. No business logic in the UI.

**Tech Stack:** Rust (Tauri 2, tonic gRPC client), Svelte 5, SvelteKit, TypeScript, CSS custom properties (no UI library)

**Spec:** `docs/superpowers/specs/2026-03-26-syswall-premium-ui-design.md`

---

## File Map

### src-tauri/src/ (Rust)
| File | Responsibility |
|---|---|
| `Cargo.toml` | Add tonic, tokio, syswall-proto dependencies |
| `src/lib.rs` | Tauri builder with managed state, commands, setup hook |
| `src/grpc_client.rs` | GrpcClient: connect to Unix socket, hold channel |
| `src/streams.rs` | Subscribe to gRPC events, emit Tauri events |
| `src/commands/mod.rs` | Re-export command modules |
| `src/commands/status.rs` | get_status command |
| `src/commands/rules.rs` | list_rules, create_rule, delete_rule, toggle_rule |
| `src/commands/decisions.rs` | list_pending_decisions, respond_to_decision |

### src/ (Svelte)
| File | Responsibility |
|---|---|
| `app.html` | HTML shell, lang="fr" |
| `app.css` | Design tokens, global theme, utility classes |
| `lib/types/index.ts` | TypeScript interfaces mirroring proto |
| `lib/api/client.ts` | Typed invoke() wrappers |
| `lib/stores/status.ts` | Firewall status store |
| `lib/stores/connections.ts` | Connections store + filters |
| `lib/stores/rules.ts` | Rules store |
| `lib/stores/decisions.ts` | Pending decisions store |
| `lib/stores/audit.ts` | Audit events store |
| `lib/stores/dashboard.ts` | Derived dashboard stats |
| `lib/i18n/fr.ts` | French labels |
| `lib/components/ui/*.svelte` | Design system components (11 files) |
| `lib/components/dashboard/*.svelte` | Dashboard widgets (4 files) |
| `lib/components/connections/*.svelte` | Connection components (3 files) |
| `lib/components/rules/*.svelte` | Rule components (4 files) |
| `lib/components/learning/*.svelte` | Decision components (3 files) |
| `lib/components/audit/*.svelte` | Audit components (2 files) |
| `routes/+layout.svelte` | Main layout with sidebar |
| `routes/+layout.ts` | SSR disabled |
| `routes/+page.svelte` | Redirect to /dashboard |
| `routes/dashboard/+page.svelte` | Dashboard view |
| `routes/connections/+page.svelte` | Connections view |
| `routes/rules/+page.svelte` | Rules view |
| `routes/learning/+page.svelte` | Auto-learning view |
| `routes/audit/+page.svelte` | Audit view |
| `routes/settings/+page.svelte` | Settings view |

---

### Task 1: Design System CSS

**Files:**
- Modify: `crates/ui/src/app.html`
- Rewrite: `crates/ui/src/app.css` (currently contains Tauri default styles)

- [ ] **Step 1: Update app.html with French locale and dark background**

`crates/ui/src/app.html`:
```html
<!doctype html>
<html lang="fr">
  <head>
    <meta charset="utf-8" />
    <link rel="icon" href="%sveltekit.assets%/favicon.png" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>SysWall</title>
    <meta name="color-scheme" content="dark" />
    %sveltekit.head%
  </head>
  <body data-sveltekit-preload-data="hover" data-theme="dark">
    <div style="display: contents">%sveltekit.body%</div>
  </body>
</html>
```

- [ ] **Step 2: Create the complete design system CSS**

`crates/ui/src/app.css`:
```css
/* ============================================================
   SysWall Design System — Design Tokens & Global Theme
   ============================================================ */

/* --- Fonts --- */
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600;700&display=swap');

/* --- Design Tokens --- */
:root {
  /* Backgrounds */
  --bg-primary: #0d1117;
  --bg-secondary: #161b22;
  --bg-tertiary: #1c2333;
  --bg-hover: #21283b;

  /* Borders */
  --border-primary: #30363d;
  --border-subtle: #21262d;

  /* Text */
  --text-primary: #e6edf3;
  --text-secondary: #8b949e;
  --text-tertiary: #6e7681;

  /* Accent Colors */
  --accent-cyan: #00d4ff;
  --accent-green: #00ff88;
  --accent-red: #ff4444;
  --accent-orange: #ff8c00;
  --accent-purple: #a855f7;

  /* Accent with alpha */
  --accent-cyan-15: rgba(0, 212, 255, 0.15);
  --accent-green-15: rgba(0, 255, 136, 0.15);
  --accent-red-15: rgba(255, 68, 68, 0.15);
  --accent-orange-15: rgba(255, 140, 0, 0.15);
  --accent-purple-15: rgba(168, 85, 247, 0.15);

  /* Glow */
  --glow-cyan: 0 0 12px rgba(0, 212, 255, 0.3);
  --glow-green: 0 0 12px rgba(0, 255, 136, 0.3);
  --glow-red: 0 0 12px rgba(255, 68, 68, 0.3);
  --glow-orange: 0 0 12px rgba(255, 140, 0, 0.3);
  --glow-purple: 0 0 12px rgba(168, 85, 247, 0.3);

  /* Glassmorphism */
  --glass-bg: rgba(22, 27, 34, 0.8);
  --glass-border: rgba(48, 54, 61, 0.5);
  --glass-blur: 12px;

  /* Typography */
  --font-mono: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
  --font-sans: 'Inter', 'Segoe UI', system-ui, sans-serif;

  --font-size-xs: 0.75rem;
  --font-size-sm: 0.875rem;
  --font-size-base: 1rem;
  --font-size-lg: 1.25rem;
  --font-size-xl: 1.5rem;
  --font-size-2xl: 2rem;

  --font-weight-normal: 400;
  --font-weight-medium: 500;
  --font-weight-semibold: 600;
  --font-weight-bold: 700;

  /* Spacing */
  --space-1: 0.25rem;
  --space-2: 0.5rem;
  --space-3: 0.75rem;
  --space-4: 1rem;
  --space-5: 1.25rem;
  --space-6: 1.5rem;
  --space-8: 2rem;
  --space-10: 2.5rem;
  --space-12: 3rem;

  /* Radius */
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-xl: 16px;
  --radius-full: 9999px;

  /* Transitions */
  --transition-fast: 150ms ease;
  --transition-base: 250ms ease;
  --transition-slow: 400ms ease;

  /* Sidebar */
  --sidebar-width: 240px;
}

/* --- Global Reset --- */
*,
*::before,
*::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html {
  font-size: 16px;
  line-height: 1.5;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-rendering: optimizeLegibility;
}

body {
  font-family: var(--font-sans);
  font-size: var(--font-size-base);
  font-weight: var(--font-weight-normal);
  color: var(--text-primary);
  background-color: var(--bg-primary);
  overflow: hidden;
}

/* --- Scrollbar Styling --- */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: var(--bg-primary);
}

::-webkit-scrollbar-thumb {
  background: var(--border-primary);
  border-radius: var(--radius-full);
}

::-webkit-scrollbar-thumb:hover {
  background: var(--text-tertiary);
}

/* --- Focus Styles --- */
:focus-visible {
  outline: 2px solid var(--accent-cyan);
  outline-offset: 2px;
}

/* --- Utility Classes --- */
.font-mono {
  font-family: var(--font-mono);
}

.font-sans {
  font-family: var(--font-sans);
}

.text-primary { color: var(--text-primary); }
.text-secondary { color: var(--text-secondary); }
.text-tertiary { color: var(--text-tertiary); }
.text-cyan { color: var(--accent-cyan); }
.text-green { color: var(--accent-green); }
.text-red { color: var(--accent-red); }
.text-orange { color: var(--accent-orange); }
.text-purple { color: var(--accent-purple); }

.text-xs { font-size: var(--font-size-xs); }
.text-sm { font-size: var(--font-size-sm); }
.text-base { font-size: var(--font-size-base); }
.text-lg { font-size: var(--font-size-lg); }
.text-xl { font-size: var(--font-size-xl); }
.text-2xl { font-size: var(--font-size-2xl); }

.font-normal { font-weight: var(--font-weight-normal); }
.font-medium { font-weight: var(--font-weight-medium); }
.font-semibold { font-weight: var(--font-weight-semibold); }
.font-bold { font-weight: var(--font-weight-bold); }

.truncate {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* --- Animations --- */
@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes slideUp {
  from { opacity: 0; transform: translateY(12px); }
  to { opacity: 1; transform: translateY(0); }
}

@keyframes slideInRight {
  from { opacity: 0; transform: translateX(24px); }
  to { opacity: 1; transform: translateX(0); }
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

@keyframes glowPulse {
  0%, 100% { box-shadow: var(--glow-cyan); }
  50% { box-shadow: 0 0 20px rgba(0, 212, 255, 0.5); }
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}

/* --- Selection --- */
::selection {
  background: var(--accent-cyan-15);
  color: var(--accent-cyan);
}
```

- [ ] **Step 3: Verify by opening the page**

Open the project and verify the dark background loads correctly:
```bash
cd /home/seb/Dev/SysWall/crates/ui && npm run dev
```
Kill the dev server after visual check.

---

### Task 2: Tauri gRPC Client (Rust Backend)

**Files:**
- Modify: `crates/ui/src-tauri/Cargo.toml`
- Rewrite: `crates/ui/src-tauri/src/lib.rs`
- Create: `crates/ui/src-tauri/src/grpc_client.rs`
- Create: `crates/ui/src-tauri/src/streams.rs`
- Create: `crates/ui/src-tauri/src/commands/mod.rs`
- Create: `crates/ui/src-tauri/src/commands/status.rs`
- Create: `crates/ui/src-tauri/src/commands/rules.rs`
- Create: `crates/ui/src-tauri/src/commands/decisions.rs`

- [ ] **Step 1: Update Cargo.toml with gRPC and async dependencies**

`crates/ui/src-tauri/Cargo.toml`:
```toml
[package]
name = "ui"
version = "0.1.0"
description = "SysWall Desktop Firewall UI"
authors = ["you"]
edition = "2021"

[lib]
name = "ui_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tonic = "0.12"
prost = "0.13"
tower = "0.5"
hyper-util = { version = "0.1", features = ["tokio"] }
http = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

syswall-proto = { path = "../../proto" }
```

- [ ] **Step 2: Create grpc_client.rs**

`crates/ui/src-tauri/src/grpc_client.rs`:
```rust
//! gRPC client for connecting to the SysWall daemon over Unix socket.
//! Client gRPC pour la connexion au daemon SysWall via socket Unix.

use std::sync::Arc;
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;
use tracing::{error, info};

use syswall_proto::syswall::sys_wall_control_client::SysWallControlClient;
use syswall_proto::syswall::sys_wall_events_client::SysWallEventsClient;

/// Path to the daemon Unix socket.
/// Chemin vers le socket Unix du daemon.
const DEFAULT_SOCKET_PATH: &str = "/var/run/syswall/syswall.sock";

/// Holds the gRPC channel and typed clients.
/// Contient le canal gRPC et les clients typés.
#[derive(Clone)]
pub struct GrpcClient {
    pub control: SysWallControlClient<Channel>,
    pub events: SysWallEventsClient<Channel>,
}

impl GrpcClient {
    /// Connect to the daemon Unix socket.
    /// Se connecte au socket Unix du daemon.
    pub async fn connect(socket_path: Option<&str>) -> Result<Self, String> {
        let path = socket_path.unwrap_or(DEFAULT_SOCKET_PATH).to_string();

        info!("Connecting to daemon at {}", path);

        let channel = Endpoint::try_from("http://[::]:50051")
            .map_err(|e| format!("Failed to create endpoint: {}", e))?
            .connect_with_connector(service_fn(move |_: Uri| {
                let path = path.clone();
                async move {
                    let stream = UnixStream::connect(path).await?;
                    Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(stream))
                }
            }))
            .await
            .map_err(|e| format!("Failed to connect to daemon socket: {}", e))?;

        info!("Connected to daemon successfully");

        Ok(Self {
            control: SysWallControlClient::new(channel.clone()),
            events: SysWallEventsClient::new(channel),
        })
    }
}

/// Thread-safe wrapper for the gRPC client, stored as Tauri managed state.
/// Wrapper thread-safe pour le client gRPC, stocké en état géré par Tauri.
pub struct GrpcState {
    pub client: Arc<Mutex<Option<GrpcClient>>>,
}

impl GrpcState {
    /// Create a new empty state (client connects during setup).
    /// Crée un nouvel état vide (le client se connecte au démarrage).
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// Get or reconnect the client.
    /// Obtient ou reconnecte le client.
    pub async fn get_client(&self) -> Result<GrpcClient, String> {
        let mut guard = self.client.lock().await;
        if let Some(ref client) = *guard {
            return Ok(client.clone());
        }

        let client = GrpcClient::connect(None).await?;
        *guard = Some(client.clone());
        Ok(client)
    }

    /// Force reconnection (e.g., after transport error).
    /// Force la reconnexion (par ex. après erreur de transport).
    pub async fn reconnect(&self) -> Result<GrpcClient, String> {
        let mut guard = self.client.lock().await;
        let client = GrpcClient::connect(None).await?;
        *guard = Some(client.clone());
        Ok(client)
    }
}
```

- [ ] **Step 3: Create streams.rs**

`crates/ui/src-tauri/src/streams.rs`:
```rust
//! Event stream subscriber — listens to daemon gRPC events and emits Tauri events.
//! Abonné au flux d'événements — écoute les événements gRPC du daemon et émet des événements Tauri.

use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use syswall_proto::syswall::SubscribeRequest;

use crate::grpc_client::GrpcClient;

/// Map gRPC event_type strings to Tauri event names.
/// Associe les chaînes event_type gRPC aux noms d'événements Tauri.
fn map_event_name(event_type: &str) -> &str {
    match event_type {
        "connection_detected" => "syswall://connection-detected",
        "connection_updated" => "syswall://connection-updated",
        "connection_closed" => "syswall://connection-closed",
        "rule_created" => "syswall://rule-created",
        "rule_updated" => "syswall://rule-updated",
        "rule_deleted" => "syswall://rule-deleted",
        "rule_matched" => "syswall://rule-matched",
        "decision_required" => "syswall://decision-required",
        "decision_resolved" => "syswall://decision-resolved",
        "decision_expired" => "syswall://decision-expired",
        "firewall_status_changed" => "syswall://status-changed",
        "system_error" => "syswall://system-error",
        other => {
            warn!("Unknown event type: {}", other);
            "syswall://unknown"
        }
    }
}

/// Payload emitted to the frontend for each event.
/// Charge utile émise vers le frontend pour chaque événement.
#[derive(Clone, serde::Serialize)]
pub struct EventPayload {
    pub event_type: String,
    pub payload_json: String,
    pub timestamp: String,
}

/// Subscribe to the daemon event stream and forward events to the Tauri frontend.
/// S'abonne au flux d'événements du daemon et transmet les événements au frontend Tauri.
pub async fn subscribe_and_forward(
    app_handle: AppHandle,
    client: Arc<Mutex<Option<GrpcClient>>>,
) {
    loop {
        let events_client = {
            let guard = client.lock().await;
            match guard.as_ref() {
                Some(c) => c.events.clone(),
                None => {
                    warn!("No gRPC client available, waiting before retry...");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            }
        };

        info!("Subscribing to daemon event stream...");

        let stream_result = events_client
            .clone()
            .subscribe_events(SubscribeRequest {})
            .await;

        match stream_result {
            Ok(response) => {
                let mut stream = response.into_inner();
                info!("Event stream connected");

                loop {
                    match stream.message().await {
                        Ok(Some(msg)) => {
                            let event_name = map_event_name(&msg.event_type);
                            let payload = EventPayload {
                                event_type: msg.event_type.clone(),
                                payload_json: msg.payload_json.clone(),
                                timestamp: msg.timestamp.clone(),
                            };

                            if let Err(e) = app_handle.emit(event_name, &payload) {
                                error!("Failed to emit event {}: {}", event_name, e);
                            }
                        }
                        Ok(None) => {
                            warn!("Event stream ended, will reconnect...");
                            break;
                        }
                        Err(e) => {
                            error!("Event stream error: {}, will reconnect...", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to subscribe to events: {}", e);
            }
        }

        // Wait before reconnecting
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
```

- [ ] **Step 4: Create commands/mod.rs**

`crates/ui/src-tauri/src/commands/mod.rs`:
```rust
//! Tauri command modules — thin wrappers around gRPC calls.
//! Modules de commandes Tauri — wrappers fins autour des appels gRPC.

pub mod status;
pub mod rules;
pub mod decisions;
```

- [ ] **Step 5: Create commands/status.rs**

`crates/ui/src-tauri/src/commands/status.rs`:
```rust
//! Status command — get firewall status.
//! Commande de statut — obtient l'état du pare-feu.

use tauri::State;

use syswall_proto::syswall::Empty;

use crate::grpc_client::GrpcState;

/// Response type for the frontend (serializable).
/// Type de réponse pour le frontend (sérialisable).
#[derive(serde::Serialize)]
pub struct StatusResult {
    pub enabled: bool,
    pub active_rules_count: u32,
    pub nftables_synced: bool,
    pub uptime_secs: u64,
    pub version: String,
}

/// Get the current firewall status from the daemon.
/// Obtient l'état actuel du pare-feu depuis le daemon.
#[tauri::command]
pub async fn get_status(state: State<'_, GrpcState>) -> Result<StatusResult, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .get_status(Empty {})
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    let status = response.into_inner();

    Ok(StatusResult {
        enabled: status.enabled,
        active_rules_count: status.active_rules_count,
        nftables_synced: status.nftables_synced,
        uptime_secs: status.uptime_secs,
        version: status.version,
    })
}
```

- [ ] **Step 6: Create commands/rules.rs**

`crates/ui/src-tauri/src/commands/rules.rs`:
```rust
//! Rule commands — CRUD operations on firewall rules.
//! Commandes de règles — opérations CRUD sur les règles du pare-feu.

use tauri::State;

use syswall_proto::syswall::{
    CreateRuleRequest as ProtoCreateRule, RuleFiltersRequest, RuleIdRequest, ToggleRuleRequest,
};

use crate::grpc_client::GrpcState;

/// Serializable rule for the frontend.
/// Règle sérialisable pour le frontend.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct RuleResult {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub enabled: bool,
    pub criteria_json: String,
    pub effect: String,
    pub scope_json: String,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Input for creating a rule from the frontend.
/// Entrée pour créer une règle depuis le frontend.
#[derive(serde::Deserialize)]
pub struct CreateRuleInput {
    pub name: String,
    pub priority: u32,
    pub criteria_json: String,
    pub effect: String,
    pub scope_json: String,
    pub source: String,
}

/// List all rules from the daemon.
/// Liste toutes les règles depuis le daemon.
#[tauri::command]
pub async fn list_rules(
    state: State<'_, GrpcState>,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<Vec<RuleResult>, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .list_rules(RuleFiltersRequest {
            offset: offset.unwrap_or(0),
            limit: limit.unwrap_or(1000),
        })
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    let rules = response
        .into_inner()
        .rules
        .into_iter()
        .map(|r| RuleResult {
            id: r.id,
            name: r.name,
            priority: r.priority,
            enabled: r.enabled,
            criteria_json: r.criteria_json,
            effect: r.effect,
            scope_json: r.scope_json,
            source: r.source,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok(rules)
}

/// Create a new rule.
/// Crée une nouvelle règle.
#[tauri::command]
pub async fn create_rule(
    state: State<'_, GrpcState>,
    input: CreateRuleInput,
) -> Result<RuleResult, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .create_rule(ProtoCreateRule {
            name: input.name,
            priority: input.priority,
            criteria_json: input.criteria_json,
            effect: input.effect,
            scope_json: input.scope_json,
            source: input.source,
        })
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    let rule = response
        .into_inner()
        .rule
        .ok_or_else(|| "No rule in response".to_string())?;

    Ok(RuleResult {
        id: rule.id,
        name: rule.name,
        priority: rule.priority,
        enabled: rule.enabled,
        criteria_json: rule.criteria_json,
        effect: rule.effect,
        scope_json: rule.scope_json,
        source: rule.source,
        created_at: rule.created_at,
        updated_at: rule.updated_at,
    })
}

/// Delete a rule by ID.
/// Supprime une règle par ID.
#[tauri::command]
pub async fn delete_rule(state: State<'_, GrpcState>, id: String) -> Result<(), String> {
    let mut client = state.get_client().await?;

    client
        .control
        .delete_rule(RuleIdRequest { id })
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    Ok(())
}

/// Toggle a rule enabled/disabled.
/// Active/désactive une règle.
#[tauri::command]
pub async fn toggle_rule(
    state: State<'_, GrpcState>,
    id: String,
    enabled: bool,
) -> Result<RuleResult, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .toggle_rule(ToggleRuleRequest { id, enabled })
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    let rule = response
        .into_inner()
        .rule
        .ok_or_else(|| "No rule in response".to_string())?;

    Ok(RuleResult {
        id: rule.id,
        name: rule.name,
        priority: rule.priority,
        enabled: rule.enabled,
        criteria_json: rule.criteria_json,
        effect: rule.effect,
        scope_json: rule.scope_json,
        source: rule.source,
        created_at: rule.created_at,
        updated_at: rule.updated_at,
    })
}
```

- [ ] **Step 7: Create commands/decisions.rs**

`crates/ui/src-tauri/src/commands/decisions.rs`:
```rust
//! Decision commands — manage pending auto-learning decisions.
//! Commandes de décision — gère les décisions d'auto-apprentissage en attente.

use tauri::State;

use syswall_proto::syswall::{DecisionResponseRequest, Empty};

use crate::grpc_client::GrpcState;

/// Serializable pending decision for the frontend.
/// Décision en attente sérialisable pour le frontend.
#[derive(serde::Serialize, Clone)]
pub struct PendingDecisionResult {
    pub id: String,
    pub snapshot_json: String,
    pub requested_at: String,
    pub expires_at: String,
    pub status: String,
}

/// Input for responding to a decision.
/// Entrée pour répondre à une décision.
#[derive(serde::Deserialize)]
pub struct DecisionResponseInput {
    pub pending_decision_id: String,
    pub action: String,
    pub granularity: String,
}

/// List all pending decisions.
/// Liste toutes les décisions en attente.
#[tauri::command]
pub async fn list_pending_decisions(
    state: State<'_, GrpcState>,
) -> Result<Vec<PendingDecisionResult>, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .list_pending_decisions(Empty {})
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    let decisions = response
        .into_inner()
        .decisions
        .into_iter()
        .map(|d| PendingDecisionResult {
            id: d.id,
            snapshot_json: d.snapshot_json,
            requested_at: d.requested_at,
            expires_at: d.expires_at,
            status: d.status,
        })
        .collect();

    Ok(decisions)
}

/// Respond to a pending decision.
/// Répond à une décision en attente.
#[tauri::command]
pub async fn respond_to_decision(
    state: State<'_, GrpcState>,
    input: DecisionResponseInput,
) -> Result<String, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .respond_to_decision(DecisionResponseRequest {
            pending_decision_id: input.pending_decision_id,
            action: input.action,
            granularity: input.granularity,
        })
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    Ok(response.into_inner().decision_id)
}
```

- [ ] **Step 8: Rewrite lib.rs with Tauri builder, managed state, and setup hook**

`crates/ui/src-tauri/src/lib.rs`:
```rust
//! SysWall UI — Tauri application entry point.
//! SysWall UI — Point d'entrée de l'application Tauri.

mod commands;
mod grpc_client;
mod streams;

use grpc_client::{GrpcClient, GrpcState};
use tracing::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let grpc_state = GrpcState::new();
    let client_arc = grpc_state.client.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(grpc_state)
        .invoke_handler(tauri::generate_handler![
            commands::status::get_status,
            commands::rules::list_rules,
            commands::rules::create_rule,
            commands::rules::delete_rule,
            commands::rules::toggle_rule,
            commands::decisions::list_pending_decisions,
            commands::decisions::respond_to_decision,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            let client = client_arc.clone();

            // Spawn background task: connect to daemon and subscribe to events
            tauri::async_runtime::spawn(async move {
                info!("Attempting initial connection to daemon...");

                match GrpcClient::connect(None).await {
                    Ok(grpc_client) => {
                        info!("Initial daemon connection successful");
                        let mut guard = client.lock().await;
                        *guard = Some(grpc_client);
                        drop(guard);

                        // Start event stream forwarding
                        streams::subscribe_and_forward(app_handle, client).await;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Initial daemon connection failed: {}. Will retry on demand.",
                            e
                        );
                        // Event stream will retry in its loop
                        streams::subscribe_and_forward(app_handle, client).await;
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 9: Verify the Rust side compiles**

```bash
cd /home/seb/Dev/SysWall/crates/ui/src-tauri && cargo check
```

---

### Task 3: TypeScript Types + API Client + i18n

**Files:**
- Create: `crates/ui/src/lib/types/index.ts`
- Create: `crates/ui/src/lib/api/client.ts`
- Create: `crates/ui/src/lib/i18n/fr.ts`

- [ ] **Step 1: Create TypeScript types mirroring proto messages**

`crates/ui/src/lib/types/index.ts`:
```typescript
// TypeScript types mirroring proto messages and domain types.
// Types TypeScript reflétant les messages proto et types du domaine.

export interface StatusResponse {
  enabled: boolean;
  active_rules_count: number;
  nftables_synced: boolean;
  uptime_secs: number;
  version: string;
}

export interface RuleMessage {
  id: string;
  name: string;
  priority: number;
  enabled: boolean;
  criteria_json: string;
  effect: string;
  scope_json: string;
  source: string;
  created_at: string;
  updated_at: string;
}

export interface RuleCriteria {
  application?: { name?: string; path?: string };
  user?: string;
  remote_ip?: { exact?: string; cidr?: string };
  remote_port?: { exact?: number; range?: [number, number] };
  local_port?: { exact?: number; range?: [number, number] };
  protocol?: string;
  direction?: string;
}

export interface RuleScope {
  type: 'permanent' | 'temporary';
  expires_at?: string;
}

export interface CreateRuleRequest {
  name: string;
  priority: number;
  criteria_json: string;
  effect: string;
  scope_json: string;
  source: string;
}

export interface PendingDecisionMessage {
  id: string;
  snapshot_json: string;
  requested_at: string;
  expires_at: string;
  status: string;
}

export interface ConnectionSnapshot {
  protocol: string;
  source: { ip: string; port: number };
  destination: { ip: string; port: number };
  direction: string;
  process_name?: string;
  process_path?: string;
  user?: string;
}

export interface DecisionResponse {
  pending_decision_id: string;
  action: string;
  granularity: string;
}

export interface DomainEventPayload {
  event_type: string;
  payload_json: string;
  timestamp: string;
}

export interface ConnectionEvent {
  id: string;
  protocol: string;
  source: { ip: string; port: number };
  destination: { ip: string; port: number };
  direction: string;
  state: string;
  process_name?: string;
  process_path?: string;
  pid?: number;
  user?: string;
  bytes_sent: number;
  bytes_received: number;
  started_at: string;
  verdict: string;
  matched_rule?: string;
}

export interface AuditEvent {
  id: string;
  timestamp: string;
  severity: string;
  category: string;
  description: string;
  metadata: Record<string, string>;
}

export type Verdict = 'allowed' | 'blocked' | 'pending_decision' | 'unknown' | 'ignored';
export type Protocol = 'tcp' | 'udp' | 'icmp' | 'other';
export type Direction = 'inbound' | 'outbound';
export type Severity = 'debug' | 'info' | 'warning' | 'error' | 'critical';
export type EventCategory = 'connection' | 'rule' | 'decision' | 'system' | 'config';
export type RuleEffect = 'allow' | 'block' | 'ask' | 'observe';
export type RuleSource = 'manual' | 'auto_learning' | 'import' | 'system';
export type DecisionAction = 'allow_once' | 'block_once' | 'always_allow' | 'always_block' | 'create_rule' | 'ignore';
export type DecisionGranularity = 'app_only' | 'app_and_destination' | 'app_and_protocol' | 'full';
```

- [ ] **Step 2: Create API client with typed invoke wrappers**

`crates/ui/src/lib/api/client.ts`:
```typescript
// Typed API client — wrappers around Tauri invoke().
// Client API typé — wrappers autour de Tauri invoke().

import { invoke } from '@tauri-apps/api/core';
import type {
  StatusResponse,
  RuleMessage,
  CreateRuleRequest,
  PendingDecisionMessage,
  DecisionResponse,
} from '$lib/types';

// --- Status ---

export async function getStatus(): Promise<StatusResponse> {
  return invoke<StatusResponse>('get_status');
}

// --- Rules ---

export async function listRules(offset = 0, limit = 1000): Promise<RuleMessage[]> {
  return invoke<RuleMessage[]>('list_rules', { offset, limit });
}

export async function createRule(input: CreateRuleRequest): Promise<RuleMessage> {
  return invoke<RuleMessage>('create_rule', { input });
}

export async function deleteRule(id: string): Promise<void> {
  return invoke<void>('delete_rule', { id });
}

export async function toggleRule(id: string, enabled: boolean): Promise<RuleMessage> {
  return invoke<RuleMessage>('toggle_rule', { id, enabled });
}

// --- Decisions ---

export async function listPendingDecisions(): Promise<PendingDecisionMessage[]> {
  return invoke<PendingDecisionMessage[]>('list_pending_decisions');
}

export async function respondToDecision(input: DecisionResponse): Promise<string> {
  return invoke<string>('respond_to_decision', { input });
}
```

- [ ] **Step 3: Create French i18n labels**

`crates/ui/src/lib/i18n/fr.ts`:
```typescript
// French locale labels for all UI text.
// Labels en français pour tout le texte de l'interface.

export const fr = {
  // Navigation
  nav_dashboard: 'Tableau de bord',
  nav_connections: 'Connexions',
  nav_rules: 'Règles',
  nav_learning: 'Apprentissage',
  nav_audit: 'Journal',
  nav_settings: 'Paramètres',

  // Status
  status_active: 'Actif',
  status_inactive: 'Inactif',
  status_synced: 'Synchronisé',
  status_not_synced: 'Non synchronisé',

  // Dashboard
  dash_active_connections: 'Connexions actives',
  dash_allowed: 'Autorisées',
  dash_blocked: 'Bloquées',
  dash_alerts: 'Alertes',
  dash_top_apps: 'Top applications',
  dash_top_destinations: 'Top destinations',
  dash_traffic_trend: 'Tendance du trafic',
  dash_recent_alerts: 'Alertes récentes',
  dash_firewall_status: 'État du pare-feu',
  dash_version: 'Version',
  dash_uptime: 'Disponibilité',
  dash_nftables: 'nftables',
  dash_waiting: 'En attente de connexions...',

  // Connections
  conn_application: 'Application',
  conn_pid: 'PID',
  conn_user: 'Utilisateur',
  conn_local_addr: 'Adresse locale',
  conn_remote_addr: 'Adresse distante',
  conn_protocol: 'Protocole',
  conn_state: 'État',
  conn_verdict: 'Verdict',
  conn_rule: 'Règle',
  conn_unknown: 'Inconnu',
  conn_search: 'Rechercher...',
  conn_filter_protocol: 'Protocole',
  conn_filter_verdict: 'Verdict',
  conn_filter_direction: 'Direction',
  conn_filter_all: 'Tous',
  conn_allowed: 'Autorisé',
  conn_blocked: 'Bloqué',
  conn_pending: 'En attente',
  conn_inbound: 'Entrant',
  conn_outbound: 'Sortant',
  conn_clear_filters: 'Effacer les filtres',
  conn_bytes_sent: 'Octets envoyés',
  conn_bytes_received: 'Octets reçus',
  conn_started_at: 'Début',
  conn_connection_id: 'ID connexion',
  conn_no_connections: 'Aucune connexion active',

  // Rules
  rules_title: 'Règles de pare-feu',
  rules_new: 'Nouvelle règle',
  rules_import: 'Importer',
  rules_export: 'Exporter',
  rules_coming_soon: 'Bientôt disponible',
  rules_name: 'Nom',
  rules_priority: 'Priorité',
  rules_effect: 'Effet',
  rules_source: 'Source',
  rules_scope: 'Portée',
  rules_criteria: 'Critères',
  rules_created_at: 'Créé le',
  rules_allow: 'Autoriser',
  rules_block: 'Bloquer',
  rules_ask: 'Demander',
  rules_observe: 'Observer',
  rules_manual: 'Manuelle',
  rules_auto_learning: 'Auto-apprentissage',
  rules_import_source: 'Importée',
  rules_system: 'Système',
  rules_permanent: 'Permanente',
  rules_temporary: 'Temporaire',
  rules_edit: 'Modifier',
  rules_delete: 'Supprimer',
  rules_delete_confirm: 'Supprimer cette règle ?',
  rules_delete_message: 'Cette action est irréversible.',
  rules_cancel: 'Annuler',
  rules_save: 'Enregistrer',
  rules_create: 'Créer',
  rules_no_rules: 'Aucune règle configurée. Créez votre première règle.',

  // Criteria builder
  criteria_application: 'Application',
  criteria_user: 'Utilisateur',
  criteria_remote_ip: 'IP distante',
  criteria_remote_port: 'Port distant',
  criteria_local_port: 'Port local',
  criteria_protocol: 'Protocole',
  criteria_direction: 'Direction',
  criteria_add: 'Ajouter un critère',
  criteria_remove: 'Retirer',

  // Learning / Decision
  learn_title: 'Apprentissage',
  learn_new_connection: 'Nouvelle connexion détectée',
  learn_path: 'Chemin',
  learn_destination: 'Destination',
  learn_expires_in: 'Expire dans',
  learn_granularity: 'Granularité de la règle',
  learn_app_only: 'Application seule',
  learn_app_destination: 'Application + destination',
  learn_app_protocol: 'Application + protocole',
  learn_full_match: 'Correspondance complète',
  learn_allow_once: 'Autoriser une fois',
  learn_block_once: 'Bloquer une fois',
  learn_always_allow: 'Toujours autoriser',
  learn_always_block: 'Toujours bloquer',
  learn_create_rule: 'Créer une règle',
  learn_ignore: 'Ignorer',
  learn_queue: "File d'attente",
  learn_decision_of: 'sur',
  learn_toast: 'Nouvelle connexion détectée — Cliquez pour décider',
  learn_no_pending: 'Aucune décision en attente.',

  // Audit
  audit_title: "Journal d'audit",
  audit_timestamp: 'Horodatage',
  audit_severity: 'Sévérité',
  audit_category: 'Catégorie',
  audit_description: 'Description',
  audit_search: 'Rechercher...',
  audit_debug: 'Debug',
  audit_info: 'Info',
  audit_warning: 'Avertissement',
  audit_error: 'Erreur',
  audit_critical: 'Critique',
  audit_connection: 'Connexion',
  audit_rule: 'Règle',
  audit_decision: 'Décision',
  audit_system: 'Système',
  audit_config: 'Configuration',
  audit_previous: 'Précédent',
  audit_next: 'Suivant',
  audit_page: 'Page',
  audit_no_events: 'Aucun événement enregistré',

  // Settings
  settings_title: 'Paramètres',
  settings_firewall: 'Pare-feu',
  settings_learning: 'Apprentissage',
  settings_interface: 'Interface',
  settings_daemon: 'Daemon',
  settings_status: 'Statut',
  settings_default_policy: 'Politique par défaut',
  settings_nftables_table: 'Table nftables',
  settings_enabled: 'Activé',
  settings_timeout: "Délai d'expiration",
  settings_default_action: 'Action par défaut',
  settings_max_pending: 'Décisions en attente max',
  settings_theme: 'Thème',
  settings_theme_dark: 'Sombre',
  settings_locale: 'Langue',
  settings_locale_fr: 'Français',
  settings_refresh_interval: 'Intervalle de rafraîchissement',
  settings_socket: 'Socket',
  settings_version: 'Version',
  settings_uptime: 'Temps de fonctionnement',

  // Common
  common_loading: 'Chargement...',
  common_error: 'Une erreur est survenue',
  common_retry: 'Réessayer',
  common_empty: 'Aucune donnée',
  common_seconds: 'secondes',
  common_connection_error:
    'Impossible de se connecter au daemon SysWall. Vérifiez que le service est actif.',
} as const;

export type I18nKey = keyof typeof fr;
```

---

### Task 4: Svelte Stores

**Files:**
- Create: `crates/ui/src/lib/stores/status.ts`
- Create: `crates/ui/src/lib/stores/connections.ts`
- Create: `crates/ui/src/lib/stores/rules.ts`
- Create: `crates/ui/src/lib/stores/decisions.ts`
- Create: `crates/ui/src/lib/stores/audit.ts`
- Create: `crates/ui/src/lib/stores/dashboard.ts`

- [ ] **Step 1: Create status store**

`crates/ui/src/lib/stores/status.ts`:
```typescript
// Firewall status store — fetched on demand, updated by events.
// Store de statut du pare-feu — récupéré à la demande, mis à jour par les événements.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { getStatus } from '$lib/api/client';
import type { StatusResponse, DomainEventPayload } from '$lib/types';

const defaultStatus: StatusResponse = {
  enabled: false,
  active_rules_count: 0,
  nftables_synced: false,
  uptime_secs: 0,
  version: '',
};

export const firewallStatus = writable<StatusResponse>(defaultStatus);
export const statusError = writable<string | null>(null);
export const statusLoading = writable(true);

export const isFirewallActive = derived(firewallStatus, ($s) => $s.enabled);

export async function fetchStatus(): Promise<void> {
  statusLoading.set(true);
  statusError.set(null);
  try {
    const status = await getStatus();
    firewallStatus.set(status);
  } catch (e) {
    statusError.set(String(e));
  } finally {
    statusLoading.set(false);
  }
}

export function initStatusListener(): () => void {
  let unlisten: (() => void) | undefined;

  listen<DomainEventPayload>('syswall://status-changed', (event) => {
    try {
      const status: StatusResponse = JSON.parse(event.payload.payload_json);
      firewallStatus.set(status);
    } catch {
      // Ignore parse errors
    }
  }).then((fn) => {
    unlisten = fn;
  });

  return () => {
    unlisten?.();
  };
}
```

- [ ] **Step 2: Create connections store**

`crates/ui/src/lib/stores/connections.ts`:
```typescript
// Connections store — fed by real-time events, with filters.
// Store de connexions — alimenté par les événements en temps réel, avec filtres.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import type { ConnectionEvent, DomainEventPayload } from '$lib/types';

// Connection map keyed by ID
export const connections = writable<Map<string, ConnectionEvent>>(new Map());

// Filters
export const connectionFilters = writable({
  search: '',
  protocol: '',
  verdict: '',
  direction: '',
});

// Sorted connection list
export const connectionList = derived(connections, ($conns) => {
  return Array.from($conns.values()).sort(
    (a, b) => new Date(b.started_at).getTime() - new Date(a.started_at).getTime()
  );
});

// Filtered connections
export const filteredConnections = derived(
  [connectionList, connectionFilters],
  ([$list, $filters]) => {
    return $list.filter((conn) => {
      // Search filter
      if ($filters.search) {
        const q = $filters.search.toLowerCase();
        const searchable = [
          conn.process_name,
          conn.source?.ip,
          conn.destination?.ip,
          conn.pid?.toString(),
          conn.user,
        ]
          .filter(Boolean)
          .join(' ')
          .toLowerCase();
        if (!searchable.includes(q)) return false;
      }

      // Protocol filter
      if ($filters.protocol && conn.protocol.toLowerCase() !== $filters.protocol.toLowerCase()) {
        return false;
      }

      // Verdict filter
      if ($filters.verdict && conn.verdict !== $filters.verdict) {
        return false;
      }

      // Direction filter
      if ($filters.direction && conn.direction !== $filters.direction) {
        return false;
      }

      return true;
    });
  }
);

// Connection counts
export const connectionCounts = derived(connections, ($conns) => {
  let total = 0;
  let allowed = 0;
  let blocked = 0;
  let pending = 0;

  for (const conn of $conns.values()) {
    if (conn.state !== 'closed') {
      total++;
    }
    if (conn.verdict === 'allowed') allowed++;
    else if (conn.verdict === 'blocked') blocked++;
    else if (conn.verdict === 'pending_decision') pending++;
  }

  return { total, allowed, blocked, pending };
});

export function initConnectionListeners(): () => void {
  const unlisteners: (() => void)[] = [];

  listen<DomainEventPayload>('syswall://connection-detected', (event) => {
    try {
      const conn: ConnectionEvent = JSON.parse(event.payload.payload_json);
      connections.update((map) => {
        map.set(conn.id, conn);
        return new Map(map);
      });
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://connection-updated', (event) => {
    try {
      const update = JSON.parse(event.payload.payload_json);
      connections.update((map) => {
        const existing = map.get(update.id);
        if (existing) {
          map.set(update.id, { ...existing, state: update.state });
        }
        return new Map(map);
      });
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://connection-closed', (event) => {
    try {
      const payload = JSON.parse(event.payload.payload_json);
      const id = typeof payload === 'string' ? payload : payload.id || payload;
      connections.update((map) => {
        const existing = map.get(id);
        if (existing) {
          map.set(id, { ...existing, state: 'closed' });
        }
        return new Map(map);
      });
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://rule-matched', (event) => {
    try {
      const payload = JSON.parse(event.payload.payload_json);
      connections.update((map) => {
        const existing = map.get(payload.connection_id);
        if (existing) {
          map.set(payload.connection_id, {
            ...existing,
            verdict: payload.verdict,
            matched_rule: payload.rule_id,
          });
        }
        return new Map(map);
      });
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  return () => {
    unlisteners.forEach((fn) => fn());
  };
}
```

- [ ] **Step 3: Create rules store**

`crates/ui/src/lib/stores/rules.ts`:
```typescript
// Rules store — fetched on demand, updated by events.
// Store de règles — récupéré à la demande, mis à jour par les événements.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { listRules } from '$lib/api/client';
import type { RuleMessage, DomainEventPayload } from '$lib/types';

export const rules = writable<RuleMessage[]>([]);
export const rulesError = writable<string | null>(null);
export const rulesLoading = writable(true);

export const rulesCount = derived(rules, ($r) => $r.length);
export const activeRulesCount = derived(rules, ($r) => $r.filter((r) => r.enabled).length);

export async function fetchRules(): Promise<void> {
  rulesLoading.set(true);
  rulesError.set(null);
  try {
    const result = await listRules();
    rules.set(result);
  } catch (e) {
    rulesError.set(String(e));
  } finally {
    rulesLoading.set(false);
  }
}

export function initRuleListeners(): () => void {
  const unlisteners: (() => void)[] = [];

  listen<DomainEventPayload>('syswall://rule-created', (event) => {
    try {
      const rule: RuleMessage = JSON.parse(event.payload.payload_json);
      rules.update((list) => [rule, ...list]);
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://rule-updated', (event) => {
    try {
      const rule: RuleMessage = JSON.parse(event.payload.payload_json);
      rules.update((list) => list.map((r) => (r.id === rule.id ? rule : r)));
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://rule-deleted', (event) => {
    try {
      const payload = JSON.parse(event.payload.payload_json);
      const id = typeof payload === 'string' ? payload : payload.id || payload;
      rules.update((list) => list.filter((r) => r.id !== id));
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  return () => {
    unlisteners.forEach((fn) => fn());
  };
}
```

- [ ] **Step 4: Create decisions store**

`crates/ui/src/lib/stores/decisions.ts`:
```typescript
// Pending decisions store — fed by real-time events.
// Store de décisions en attente — alimenté par les événements en temps réel.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { listPendingDecisions } from '$lib/api/client';
import type { PendingDecisionMessage, DomainEventPayload } from '$lib/types';

export const pendingDecisions = writable<PendingDecisionMessage[]>([]);
export const decisionsError = writable<string | null>(null);
export const decisionsLoading = writable(true);

export const pendingCount = derived(pendingDecisions, ($d) => $d.length);
export const showDecisionOverlay = derived(pendingDecisions, ($d) => $d.length > 0);

// Index of the currently displayed decision in the queue
export const currentDecisionIndex = writable(0);

export const currentDecision = derived(
  [pendingDecisions, currentDecisionIndex],
  ([$decisions, $index]) => {
    if ($decisions.length === 0) return null;
    return $decisions[Math.min($index, $decisions.length - 1)] ?? null;
  }
);

export async function fetchPendingDecisions(): Promise<void> {
  decisionsLoading.set(true);
  decisionsError.set(null);
  try {
    const result = await listPendingDecisions();
    pendingDecisions.set(result);
    currentDecisionIndex.set(0);
  } catch (e) {
    decisionsError.set(String(e));
  } finally {
    decisionsLoading.set(false);
  }
}

export function initDecisionListeners(): () => void {
  const unlisteners: (() => void)[] = [];

  listen<DomainEventPayload>('syswall://decision-required', (event) => {
    try {
      const decision: PendingDecisionMessage = JSON.parse(event.payload.payload_json);
      pendingDecisions.update((list) => [decision, ...list]);
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://decision-resolved', (event) => {
    try {
      const payload = JSON.parse(event.payload.payload_json);
      const id = payload.id || payload.decision_id || payload;
      pendingDecisions.update((list) => list.filter((d) => d.id !== id));
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://decision-expired', (event) => {
    try {
      const payload = JSON.parse(event.payload.payload_json);
      const id = typeof payload === 'string' ? payload : payload.id || payload;
      pendingDecisions.update((list) => list.filter((d) => d.id !== id));
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  return () => {
    unlisteners.forEach((fn) => fn());
  };
}
```

- [ ] **Step 5: Create audit store**

`crates/ui/src/lib/stores/audit.ts`:
```typescript
// Audit events store — populated from domain events.
// Store d'événements d'audit — alimenté par les événements du domaine.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import type { AuditEvent, DomainEventPayload } from '$lib/types';

// Maximum events to keep in memory
const MAX_AUDIT_EVENTS = 5000;

export const auditEvents = writable<AuditEvent[]>([]);

// Filters
export const auditFilters = writable({
  search: '',
  severity: '',
  category: '',
  dateStart: '',
  dateEnd: '',
});

// Pagination
export const auditPage = writable(0);
export const auditPageSize = 50;

export const filteredAuditEvents = derived(
  [auditEvents, auditFilters],
  ([$events, $filters]) => {
    return $events.filter((evt) => {
      if ($filters.search) {
        const q = $filters.search.toLowerCase();
        if (!evt.description.toLowerCase().includes(q)) return false;
      }
      if ($filters.severity && evt.severity !== $filters.severity) return false;
      if ($filters.category && evt.category !== $filters.category) return false;
      if ($filters.dateStart && evt.timestamp < $filters.dateStart) return false;
      if ($filters.dateEnd && evt.timestamp > $filters.dateEnd) return false;
      return true;
    });
  }
);

export const totalFilteredCount = derived(filteredAuditEvents, ($e) => $e.length);
export const totalPages = derived(totalFilteredCount, ($c) =>
  Math.max(1, Math.ceil($c / auditPageSize))
);

export const paginatedAuditEvents = derived(
  [filteredAuditEvents, auditPage],
  ([$events, $page]) => {
    const start = $page * auditPageSize;
    return $events.slice(start, start + auditPageSize);
  }
);

function eventToAudit(eventType: string, payloadJson: string, timestamp: string): AuditEvent | null {
  try {
    const payload = JSON.parse(payloadJson);
    const categoryMap: Record<string, string> = {
      connection_detected: 'connection',
      connection_updated: 'connection',
      connection_closed: 'connection',
      rule_created: 'rule',
      rule_updated: 'rule',
      rule_deleted: 'rule',
      rule_matched: 'rule',
      decision_required: 'decision',
      decision_resolved: 'decision',
      decision_expired: 'decision',
      firewall_status_changed: 'system',
      system_error: 'system',
    };

    const severityMap: Record<string, string> = {
      connection_detected: 'info',
      connection_updated: 'debug',
      connection_closed: 'info',
      rule_created: 'info',
      rule_updated: 'info',
      rule_deleted: 'warning',
      rule_matched: 'debug',
      decision_required: 'warning',
      decision_resolved: 'info',
      decision_expired: 'warning',
      firewall_status_changed: 'info',
      system_error: payload.severity || 'error',
    };

    const description =
      payload.message || payload.description || `${eventType}: ${payloadJson.slice(0, 100)}`;

    return {
      id: crypto.randomUUID(),
      timestamp,
      severity: severityMap[eventType] || 'info',
      category: categoryMap[eventType] || 'system',
      description,
      metadata: typeof payload === 'object' && payload !== null ? payload : {},
    };
  } catch {
    return null;
  }
}

export function initAuditListener(): () => void {
  const eventTypes = [
    'syswall://connection-detected',
    'syswall://connection-closed',
    'syswall://rule-created',
    'syswall://rule-updated',
    'syswall://rule-deleted',
    'syswall://decision-required',
    'syswall://decision-resolved',
    'syswall://decision-expired',
    'syswall://status-changed',
    'syswall://system-error',
  ];

  const unlisteners: (() => void)[] = [];

  for (const eventName of eventTypes) {
    listen<DomainEventPayload>(eventName, (event) => {
      const audit = eventToAudit(
        event.payload.event_type,
        event.payload.payload_json,
        event.payload.timestamp
      );
      if (audit) {
        auditEvents.update((list) => {
          const updated = [audit, ...list];
          if (updated.length > MAX_AUDIT_EVENTS) {
            return updated.slice(0, MAX_AUDIT_EVENTS);
          }
          return updated;
        });
      }
    }).then((fn) => unlisteners.push(fn));
  }

  return () => {
    unlisteners.forEach((fn) => fn());
  };
}
```

- [ ] **Step 6: Create dashboard store**

`crates/ui/src/lib/stores/dashboard.ts`:
```typescript
// Dashboard derived stats — aggregated from other stores.
// Stats dérivées du tableau de bord — agrégées depuis les autres stores.

import { derived, writable } from 'svelte/store';
import { connectionCounts, connections } from './connections';
import { firewallStatus } from './status';
import { auditEvents } from './audit';
import type { ConnectionEvent } from '$lib/types';

// Traffic trend: ring buffer of data points (connections per second)
const TREND_BUFFER_SIZE = 60;
export const trafficTrend = writable<{ allowed: number; blocked: number }[]>(
  Array(TREND_BUFFER_SIZE).fill({ allowed: 0, blocked: 0 })
);

// Periodically sample connection counts for the trend chart
let trendInterval: ReturnType<typeof setInterval> | null = null;

export function startTrafficTrend(): void {
  if (trendInterval) return;

  let prevAllowed = 0;
  let prevBlocked = 0;

  trendInterval = setInterval(() => {
    connectionCounts.subscribe(($c) => {
      const newAllowed = $c.allowed - prevAllowed;
      const newBlocked = $c.blocked - prevBlocked;
      prevAllowed = $c.allowed;
      prevBlocked = $c.blocked;

      trafficTrend.update((buf) => {
        const updated = [...buf.slice(1), { allowed: Math.max(0, newAllowed), blocked: Math.max(0, newBlocked) }];
        return updated;
      });
    })();
  }, 1000);
}

export function stopTrafficTrend(): void {
  if (trendInterval) {
    clearInterval(trendInterval);
    trendInterval = null;
  }
}

// Top applications by connection count
export const topApps = derived(connections, ($conns) => {
  const counts = new Map<string, number>();
  for (const conn of $conns.values()) {
    if (conn.state === 'closed') continue;
    const name = conn.process_name || 'Inconnu';
    counts.set(name, (counts.get(name) || 0) + 1);
  }
  return Array.from(counts.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([name, count]) => ({ name, count }));
});

// Top destinations by IP
export const topDestinations = derived(connections, ($conns) => {
  const counts = new Map<string, number>();
  for (const conn of $conns.values()) {
    if (conn.state === 'closed') continue;
    const ip = conn.destination?.ip || 'Inconnu';
    counts.set(ip, (counts.get(ip) || 0) + 1);
  }
  return Array.from(counts.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([ip, count]) => ({ ip, count }));
});

// Recent alerts (system errors from audit)
export const recentAlerts = derived(auditEvents, ($events) => {
  return $events
    .filter((e) => e.severity === 'error' || e.severity === 'warning' || e.severity === 'critical')
    .slice(0, 5);
});

// Dashboard summary
export const dashboardSummary = derived(
  [connectionCounts, firewallStatus],
  ([$counts, $status]) => ({
    activeConnections: $counts.total,
    allowed: $counts.allowed,
    blocked: $counts.blocked,
    firewallEnabled: $status.enabled,
    version: $status.version,
    uptime: $status.uptime_secs,
    nftablesSynced: $status.nftables_synced,
  })
);
```

---

### Task 5: Design System UI Components

**Files:**
- Create: `crates/ui/src/lib/components/ui/Card.svelte`
- Create: `crates/ui/src/lib/components/ui/Badge.svelte`
- Create: `crates/ui/src/lib/components/ui/Button.svelte`
- Create: `crates/ui/src/lib/components/ui/Input.svelte`
- Create: `crates/ui/src/lib/components/ui/Table.svelte`
- Create: `crates/ui/src/lib/components/ui/Modal.svelte`
- Create: `crates/ui/src/lib/components/ui/StatCard.svelte`
- Create: `crates/ui/src/lib/components/ui/Sidebar.svelte`
- Create: `crates/ui/src/lib/components/ui/EmptyState.svelte`
- Create: `crates/ui/src/lib/components/ui/LoadingSpinner.svelte`
- Create: `crates/ui/src/lib/components/ui/ErrorBanner.svelte`

- [ ] **Step 1: Card.svelte**

`crates/ui/src/lib/components/ui/Card.svelte`:
```svelte
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title?: string;
    padding?: 'sm' | 'md' | 'lg';
    glow?: 'cyan' | 'green' | 'red' | 'none';
    children: Snippet;
  }

  let { title, padding = 'md', glow = 'none', children }: Props = $props();

  const paddingMap = { sm: 'var(--space-3)', md: 'var(--space-4)', lg: 'var(--space-6)' };
  const glowMap: Record<string, string> = {
    cyan: 'var(--glow-cyan)',
    green: 'var(--glow-green)',
    red: 'var(--glow-red)',
    none: 'none',
  };
</script>

<div
  class="card"
  style="padding: {paddingMap[padding]}; box-shadow: {glowMap[glow]};"
>
  {#if title}
    <h3 class="card-title">{title}</h3>
  {/if}
  <div class="card-body">
    {@render children()}
  </div>
</div>

<style>
  .card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    transition: box-shadow var(--transition-base);
  }

  .card-title {
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: var(--space-3);
  }

  .card-body {
    width: 100%;
  }
</style>
```

- [ ] **Step 2: Badge.svelte**

`crates/ui/src/lib/components/ui/Badge.svelte`:
```svelte
<script lang="ts">
  interface Props {
    variant?: 'cyan' | 'green' | 'red' | 'orange' | 'purple' | 'neutral';
    label: string;
    dot?: boolean;
  }

  let { variant = 'neutral', label, dot = false }: Props = $props();

  const colorMap: Record<string, { bg: string; fg: string }> = {
    cyan: { bg: 'var(--accent-cyan-15)', fg: 'var(--accent-cyan)' },
    green: { bg: 'var(--accent-green-15)', fg: 'var(--accent-green)' },
    red: { bg: 'var(--accent-red-15)', fg: 'var(--accent-red)' },
    orange: { bg: 'var(--accent-orange-15)', fg: 'var(--accent-orange)' },
    purple: { bg: 'var(--accent-purple-15)', fg: 'var(--accent-purple)' },
    neutral: { bg: 'rgba(139, 148, 158, 0.15)', fg: 'var(--text-secondary)' },
  };

  const colors = $derived(colorMap[variant] || colorMap.neutral);
</script>

<span
  class="badge"
  style="background: {colors.bg}; color: {colors.fg};"
>
  {#if dot}
    <span class="dot" style="background: {colors.fg};"></span>
  {/if}
  {label}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-full);
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    line-height: 1;
    white-space: nowrap;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }
</style>
```

- [ ] **Step 3: Button.svelte**

`crates/ui/src/lib/components/ui/Button.svelte`:
```svelte
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    variant?: 'primary' | 'success' | 'danger' | 'ghost';
    size?: 'sm' | 'md' | 'lg';
    disabled?: boolean;
    loading?: boolean;
    onclick?: () => void;
    type?: 'button' | 'submit';
    children: Snippet;
  }

  let {
    variant = 'primary',
    size = 'md',
    disabled = false,
    loading = false,
    onclick,
    type = 'button',
    children,
  }: Props = $props();
</script>

<button
  class="btn btn-{variant} btn-{size}"
  {type}
  disabled={disabled || loading}
  {onclick}
>
  {#if loading}
    <span class="spinner"></span>
  {/if}
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    font-family: var(--font-sans);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition: all var(--transition-fast);
    white-space: nowrap;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Sizes */
  .btn-sm { padding: var(--space-1) var(--space-3); font-size: var(--font-size-xs); }
  .btn-md { padding: var(--space-2) var(--space-4); font-size: var(--font-size-sm); }
  .btn-lg { padding: var(--space-3) var(--space-6); font-size: var(--font-size-base); }

  /* Variants */
  .btn-primary {
    background: var(--accent-cyan);
    color: var(--bg-primary);
  }
  .btn-primary:hover:not(:disabled) {
    box-shadow: var(--glow-cyan);
  }

  .btn-success {
    background: var(--accent-green);
    color: var(--bg-primary);
  }
  .btn-success:hover:not(:disabled) {
    box-shadow: var(--glow-green);
  }

  .btn-danger {
    background: var(--accent-red);
    color: var(--bg-primary);
  }
  .btn-danger:hover:not(:disabled) {
    box-shadow: var(--glow-red);
  }

  .btn-ghost {
    background: transparent;
    color: var(--text-primary);
    border-color: var(--border-primary);
  }
  .btn-ghost:hover:not(:disabled) {
    background: var(--bg-hover);
    border-color: var(--accent-cyan);
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid currentColor;
    border-top-color: transparent;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
```

- [ ] **Step 4: Input.svelte**

`crates/ui/src/lib/components/ui/Input.svelte`:
```svelte
<script lang="ts">
  interface Props {
    type?: 'text' | 'number' | 'search' | 'date';
    placeholder?: string;
    value?: string;
    label?: string;
    oninput?: (e: Event) => void;
  }

  let { type = 'text', placeholder = '', value = $bindable(''), label, oninput }: Props = $props();
</script>

<div class="input-group">
  {#if label}
    <label class="input-label">{label}</label>
  {/if}
  <input
    class="input"
    {type}
    {placeholder}
    bind:value
    {oninput}
  />
</div>

<style>
  .input-group {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .input-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .input {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: var(--space-2) var(--space-3);
    color: var(--text-primary);
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    transition: border-color var(--transition-fast);
    outline: none;
    width: 100%;
  }

  .input::placeholder {
    color: var(--text-tertiary);
  }

  .input:focus {
    border-color: var(--accent-cyan);
    box-shadow: 0 0 0 1px var(--accent-cyan);
  }

  .input[type='search'] {
    font-family: var(--font-mono);
  }
</style>
```

- [ ] **Step 5: Table.svelte**

`crates/ui/src/lib/components/ui/Table.svelte`:
```svelte
<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';

  interface Column {
    key: string;
    label: string;
    mono?: boolean;
    width?: string;
  }

  interface Props {
    columns: Column[];
    rows: T[];
    rowHeight?: number;
    maxHeight?: string;
    onrowclick?: (row: T) => void;
    renderCell?: Snippet<[{ row: T; column: Column }]>;
  }

  let {
    columns,
    rows,
    rowHeight = 40,
    maxHeight = '100%',
    onrowclick,
    renderCell,
  }: Props = $props();

  // Virtual scroll state
  let scrollContainer: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let containerHeight = $state(600);

  const BUFFER = 5;
  const totalHeight = $derived(rows.length * rowHeight);
  const startIndex = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - BUFFER));
  const endIndex = $derived(
    Math.min(rows.length, Math.ceil((scrollTop + containerHeight) / rowHeight) + BUFFER)
  );
  const visibleRows = $derived(rows.slice(startIndex, endIndex));
  const offsetY = $derived(startIndex * rowHeight);

  function handleScroll(e: Event) {
    const target = e.target as HTMLDivElement;
    scrollTop = target.scrollTop;
    containerHeight = target.clientHeight;
  }

  function getCellValue(row: T, key: string): string {
    const val = (row as Record<string, unknown>)[key];
    if (val === null || val === undefined) return '--';
    return String(val);
  }
</script>

<div class="table-wrapper" style="max-height: {maxHeight};">
  <div class="table-header">
    <div class="table-row header-row">
      {#each columns as col}
        <div class="table-cell header-cell" style={col.width ? `width: ${col.width}` : ''}>
          {col.label}
        </div>
      {/each}
    </div>
  </div>
  <div
    class="table-body"
    bind:this={scrollContainer}
    onscroll={handleScroll}
  >
    <div class="virtual-spacer" style="height: {totalHeight}px;">
      <div class="virtual-content" style="transform: translateY({offsetY}px);">
        {#each visibleRows as row, i (startIndex + i)}
          <div
            class="table-row body-row"
            style="height: {rowHeight}px;"
            onclick={() => onrowclick?.(row)}
            role="button"
            tabindex="0"
            onkeydown={(e) => e.key === 'Enter' && onrowclick?.(row)}
          >
            {#each columns as col}
              <div
                class="table-cell"
                class:font-mono={col.mono}
                style={col.width ? `width: ${col.width}` : ''}
              >
                {#if renderCell}
                  {@render renderCell({ row, column: col })}
                {:else}
                  {getCellValue(row, col.key)}
                {/if}
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>

<style>
  .table-wrapper {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }

  .table-header {
    flex-shrink: 0;
  }

  .table-body {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .virtual-spacer {
    position: relative;
  }

  .virtual-content {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
  }

  .table-row {
    display: flex;
    align-items: center;
    padding: 0 var(--space-4);
  }

  .header-row {
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
    height: 36px;
  }

  .body-row {
    border-bottom: 1px solid var(--border-subtle);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .body-row:hover {
    background: var(--bg-hover);
  }

  .table-cell {
    flex: 1;
    font-size: var(--font-size-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 var(--space-2);
  }

  .header-cell {
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
</style>
```

- [ ] **Step 6: Modal.svelte**

`crates/ui/src/lib/components/ui/Modal.svelte`:
```svelte
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    open: boolean;
    title: string;
    size?: 'sm' | 'md' | 'lg';
    onclose?: () => void;
    children: Snippet;
    footer?: Snippet;
  }

  let { open, title, size = 'md', onclose, children, footer }: Props = $props();

  const sizeMap = { sm: '400px', md: '560px', lg: '720px' };

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose?.();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose?.();
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="modal-backdrop"
    onclick={handleBackdrop}
    onkeydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-label={title}
  >
    <div class="modal-content" style="max-width: {sizeMap[size]};">
      <div class="modal-header">
        <h2 class="modal-title">{title}</h2>
        <button class="modal-close" onclick={onclose} aria-label="Fermer">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div class="modal-body">
        {@render children()}
      </div>
      {#if footer}
        <div class="modal-footer">
          {@render footer()}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(var(--glass-blur));
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: fadeIn 200ms ease;
  }

  .modal-content {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-xl);
    box-shadow: var(--glow-cyan);
    width: 90%;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    animation: slideUp 300ms ease;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-4) var(--space-6);
    border-bottom: 1px solid var(--border-primary);
  }

  .modal-title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }

  .modal-close {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    padding: var(--space-1);
    border-radius: var(--radius-sm);
    transition: color var(--transition-fast);
  }

  .modal-close:hover {
    color: var(--text-primary);
  }

  .modal-body {
    padding: var(--space-6);
    overflow-y: auto;
    flex: 1;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-6);
    border-top: 1px solid var(--border-primary);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes slideUp {
    from { opacity: 0; transform: translateY(12px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
```

- [ ] **Step 7: StatCard.svelte**

`crates/ui/src/lib/components/ui/StatCard.svelte`:
```svelte
<script lang="ts">
  interface Props {
    label: string;
    value: string | number;
    icon?: string;
    color?: 'cyan' | 'green' | 'red' | 'orange' | 'purple';
  }

  let { label, value, icon, color = 'cyan' }: Props = $props();

  const colorVarMap: Record<string, string> = {
    cyan: 'var(--accent-cyan)',
    green: 'var(--accent-green)',
    red: 'var(--accent-red)',
    orange: 'var(--accent-orange)',
    purple: 'var(--accent-purple)',
  };

  const glowVarMap: Record<string, string> = {
    cyan: 'var(--glow-cyan)',
    green: 'var(--glow-green)',
    red: 'var(--glow-red)',
    orange: 'var(--glow-orange)',
    purple: 'var(--glow-purple)',
  };
</script>

<div class="stat-card" style="box-shadow: {glowVarMap[color]};">
  <div class="stat-content">
    <span class="stat-value" style="color: {colorVarMap[color]};">{value}</span>
    <span class="stat-label">{label}</span>
  </div>
  {#if icon}
    <div class="stat-icon" style="color: {colorVarMap[color]};">
      {@html icon}
    </div>
  {/if}
</div>

<style>
  .stat-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    padding: var(--space-4) var(--space-5);
    display: flex;
    align-items: center;
    justify-content: space-between;
    transition: box-shadow var(--transition-base);
  }

  .stat-card:hover {
    border-color: var(--border-primary);
  }

  .stat-content {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .stat-value {
    font-family: var(--font-mono);
    font-size: var(--font-size-2xl);
    font-weight: var(--font-weight-bold);
    line-height: 1;
  }

  .stat-label {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .stat-icon {
    opacity: 0.6;
    flex-shrink: 0;
  }
</style>
```

- [ ] **Step 8: Sidebar.svelte**

`crates/ui/src/lib/components/ui/Sidebar.svelte`:
```svelte
<script lang="ts">
  import { page } from '$app/stores';
  import Badge from './Badge.svelte';
  import { fr } from '$lib/i18n/fr';

  interface NavItem {
    label: string;
    route: string;
    icon: string;
    badge?: number;
    pulsing?: boolean;
  }

  interface Props {
    firewallEnabled: boolean;
    items: NavItem[];
  }

  let { firewallEnabled, items }: Props = $props();

  const currentPath = $derived($page.url.pathname);
</script>

<nav class="sidebar" aria-label="Navigation principale">
  <div class="sidebar-header">
    <div class="logo">
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--accent-cyan)" stroke-width="2">
        <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
      </svg>
      <span class="logo-text">SysWall</span>
    </div>
    <div class="status-indicator">
      <span class="status-dot" class:active={firewallEnabled}></span>
      <span class="status-text">
        {firewallEnabled ? fr.status_active : fr.status_inactive}
      </span>
    </div>
  </div>

  <div class="nav-items">
    {#each items as item}
      <a
        href={item.route}
        class="nav-item"
        class:active={currentPath === item.route || currentPath.startsWith(item.route + '/')}
        aria-current={currentPath === item.route ? 'page' : undefined}
      >
        <span class="nav-icon">
          {@html item.icon}
        </span>
        <span class="nav-label">{item.label}</span>
        {#if item.badge && item.badge > 0}
          <span class="nav-badge" class:pulsing={item.pulsing}>
            <Badge variant="cyan" label={String(item.badge)} />
          </span>
        {/if}
      </a>
    {/each}
  </div>
</nav>

<style>
  .sidebar {
    width: var(--sidebar-width);
    height: 100vh;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border-primary);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    position: fixed;
    top: 0;
    left: 0;
    z-index: 50;
  }

  .sidebar-header {
    padding: var(--space-5) var(--space-4);
    border-bottom: 1px solid var(--border-primary);
  }

  .logo {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-3);
  }

  .logo-text {
    font-family: var(--font-sans);
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-bold);
    color: var(--accent-cyan);
    letter-spacing: 0.02em;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-red);
    flex-shrink: 0;
  }

  .status-dot.active {
    background: var(--accent-green);
    box-shadow: var(--glow-green);
  }

  .status-text {
    font-size: var(--font-size-xs);
    color: var(--text-secondary);
  }

  .nav-items {
    flex: 1;
    padding: var(--space-3) 0;
    overflow-y: auto;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    color: var(--text-secondary);
    text-decoration: none;
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    border-left: 3px solid transparent;
    transition: all var(--transition-fast);
    margin: var(--space-1) 0;
  }

  .nav-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .nav-item.active {
    border-left-color: var(--accent-cyan);
    background: var(--bg-hover);
    color: var(--accent-cyan);
  }

  .nav-icon {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .nav-label {
    flex: 1;
  }

  .nav-badge {
    flex-shrink: 0;
  }

  .nav-badge.pulsing {
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }
</style>
```

- [ ] **Step 9: EmptyState.svelte**

`crates/ui/src/lib/components/ui/EmptyState.svelte`:
```svelte
<script lang="ts">
  interface Props {
    message: string;
    icon?: string;
  }

  let { message, icon }: Props = $props();
</script>

<div class="empty-state">
  {#if icon}
    <div class="empty-icon">
      {@html icon}
    </div>
  {/if}
  <p class="empty-message">{message}</p>
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--space-12) var(--space-8);
    text-align: center;
  }

  .empty-icon {
    color: var(--text-tertiary);
    margin-bottom: var(--space-4);
    opacity: 0.5;
  }

  .empty-message {
    color: var(--text-secondary);
    font-size: var(--font-size-sm);
  }
</style>
```

- [ ] **Step 10: LoadingSpinner.svelte**

`crates/ui/src/lib/components/ui/LoadingSpinner.svelte`:
```svelte
<script lang="ts">
  import { fr } from '$lib/i18n/fr';

  interface Props {
    size?: 'sm' | 'md' | 'lg';
  }

  let { size = 'md' }: Props = $props();

  const sizeMap = { sm: '16px', md: '32px', lg: '48px' };
</script>

<div class="spinner-container">
  <div
    class="spinner"
    style="width: {sizeMap[size]}; height: {sizeMap[size]};"
    role="status"
    aria-label={fr.common_loading}
  ></div>
</div>

<style>
  .spinner-container {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-8);
  }

  .spinner {
    border: 3px solid var(--border-primary);
    border-top-color: var(--accent-cyan);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
```

- [ ] **Step 11: ErrorBanner.svelte**

`crates/ui/src/lib/components/ui/ErrorBanner.svelte`:
```svelte
<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import Button from './Button.svelte';

  interface Props {
    message: string;
    onretry?: () => void;
  }

  let { message, onretry }: Props = $props();
</script>

<div class="error-banner" role="alert">
  <div class="error-content">
    <svg class="error-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="12" cy="12" r="10" />
      <line x1="12" y1="8" x2="12" y2="12" />
      <line x1="12" y1="16" x2="12.01" y2="16" />
    </svg>
    <span class="error-message">{message}</span>
  </div>
  {#if onretry}
    <Button variant="ghost" size="sm" onclick={onretry}>
      {fr.common_retry}
    </Button>
  {/if}
</div>

<style>
  .error-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-4);
    background: var(--accent-red-15);
    border: 1px solid var(--accent-red);
    border-radius: var(--radius-md);
    gap: var(--space-3);
  }

  .error-content {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .error-icon {
    color: var(--accent-red);
    flex-shrink: 0;
  }

  .error-message {
    color: var(--accent-red);
    font-size: var(--font-size-sm);
  }
</style>
```

---

### Task 6: Layout + Navigation

**Files:**
- Rewrite: `crates/ui/src/routes/+layout.svelte`
- Keep: `crates/ui/src/routes/+layout.ts`
- Rewrite: `crates/ui/src/routes/+page.svelte`

- [ ] **Step 1: Create the main layout with sidebar and event initialization**

`crates/ui/src/routes/+layout.svelte`:
```svelte
<script lang="ts">
  import '../app.css';
  import Sidebar from '$lib/components/ui/Sidebar.svelte';
  import ErrorBanner from '$lib/components/ui/ErrorBanner.svelte';
  import { fr } from '$lib/i18n/fr';
  import { firewallStatus, fetchStatus, initStatusListener, statusError } from '$lib/stores/status';
  import { initConnectionListeners, connectionCounts } from '$lib/stores/connections';
  import { fetchRules, initRuleListeners, rulesCount } from '$lib/stores/rules';
  import { fetchPendingDecisions, initDecisionListeners, pendingCount } from '$lib/stores/decisions';
  import { initAuditListener } from '$lib/stores/audit';
  import { startTrafficTrend, stopTrafficTrend } from '$lib/stores/dashboard';
  import { onMount } from 'svelte';
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();

  // SVG icons for sidebar navigation
  const icons = {
    grid: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>',
    activity: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>',
    shield: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>',
    brain: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2a7 7 0 0 0-7 7c0 3 2 5.5 4 7l3 3 3-3c2-1.5 4-4 4-7a7 7 0 0 0-7-7z"/><circle cx="12" cy="10" r="2"/></svg>',
    scroll: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6z"/><polyline points="14 2 14 8 20 8"/><line x1="8" y1="13" x2="16" y2="13"/><line x1="8" y1="17" x2="16" y2="17"/></svg>',
    settings: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>',
  };

  const navItems = $derived([
    { label: fr.nav_dashboard, route: '/dashboard', icon: icons.grid },
    { label: fr.nav_connections, route: '/connections', icon: icons.activity, badge: $connectionCounts.total },
    { label: fr.nav_rules, route: '/rules', icon: icons.shield, badge: $rulesCount },
    { label: fr.nav_learning, route: '/learning', icon: icons.brain, badge: $pendingCount, pulsing: $pendingCount > 0 },
    { label: fr.nav_audit, route: '/audit', icon: icons.scroll },
    { label: fr.nav_settings, route: '/settings', icon: icons.settings },
  ]);

  onMount(() => {
    // Fetch initial data
    fetchStatus();
    fetchRules();
    fetchPendingDecisions();

    // Subscribe to real-time events
    const unStatus = initStatusListener();
    const unConnections = initConnectionListeners();
    const unRules = initRuleListeners();
    const unDecisions = initDecisionListeners();
    const unAudit = initAuditListener();
    startTrafficTrend();

    return () => {
      unStatus();
      unConnections();
      unRules();
      unDecisions();
      unAudit();
      stopTrafficTrend();
    };
  });
</script>

<div class="app-layout">
  <Sidebar firewallEnabled={$firewallStatus.enabled} items={navItems} />

  <main class="content">
    {#if $statusError}
      <ErrorBanner message={fr.common_connection_error} onretry={fetchStatus} />
    {/if}
    {@render children()}
  </main>
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }

  .content {
    margin-left: var(--sidebar-width);
    flex: 1;
    padding: var(--space-8);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }
</style>
```

- [ ] **Step 2: Update root page to redirect to dashboard**

`crates/ui/src/routes/+page.svelte`:
```svelte
<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  onMount(() => {
    goto('/dashboard', { replaceState: true });
  });
</script>
```

---

### Task 7: Dashboard View

**Files:**
- Create: `crates/ui/src/routes/dashboard/+page.svelte`
- Create: `crates/ui/src/lib/components/dashboard/TrafficChart.svelte`
- Create: `crates/ui/src/lib/components/dashboard/TopApps.svelte`
- Create: `crates/ui/src/lib/components/dashboard/TopDestinations.svelte`
- Create: `crates/ui/src/lib/components/dashboard/RecentAlerts.svelte`

- [ ] **Step 1: Create TrafficChart.svelte (SVG time series)**

`crates/ui/src/lib/components/dashboard/TrafficChart.svelte`:
```svelte
<script lang="ts">
  interface Props {
    data: { allowed: number; blocked: number }[];
  }

  let { data }: Props = $props();

  const width = 500;
  const height = 120;
  const padding = { top: 10, right: 10, bottom: 10, left: 10 };

  const chartW = $derived(width - padding.left - padding.right);
  const chartH = $derived(height - padding.top - padding.bottom);

  const maxVal = $derived(Math.max(1, ...data.map((d) => Math.max(d.allowed, d.blocked))));

  function toPath(values: number[]): string {
    if (values.length === 0) return '';
    const stepX = chartW / Math.max(1, values.length - 1);
    return values
      .map((v, i) => {
        const x = padding.left + i * stepX;
        const y = padding.top + chartH - (v / maxVal) * chartH;
        return `${i === 0 ? 'M' : 'L'}${x},${y}`;
      })
      .join(' ');
  }

  const allowedPath = $derived(toPath(data.map((d) => d.allowed)));
  const blockedPath = $derived(toPath(data.map((d) => d.blocked)));
</script>

<svg viewBox="0 0 {width} {height}" class="traffic-chart" aria-label="Tendance du trafic">
  <!-- Grid lines -->
  {#each [0.25, 0.5, 0.75] as pct}
    <line
      x1={padding.left}
      y1={padding.top + chartH * (1 - pct)}
      x2={padding.left + chartW}
      y2={padding.top + chartH * (1 - pct)}
      stroke="var(--border-subtle)"
      stroke-width="0.5"
    />
  {/each}

  <!-- Allowed line (cyan) -->
  {#if allowedPath}
    <path d={allowedPath} fill="none" stroke="var(--accent-cyan)" stroke-width="2" opacity="0.8" />
  {/if}

  <!-- Blocked line (red) -->
  {#if blockedPath}
    <path d={blockedPath} fill="none" stroke="var(--accent-red)" stroke-width="2" opacity="0.8" />
  {/if}

  <!-- Legend -->
  <circle cx={padding.left + 8} cy={height - 4} r="3" fill="var(--accent-cyan)" />
  <text x={padding.left + 16} y={height - 1} fill="var(--text-secondary)" font-size="8" font-family="var(--font-sans)">Autorisé</text>
  <circle cx={padding.left + 80} cy={height - 4} r="3" fill="var(--accent-red)" />
  <text x={padding.left + 88} y={height - 1} fill="var(--text-secondary)" font-size="8" font-family="var(--font-sans)">Bloqué</text>
</svg>

<style>
  .traffic-chart {
    width: 100%;
    height: auto;
  }
</style>
```

- [ ] **Step 2: Create TopApps.svelte**

`crates/ui/src/lib/components/dashboard/TopApps.svelte`:
```svelte
<script lang="ts">
  import Badge from '$lib/components/ui/Badge.svelte';

  interface Props {
    apps: { name: string; count: number }[];
  }

  let { apps }: Props = $props();
</script>

<div class="top-list">
  {#each apps as app, i}
    <div class="top-item">
      <span class="rank">{i + 1}</span>
      <span class="name truncate">{app.name}</span>
      <Badge variant="cyan" label={String(app.count)} />
    </div>
  {:else}
    <p class="empty">--</p>
  {/each}
</div>

<style>
  .top-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .top-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .top-item:last-child {
    border-bottom: none;
  }

  .rank {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text-tertiary);
    width: 20px;
    text-align: center;
  }

  .name {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    color: var(--text-primary);
  }

  .empty {
    color: var(--text-tertiary);
    font-size: var(--font-size-sm);
    text-align: center;
    padding: var(--space-4);
  }
</style>
```

- [ ] **Step 3: Create TopDestinations.svelte**

`crates/ui/src/lib/components/dashboard/TopDestinations.svelte`:
```svelte
<script lang="ts">
  import Badge from '$lib/components/ui/Badge.svelte';

  interface Props {
    destinations: { ip: string; count: number }[];
  }

  let { destinations }: Props = $props();
</script>

<div class="top-list">
  {#each destinations as dest, i}
    <div class="top-item">
      <span class="rank">{i + 1}</span>
      <span class="ip font-mono truncate">{dest.ip}</span>
      <Badge variant="purple" label={String(dest.count)} />
    </div>
  {:else}
    <p class="empty">--</p>
  {/each}
</div>

<style>
  .top-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .top-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .top-item:last-child {
    border-bottom: none;
  }

  .rank {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text-tertiary);
    width: 20px;
    text-align: center;
  }

  .ip {
    flex: 1;
    font-size: var(--font-size-sm);
    color: var(--text-primary);
  }

  .empty {
    color: var(--text-tertiary);
    font-size: var(--font-size-sm);
    text-align: center;
    padding: var(--space-4);
  }
</style>
```

- [ ] **Step 4: Create RecentAlerts.svelte**

`crates/ui/src/lib/components/dashboard/RecentAlerts.svelte`:
```svelte
<script lang="ts">
  import Badge from '$lib/components/ui/Badge.svelte';
  import type { AuditEvent } from '$lib/types';

  interface Props {
    alerts: AuditEvent[];
  }

  let { alerts }: Props = $props();

  function severityVariant(severity: string): 'red' | 'orange' | 'cyan' {
    if (severity === 'error' || severity === 'critical') return 'red';
    if (severity === 'warning') return 'orange';
    return 'cyan';
  }

  function formatTime(ts: string): string {
    try {
      return new Date(ts).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    } catch {
      return ts;
    }
  }
</script>

<div class="alerts-list">
  {#each alerts as alert}
    <div class="alert-item">
      <span class="alert-time font-mono">{formatTime(alert.timestamp)}</span>
      <Badge variant={severityVariant(alert.severity)} label={alert.severity} />
      <span class="alert-msg truncate">{alert.description}</span>
    </div>
  {:else}
    <p class="empty">--</p>
  {/each}
</div>

<style>
  .alerts-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .alert-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .alert-item:last-child {
    border-bottom: none;
  }

  .alert-time {
    font-size: var(--font-size-xs);
    color: var(--text-tertiary);
    flex-shrink: 0;
  }

  .alert-msg {
    flex: 1;
    font-size: var(--font-size-sm);
    color: var(--text-primary);
  }

  .empty {
    color: var(--text-tertiary);
    font-size: var(--font-size-sm);
    text-align: center;
    padding: var(--space-4);
  }
</style>
```

- [ ] **Step 5: Create Dashboard page**

`crates/ui/src/routes/dashboard/+page.svelte`:
```svelte
<script lang="ts">
  import StatCard from '$lib/components/ui/StatCard.svelte';
  import Card from '$lib/components/ui/Card.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';
  import TrafficChart from '$lib/components/dashboard/TrafficChart.svelte';
  import TopApps from '$lib/components/dashboard/TopApps.svelte';
  import TopDestinations from '$lib/components/dashboard/TopDestinations.svelte';
  import RecentAlerts from '$lib/components/dashboard/RecentAlerts.svelte';
  import { fr } from '$lib/i18n/fr';
  import { dashboardSummary, topApps, topDestinations, recentAlerts, trafficTrend } from '$lib/stores/dashboard';
  import { firewallStatus } from '$lib/stores/status';

  function formatUptime(secs: number): string {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    return `${h}h ${m}m ${s}s`;
  }
</script>

<div class="dashboard">
  <h1 class="page-title">{fr.nav_dashboard}</h1>

  <!-- Stat cards row -->
  <div class="stats-row">
    <StatCard
      label={fr.dash_active_connections}
      value={$dashboardSummary.activeConnections}
      color="cyan"
      icon='<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>'
    />
    <StatCard
      label={fr.dash_allowed}
      value={$dashboardSummary.allowed}
      color="green"
      icon='<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>'
    />
    <StatCard
      label={fr.dash_blocked}
      value={$dashboardSummary.blocked}
      color="red"
      icon='<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/></svg>'
    />
    <StatCard
      label={fr.dash_alerts}
      value={$recentAlerts.length}
      color="orange"
      icon='<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>'
    />
  </div>

  <!-- Main grid -->
  <div class="dashboard-grid">
    <!-- Firewall status -->
    <Card title={fr.dash_firewall_status}>
      <div class="status-details">
        <div class="status-row">
          <span class="label">{fr.settings_status}</span>
          <Badge
            variant={$firewallStatus.enabled ? 'green' : 'red'}
            label={$firewallStatus.enabled ? fr.status_active : fr.status_inactive}
            dot
          />
        </div>
        <div class="status-row">
          <span class="label">{fr.dash_version}</span>
          <span class="value font-mono">{$firewallStatus.version || '--'}</span>
        </div>
        <div class="status-row">
          <span class="label">{fr.dash_uptime}</span>
          <span class="value font-mono">{formatUptime($firewallStatus.uptime_secs)}</span>
        </div>
        <div class="status-row">
          <span class="label">{fr.dash_nftables}</span>
          <Badge
            variant={$firewallStatus.nftables_synced ? 'green' : 'orange'}
            label={$firewallStatus.nftables_synced ? fr.status_synced : fr.status_not_synced}
          />
        </div>
      </div>
    </Card>

    <!-- Traffic trend -->
    <Card title={fr.dash_traffic_trend}>
      <TrafficChart data={$trafficTrend} />
    </Card>

    <!-- Top apps -->
    <Card title={fr.dash_top_apps}>
      <TopApps apps={$topApps} />
    </Card>

    <!-- Top destinations -->
    <Card title={fr.dash_top_destinations}>
      <TopDestinations destinations={$topDestinations} />
    </Card>

    <!-- Recent alerts -->
    <Card title={fr.dash_recent_alerts}>
      <RecentAlerts alerts={$recentAlerts} />
    </Card>
  </div>
</div>

<style>
  .dashboard {
    animation: fadeIn 300ms ease;
  }

  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-bold);
    color: var(--text-primary);
    margin-bottom: var(--space-2);
  }

  .stats-row {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--space-4);
    margin-bottom: var(--space-6);
  }

  .dashboard-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-4);
  }

  .status-details {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .status-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .label {
    color: var(--text-secondary);
    font-size: var(--font-size-sm);
  }

  .value {
    color: var(--text-primary);
    font-size: var(--font-size-sm);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @media (max-width: 1200px) {
    .stats-row { grid-template-columns: repeat(2, 1fr); }
    .dashboard-grid { grid-template-columns: repeat(2, 1fr); }
  }
</style>
```

---

### Task 8: Connections View

**Files:**
- Create: `crates/ui/src/lib/components/connections/ConnectionFilters.svelte`
- Create: `crates/ui/src/lib/components/connections/ConnectionDetail.svelte`
- Create: `crates/ui/src/lib/components/connections/ConnectionTable.svelte`
- Create: `crates/ui/src/routes/connections/+page.svelte`

- [ ] **Step 1: ConnectionFilters.svelte**

`crates/ui/src/lib/components/connections/ConnectionFilters.svelte`:
```svelte
<script lang="ts">
  import Input from '$lib/components/ui/Input.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { fr } from '$lib/i18n/fr';
  import { connectionFilters } from '$lib/stores/connections';

  function clearFilters() {
    connectionFilters.set({ search: '', protocol: '', verdict: '', direction: '' });
  }

  let filters = $state({ search: '', protocol: '', verdict: '', direction: '' });

  // Debounced search
  let searchTimeout: ReturnType<typeof setTimeout>;

  function onSearchInput(e: Event) {
    const val = (e.target as HTMLInputElement).value;
    filters.search = val;
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      connectionFilters.update((f) => ({ ...f, search: val }));
    }, 300);
  }

  function onFilterChange(key: 'protocol' | 'verdict' | 'direction', val: string) {
    filters[key] = val;
    connectionFilters.update((f) => ({ ...f, [key]: val }));
  }
</script>

<div class="filters-bar">
  <div class="filter-group">
    <Input
      type="search"
      placeholder={fr.conn_search}
      value={filters.search}
      oninput={onSearchInput}
    />
  </div>

  <div class="filter-group">
    <select
      class="filter-select"
      value={filters.protocol}
      onchange={(e) => onFilterChange('protocol', (e.target as HTMLSelectElement).value)}
    >
      <option value="">{fr.conn_filter_all}</option>
      <option value="tcp">TCP</option>
      <option value="udp">UDP</option>
      <option value="icmp">ICMP</option>
    </select>
  </div>

  <div class="filter-group">
    <select
      class="filter-select"
      value={filters.verdict}
      onchange={(e) => onFilterChange('verdict', (e.target as HTMLSelectElement).value)}
    >
      <option value="">{fr.conn_filter_all}</option>
      <option value="allowed">{fr.conn_allowed}</option>
      <option value="blocked">{fr.conn_blocked}</option>
      <option value="pending_decision">{fr.conn_pending}</option>
    </select>
  </div>

  <div class="filter-group">
    <select
      class="filter-select"
      value={filters.direction}
      onchange={(e) => onFilterChange('direction', (e.target as HTMLSelectElement).value)}
    >
      <option value="">{fr.conn_filter_all}</option>
      <option value="outbound">{fr.conn_outbound}</option>
      <option value="inbound">{fr.conn_inbound}</option>
    </select>
  </div>

  <Button variant="ghost" size="sm" onclick={clearFilters}>
    {fr.conn_clear_filters}
  </Button>
</div>

<style>
  .filters-bar {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-wrap: wrap;
  }

  .filter-group {
    flex-shrink: 0;
  }

  .filter-group:first-child {
    flex: 1;
    min-width: 200px;
  }

  .filter-select {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: var(--space-2) var(--space-3);
    color: var(--text-primary);
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    outline: none;
    cursor: pointer;
  }

  .filter-select:focus {
    border-color: var(--accent-cyan);
  }
</style>
```

- [ ] **Step 2: ConnectionDetail.svelte**

`crates/ui/src/lib/components/connections/ConnectionDetail.svelte`:
```svelte
<script lang="ts">
  import Badge from '$lib/components/ui/Badge.svelte';
  import { fr } from '$lib/i18n/fr';
  import type { ConnectionEvent } from '$lib/types';

  interface Props {
    connection: ConnectionEvent;
  }

  let { connection }: Props = $props();
</script>

<div class="detail-panel">
  <div class="detail-grid">
    <div class="detail-item">
      <span class="detail-label">{fr.conn_application}</span>
      <span class="detail-value font-mono">{connection.process_path || connection.process_name || fr.conn_unknown}</span>
    </div>
    <div class="detail-item">
      <span class="detail-label">{fr.conn_pid}</span>
      <span class="detail-value font-mono">{connection.pid ?? '--'}</span>
    </div>
    <div class="detail-item">
      <span class="detail-label">{fr.conn_bytes_sent}</span>
      <span class="detail-value font-mono">{connection.bytes_sent.toLocaleString('fr-FR')}</span>
    </div>
    <div class="detail-item">
      <span class="detail-label">{fr.conn_bytes_received}</span>
      <span class="detail-value font-mono">{connection.bytes_received.toLocaleString('fr-FR')}</span>
    </div>
    <div class="detail-item">
      <span class="detail-label">{fr.conn_started_at}</span>
      <span class="detail-value font-mono">{new Date(connection.started_at).toLocaleString('fr-FR')}</span>
    </div>
    <div class="detail-item">
      <span class="detail-label">{fr.conn_rule}</span>
      <span class="detail-value font-mono">{connection.matched_rule || '--'}</span>
    </div>
    <div class="detail-item">
      <span class="detail-label">{fr.conn_connection_id}</span>
      <span class="detail-value font-mono text-tertiary">{connection.id}</span>
    </div>
  </div>
</div>

<style>
  .detail-panel {
    padding: var(--space-4) var(--space-6);
    background: var(--bg-tertiary);
    border-top: 1px solid var(--border-subtle);
    animation: slideUp 200ms ease;
  }

  .detail-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-3) var(--space-6);
  }

  .detail-item {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .detail-label {
    font-size: var(--font-size-xs);
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .detail-value {
    font-size: var(--font-size-sm);
    color: var(--text-primary);
  }

  @keyframes slideUp {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
```

- [ ] **Step 3: ConnectionTable.svelte**

`crates/ui/src/lib/components/connections/ConnectionTable.svelte`:
```svelte
<script lang="ts">
  import Badge from '$lib/components/ui/Badge.svelte';
  import ConnectionDetail from './ConnectionDetail.svelte';
  import { fr } from '$lib/i18n/fr';
  import type { ConnectionEvent } from '$lib/types';

  interface Props {
    connections: ConnectionEvent[];
  }

  let { connections }: Props = $props();

  let expandedId = $state<string | null>(null);

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }

  function verdictVariant(verdict: string): 'green' | 'red' | 'orange' | 'neutral' {
    if (verdict === 'allowed') return 'green';
    if (verdict === 'blocked') return 'red';
    if (verdict === 'pending_decision') return 'orange';
    return 'neutral';
  }

  function verdictLabel(verdict: string): string {
    if (verdict === 'allowed') return fr.conn_allowed;
    if (verdict === 'blocked') return fr.conn_blocked;
    if (verdict === 'pending_decision') return fr.conn_pending;
    return verdict;
  }

  function stateVariant(state: string): 'green' | 'cyan' | 'orange' | 'neutral' {
    if (state === 'established') return 'green';
    if (state === 'new') return 'cyan';
    if (state === 'closing') return 'orange';
    return 'neutral';
  }

  // Virtual scroll
  let scrollContainer: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let containerHeight = $state(600);
  const ROW_HEIGHT = 40;
  const BUFFER = 5;

  const totalHeight = $derived(connections.length * ROW_HEIGHT);
  const startIdx = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - BUFFER));
  const endIdx = $derived(Math.min(connections.length, Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT) + BUFFER));
  const visible = $derived(connections.slice(startIdx, endIdx));
  const offsetY = $derived(startIdx * ROW_HEIGHT);

  function onScroll(e: Event) {
    const el = e.target as HTMLDivElement;
    scrollTop = el.scrollTop;
    containerHeight = el.clientHeight;
  }
</script>

<div class="conn-table">
  <!-- Header -->
  <div class="table-header">
    <span class="col col-app">{fr.conn_application}</span>
    <span class="col col-pid">{fr.conn_pid}</span>
    <span class="col col-user">{fr.conn_user}</span>
    <span class="col col-local">{fr.conn_local_addr}</span>
    <span class="col col-remote">{fr.conn_remote_addr}</span>
    <span class="col col-proto">{fr.conn_protocol}</span>
    <span class="col col-state">{fr.conn_state}</span>
    <span class="col col-verdict">{fr.conn_verdict}</span>
    <span class="col col-rule">{fr.conn_rule}</span>
  </div>

  <!-- Body with virtual scroll -->
  <div class="table-body" bind:this={scrollContainer} onscroll={onScroll}>
    <div style="height: {totalHeight}px; position: relative;">
      <div style="transform: translateY({offsetY}px); position: absolute; left: 0; right: 0;">
        {#each visible as conn (conn.id)}
          <div
            class="table-row"
            class:expanded={expandedId === conn.id}
            onclick={() => toggleExpand(conn.id)}
            role="button"
            tabindex="0"
            onkeydown={(e) => e.key === 'Enter' && toggleExpand(conn.id)}
          >
            <span class="col col-app truncate">{conn.process_name || fr.conn_unknown}</span>
            <span class="col col-pid font-mono">{conn.pid ?? '--'}</span>
            <span class="col col-user truncate">{conn.user || '--'}</span>
            <span class="col col-local font-mono truncate">{conn.source?.ip}:{conn.source?.port}</span>
            <span class="col col-remote font-mono truncate">{conn.destination?.ip}:{conn.destination?.port}</span>
            <span class="col col-proto"><Badge variant="cyan" label={conn.protocol.toUpperCase()} /></span>
            <span class="col col-state"><Badge variant={stateVariant(conn.state)} label={conn.state} /></span>
            <span class="col col-verdict"><Badge variant={verdictVariant(conn.verdict)} label={verdictLabel(conn.verdict)} /></span>
            <span class="col col-rule truncate font-mono">{conn.matched_rule || '--'}</span>
          </div>
          {#if expandedId === conn.id}
            <ConnectionDetail connection={conn} />
          {/if}
        {/each}
      </div>
    </div>
  </div>
</div>

<style>
  .conn-table {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    overflow: hidden;
    flex: 1;
  }

  .table-header {
    display: flex;
    align-items: center;
    padding: 0 var(--space-4);
    height: 36px;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
    flex-shrink: 0;
  }

  .table-header .col {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .table-body {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .table-row {
    display: flex;
    align-items: center;
    padding: 0 var(--space-4);
    height: 40px;
    border-bottom: 1px solid var(--border-subtle);
    cursor: pointer;
    transition: background var(--transition-fast);
    font-size: var(--font-size-sm);
  }

  .table-row:hover {
    background: var(--bg-hover);
  }

  .table-row.expanded {
    background: var(--bg-hover);
  }

  .col { padding: 0 var(--space-1); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-app { flex: 1.5; }
  .col-pid { flex: 0.6; }
  .col-user { flex: 0.8; }
  .col-local { flex: 1.5; }
  .col-remote { flex: 1.5; }
  .col-proto { flex: 0.6; }
  .col-state { flex: 0.8; }
  .col-verdict { flex: 0.8; }
  .col-rule { flex: 1; }
</style>
```

- [ ] **Step 4: Connections page**

`crates/ui/src/routes/connections/+page.svelte`:
```svelte
<script lang="ts">
  import ConnectionFilters from '$lib/components/connections/ConnectionFilters.svelte';
  import ConnectionTable from '$lib/components/connections/ConnectionTable.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import { fr } from '$lib/i18n/fr';
  import { filteredConnections, connectionCounts } from '$lib/stores/connections';
</script>

<div class="connections-page">
  <div class="page-header">
    <h1 class="page-title">{fr.nav_connections}</h1>
    <span class="count font-mono">{$connectionCounts.total} actives</span>
  </div>

  <ConnectionFilters />

  {#if $filteredConnections.length > 0}
    <ConnectionTable connections={$filteredConnections} />
  {:else}
    <EmptyState message={fr.conn_no_connections} />
  {/if}
</div>

<style>
  .connections-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    height: 100%;
    animation: fadeIn 300ms ease;
  }

  .page-header {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
  }

  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-bold);
    color: var(--text-primary);
  }

  .count {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
</style>
```

---

### Task 9: Rules View

**Files:**
- Create: `crates/ui/src/lib/components/rules/RuleList.svelte`
- Create: `crates/ui/src/lib/components/rules/RuleForm.svelte`
- Create: `crates/ui/src/lib/components/rules/RuleCriteriaBuilder.svelte`
- Create: `crates/ui/src/lib/components/rules/DeleteConfirmModal.svelte`
- Create: `crates/ui/src/routes/rules/+page.svelte`

- [ ] **Step 1: RuleCriteriaBuilder.svelte**

`crates/ui/src/lib/components/rules/RuleCriteriaBuilder.svelte`:
```svelte
<script lang="ts">
  import Button from '$lib/components/ui/Button.svelte';
  import Input from '$lib/components/ui/Input.svelte';
  import { fr } from '$lib/i18n/fr';

  interface Criterion {
    field: string;
    value: string;
  }

  interface Props {
    criteria: Criterion[];
    onchange: (criteria: Criterion[]) => void;
  }

  let { criteria, onchange }: Props = $props();

  const fields = [
    { value: 'application', label: fr.criteria_application },
    { value: 'user', label: fr.criteria_user },
    { value: 'remote_ip', label: fr.criteria_remote_ip },
    { value: 'remote_port', label: fr.criteria_remote_port },
    { value: 'local_port', label: fr.criteria_local_port },
    { value: 'protocol', label: fr.criteria_protocol },
    { value: 'direction', label: fr.criteria_direction },
  ];

  function addCriterion() {
    onchange([...criteria, { field: 'application', value: '' }]);
  }

  function removeCriterion(index: number) {
    onchange(criteria.filter((_, i) => i !== index));
  }

  function updateField(index: number, field: string) {
    const updated = [...criteria];
    updated[index] = { ...updated[index], field };
    onchange(updated);
  }

  function updateValue(index: number, value: string) {
    const updated = [...criteria];
    updated[index] = { ...updated[index], value };
    onchange(updated);
  }
</script>

<div class="criteria-builder">
  {#each criteria as criterion, i}
    <div class="criterion-row">
      <select
        class="field-select"
        value={criterion.field}
        onchange={(e) => updateField(i, (e.target as HTMLSelectElement).value)}
      >
        {#each fields as f}
          <option value={f.value}>{f.label}</option>
        {/each}
      </select>

      <div class="value-input">
        <Input
          placeholder="Valeur..."
          value={criterion.value}
          oninput={(e) => updateValue(i, (e.target as HTMLInputElement).value)}
        />
      </div>

      <Button variant="ghost" size="sm" onclick={() => removeCriterion(i)}>
        {fr.criteria_remove}
      </Button>
    </div>
  {/each}

  <Button variant="ghost" size="sm" onclick={addCriterion}>
    + {fr.criteria_add}
  </Button>
</div>

<style>
  .criteria-builder {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .criterion-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .field-select {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: var(--space-2) var(--space-3);
    color: var(--text-primary);
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    outline: none;
    min-width: 160px;
  }

  .field-select:focus {
    border-color: var(--accent-cyan);
  }

  .value-input {
    flex: 1;
  }
</style>
```

- [ ] **Step 2: RuleForm.svelte**

`crates/ui/src/lib/components/rules/RuleForm.svelte`:
```svelte
<script lang="ts">
  import Modal from '$lib/components/ui/Modal.svelte';
  import Input from '$lib/components/ui/Input.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import RuleCriteriaBuilder from './RuleCriteriaBuilder.svelte';
  import { fr } from '$lib/i18n/fr';
  import { createRule } from '$lib/api/client';
  import { fetchRules } from '$lib/stores/rules';
  import type { RuleMessage } from '$lib/types';

  interface Props {
    open: boolean;
    editRule?: RuleMessage | null;
    onclose: () => void;
  }

  let { open, editRule = null, onclose }: Props = $props();

  let name = $state('');
  let priority = $state(100);
  let effect = $state('allow');
  let source = $state('manual');
  let scopeType = $state('permanent');
  let criteria = $state<{ field: string; value: string }[]>([]);
  let saving = $state(false);
  let error = $state('');

  // Reset form when opening
  $effect(() => {
    if (open) {
      if (editRule) {
        name = editRule.name;
        priority = editRule.priority;
        effect = editRule.effect;
        source = editRule.source;
        try {
          const parsed = JSON.parse(editRule.criteria_json);
          criteria = Object.entries(parsed)
            .filter(([_, v]) => v != null)
            .map(([field, v]) => ({
              field,
              value: typeof v === 'object' ? JSON.stringify(v) : String(v),
            }));
        } catch {
          criteria = [];
        }
      } else {
        name = '';
        priority = 100;
        effect = 'allow';
        source = 'manual';
        scopeType = 'permanent';
        criteria = [];
      }
      error = '';
    }
  });

  function buildCriteriaJson(): string {
    const obj: Record<string, unknown> = {};
    for (const c of criteria) {
      if (c.value) {
        // Try to parse JSON values, otherwise use as string
        try {
          obj[c.field] = JSON.parse(c.value);
        } catch {
          obj[c.field] = c.value;
        }
      }
    }
    return JSON.stringify(obj);
  }

  async function handleSubmit() {
    if (!name.trim()) {
      error = 'Le nom est requis';
      return;
    }

    saving = true;
    error = '';

    try {
      const scopeJson = scopeType === 'permanent'
        ? JSON.stringify({ type: 'permanent' })
        : JSON.stringify({ type: 'temporary' });

      await createRule({
        name: name.trim(),
        priority,
        criteria_json: buildCriteriaJson(),
        effect,
        scope_json: scopeJson,
        source,
      });

      await fetchRules();
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<Modal
  {open}
  title={editRule ? fr.rules_edit : fr.rules_new}
  size="lg"
  onclose={onclose}
>
  <form class="rule-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
    {#if error}
      <div class="form-error">{error}</div>
    {/if}

    <div class="form-row">
      <Input label={fr.rules_name} bind:value={name} placeholder="Nom de la regle..." />
    </div>

    <div class="form-row-grid">
      <Input label={fr.rules_priority} type="number" bind:value={priority as any} />

      <div class="form-field">
        <label class="field-label">{fr.rules_effect}</label>
        <select class="field-select" bind:value={effect}>
          <option value="allow">{fr.rules_allow}</option>
          <option value="block">{fr.rules_block}</option>
          <option value="ask">{fr.rules_ask}</option>
          <option value="observe">{fr.rules_observe}</option>
        </select>
      </div>

      <div class="form-field">
        <label class="field-label">{fr.rules_source}</label>
        <select class="field-select" bind:value={source}>
          <option value="manual">{fr.rules_manual}</option>
          <option value="system">{fr.rules_system}</option>
        </select>
      </div>

      <div class="form-field">
        <label class="field-label">{fr.rules_scope}</label>
        <select class="field-select" bind:value={scopeType}>
          <option value="permanent">{fr.rules_permanent}</option>
          <option value="temporary">{fr.rules_temporary}</option>
        </select>
      </div>
    </div>

    <div class="form-section">
      <h4 class="section-title">{fr.rules_criteria}</h4>
      <RuleCriteriaBuilder {criteria} onchange={(c) => criteria = c} />
    </div>

    <div class="form-actions">
      <Button variant="ghost" onclick={onclose}>{fr.rules_cancel}</Button>
      <Button variant="primary" type="submit" loading={saving}>
        {editRule ? fr.rules_save : fr.rules_create}
      </Button>
    </div>
  </form>
</Modal>

<style>
  .rule-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }

  .form-error {
    color: var(--accent-red);
    font-size: var(--font-size-sm);
    padding: var(--space-2) var(--space-3);
    background: var(--accent-red-15);
    border-radius: var(--radius-md);
  }

  .form-row {
    width: 100%;
  }

  .form-row-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--space-4);
  }

  .form-field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .field-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .field-select {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: var(--space-2) var(--space-3);
    color: var(--text-primary);
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    outline: none;
  }

  .field-select:focus {
    border-color: var(--accent-cyan);
  }

  .form-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .section-title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
    padding-top: var(--space-4);
    border-top: 1px solid var(--border-primary);
  }
</style>
```

- [ ] **Step 3: DeleteConfirmModal.svelte**

`crates/ui/src/lib/components/rules/DeleteConfirmModal.svelte`:
```svelte
<script lang="ts">
  import Modal from '$lib/components/ui/Modal.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { fr } from '$lib/i18n/fr';

  interface Props {
    open: boolean;
    ruleName: string;
    onconfirm: () => void;
    onclose: () => void;
    loading?: boolean;
  }

  let { open, ruleName, onconfirm, onclose, loading = false }: Props = $props();
</script>

<Modal {open} title={fr.rules_delete_confirm} size="sm" onclose={onclose}>
  <p class="confirm-message">
    {fr.rules_delete_message}
  </p>
  <p class="rule-name font-mono">{ruleName}</p>

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>{fr.rules_cancel}</Button>
    <Button variant="danger" onclick={onconfirm} {loading}>{fr.rules_delete}</Button>
  {/snippet}
</Modal>

<style>
  .confirm-message {
    color: var(--text-secondary);
    font-size: var(--font-size-sm);
    margin-bottom: var(--space-3);
  }

  .rule-name {
    color: var(--text-primary);
    font-size: var(--font-size-base);
    padding: var(--space-3);
    background: var(--bg-tertiary);
    border-radius: var(--radius-md);
  }
</style>
```

- [ ] **Step 4: RuleList.svelte**

`crates/ui/src/lib/components/rules/RuleList.svelte`:
```svelte
<script lang="ts">
  import Card from '$lib/components/ui/Card.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { fr } from '$lib/i18n/fr';
  import { toggleRule, deleteRule as apiDeleteRule } from '$lib/api/client';
  import { fetchRules } from '$lib/stores/rules';
  import type { RuleMessage } from '$lib/types';

  interface Props {
    rules: RuleMessage[];
    onedit: (rule: RuleMessage) => void;
    ondelete: (rule: RuleMessage) => void;
  }

  let { rules, onedit, ondelete }: Props = $props();

  function effectVariant(effect: string): 'green' | 'red' | 'orange' | 'purple' {
    if (effect === 'allow') return 'green';
    if (effect === 'block') return 'red';
    if (effect === 'ask') return 'orange';
    return 'purple';
  }

  function effectLabel(effect: string): string {
    if (effect === 'allow') return fr.rules_allow;
    if (effect === 'block') return fr.rules_block;
    if (effect === 'ask') return fr.rules_ask;
    return fr.rules_observe;
  }

  function sourceVariant(source: string): 'cyan' | 'purple' | 'neutral' {
    if (source === 'manual') return 'cyan';
    if (source === 'auto_learning') return 'purple';
    return 'neutral';
  }

  function sourceLabel(source: string): string {
    if (source === 'manual') return fr.rules_manual;
    if (source === 'auto_learning') return fr.rules_auto_learning;
    if (source === 'system') return fr.rules_system;
    return source;
  }

  async function handleToggle(rule: RuleMessage) {
    try {
      await toggleRule(rule.id, !rule.enabled);
      await fetchRules();
    } catch {
      // Ignore, event will update
    }
  }

  function formatDate(dateStr: string): string {
    try {
      return new Date(dateStr).toLocaleDateString('fr-FR');
    } catch {
      return dateStr;
    }
  }

  function summarizeCriteria(json: string): string {
    try {
      const criteria = JSON.parse(json);
      const parts: string[] = [];
      if (criteria.application) parts.push(`App: ${criteria.application.name || criteria.application.path || '...'}`);
      if (criteria.user) parts.push(`User: ${criteria.user}`);
      if (criteria.remote_ip) parts.push(`IP: ${criteria.remote_ip.exact || criteria.remote_ip.cidr || '...'}`);
      if (criteria.protocol) parts.push(criteria.protocol.toUpperCase());
      if (criteria.direction) parts.push(criteria.direction);
      return parts.join(', ') || 'Tous';
    } catch {
      return '--';
    }
  }
</script>

<div class="rule-list">
  {#each rules as rule (rule.id)}
    <div class="rule-card">
      <div class="rule-left">
        <Badge variant="purple" label={String(rule.priority)} />
        <button
          class="toggle"
          class:enabled={rule.enabled}
          onclick={() => handleToggle(rule)}
          aria-label={rule.enabled ? 'Désactiver' : 'Activer'}
        >
          <span class="toggle-dot"></span>
        </button>
      </div>

      <div class="rule-center">
        <div class="rule-name-row">
          <span class="rule-name">{rule.name}</span>
          <Badge variant={effectVariant(rule.effect)} label={effectLabel(rule.effect)} />
          <Badge variant={sourceVariant(rule.source)} label={sourceLabel(rule.source)} />
        </div>
        <div class="rule-details">
          <span class="criteria truncate">{summarizeCriteria(rule.criteria_json)}</span>
          <span class="date font-mono">{formatDate(rule.created_at)}</span>
        </div>
      </div>

      <div class="rule-actions">
        <Button variant="ghost" size="sm" onclick={() => onedit(rule)}>{fr.rules_edit}</Button>
        <Button variant="ghost" size="sm" onclick={() => ondelete(rule)}>
          <span class="text-red">{fr.rules_delete}</span>
        </Button>
      </div>
    </div>
  {/each}
</div>

<style>
  .rule-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .rule-card {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4);
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    transition: border-color var(--transition-fast);
  }

  .rule-card:hover {
    border-color: var(--accent-cyan);
  }

  .rule-left {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-shrink: 0;
  }

  .toggle {
    width: 36px;
    height: 20px;
    border-radius: var(--radius-full);
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    cursor: pointer;
    position: relative;
    transition: all var(--transition-fast);
    padding: 0;
  }

  .toggle.enabled {
    background: var(--accent-cyan-15);
    border-color: var(--accent-cyan);
  }

  .toggle-dot {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text-tertiary);
    transition: all var(--transition-fast);
  }

  .toggle.enabled .toggle-dot {
    left: 18px;
    background: var(--accent-cyan);
  }

  .rule-center {
    flex: 1;
    min-width: 0;
  }

  .rule-name-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-1);
  }

  .rule-name {
    font-weight: var(--font-weight-semibold);
    font-size: var(--font-size-sm);
    color: var(--text-primary);
  }

  .rule-details {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }

  .criteria {
    font-size: var(--font-size-xs);
    color: var(--text-secondary);
    flex: 1;
  }

  .date {
    font-size: var(--font-size-xs);
    color: var(--text-tertiary);
    flex-shrink: 0;
  }

  .rule-actions {
    display: flex;
    gap: var(--space-2);
    flex-shrink: 0;
  }
</style>
```

- [ ] **Step 5: Rules page**

`crates/ui/src/routes/rules/+page.svelte`:
```svelte
<script lang="ts">
  import Button from '$lib/components/ui/Button.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import LoadingSpinner from '$lib/components/ui/LoadingSpinner.svelte';
  import RuleList from '$lib/components/rules/RuleList.svelte';
  import RuleForm from '$lib/components/rules/RuleForm.svelte';
  import DeleteConfirmModal from '$lib/components/rules/DeleteConfirmModal.svelte';
  import { fr } from '$lib/i18n/fr';
  import { rules, rulesLoading, fetchRules } from '$lib/stores/rules';
  import { deleteRule as apiDeleteRule } from '$lib/api/client';
  import type { RuleMessage } from '$lib/types';

  let showForm = $state(false);
  let editingRule = $state<RuleMessage | null>(null);
  let deletingRule = $state<RuleMessage | null>(null);
  let deleteLoading = $state(false);

  function openCreate() {
    editingRule = null;
    showForm = true;
  }

  function openEdit(rule: RuleMessage) {
    editingRule = rule;
    showForm = true;
  }

  function openDelete(rule: RuleMessage) {
    deletingRule = rule;
  }

  async function confirmDelete() {
    if (!deletingRule) return;
    deleteLoading = true;
    try {
      await apiDeleteRule(deletingRule.id);
      await fetchRules();
      deletingRule = null;
    } catch {
      // Ignore
    } finally {
      deleteLoading = false;
    }
  }
</script>

<div class="rules-page">
  <div class="page-header">
    <h1 class="page-title">{fr.rules_title}</h1>
    <div class="header-actions">
      <Button variant="ghost" size="sm" disabled>
        {fr.rules_import}
        <span class="coming-soon" title={fr.rules_coming_soon}>*</span>
      </Button>
      <Button variant="ghost" size="sm" disabled>
        {fr.rules_export}
        <span class="coming-soon" title={fr.rules_coming_soon}>*</span>
      </Button>
      <Button variant="primary" size="md" onclick={openCreate}>
        + {fr.rules_new}
      </Button>
    </div>
  </div>

  {#if $rulesLoading}
    <LoadingSpinner />
  {:else if $rules.length === 0}
    <EmptyState message={fr.rules_no_rules} />
  {:else}
    <RuleList rules={$rules} onedit={openEdit} ondelete={openDelete} />
  {/if}
</div>

<RuleForm
  open={showForm}
  editRule={editingRule}
  onclose={() => { showForm = false; editingRule = null; }}
/>

{#if deletingRule}
  <DeleteConfirmModal
    open={true}
    ruleName={deletingRule.name}
    onconfirm={confirmDelete}
    onclose={() => { deletingRule = null; }}
    loading={deleteLoading}
  />
{/if}

<style>
  .rules-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    animation: fadeIn 300ms ease;
  }

  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-bold);
    color: var(--text-primary);
  }

  .header-actions {
    display: flex;
    gap: var(--space-3);
  }

  .coming-soon {
    color: var(--accent-orange);
    font-size: var(--font-size-xs);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
</style>
```

---

### Task 10: Auto-Learning View

**Files:**
- Create: `crates/ui/src/lib/components/learning/DecisionCountdown.svelte`
- Create: `crates/ui/src/lib/components/learning/DecisionPrompt.svelte`
- Create: `crates/ui/src/lib/components/learning/DecisionQueue.svelte`
- Create: `crates/ui/src/routes/learning/+page.svelte`

- [ ] **Step 1: DecisionCountdown.svelte**

`crates/ui/src/lib/components/learning/DecisionCountdown.svelte`:
```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { fr } from '$lib/i18n/fr';

  interface Props {
    expiresAt: string;
  }

  let { expiresAt }: Props = $props();

  let remaining = $state(0);
  let total = $state(60);

  function update() {
    const now = Date.now();
    const expiry = new Date(expiresAt).getTime();
    remaining = Math.max(0, Math.floor((expiry - now) / 1000));
  }

  onMount(() => {
    update();
    // Estimate total from the first calculation
    total = Math.max(remaining, 1);
    const interval = setInterval(update, 1000);
    return () => clearInterval(interval);
  });

  const pct = $derived(Math.max(0, Math.min(100, (remaining / total) * 100)));
  const color = $derived(remaining <= 10 ? 'var(--accent-red)' : remaining <= 20 ? 'var(--accent-orange)' : 'var(--accent-cyan)');
</script>

<div class="countdown">
  <div class="countdown-bar">
    <div
      class="countdown-fill"
      style="width: {pct}%; background: {color};"
    ></div>
  </div>
  <span class="countdown-text" style="color: {color};">
    {fr.learn_expires_in} {remaining} {fr.common_seconds}
  </span>
</div>

<style>
  .countdown {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .countdown-bar {
    height: 4px;
    background: var(--bg-tertiary);
    border-radius: var(--radius-full);
    overflow: hidden;
  }

  .countdown-fill {
    height: 100%;
    border-radius: var(--radius-full);
    transition: width 1s linear, background var(--transition-base);
  }

  .countdown-text {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    text-align: center;
  }
</style>
```

- [ ] **Step 2: DecisionPrompt.svelte**

`crates/ui/src/lib/components/learning/DecisionPrompt.svelte`:
```svelte
<script lang="ts">
  import Card from '$lib/components/ui/Card.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import DecisionCountdown from './DecisionCountdown.svelte';
  import { fr } from '$lib/i18n/fr';
  import { respondToDecision } from '$lib/api/client';
  import { fetchPendingDecisions } from '$lib/stores/decisions';
  import type { PendingDecisionMessage, ConnectionSnapshot } from '$lib/types';

  interface Props {
    decision: PendingDecisionMessage;
    index: number;
    total: number;
  }

  let { decision, index, total }: Props = $props();

  let granularity = $state('app_and_destination');
  let responding = $state(false);

  let snapshot: ConnectionSnapshot | null = $derived.by(() => {
    try {
      return JSON.parse(decision.snapshot_json);
    } catch {
      return null;
    }
  });

  async function respond(action: string) {
    responding = true;
    try {
      await respondToDecision({
        pending_decision_id: decision.id,
        action,
        granularity,
      });
      await fetchPendingDecisions();
    } catch {
      // Ignore
    } finally {
      responding = false;
    }
  }
</script>

<div class="decision-prompt">
  <div class="prompt-header">
    <div class="header-left">
      <span class="pulse-dot"></span>
      <h2 class="prompt-title">{fr.learn_new_connection}</h2>
    </div>
    {#if total > 1}
      <span class="queue-indicator font-mono">{index + 1} {fr.learn_decision_of} {total}</span>
    {/if}
  </div>

  {#if snapshot}
    <Card padding="md" glow="cyan">
      <div class="connection-info">
        <div class="info-main">
          <span class="app-name">{snapshot.process_name || fr.conn_unknown}</span>
          {#if snapshot.process_path}
            <span class="app-path font-mono">{fr.learn_path}: {snapshot.process_path}</span>
          {/if}
        </div>

        <div class="info-grid">
          <div class="info-item">
            <span class="info-label">{fr.learn_destination}</span>
            <span class="info-value font-mono">{snapshot.destination?.ip}:{snapshot.destination?.port}</span>
          </div>
          <div class="info-item">
            <span class="info-label">{fr.conn_protocol}</span>
            <Badge variant="cyan" label={snapshot.protocol?.toUpperCase() || '?'} />
          </div>
          <div class="info-item">
            <span class="info-label">{fr.criteria_direction}</span>
            <Badge
              variant={snapshot.direction === 'outbound' ? 'green' : 'orange'}
              label={snapshot.direction === 'outbound' ? fr.conn_outbound : fr.conn_inbound}
            />
          </div>
          {#if snapshot.user}
            <div class="info-item">
              <span class="info-label">{fr.conn_user}</span>
              <span class="info-value font-mono">{snapshot.user}</span>
            </div>
          {/if}
        </div>
      </div>
    </Card>
  {/if}

  <DecisionCountdown expiresAt={decision.expires_at} />

  <!-- Granularity selector -->
  <div class="granularity-section">
    <span class="section-label">{fr.learn_granularity}</span>
    <div class="granularity-options">
      {#each [
        { value: 'app_only', label: fr.learn_app_only },
        { value: 'app_and_destination', label: fr.learn_app_destination },
        { value: 'app_and_protocol', label: fr.learn_app_protocol },
        { value: 'full', label: fr.learn_full_match },
      ] as opt}
        <label class="radio-option" class:selected={granularity === opt.value}>
          <input type="radio" name="granularity" value={opt.value} bind:group={granularity} />
          {opt.label}
        </label>
      {/each}
    </div>
  </div>

  <!-- Action buttons -->
  <div class="action-grid">
    <Button variant="ghost" onclick={() => respond('allow_once')} disabled={responding}>
      <span class="text-green">{fr.learn_allow_once}</span>
    </Button>
    <Button variant="ghost" onclick={() => respond('block_once')} disabled={responding}>
      <span class="text-red">{fr.learn_block_once}</span>
    </Button>
    <Button variant="success" onclick={() => respond('always_allow')} disabled={responding}>
      {fr.learn_always_allow}
    </Button>
    <Button variant="danger" onclick={() => respond('always_block')} disabled={responding}>
      {fr.learn_always_block}
    </Button>
    <Button variant="primary" onclick={() => respond('create_rule')} disabled={responding}>
      {fr.learn_create_rule}
    </Button>
    <Button variant="ghost" onclick={() => respond('ignore')} disabled={responding}>
      {fr.learn_ignore}
    </Button>
  </div>
</div>

<style>
  .decision-prompt {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    animation: slideInRight 300ms ease;
  }

  .prompt-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .pulse-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent-cyan);
    box-shadow: var(--glow-cyan);
    animation: pulse 2s infinite;
  }

  .prompt-title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }

  .queue-indicator {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }

  .connection-info {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .info-main {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .app-name {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-bold);
    color: var(--text-primary);
  }

  .app-path {
    font-size: var(--font-size-xs);
    color: var(--text-secondary);
    word-break: break-all;
  }

  .info-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--space-3);
  }

  .info-item {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .info-label {
    font-size: var(--font-size-xs);
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .info-value {
    font-size: var(--font-size-sm);
    color: var(--text-primary);
  }

  .granularity-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .section-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .granularity-options {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--space-2);
  }

  .radio-option {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    color: var(--text-primary);
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .radio-option.selected {
    border-color: var(--accent-cyan);
    background: var(--accent-cyan-15);
  }

  .radio-option input[type='radio'] {
    appearance: none;
    width: 12px;
    height: 12px;
    border: 2px solid var(--border-primary);
    border-radius: 50%;
    flex-shrink: 0;
  }

  .radio-option.selected input[type='radio'] {
    border-color: var(--accent-cyan);
    background: var(--accent-cyan);
    box-shadow: inset 0 0 0 2px var(--bg-tertiary);
  }

  .action-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-3);
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  @keyframes slideInRight {
    from { opacity: 0; transform: translateX(24px); }
    to { opacity: 1; transform: translateX(0); }
  }
</style>
```

- [ ] **Step 3: DecisionQueue.svelte**

`crates/ui/src/lib/components/learning/DecisionQueue.svelte`:
```svelte
<script lang="ts">
  import Badge from '$lib/components/ui/Badge.svelte';
  import { fr } from '$lib/i18n/fr';
  import type { PendingDecisionMessage, ConnectionSnapshot } from '$lib/types';

  interface Props {
    decisions: PendingDecisionMessage[];
    currentIndex: number;
    onselect: (index: number) => void;
  }

  let { decisions, currentIndex, onselect }: Props = $props();

  function parseSnapshot(json: string): ConnectionSnapshot | null {
    try { return JSON.parse(json); } catch { return null; }
  }
</script>

<div class="queue">
  <h3 class="queue-title">{fr.learn_queue} ({decisions.length})</h3>
  <div class="queue-list">
    {#each decisions as decision, i (decision.id)}
      <button
        class="queue-item"
        class:active={i === currentIndex}
        onclick={() => onselect(i)}
      >
        {@const snap = parseSnapshot(decision.snapshot_json)}
        <span class="queue-app">{snap?.process_name || fr.conn_unknown}</span>
        <span class="queue-dest font-mono">{snap?.destination?.ip || '?'}:{snap?.destination?.port || '?'}</span>
        <Badge variant="cyan" label={snap?.protocol?.toUpperCase() || '?'} />
      </button>
    {/each}
  </div>
</div>

<style>
  .queue {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .queue-title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .queue-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .queue-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    cursor: pointer;
    text-align: left;
    color: var(--text-primary);
    font-size: var(--font-size-sm);
    transition: all var(--transition-fast);
  }

  .queue-item:hover {
    border-color: var(--accent-cyan);
  }

  .queue-item.active {
    border-color: var(--accent-cyan);
    background: var(--accent-cyan-15);
  }

  .queue-app {
    flex: 1;
    font-weight: var(--font-weight-medium);
  }

  .queue-dest {
    color: var(--text-secondary);
    font-size: var(--font-size-xs);
  }
</style>
```

- [ ] **Step 4: Learning page**

`crates/ui/src/routes/learning/+page.svelte`:
```svelte
<script lang="ts">
  import DecisionPrompt from '$lib/components/learning/DecisionPrompt.svelte';
  import DecisionQueue from '$lib/components/learning/DecisionQueue.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import { fr } from '$lib/i18n/fr';
  import {
    pendingDecisions,
    currentDecision,
    currentDecisionIndex,
    pendingCount,
  } from '$lib/stores/decisions';
</script>

<div class="learning-page">
  <h1 class="page-title">{fr.learn_title}</h1>

  {#if $pendingCount === 0}
    <EmptyState message={fr.learn_no_pending} />
  {:else}
    <div class="learning-layout">
      <div class="prompt-section">
        {#if $currentDecision}
          <DecisionPrompt
            decision={$currentDecision}
            index={$currentDecisionIndex}
            total={$pendingCount}
          />
        {/if}
      </div>

      {#if $pendingCount > 1}
        <div class="queue-section">
          <DecisionQueue
            decisions={$pendingDecisions}
            currentIndex={$currentDecisionIndex}
            onselect={(i) => currentDecisionIndex.set(i)}
          />
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .learning-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    animation: fadeIn 300ms ease;
  }

  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-bold);
    color: var(--text-primary);
  }

  .learning-layout {
    display: grid;
    grid-template-columns: 1fr 320px;
    gap: var(--space-6);
  }

  .prompt-section {
    min-width: 0;
  }

  .queue-section {
    flex-shrink: 0;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @media (max-width: 1000px) {
    .learning-layout {
      grid-template-columns: 1fr;
    }
  }
</style>
```

---

### Task 11: Audit View

**Files:**
- Create: `crates/ui/src/lib/components/audit/AuditFilters.svelte`
- Create: `crates/ui/src/lib/components/audit/AuditTable.svelte`
- Create: `crates/ui/src/routes/audit/+page.svelte`

- [ ] **Step 1: AuditFilters.svelte**

`crates/ui/src/lib/components/audit/AuditFilters.svelte`:
```svelte
<script lang="ts">
  import Input from '$lib/components/ui/Input.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { fr } from '$lib/i18n/fr';
  import { auditFilters } from '$lib/stores/audit';

  let filters = $state({ search: '', severity: '', category: '', dateStart: '', dateEnd: '' });
  let searchTimeout: ReturnType<typeof setTimeout>;

  function onSearchInput(e: Event) {
    const val = (e.target as HTMLInputElement).value;
    filters.search = val;
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      auditFilters.update((f) => ({ ...f, search: val }));
    }, 300);
  }

  function onFilterChange(key: 'severity' | 'category', val: string) {
    filters[key] = val;
    auditFilters.update((f) => ({ ...f, [key]: val }));
  }

  function clearFilters() {
    filters = { search: '', severity: '', category: '', dateStart: '', dateEnd: '' };
    auditFilters.set({ search: '', severity: '', category: '', dateStart: '', dateEnd: '' });
  }
</script>

<div class="filters-bar">
  <div class="filter-group search">
    <Input type="search" placeholder={fr.audit_search} value={filters.search} oninput={onSearchInput} />
  </div>

  <select class="filter-select" value={filters.severity} onchange={(e) => onFilterChange('severity', (e.target as HTMLSelectElement).value)}>
    <option value="">{fr.audit_severity}</option>
    <option value="debug">{fr.audit_debug}</option>
    <option value="info">{fr.audit_info}</option>
    <option value="warning">{fr.audit_warning}</option>
    <option value="error">{fr.audit_error}</option>
    <option value="critical">{fr.audit_critical}</option>
  </select>

  <select class="filter-select" value={filters.category} onchange={(e) => onFilterChange('category', (e.target as HTMLSelectElement).value)}>
    <option value="">{fr.audit_category}</option>
    <option value="connection">{fr.audit_connection}</option>
    <option value="rule">{fr.audit_rule}</option>
    <option value="decision">{fr.audit_decision}</option>
    <option value="system">{fr.audit_system}</option>
    <option value="config">{fr.audit_config}</option>
  </select>

  <Button variant="ghost" size="sm" onclick={clearFilters}>{fr.conn_clear_filters}</Button>
</div>

<style>
  .filters-bar {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-wrap: wrap;
  }

  .search {
    flex: 1;
    min-width: 200px;
  }

  .filter-select {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: var(--space-2) var(--space-3);
    color: var(--text-primary);
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    outline: none;
    cursor: pointer;
  }

  .filter-select:focus { border-color: var(--accent-cyan); }
</style>
```

- [ ] **Step 2: AuditTable.svelte**

`crates/ui/src/lib/components/audit/AuditTable.svelte`:
```svelte
<script lang="ts">
  import Badge from '$lib/components/ui/Badge.svelte';
  import { fr } from '$lib/i18n/fr';
  import type { AuditEvent } from '$lib/types';

  interface Props {
    events: AuditEvent[];
  }

  let { events }: Props = $props();

  let expandedId = $state<string | null>(null);

  function severityVariant(sev: string): 'neutral' | 'cyan' | 'orange' | 'red' {
    if (sev === 'debug') return 'neutral';
    if (sev === 'info') return 'cyan';
    if (sev === 'warning') return 'orange';
    return 'red';
  }

  function severityLabel(sev: string): string {
    const map: Record<string, string> = {
      debug: fr.audit_debug,
      info: fr.audit_info,
      warning: fr.audit_warning,
      error: fr.audit_error,
      critical: fr.audit_critical,
    };
    return map[sev] || sev;
  }

  function categoryLabel(cat: string): string {
    const map: Record<string, string> = {
      connection: fr.audit_connection,
      rule: fr.audit_rule,
      decision: fr.audit_decision,
      system: fr.audit_system,
      config: fr.audit_config,
    };
    return map[cat] || cat;
  }

  function formatTimestamp(ts: string): string {
    try {
      return new Date(ts).toLocaleString('fr-FR');
    } catch { return ts; }
  }

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }
</script>

<div class="audit-table">
  <div class="table-header">
    <span class="col col-time">{fr.audit_timestamp}</span>
    <span class="col col-sev">{fr.audit_severity}</span>
    <span class="col col-cat">{fr.audit_category}</span>
    <span class="col col-desc">{fr.audit_description}</span>
  </div>
  <div class="table-body">
    {#each events as evt (evt.id)}
      <div
        class="table-row"
        onclick={() => toggleExpand(evt.id)}
        role="button"
        tabindex="0"
        onkeydown={(e) => e.key === 'Enter' && toggleExpand(evt.id)}
      >
        <span class="col col-time font-mono">{formatTimestamp(evt.timestamp)}</span>
        <span class="col col-sev"><Badge variant={severityVariant(evt.severity)} label={severityLabel(evt.severity)} /></span>
        <span class="col col-cat"><Badge variant="purple" label={categoryLabel(evt.category)} /></span>
        <span class="col col-desc truncate">{evt.description}</span>
      </div>
      {#if expandedId === evt.id}
        <div class="detail-panel">
          <p class="detail-description">{evt.description}</p>
          {#if Object.keys(evt.metadata).length > 0}
            <div class="metadata">
              {#each Object.entries(evt.metadata) as [key, val]}
                <div class="meta-row">
                  <span class="meta-key font-mono">{key}</span>
                  <span class="meta-val font-mono">{typeof val === 'string' ? val : JSON.stringify(val)}</span>
                </div>
              {/each}
            </div>
          {/if}
          <span class="event-id font-mono text-tertiary">ID: {evt.id}</span>
        </div>
      {/if}
    {/each}
  </div>
</div>

<style>
  .audit-table {
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }

  .table-header {
    display: flex;
    padding: 0 var(--space-4);
    height: 36px;
    align-items: center;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
  }

  .table-header .col {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .table-body {
    max-height: 600px;
    overflow-y: auto;
  }

  .table-row {
    display: flex;
    align-items: center;
    padding: 0 var(--space-4);
    height: 40px;
    border-bottom: 1px solid var(--border-subtle);
    cursor: pointer;
    transition: background var(--transition-fast);
    font-size: var(--font-size-sm);
  }

  .table-row:hover { background: var(--bg-hover); }

  .col { padding: 0 var(--space-1); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-time { flex: 1.2; }
  .col-sev { flex: 0.8; }
  .col-cat { flex: 0.8; }
  .col-desc { flex: 3; }

  .detail-panel {
    padding: var(--space-4) var(--space-6);
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    animation: slideUp 200ms ease;
  }

  .detail-description { color: var(--text-primary); font-size: var(--font-size-sm); }

  .metadata {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2);
    background: var(--bg-secondary);
    border-radius: var(--radius-md);
  }

  .meta-row {
    display: flex;
    gap: var(--space-4);
    font-size: var(--font-size-xs);
  }

  .meta-key { color: var(--text-secondary); min-width: 120px; }
  .meta-val { color: var(--text-primary); word-break: break-all; }

  .event-id { font-size: var(--font-size-xs); }

  @keyframes slideUp {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
```

- [ ] **Step 3: Audit page**

`crates/ui/src/routes/audit/+page.svelte`:
```svelte
<script lang="ts">
  import AuditFilters from '$lib/components/audit/AuditFilters.svelte';
  import AuditTable from '$lib/components/audit/AuditTable.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import { fr } from '$lib/i18n/fr';
  import {
    paginatedAuditEvents,
    totalFilteredCount,
    totalPages,
    auditPage,
  } from '$lib/stores/audit';
</script>

<div class="audit-page">
  <h1 class="page-title">{fr.audit_title}</h1>

  <AuditFilters />

  {#if $totalFilteredCount === 0}
    <EmptyState message={fr.audit_no_events} />
  {:else}
    <AuditTable events={$paginatedAuditEvents} />

    <div class="pagination">
      <Button
        variant="ghost"
        size="sm"
        disabled={$auditPage === 0}
        onclick={() => auditPage.update((p) => Math.max(0, p - 1))}
      >
        {fr.audit_previous}
      </Button>
      <span class="page-info font-mono">
        {fr.audit_page} {$auditPage + 1} / {$totalPages}
      </span>
      <Button
        variant="ghost"
        size="sm"
        disabled={$auditPage >= $totalPages - 1}
        onclick={() => auditPage.update((p) => p + 1)}
      >
        {fr.audit_next}
      </Button>
    </div>
  {/if}
</div>

<style>
  .audit-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    animation: fadeIn 300ms ease;
  }

  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-bold);
    color: var(--text-primary);
  }

  .pagination {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
  }

  .page-info {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
</style>
```

---

### Task 12: Settings View

**Files:**
- Create: `crates/ui/src/routes/settings/+page.svelte`

- [ ] **Step 1: Settings page**

`crates/ui/src/routes/settings/+page.svelte`:
```svelte
<script lang="ts">
  import Card from '$lib/components/ui/Card.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';
  import { fr } from '$lib/i18n/fr';
  import { firewallStatus } from '$lib/stores/status';

  function formatUptime(secs: number): string {
    const d = Math.floor(secs / 86400);
    const h = Math.floor((secs % 86400) / 3600);
    const m = Math.floor((secs % 3600) / 60);
    if (d > 0) return `${d}j ${h}h ${m}m`;
    if (h > 0) return `${h}h ${m}m`;
    return `${m}m ${secs % 60}s`;
  }
</script>

<div class="settings-page">
  <h1 class="page-title">{fr.settings_title}</h1>

  <div class="settings-grid">
    <!-- Firewall -->
    <Card title={fr.settings_firewall}>
      <div class="setting-list">
        <div class="setting-row">
          <span class="setting-label">{fr.settings_status}</span>
          <Badge
            variant={$firewallStatus.enabled ? 'green' : 'red'}
            label={$firewallStatus.enabled ? fr.status_active : fr.status_inactive}
            dot
          />
        </div>
        <div class="setting-row">
          <span class="setting-label">{fr.settings_nftables_table}</span>
          <span class="setting-value font-mono">syswall</span>
        </div>
        <div class="setting-row">
          <span class="setting-label">{fr.status_synced}</span>
          <Badge
            variant={$firewallStatus.nftables_synced ? 'green' : 'orange'}
            label={$firewallStatus.nftables_synced ? fr.status_synced : fr.status_not_synced}
          />
        </div>
      </div>
    </Card>

    <!-- Learning -->
    <Card title={fr.settings_learning}>
      <div class="setting-list">
        <div class="setting-row">
          <span class="setting-label">{fr.settings_enabled}</span>
          <Badge variant="green" label={fr.status_active} dot />
        </div>
        <div class="setting-row">
          <span class="setting-label">{fr.settings_timeout}</span>
          <span class="setting-value font-mono">60 {fr.common_seconds}</span>
        </div>
        <div class="setting-row">
          <span class="setting-label">{fr.settings_default_action}</span>
          <Badge variant="red" label="block" />
        </div>
        <div class="setting-row">
          <span class="setting-label">{fr.settings_max_pending}</span>
          <span class="setting-value font-mono">50</span>
        </div>
      </div>
    </Card>

    <!-- Interface -->
    <Card title={fr.settings_interface}>
      <div class="setting-list">
        <div class="setting-row">
          <span class="setting-label">{fr.settings_theme}</span>
          <Badge variant="neutral" label={fr.settings_theme_dark} />
        </div>
        <div class="setting-row">
          <span class="setting-label">{fr.settings_locale}</span>
          <Badge variant="cyan" label={fr.settings_locale_fr} />
        </div>
        <div class="setting-row">
          <span class="setting-label">{fr.settings_refresh_interval}</span>
          <span class="setting-value font-mono">1000 ms</span>
        </div>
      </div>
    </Card>

    <!-- Daemon -->
    <Card title={fr.settings_daemon}>
      <div class="setting-list">
        <div class="setting-row">
          <span class="setting-label">{fr.settings_socket}</span>
          <span class="setting-value font-mono">/var/run/syswall/syswall.sock</span>
        </div>
        <div class="setting-row">
          <span class="setting-label">{fr.settings_version}</span>
          <span class="setting-value font-mono">{$firewallStatus.version || '--'}</span>
        </div>
        <div class="setting-row">
          <span class="setting-label">{fr.settings_uptime}</span>
          <span class="setting-value font-mono">{formatUptime($firewallStatus.uptime_secs)}</span>
        </div>
      </div>
    </Card>
  </div>
</div>

<style>
  .settings-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    animation: fadeIn 300ms ease;
  }

  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-bold);
    color: var(--text-primary);
  }

  .settings-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--space-4);
  }

  .setting-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border-subtle);
  }

  .setting-row:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  .setting-label {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }

  .setting-value {
    font-size: var(--font-size-sm);
    color: var(--text-primary);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @media (max-width: 900px) {
    .settings-grid { grid-template-columns: 1fr; }
  }
</style>
```

---

### Task 13: Final Verification

- [ ] **Step 1: Verify SvelteKit check passes**

```bash
cd /home/seb/Dev/SysWall/crates/ui && npm run check
```

- [ ] **Step 2: Verify Rust side compiles**

```bash
cd /home/seb/Dev/SysWall/crates/ui/src-tauri && cargo check
```

- [ ] **Step 3: Verify dev server starts**

```bash
cd /home/seb/Dev/SysWall/crates/ui && npm run dev
```
Open browser, verify no blank page, verify sidebar renders.

- [ ] **Step 4: Commit all changes**

```bash
cd /home/seb/Dev/SysWall
git add -A
git commit -m "feat: implement premium UI design system, Tauri gRPC bridge, and all views

Sub-project 4: Design tokens, 11 reusable components, 6 stores,
typed API layer, sidebar navigation, and 6 views (Dashboard,
Connections, Rules, Learning, Audit, Settings). All labels in French."
```
