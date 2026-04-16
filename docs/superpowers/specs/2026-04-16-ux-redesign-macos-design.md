# SysWall UX Redesign — macOS-style Design
# SysWall Refonte UX — Design style macOS

**Date**: 2026-04-16
**Status**: Approved / Approuvé
**Inspiration**: Little Snitch, Lulu, macOS System Preferences

---

## Color Palette / Palette de couleurs

macOS dark mode system colors:

| CSS Variable | Value | Usage |
|---|---|---|
| `--bg-primary` | `#1c1c1e` | Main background / Fond principal |
| `--bg-secondary` | `#2c2c2e` | Cards, panels / Cartes, panneaux |
| `--bg-tertiary` | `#3a3a3c` | Nested elements / Éléments imbriqués |
| `--bg-hover` | `rgba(255,255,255,0.06)` | Hover states |
| `--bg-sidebar` | `rgba(28,28,30,0.95)` | Sidebar with blur |
| `--accent-blue` | `#0a84ff` | Primary actions, links, selection |
| `--accent-green` | `#34c759` | Allowed, success |
| `--accent-red` | `#ff453a` | Blocked, error |
| `--accent-orange` | `#ff9f0a` | Warning, pending |
| `--accent-purple` | `#5e5ce6` | Critical, secondary accent |
| `--text-primary` | `#e5e5ea` | Primary text |
| `--text-secondary` | `#8e8e93` | Labels, secondary text |
| `--text-tertiary` | `#636366` | Disabled, placeholder |
| `--border-primary` | `rgba(255,255,255,0.06)` | Subtle borders |
| `--border-focus` | `rgba(10,132,255,0.5)` | Focus ring |

## Typography / Typographie

- Font family: `-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'Segoe UI', Roboto, sans-serif`
- Monospace: `'SF Mono', 'Fira Code', 'Cascadia Code', monospace`
- Base size: 13px
- Weights: 400 (regular), 500 (medium), 600 (semibold), 700 (bold)
- Letter spacing: -0.2px for headings, 0.5px for uppercase labels

## Border Radius / Arrondis

- `--radius-sm`: 6px (buttons, inputs, badges)
- `--radius-md`: 8px (info cards, small panels)
- `--radius-lg`: 10px (cards, modal panels)
- `--radius-xl`: 12px (popup window, main containers)
- `--radius-pill`: 20px (pill buttons, kill-switch indicator)

## Layout

### Sidebar (180px width)
- Background: `--bg-sidebar` with `backdrop-filter: blur(20px)`
- Logo: 26px icon with gradient `#0a84ff → #5e5ce6` + "SysWall" text
- Nav items: 14px emoji icon + 12px label, active item `rgba(10,132,255,0.12)` bg with blue text
- Badges: pill shape, right-aligned. Orange bg for pending count, subtle gray for counts
- Separator: 1px `--border-primary` between main nav and secondary nav
- Settings at bottom, gray text
- Responsive: collapses to icons on tablet, bottom bar on mobile

### Top bar (40px height)
- Background: `rgba(44,44,46,0.6)` with `backdrop-filter: blur(20px)`
- Left: page title (15px, semibold, white)
- Center-right: search bar (220px, subtle bg, magnifier icon, 12px placeholder)
- Right: kill-switch pill (green dot + "Réseau actif" or red dot + "Réseau coupé")
- Border bottom: `--border-primary`

### Content area
- Background: `--bg-primary`
- Padding: 20px
- Scrollable

## Components / Composants

### Stat Cards
- Background: `--bg-secondary`
- Border radius: `--radius-lg` (10px)
- Padding: 14px
- Label: 10px uppercase, `--text-secondary`, letter-spacing 0.5px
- Value: 22px semibold, color depends on metric (white default, red for blocked, green for allowed, orange for pending)
- Optional trend indicator: "↑ 12%" in green or "↓ 5%" in red

### Data Tables
- No visible outer border
- Header: 10px uppercase, `--text-secondary`, `--bg-tertiary` background
- Rows: separated by `rgba(255,255,255,0.04)` bottom border
- Hover: `--bg-hover`
- Expanded row: `--bg-tertiary` background, slide-down animation
- Font: 12px for cells, monospace for IPs/ports/IDs

### Badges / Pills
- Border radius: `--radius-sm` (6px) for inline badges, `--radius-pill` (20px) for pills
- Font: 9-10px, medium weight
- Variants: colored bg at 12% opacity + colored text (e.g., `rgba(10,132,255,0.12)` + `#0a84ff`)

### Buttons
- **Primary**: solid colored bg (`#34c759` for allow, `#ff453a` for block, `#0a84ff` for action), white text, 13px semibold, 8px radius
- **Secondary**: `rgba(255,255,255,0.04)` bg + `rgba(255,255,255,0.08)` border, `--text-primary` text, 11px
- **Ghost/Link**: no bg, colored text (blue for actions, gray for dismiss), 11px

### Traffic Chart
- uPlot with transparent dark theme
- Series: allowed = `#0a84ff` (blue), blocked = `#ff453a` (red)
- Grid: `rgba(255,255,255,0.04)`
- Legend: small dots + labels below chart

## Decision Popup / Popup de décision

Style B (extended):

- Window: 420px wide, `--bg-secondary` background, `--radius-xl` corners, always-on-top
- **Timer bar**: 3px height at very top, `--accent-orange` fill, animated width decreasing
- **Header**: 48px app icon (rounded 12px) + "Firefox veut se connecter" (15px semibold) + hostname in blue (13px) + connection details in gray mono (11px) + countdown number (22px bold orange)
- **Info cards**: 2x2 grid, `--bg-tertiary` background, 8px radius. Labels: 9px uppercase gray. Values: 11px mono.
  - Source: `192.168.1.42:52847`
  - Direction: `↗ Sortante`
  - Exécutable: `/usr/lib/firefox/firefox`
  - Utilisateur: `seb (UID 1000)`
- **Primary actions**: two large buttons side-by-side. "Autoriser" green solid, "Bloquer" red solid. 13px semibold white text.
- **Secondary actions**: "Toujours autoriser" / "Toujours bloquer" with subtle bg + border, 11px
- **Tertiary actions**: "Créer une règle" (blue link) · "Ignorer" (gray text), centered, 11px

## Pages

### Dashboard
- 4 stat cards in a row (Connexions, Bloquées, Autorisées, En attente)
- Full-width traffic chart (uPlot, 80px height)
- Two-column grid: Top Applications (with app emojis/icons) + Top Destinations (with service badges)
- Recent alerts (if any): severity badge + description + timestamp

### Connections
- Top bar search: unified `app:X dest:Y port:Z` syntax with filter chips
- Table: App (icon+name), PID, User, Source IP, Source Port, Dest IP, Dest Port, Protocol badge, State, Verdict badge
- Expandable rows: connection details + process details (exe, cmdline, cwd, threads, memory, open ports)
- Clickable items in dashboard navigate here with pre-filled filters

### Rules
- Table with enable/disable toggles, priority ordering, effect badge (Allow green, Block red, Ask orange)
- Create rule form in modal
- System rules visually distinct (gray, non-deletable)

### Learning
- Queue of pending decisions with navigation
- Each decision uses the popup component inline (not as a separate window — separate window is for the always-on-top popup)

### Blocklists
- Table of loaded blocklists: name, entry count, enabled toggle, source
- Import button + reload button
- Stats: total blocked, top blocked domains

### Audit / Journal
- Table with colored severity band on left (blue=info, orange=warning, red=error, purple=critical)
- Colorized descriptions (IPs in blue mono, verdicts in green/red, protocols highlighted)
- Expandable metadata badges
- Filters + pagination + JSON export

### Settings
- Grouped settings in macOS-style sections
- Toggles, dropdowns, input fields

## Animations / Transitions

- Page transitions: `fade` 150ms in, 100ms out
- Card hover: subtle brightness increase
- Sidebar item hover: `--bg-hover` with 100ms transition
- Expanded rows: slide-down 200ms ease
- Toast notifications: fly-in from bottom-right, fade-out
- Popup timer bar: smooth width animation matching countdown
- Kill-switch dot: subtle pulse glow when active

---

## Files to modify

### CSS overhaul
- `crates/ui/src/app.css` — Complete palette + typography + spacing rewrite

### Layout
- `crates/ui/src/routes/+layout.svelte` — Hybrid layout (sidebar + top bar)
- `crates/ui/src/lib/components/ui/Sidebar.svelte` — macOS-style sidebar

### Components
- All components in `crates/ui/src/lib/components/ui/` — Update to new design tokens
- `crates/ui/src/lib/components/learning/DecisionPrompt.svelte` — Style B popup design

### Pages
- All pages in `crates/ui/src/routes/*/+page.svelte` — Apply new design
- `crates/ui/src/routes/popup/decision/+page.svelte` — Match popup design
