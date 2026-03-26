# SysWall -- Premium UI Design Spec

**Date:** 2026-03-26
**Scope:** Sub-project 4 -- Premium UI (design system, Tauri gRPC client, Svelte views, real-time event streaming)
**Status:** Draft

---

## 1. Overview

This spec covers the complete frontend UI for SysWall. The daemon (sub-projects 1--3) is fully operational, exposing `SysWallControl` (7 RPCs) and `SysWallEvents` (server-streaming) over a Unix socket at `/var/run/syswall/syswall.sock`. The UI crate already has a Tauri 2 + SvelteKit 5 scaffold with a placeholder page.

This sub-project delivers:
1. A design system with CSS custom properties, reusable Svelte components, and a cyber/neon dark theme
2. A Tauri-side gRPC client (Rust) that connects to the daemon and exposes Tauri commands + event streaming
3. TypeScript types, API wrappers, and reactive Svelte stores fed by real-time events
4. Six complete views: Dashboard, Connections, Rules, Auto-learning, Audit, Settings
5. A sidebar navigation shell with firewall status header

**Locale:** All user-facing labels and text are in French (FR).

---

## 2. Architecture

### 2.1 Data Flow

```
Daemon (Unix socket)
  |  gRPC (tonic client)
  v
Tauri Rust backend (grpc_client.rs, commands/, streams.rs)
  |  invoke() / app.emit()
  v
Svelte frontend (api/, stores/, components/, routes/)
```

The frontend never connects to the daemon directly. All communication transits through the Tauri Rust process which acts as a gRPC client bridge.

### 2.2 Key Constraint

**No business logic in the UI.** The frontend only:
1. Calls Tauri commands (thin wrappers around gRPC RPCs)
2. Listens to Tauri events (forwarded from gRPC stream)
3. Displays data
4. Collects user input

### 2.3 Tauri Rust Side

```
src-tauri/src/
  lib.rs              -- Tauri builder, plugin init, command registration
  main.rs             -- Binary entrypoint (unchanged)
  grpc_client.rs      -- GrpcClient struct: connect to Unix socket, hold channel
  streams.rs          -- subscribe_events(): spawn task, listen to gRPC stream, app.emit()
  commands/
    mod.rs            -- Re-export all command modules
    status.rs         -- get_status
    rules.rs          -- list_rules, create_rule, delete_rule, toggle_rule
    decisions.rs      -- list_pending_decisions, respond_to_decision
```

### 2.4 Svelte Frontend Side

```
src/
  app.html            -- HTML shell (lang="fr")
  app.css             -- Design tokens + global theme
  routes/
    +layout.svelte    -- Main layout: sidebar + content area + decision overlay
    +layout.ts        -- ssr = false
    +page.svelte      -- Redirects to /dashboard
    dashboard/
      +page.svelte    -- Dashboard view
    connections/
      +page.svelte    -- Connections view
    rules/
      +page.svelte    -- Rules view
    learning/
      +page.svelte    -- Auto-learning view
    audit/
      +page.svelte    -- Audit/journal view
    settings/
      +page.svelte    -- Settings view
  lib/
    types/
      index.ts        -- TypeScript interfaces mirroring proto messages
    api/
      client.ts       -- Typed wrappers around invoke()
    stores/
      status.ts       -- Firewall status store
      connections.ts  -- Connections store (fed by events)
      rules.ts        -- Rules store
      decisions.ts    -- Pending decisions store (fed by events)
      audit.ts        -- Audit events store
      dashboard.ts    -- Derived dashboard stats
    components/
      ui/             -- Design system primitives
        Card.svelte
        Badge.svelte
        Button.svelte
        Input.svelte
        Table.svelte
        Modal.svelte
        StatCard.svelte
        Sidebar.svelte
        EmptyState.svelte
        LoadingSpinner.svelte
        ErrorBanner.svelte
      dashboard/
        TrafficChart.svelte
        TopApps.svelte
        TopDestinations.svelte
        RecentAlerts.svelte
      connections/
        ConnectionTable.svelte
        ConnectionRow.svelte
        ConnectionDetail.svelte
        ConnectionFilters.svelte
      rules/
        RuleList.svelte
        RuleRow.svelte
        RuleForm.svelte
        RuleCriteriaBuilder.svelte
        DeleteConfirmModal.svelte
      learning/
        DecisionPrompt.svelte
        DecisionQueue.svelte
        DecisionCountdown.svelte
      audit/
        AuditTable.svelte
        AuditFilters.svelte
        AuditRow.svelte
    i18n/
      fr.ts           -- French labels for all UI text
```

---

## 3. Design System

### 3.1 Design Tokens (CSS Custom Properties)

All visual tokens are defined as CSS custom properties on `:root` in `app.css`.

#### Colors

| Token | Value | Usage |
|---|---|---|
| `--bg-primary` | `#0d1117` | Main background |
| `--bg-secondary` | `#161b22` | Cards, sidebar |
| `--bg-tertiary` | `#1c2333` | Elevated surfaces, inputs |
| `--bg-hover` | `#21283b` | Hover states |
| `--border-primary` | `#30363d` | Card borders, separators |
| `--border-subtle` | `#21262d` | Subtle dividers |
| `--text-primary` | `#e6edf3` | Main text |
| `--text-secondary` | `#8b949e` | Muted text, labels |
| `--text-tertiary` | `#6e7681` | Disabled, placeholder |
| `--accent-cyan` | `#00d4ff` | Primary accent, links, active states |
| `--accent-green` | `#00ff88` | Allow, success, active connections |
| `--accent-red` | `#ff4444` | Block, error, danger |
| `--accent-orange` | `#ff8c00` | Warning, pending |
| `--accent-purple` | `#a855f7` | System, auto-learning |
| `--glow-cyan` | `0 0 12px rgba(0, 212, 255, 0.3)` | Neon glow for focused/active elements |
| `--glow-green` | `0 0 12px rgba(0, 255, 136, 0.3)` | Success glow |
| `--glow-red` | `0 0 12px rgba(255, 68, 68, 0.3)` | Error glow |
| `--glass-bg` | `rgba(22, 27, 34, 0.8)` | Glassmorphism overlay |
| `--glass-border` | `rgba(48, 54, 61, 0.5)` | Glass border |
| `--glass-blur` | `12px` | Backdrop blur radius |

#### Typography

| Token | Value | Usage |
|---|---|---|
| `--font-mono` | `'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace` | Data, code, IPs, PIDs |
| `--font-sans` | `'Inter', 'Segoe UI', system-ui, sans-serif` | Headings, labels, body |
| `--font-size-xs` | `0.75rem` | Tiny labels |
| `--font-size-sm` | `0.875rem` | Table cells, secondary |
| `--font-size-base` | `1rem` | Body text |
| `--font-size-lg` | `1.25rem` | Section headings |
| `--font-size-xl` | `1.5rem` | Page titles |
| `--font-size-2xl` | `2rem` | Stat numbers |
| `--font-weight-normal` | `400` | Body text |
| `--font-weight-medium` | `500` | Labels |
| `--font-weight-semibold` | `600` | Headings |
| `--font-weight-bold` | `700` | Stat numbers |

#### Spacing

| Token | Value |
|---|---|
| `--space-1` | `0.25rem` |
| `--space-2` | `0.5rem` |
| `--space-3` | `0.75rem` |
| `--space-4` | `1rem` |
| `--space-5` | `1.25rem` |
| `--space-6` | `1.5rem` |
| `--space-8` | `2rem` |
| `--space-10` | `2.5rem` |
| `--space-12` | `3rem` |

#### Borders & Radius

| Token | Value |
|---|---|
| `--radius-sm` | `4px` |
| `--radius-md` | `8px` |
| `--radius-lg` | `12px` |
| `--radius-xl` | `16px` |
| `--radius-full` | `9999px` |

#### Transitions

| Token | Value |
|---|---|
| `--transition-fast` | `150ms ease` |
| `--transition-base` | `250ms ease` |
| `--transition-slow` | `400ms ease` |

### 3.2 Component Library

All reusable components live in `src/lib/components/ui/`.

#### Card

Glass-effect container with subtle border and backdrop blur.

```
Props: title?: string, padding?: 'sm' | 'md' | 'lg', glow?: 'cyan' | 'green' | 'red' | 'none'
Visual: bg-secondary, border-primary, radius-lg, optional glow shadow
Slot: default content
```

#### Badge

Small pill for status indicators.

```
Props: variant: 'cyan' | 'green' | 'red' | 'orange' | 'purple' | 'neutral', label: string, dot?: boolean
Visual: pill shape (radius-full), color-coded background at 15% opacity, text in full color, optional dot indicator
```

#### Button

```
Props: variant: 'primary' | 'success' | 'danger' | 'ghost', size: 'sm' | 'md' | 'lg', disabled?: boolean, loading?: boolean
Visual: primary=cyan, success=green, danger=red, ghost=transparent with border
Hover: subtle glow matching variant color
```

#### Input

```
Props: type: 'text' | 'number' | 'search', placeholder?: string, value: string, label?: string
Visual: bg-tertiary, border-primary, focus ring in accent-cyan
```

#### Table

```
Props: columns: Column[], rows: T[], sortable?: boolean, virtualScroll?: boolean
Visual: header bg-tertiary, alternating row colors, hover bg-hover
Slot: custom cell renderers via snippets
```

#### Modal

```
Props: open: boolean, title: string, size: 'sm' | 'md' | 'lg'
Visual: glass-bg backdrop, centered, radius-xl, shadow glow-cyan, slide-in animation
Slot: body and footer
```

#### StatCard

```
Props: label: string, value: string | number, icon: string, trend?: 'up' | 'down' | 'stable', color: 'cyan' | 'green' | 'red' | 'orange' | 'purple'
Visual: Card with large number (font-size-2xl, font-mono, bold), label below, icon on right, subtle glow matching color
```

#### Sidebar

```
Props: items: NavItem[], activeRoute: string, firewallStatus: StatusResponse
Visual: fixed left, 240px wide, bg-secondary, logo/status header, nav items with icons and active highlight
```

#### EmptyState

```
Props: message: string, icon?: string
Visual: centered, muted text, icon above
```

#### LoadingSpinner

```
Props: size: 'sm' | 'md' | 'lg'
Visual: spinning ring in accent-cyan
```

#### ErrorBanner

```
Props: message: string, retryAction?: () => void
Visual: bg-red at 10% opacity, border-red, text-red, optional retry button
```

### 3.3 Glassmorphism Rules

- Used sparingly: modals, decision prompt overlay, sidebar header
- `backdrop-filter: blur(var(--glass-blur))`
- Semi-transparent background (`var(--glass-bg)`)
- Thin border (`var(--glass-border)`)
- Never on primary content areas (readability first)

### 3.4 Animations

- Page transitions: fade 200ms
- Modal: slide up + fade in 300ms
- Table row appear: fade in 150ms staggered
- Badge pulse: subtle scale animation on new items
- Sidebar hover: background color transition 150ms
- Decision prompt: slide in from right + glow pulse
- All animations respect `prefers-reduced-motion`

### 3.5 Accessibility

- Minimum contrast ratio: 4.5:1 for text, 3:1 for UI elements
- Focus visible outlines (2px solid accent-cyan with 2px offset)
- Keyboard navigation for all interactive elements
- ARIA labels on icon-only buttons
- Role and state attributes on custom widgets

---

## 4. Tauri gRPC Client (Rust)

### 4.1 grpc_client.rs

A `GrpcClient` struct that holds a connected gRPC channel to the daemon.

```rust
pub struct GrpcClient {
    control: SysWallControlClient<Channel>,
    events: SysWallEventsClient<Channel>,
}
```

**Connection strategy:**
- Connect to the Unix socket at the configured path (default: `/var/run/syswall/syswall.sock`)
- Use `tonic::transport::Endpoint::from_static("http://[::]:50051")` with a custom `connect_with_connector` using a `tower::service_fn` that opens a `tokio::net::UnixStream`
- Store as `Mutex<Option<GrpcClient>>` in Tauri managed state
- Reconnection: on any command failure with transport error, attempt one reconnection before returning error

### 4.2 Tauri Commands

Each command is a thin `#[tauri::command]` async function that:
1. Acquires the `GrpcClient` from Tauri state
2. Calls the appropriate gRPC method
3. Deserializes the proto response into a serializable Rust struct (serde)
4. Returns `Result<T, String>` (Tauri command convention)

| Command | gRPC RPC | Returns |
|---|---|---|
| `get_status` | `GetStatus` | `StatusResponse` |
| `list_rules` | `ListRules` | `Vec<RuleMessage>` |
| `create_rule` | `CreateRule` | `RuleMessage` |
| `delete_rule` | `DeleteRule` | `()` |
| `toggle_rule` | `ToggleRule` | `RuleMessage` |
| `list_pending_decisions` | `ListPendingDecisions` | `Vec<PendingDecisionMessage>` |
| `respond_to_decision` | `RespondToDecision` | `DecisionAck` |

### 4.3 Event Streaming (streams.rs)

A background task started during Tauri setup:

1. Calls `SysWallEvents::SubscribeEvents` to get a server-streaming response
2. For each `DomainEventMessage` received:
   - Parse `event_type` string
   - Emit a typed Tauri event via `app_handle.emit(event_name, payload_json)`
3. Event name mapping:

| gRPC event_type | Tauri event name |
|---|---|
| `connection_detected` | `syswall://connection-detected` |
| `connection_updated` | `syswall://connection-updated` |
| `connection_closed` | `syswall://connection-closed` |
| `rule_created` | `syswall://rule-created` |
| `rule_updated` | `syswall://rule-updated` |
| `rule_deleted` | `syswall://rule-deleted` |
| `rule_matched` | `syswall://rule-matched` |
| `decision_required` | `syswall://decision-required` |
| `decision_resolved` | `syswall://decision-resolved` |
| `decision_expired` | `syswall://decision-expired` |
| `firewall_status_changed` | `syswall://status-changed` |
| `system_error` | `syswall://system-error` |

4. On stream end or error: log warning, wait 2 seconds, reconnect

---

## 5. TypeScript Types

All types in `src/lib/types/index.ts`, mirroring proto messages:

```typescript
interface StatusResponse {
  enabled: boolean;
  active_rules_count: number;
  nftables_synced: boolean;
  uptime_secs: number;
  version: string;
}

interface RuleMessage {
  id: string;
  name: string;
  priority: number;
  enabled: boolean;
  criteria_json: string;   // JSON string, parsed on demand
  effect: string;          // "allow" | "block" | "ask" | "observe"
  scope_json: string;      // JSON string
  source: string;          // "manual" | "auto_learning" | "import" | "system"
  created_at: string;      // ISO 8601
  updated_at: string;
}

interface RuleCriteria {
  application?: { name?: string; path?: string };
  user?: string;
  remote_ip?: { exact?: string; cidr?: string };
  remote_port?: { exact?: number; range?: [number, number] };
  local_port?: { exact?: number; range?: [number, number] };
  protocol?: string;
  direction?: string;
}

interface PendingDecisionMessage {
  id: string;
  snapshot_json: string;   // ConnectionSnapshot as JSON
  requested_at: string;
  expires_at: string;
  status: string;          // "pending" | "resolved" | "expired" | "cancelled"
}

interface ConnectionSnapshot {
  protocol: string;
  source: { ip: string; port: number };
  destination: { ip: string; port: number };
  direction: string;
  process_name?: string;
  process_path?: string;
  user?: string;
}

interface DomainEventMessage {
  event_type: string;
  payload_json: string;
  timestamp: string;
}

interface AuditEvent {
  id: string;
  timestamp: string;
  severity: string;       // "debug" | "info" | "warning" | "error" | "critical"
  category: string;       // "connection" | "rule" | "decision" | "system" | "config"
  description: string;
  metadata: Record<string, string>;
}

interface CreateRuleRequest {
  name: string;
  priority: number;
  criteria_json: string;
  effect: string;
  scope_json: string;
  source: string;
}

interface DecisionResponse {
  pending_decision_id: string;
  action: string;          // "allow_once" | "block_once" | "always_allow" | "always_block" | "create_rule" | "ignore"
  granularity: string;     // "app_only" | "app_and_destination" | "app_and_protocol" | "full"
}
```

---

## 6. Svelte Stores

All stores in `src/lib/stores/`. They use Svelte 5 runes ($state, $derived) where possible, and `writable` from `svelte/store` for cross-component reactivity.

### 6.1 status.ts

- `firewallStatus`: writable store holding `StatusResponse`
- `fetchStatus()`: calls `get_status` command, updates store
- Listens to `syswall://status-changed` events to auto-update

### 6.2 connections.ts

- `connections`: writable store holding `Map<string, ConnectionEvent>` (keyed by connection ID)
- `connectionList`: derived store that returns sorted array
- Listens to:
  - `syswall://connection-detected`: add to map
  - `syswall://connection-updated`: update entry
  - `syswall://connection-closed`: mark as closed
- `connectionFilters`: writable store for active filters (search, protocol, verdict, direction)
- `filteredConnections`: derived store applying filters

### 6.3 rules.ts

- `rules`: writable store holding `RuleMessage[]`
- `fetchRules()`: calls `list_rules`, updates store
- Listens to:
  - `syswall://rule-created`: append
  - `syswall://rule-updated`: replace
  - `syswall://rule-deleted`: remove

### 6.4 decisions.ts

- `pendingDecisions`: writable store holding `PendingDecisionMessage[]`
- `pendingCount`: derived store for badge count
- `fetchPendingDecisions()`: calls `list_pending_decisions`, updates store
- Listens to:
  - `syswall://decision-required`: prepend
  - `syswall://decision-resolved`: remove
  - `syswall://decision-expired`: remove
- `showDecisionOverlay`: derived boolean (pendingCount > 0)

### 6.5 audit.ts

- `auditEvents`: writable store holding paginated `AuditEvent[]`
- `auditFilters`: writable store (severity, category, search, dateRange)
- Pagination: offset/limit state
- Note: audit events are fetched on demand (no gRPC stream for audit specifically; audit is populated from domain events arriving on the stream that match categories connection/rule/decision/system)

### 6.6 dashboard.ts

- `dashboardStats`: derived store aggregating:
  - Total active connections (from connections store, state != "closed")
  - Allowed count (verdict == "allowed")
  - Blocked count (verdict == "blocked")
  - Recent alerts (last 10 system_error events)
  - Top apps (group connections by process_name, count)
  - Top destinations (group by destination IP, count)
  - Firewall status (from status store)

---

## 7. Views

### 7.1 Dashboard (`/dashboard`)

**Layout:** 3-column grid on large screens, 2-column on medium.

**Sections:**
1. **Top stat cards row** (4 cards):
   - "Connexions actives" -- count, cyan accent, trending arrow
   - "Autorisees" -- allowed count, green accent
   - "Bloquees" -- blocked count, red accent
   - "Alertes" -- recent alert count, orange accent

2. **Firewall status card**: enabled/disabled indicator with green/red badge, uptime, version, nftables sync status

3. **Top applications** (Card): vertical bar chart or ranked list showing top 5 apps by connection count. Each row: app icon placeholder, app name (mono), connection count badge (cyan)

4. **Top destinations** (Card): ranked list showing top 5 remote IPs. Each row: IP (mono), count badge, flag placeholder

5. **Tendance du trafic** (Card): simple SVG time series showing connections over the last 60 data points (1 per second). Cyan line for allowed, red line for blocked. CSS-only or inline SVG (no heavy chart library)

6. **Alertes recentes** (Card): list of last 5 system errors/warnings. Each: timestamp (mono), severity badge, message truncated

### 7.2 Connections (`/connections`)

**Layout:** Full-width table with filters bar above.

**Filters bar:**
- Search input (text search across app name, IP, PID)
- Protocol dropdown: Tous, TCP, UDP, ICMP
- Verdict dropdown: Tous, Autorise, Bloque, En attente
- Direction dropdown: Tous, Entrant, Sortant
- Clear filters button

**Table columns:**
| Column | Data | Font |
|---|---|---|
| Application | process_name or "Inconnu" | sans |
| PID | process PID | mono |
| Utilisateur | user | sans |
| Adresse locale | source IP:port | mono |
| Adresse distante | destination IP:port | mono |
| Protocole | TCP/UDP/ICMP | mono, badge |
| Etat | connection state | badge, color-coded |
| Verdict | allowed/blocked/pending | badge, green/red/orange |
| Regle | matched rule name or "--" | sans |

**Virtual scrolling:** For large connection lists (1000+), use a virtual scroll container that only renders visible rows + a buffer. Implement with a simple Svelte component that calculates visible range from scroll position and row height (40px).

**Row click:** Expands an inline detail panel below the row showing:
- Full process path
- Bytes sent/received
- Connection started_at
- Full matched rule details if any
- Connection ID

**Auto-refresh:** Table updates in real time via connection store events. New rows appear at top with a subtle fade-in animation.

### 7.3 Rules (`/rules`)

**Layout:** List of rule cards + create button.

**Header:** "Regles de pare-feu" title + "Nouvelle regle" primary button + placeholder buttons "Importer" / "Exporter" (disabled, tooltip "Bientot disponible")

**Rule list:** Each rule displayed as a Card:
- Left: priority badge (number, purple), enabled/disabled toggle switch
- Center: rule name (bold), effect badge (Autoriser=green, Bloquer=red, Demander=orange, Observer=purple), source badge (Manuelle=cyan, Auto-apprentissage=purple, Systeme=neutral)
- Right: criteria summary (truncated text showing matched fields), created date (mono, muted)
- Actions: edit button (ghost), delete button (danger ghost)

**Create/Edit form** (Modal):
- Name input
- Priority number input (1-1000)
- Effect select: Autoriser, Bloquer, Demander, Observer
- Source select: Manuelle, Systeme
- Scope select: Permanente, Temporaire (with datetime picker if temporary)
- **Criteria builder**: a dynamic form section where each criterion is a row:
  - Field select: Application, Utilisateur, IP distante, Port distant, Port local, Protocole, Direction
  - Operator: depends on field (contains, equals, CIDR match, range, etc.)
  - Value input
  - Add/remove criterion buttons
- Validate + Create/Update buttons

**Delete confirmation:** Modal with rule name, "Supprimer cette regle ?" message, "Annuler" ghost button, "Supprimer" danger button.

**Toggle:** Clicking the toggle immediately calls `toggle_rule` and updates optimistically.

### 7.4 Auto-Learning (`/learning`)

**Primary UI:** Decision prompt overlay/modal.

When `pendingDecisions` has items and `showDecisionOverlay` is true:

**Decision Prompt (Modal, glass effect, glow-cyan):**
- Header: "Nouvelle connexion detectee" with pulsing cyan dot
- Connection info (Card inside modal):
  - Application name (large, bold) + icon placeholder
  - Chemin: process path (mono, truncated)
  - PID / Utilisateur (mono)
  - Destination: IP:port (mono, large)
  - Protocole: badge (TCP/UDP)
  - Direction: badge (Entrant/Sortant)
- **Countdown timer:** circular progress or horizontal bar showing time remaining before expiration (from `expires_at`). Label: "Expire dans XX s"
- **Granularity selector:** radio group
  - "Application seule" (app_only)
  - "Application + destination" (app_and_destination)
  - "Application + protocole" (app_and_protocol)
  - "Correspondance complete" (full)
- **Action buttons** (grid of 6):
  - "Autoriser une fois" (ghost, green border)
  - "Bloquer une fois" (ghost, red border)
  - "Toujours autoriser" (success, solid green)
  - "Toujours bloquer" (danger, solid red)
  - "Creer une regle" (primary, cyan) -- opens rule form pre-filled
  - "Ignorer" (ghost, neutral)

**Decision queue:** When multiple decisions are pending:
- Queue counter badge in sidebar nav item ("Apprentissage (3)")
- Prompt shows current decision with "1/3" indicator and next/previous arrows
- List view below the prompt showing queued decisions as compact cards

**Page background** (`/learning` route): shows the decision queue as a persistent list even when no overlay is open, plus statistics: total decisions taken, allow vs block ratio.

### 7.5 Audit (`/audit`)

**Layout:** Filters bar + paginated table.

**Filters bar:**
- Search input (full-text across description)
- Severity dropdown: Tous, Debug, Info, Avertissement, Erreur, Critique
- Category dropdown: Tous, Connexion, Regle, Decision, Systeme, Configuration
- Date range: start date input + end date input
- Clear filters button

**Table columns:**
| Column | Data | Font |
|---|---|---|
| Horodatage | ISO timestamp | mono |
| Severite | severity level | badge (debug=neutral, info=cyan, warning=orange, error=red, critical=red+glow) |
| Categorie | event category | badge (purple) |
| Description | event text | sans, truncated |

**Row click:** Expands to show:
- Full description text
- Metadata key-value pairs in a mini table
- Event ID (mono, muted)

**Pagination:** "Precedent" / "Suivant" buttons with page indicator "Page 1 / N". 50 events per page.

### 7.6 Settings (`/settings`)

**Layout:** Vertical stack of config cards.

**Cards:**
1. **Pare-feu** (Card):
   - Statut: enabled/disabled with toggle (calls backend)
   - Politique par defaut: display value (ask/block/allow)
   - Table nftables: display table name
   - Synchronise: boolean badge

2. **Apprentissage** (Card):
   - Actif: enabled badge
   - Delai d'expiration: XX secondes
   - Action par defaut: display (block/allow)
   - Decisions en attente max: number

3. **Interface** (Card):
   - Theme: "Sombre" (display only, toggle infrastructure ready via data-theme attribute)
   - Langue: "Francais" (display only)
   - Intervalle de rafraichissement: XX ms

4. **Daemon** (Card):
   - Socket: path display (mono)
   - Version: display
   - Uptime: formatted duration

---

## 8. Navigation

### 8.1 Sidebar

Fixed-position sidebar on the left, 240px wide, full height.

**Header section:**
- SysWall logo/text (stylized, accent-cyan)
- Firewall status indicator: green dot + "Actif" or red dot + "Inactif"

**Navigation items:**

| Icon | Label | Route | Badge |
|---|---|---|---|
| grid | Tableau de bord | /dashboard | -- |
| activity | Connexions | /connections | active count |
| shield | Regles | /rules | rule count |
| brain | Apprentissage | /learning | pending decision count (pulsing if > 0) |
| scroll-text | Journal | /audit | -- |
| settings | Parametres | /settings | -- |

**Active state:** left border accent-cyan (3px), background bg-hover, text accent-cyan.

**Icons:** Use simple inline SVG icons (no external icon library to keep bundle small). 6 icons total, embedded as Svelte components or inline SVG strings.

### 8.2 Layout

```
+------+----------------------------------------+
|      |                                        |
| Side |           Content Area                 |
| bar  |        (route outlet)                  |
| 240  |                                        |
|  px  |                                        |
|      |                                        |
+------+----------------------------------------+
```

Content area has:
- Top padding: 2rem
- Horizontal padding: 2rem
- Max width: none (fills remaining space)
- Scrollable independently

### 8.3 Decision Overlay

When a `decision_required` event arrives and the user is not on `/learning`:
- A toast notification appears in the bottom-right corner: "Nouvelle connexion detectee -- Cliquez pour decider"
- Clicking navigates to `/learning`
- The sidebar "Apprentissage" badge pulses

---

## 9. i18n (French Labels)

All user-visible text lives in `src/lib/i18n/fr.ts` as a flat object:

```typescript
export const fr = {
  // Navigation
  nav_dashboard: "Tableau de bord",
  nav_connections: "Connexions",
  nav_rules: "Regles",
  nav_learning: "Apprentissage",
  nav_audit: "Journal",
  nav_settings: "Parametres",

  // Statut
  status_active: "Actif",
  status_inactive: "Inactif",
  status_synced: "Synchronise",
  status_not_synced: "Non synchronise",

  // Dashboard
  dash_active_connections: "Connexions actives",
  dash_allowed: "Autorisees",
  dash_blocked: "Bloquees",
  dash_alerts: "Alertes",
  dash_top_apps: "Top applications",
  dash_top_destinations: "Top destinations",
  dash_traffic_trend: "Tendance du trafic",
  dash_recent_alerts: "Alertes recentes",
  dash_firewall_status: "Etat du pare-feu",

  // Connections
  conn_application: "Application",
  conn_pid: "PID",
  conn_user: "Utilisateur",
  conn_local_addr: "Adresse locale",
  conn_remote_addr: "Adresse distante",
  conn_protocol: "Protocole",
  conn_state: "Etat",
  conn_verdict: "Verdict",
  conn_rule: "Regle",
  conn_unknown: "Inconnu",
  conn_search: "Rechercher...",
  conn_filter_protocol: "Protocole",
  conn_filter_verdict: "Verdict",
  conn_filter_direction: "Direction",
  conn_filter_all: "Tous",
  conn_allowed: "Autorise",
  conn_blocked: "Bloque",
  conn_pending: "En attente",
  conn_inbound: "Entrant",
  conn_outbound: "Sortant",
  conn_clear_filters: "Effacer les filtres",

  // Rules
  rules_title: "Regles de pare-feu",
  rules_new: "Nouvelle regle",
  rules_import: "Importer",
  rules_export: "Exporter",
  rules_coming_soon: "Bientot disponible",
  rules_name: "Nom",
  rules_priority: "Priorite",
  rules_effect: "Effet",
  rules_source: "Source",
  rules_scope: "Portee",
  rules_criteria: "Criteres",
  rules_created_at: "Cree le",
  rules_allow: "Autoriser",
  rules_block: "Bloquer",
  rules_ask: "Demander",
  rules_observe: "Observer",
  rules_manual: "Manuelle",
  rules_auto_learning: "Auto-apprentissage",
  rules_import_source: "Importee",
  rules_system: "Systeme",
  rules_permanent: "Permanente",
  rules_temporary: "Temporaire",
  rules_edit: "Modifier",
  rules_delete: "Supprimer",
  rules_delete_confirm: "Supprimer cette regle ?",
  rules_delete_message: "Cette action est irreversible.",
  rules_cancel: "Annuler",
  rules_save: "Enregistrer",
  rules_create: "Creer",

  // Criteria builder
  criteria_application: "Application",
  criteria_user: "Utilisateur",
  criteria_remote_ip: "IP distante",
  criteria_remote_port: "Port distant",
  criteria_local_port: "Port local",
  criteria_protocol: "Protocole",
  criteria_direction: "Direction",
  criteria_add: "Ajouter un critere",
  criteria_remove: "Retirer",

  // Learning / Decision
  learn_title: "Apprentissage",
  learn_new_connection: "Nouvelle connexion detectee",
  learn_path: "Chemin",
  learn_destination: "Destination",
  learn_expires_in: "Expire dans",
  learn_granularity: "Granularite de la regle",
  learn_app_only: "Application seule",
  learn_app_destination: "Application + destination",
  learn_app_protocol: "Application + protocole",
  learn_full_match: "Correspondance complete",
  learn_allow_once: "Autoriser une fois",
  learn_block_once: "Bloquer une fois",
  learn_always_allow: "Toujours autoriser",
  learn_always_block: "Toujours bloquer",
  learn_create_rule: "Creer une regle",
  learn_ignore: "Ignorer",
  learn_queue: "File d'attente",
  learn_decision_of: "sur",
  learn_toast: "Nouvelle connexion detectee -- Cliquez pour decider",

  // Audit
  audit_title: "Journal d'audit",
  audit_timestamp: "Horodatage",
  audit_severity: "Severite",
  audit_category: "Categorie",
  audit_description: "Description",
  audit_search: "Rechercher...",
  audit_debug: "Debug",
  audit_info: "Info",
  audit_warning: "Avertissement",
  audit_error: "Erreur",
  audit_critical: "Critique",
  audit_connection: "Connexion",
  audit_rule: "Regle",
  audit_decision: "Decision",
  audit_system: "Systeme",
  audit_config: "Configuration",
  audit_previous: "Precedent",
  audit_next: "Suivant",
  audit_page: "Page",

  // Settings
  settings_title: "Parametres",
  settings_firewall: "Pare-feu",
  settings_learning: "Apprentissage",
  settings_interface: "Interface",
  settings_daemon: "Daemon",
  settings_status: "Statut",
  settings_default_policy: "Politique par defaut",
  settings_nftables_table: "Table nftables",
  settings_enabled: "Active",
  settings_timeout: "Delai d'expiration",
  settings_default_action: "Action par defaut",
  settings_max_pending: "Decisions en attente max",
  settings_theme: "Theme",
  settings_theme_dark: "Sombre",
  settings_locale: "Langue",
  settings_locale_fr: "Francais",
  settings_refresh_interval: "Intervalle de rafraichissement",
  settings_socket: "Socket",
  settings_version: "Version",
  settings_uptime: "Temps de fonctionnement",

  // Common
  common_loading: "Chargement...",
  common_error: "Une erreur est survenue",
  common_retry: "Reessayer",
  common_empty: "Aucune donnee",
  common_seconds: "secondes",
};
```

---

## 10. Error Handling

### 10.1 Connection Errors

If the Tauri Rust side cannot connect to the daemon:
- All commands return an error string
- The UI shows an ErrorBanner at the top of the content area: "Impossible de se connecter au daemon SysWall. Verifiez que le service est actif."
- Retry button attempts reconnection

### 10.2 Empty States

Each view has a graceful empty state:
- Dashboard with zeroed stats and "En attente de connexions..." message
- Connections: "Aucune connexion active"
- Rules: "Aucune regle configuree. Creez votre premiere regle."
- Audit: "Aucun evenement enregistre"

### 10.3 Loading States

- Each view shows a LoadingSpinner centered during initial data fetch
- Subsequent updates (from events) are applied without full reload

---

## 11. Performance Considerations

- **Virtual scrolling** for connections table when > 100 rows
- **Debounced search** input (300ms delay)
- **Lazy rendering** of expanded row details (only mount when expanded)
- **Event batching** in stores: buffer incoming events for 100ms before triggering reactivity (prevents UI thrashing during connection bursts)
- **No heavy charting library**: traffic trend chart is a simple inline SVG with path elements, updated by appending data points to a fixed-size ring buffer
- **Minimal dependencies**: no external UI library, no icon font, no chart library -- pure CSS + Svelte components

---

## 12. File Inventory

| File | Type | Lines (estimate) |
|---|---|---|
| `src-tauri/Cargo.toml` | Modify | 30 |
| `src-tauri/src/lib.rs` | Rewrite | 60 |
| `src-tauri/src/grpc_client.rs` | Create | 90 |
| `src-tauri/src/streams.rs` | Create | 80 |
| `src-tauri/src/commands/mod.rs` | Create | 10 |
| `src-tauri/src/commands/status.rs` | Create | 25 |
| `src-tauri/src/commands/rules.rs` | Create | 70 |
| `src-tauri/src/commands/decisions.rs` | Create | 50 |
| `src/app.html` | Modify | 15 |
| `src/app.css` | Rewrite | 250 |
| `src/lib/types/index.ts` | Create | 100 |
| `src/lib/api/client.ts` | Create | 80 |
| `src/lib/stores/status.ts` | Create | 30 |
| `src/lib/stores/connections.ts` | Create | 80 |
| `src/lib/stores/rules.ts` | Create | 50 |
| `src/lib/stores/decisions.ts` | Create | 60 |
| `src/lib/stores/audit.ts` | Create | 50 |
| `src/lib/stores/dashboard.ts` | Create | 60 |
| `src/lib/i18n/fr.ts` | Create | 150 |
| `src/lib/components/ui/Card.svelte` | Create | 40 |
| `src/lib/components/ui/Badge.svelte` | Create | 50 |
| `src/lib/components/ui/Button.svelte` | Create | 55 |
| `src/lib/components/ui/Input.svelte` | Create | 40 |
| `src/lib/components/ui/Table.svelte` | Create | 100 |
| `src/lib/components/ui/Modal.svelte` | Create | 60 |
| `src/lib/components/ui/StatCard.svelte` | Create | 45 |
| `src/lib/components/ui/Sidebar.svelte` | Create | 120 |
| `src/lib/components/ui/EmptyState.svelte` | Create | 25 |
| `src/lib/components/ui/LoadingSpinner.svelte` | Create | 30 |
| `src/lib/components/ui/ErrorBanner.svelte` | Create | 35 |
| `src/routes/+layout.svelte` | Rewrite | 80 |
| `src/routes/+layout.ts` | Keep | 3 |
| `src/routes/+page.svelte` | Rewrite | 10 |
| `src/routes/dashboard/+page.svelte` | Create | 150 |
| `src/routes/connections/+page.svelte` | Create | 200 |
| `src/routes/rules/+page.svelte` | Create | 250 |
| `src/routes/learning/+page.svelte` | Create | 200 |
| `src/routes/audit/+page.svelte` | Create | 150 |
| `src/routes/settings/+page.svelte` | Create | 120 |
| `src/lib/components/dashboard/TrafficChart.svelte` | Create | 80 |
| `src/lib/components/dashboard/TopApps.svelte` | Create | 50 |
| `src/lib/components/dashboard/TopDestinations.svelte` | Create | 50 |
| `src/lib/components/dashboard/RecentAlerts.svelte` | Create | 50 |
| `src/lib/components/connections/ConnectionTable.svelte` | Create | 120 |
| `src/lib/components/connections/ConnectionDetail.svelte` | Create | 60 |
| `src/lib/components/connections/ConnectionFilters.svelte` | Create | 70 |
| `src/lib/components/rules/RuleList.svelte` | Create | 80 |
| `src/lib/components/rules/RuleForm.svelte` | Create | 180 |
| `src/lib/components/rules/RuleCriteriaBuilder.svelte` | Create | 120 |
| `src/lib/components/rules/DeleteConfirmModal.svelte` | Create | 40 |
| `src/lib/components/learning/DecisionPrompt.svelte` | Create | 150 |
| `src/lib/components/learning/DecisionQueue.svelte` | Create | 70 |
| `src/lib/components/learning/DecisionCountdown.svelte` | Create | 50 |
| `src/lib/components/audit/AuditTable.svelte` | Create | 100 |
| `src/lib/components/audit/AuditFilters.svelte` | Create | 70 |

**Total estimated:** ~4,200 lines of new/rewritten code across 53 files.
