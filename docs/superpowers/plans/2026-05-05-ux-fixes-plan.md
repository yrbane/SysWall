# UX Fixes Implementation Plan — SysWall sub-project D

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 8 blocking UX issues raised by the audit: killswitch undo, popup keyboard shortcuts, audit virtualization, modal focus trap, contrast WCAG AA, debounced filters, switch role on toggles, mobile tap targets.

**Architecture:** Pure SvelteKit/Svelte 5 changes. No new test framework — verification via `pnpm check` (svelte-check + tsc) for type safety, plus manual checks documented in each task. New utilities live in `crates/ui/src/lib/utils/` and `crates/ui/src/lib/actions/`. The `Toast` store gains an action handler. Existing `Table.svelte` is reused (already has virtual scroll).

**Tech Stack:** Svelte 5 (runes), SvelteKit, TypeScript, Tauri 2, CSS variables.

**Spec source:** `docs/superpowers/specs/2026-05-05-ux-fixes-design.md`

---

## Conventions for every task

- Comments and commit messages in **French**.
- Code identifiers in English.
- **NEVER add `Co-Authored-By Claude` lines** in any commit.
- Each commit must pass `pnpm check` (run `cd crates/ui && pnpm check`).
- Use Svelte 5 runes (`$state`, `$derived`, `$effect`, `$props`) consistently with existing code.
- Comments bilingual (EN+FR) on public APIs.

---

## File Structure

### New files

| File | Responsibility |
|---|---|
| `crates/ui/src/lib/utils/debounce.ts` | `debounce(fn, ms)` utility |
| `crates/ui/src/lib/actions/focus_trap.ts` | Svelte action: trap Tab + restore focus on destroy |

### Modified files

| File | Change |
|---|---|
| `crates/ui/src/lib/stores/toast.ts` | Add `action?: { label, handler }` to `ToastMessage`; add optional `id` overrides |
| `crates/ui/src/lib/components/ui/Toast.svelte` | Render action button + 5 s progress bar |
| `crates/ui/src/routes/+layout.svelte` | Killswitch handler with undo toast (replaces direct `setNetworkEnabled`) |
| `crates/ui/src/lib/components/learning/DecisionPrompt.svelte` | Keyboard shortcuts + `<kbd>` badges |
| `crates/ui/src/routes/audit/+page.svelte` | Replace pagination with `<Table>` virtualized |
| `crates/ui/src/lib/components/ui/Modal.svelte` | Apply `use:focusTrap` |
| `crates/ui/src/app.css` | `--text-tertiary` contrast fix + new `--text-disabled` token |
| `crates/ui/src/routes/connections/+page.svelte` | Debounce search input |
| `crates/ui/src/routes/audit/+page.svelte` | Debounce search input (combined with virtualization in same task) |
| `crates/ui/src/routes/rules/+page.svelte` | Toggle button → `role="switch"` + `aria-checked` |
| `crates/ui/src/lib/components/ui/Sidebar.svelte` | Mobile media query: padding ≥ 12 px → tap target ≥ 44 px |
| `CHANGELOG.md` | Section "UX & Accessibility" under [0.2.0] |

---

## Task 1: `debounce` utility

**Files:**
- Create: `crates/ui/src/lib/utils/debounce.ts`

- [ ] **Step 1.1: Create the utility**

```typescript
/**
 * Debounce a function: delays invocation until `delayMs` has passed without
 * a new call. Each call resets the timer.
 *
 * Diffère l'invocation jusqu'à ce que `delayMs` se soit écoulé sans nouvel appel.
 * Chaque appel réinitialise le timer.
 */
export function debounce<F extends (...args: never[]) => void>(
  fn: F,
  delayMs: number,
): F {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return ((...args: Parameters<F>) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => fn(...args), delayMs);
  }) as F;
}
```

- [ ] **Step 1.2: Verify**

Run: `cd crates/ui && pnpm check 2>&1 | tail`
Expected: 0 errors.

- [ ] **Step 1.3: Commit**

```bash
git add crates/ui/src/lib/utils/debounce.ts
git commit -m "feat(ui): utilitaire debounce(fn, delayMs)"
```

---

## Task 2: `focus_trap` action

**Files:**
- Create: `crates/ui/src/lib/actions/focus_trap.ts`

- [ ] **Step 2.1: Create the action**

```typescript
import type { Action } from 'svelte/action';

const FOCUSABLE_SELECTOR =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Svelte action: trap Tab cycling within the element and restore focus on destroy.
 *
 * Action Svelte : capture le cycle Tab à l'intérieur de l'élément et restitue
 * le focus à la fermeture.
 *
 * Usage : `<div use:focusTrap role="dialog">...</div>`
 */
export const focusTrap: Action<HTMLElement> = (node) => {
  const previouslyFocused = document.activeElement as HTMLElement | null;

  const focusables = (): HTMLElement[] =>
    Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));

  // Auto-focus the first focusable element on mount.
  // Focus automatique sur le premier élément focusable au montage.
  const first = focusables()[0];
  first?.focus();

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== 'Tab') return;
    const list = focusables();
    if (list.length === 0) return;
    const firstEl = list[0];
    const lastEl = list[list.length - 1];
    if (e.shiftKey && document.activeElement === firstEl) {
      lastEl.focus();
      e.preventDefault();
    } else if (!e.shiftKey && document.activeElement === lastEl) {
      firstEl.focus();
      e.preventDefault();
    }
  }

  node.addEventListener('keydown', onKeydown);

  return {
    destroy() {
      node.removeEventListener('keydown', onKeydown);
      previouslyFocused?.focus();
    },
  };
};
```

- [ ] **Step 2.2: Verify**

Run: `cd crates/ui && pnpm check 2>&1 | tail`
Expected: 0 errors.

- [ ] **Step 2.3: Commit**

```bash
git add crates/ui/src/lib/actions/focus_trap.ts
git commit -m "feat(ui): action focusTrap (capture Tab + restitue le focus a la fermeture)"
```

---

## Task 3: Toast extension (action button + progress bar)

**Files:**
- Modify: `crates/ui/src/lib/stores/toast.ts`
- Modify: `crates/ui/src/lib/components/ui/Toast.svelte`

- [ ] **Step 3.1: Extend the store**

Replace the contents of `crates/ui/src/lib/stores/toast.ts`:

```typescript
// Store de notifications toast
// Toast notification store

import { writable } from 'svelte/store';

export interface ToastAction {
  label: string;
  handler: () => void | Promise<void>;
}

export interface ToastMessage {
  id: string;
  message: string;
  variant: 'success' | 'error' | 'warning' | 'info';
  duration?: number;
  action?: ToastAction;
}

const { subscribe, update } = writable<ToastMessage[]>([]);

export const toasts = { subscribe };

/**
 * Ajoute une notification toast (avec action optionnelle).
 * Adds a toast notification (with optional action button).
 */
export function addToast(
  message: string,
  variant: ToastMessage['variant'] = 'info',
  duration = 4000,
  action?: ToastAction,
): string {
  const id = crypto.randomUUID();
  update((all) => [...all, { id, message, variant, duration, action }]);
  if (duration > 0) {
    setTimeout(() => removeToast(id), duration);
  }
  return id;
}

/**
 * Supprime une notification toast par son identifiant.
 * Removes a toast notification by its id.
 */
export function removeToast(id: string) {
  update((all) => all.filter((t) => t.id !== id));
}
```

- [ ] **Step 3.2: Update Toast.svelte to render action + progress bar**

Inspect existing `crates/ui/src/lib/components/ui/Toast.svelte` first to follow its style. Add:
- A `{#if action}<button class="toast-action" onclick={...}>{action.label}</button>{/if}` block.
- A 5 s linear progress bar at the bottom (CSS animation), shown only when `duration > 0`.

Pattern (adapt to existing markup):

```svelte
<script lang="ts">
  import { removeToast, type ToastMessage } from '$lib/stores/toast';

  interface Props {
    toast: ToastMessage;
  }
  let { toast }: Props = $props();

  async function onAction() {
    if (toast.action) {
      await toast.action.handler();
      removeToast(toast.id);
    }
  }
</script>

<div class="toast toast-{toast.variant}" role="alert" aria-live="polite">
  <span class="toast-message">{toast.message}</span>
  {#if toast.action}
    <button class="toast-action" onclick={onAction} type="button">
      {toast.action.label}
    </button>
  {/if}
  <button
    class="toast-close"
    onclick={() => removeToast(toast.id)}
    aria-label="Fermer"
    type="button"
  >
    ×
  </button>
  {#if toast.duration && toast.duration > 0}
    <div
      class="toast-progress"
      style:animation-duration="{toast.duration}ms"
      aria-hidden="true"
    ></div>
  {/if}
</div>

<style>
  .toast { /* existing styles preserved */ position: relative; padding-bottom: 2px; }
  .toast-action {
    background: transparent;
    border: 1px solid currentColor;
    color: inherit;
    padding: 4px 12px;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    cursor: pointer;
    margin-left: var(--space-2);
  }
  .toast-action:hover { background: rgba(255, 255, 255, 0.08); }
  .toast-progress {
    position: absolute;
    bottom: 0;
    left: 0;
    height: 2px;
    background: currentColor;
    opacity: 0.4;
    animation: toast-shrink linear forwards;
  }
  @keyframes toast-shrink {
    from { width: 100%; }
    to { width: 0%; }
  }
</style>
```

If the existing `Toast.svelte` already has a different structure, preserve its outer markup and only add the new bits (`action` button, progress bar). Don't restructure beyond what's necessary.

- [ ] **Step 3.3: Verify**

```bash
cd crates/ui && pnpm check 2>&1 | tail
```

Expected: 0 errors.

- [ ] **Step 3.4: Commit**

```bash
git add crates/ui/src/lib/stores/toast.ts crates/ui/src/lib/components/ui/Toast.svelte
git commit -m "feat(ui): toast supporte une action optionnelle + barre de progression"
```

---

## Task 4: Killswitch action immédiate + toast undo

**Files:**
- Modify: `crates/ui/src/routes/+layout.svelte`

- [ ] **Step 4.1: Inspect existing handler**

```bash
grep -n 'toggleNetwork\|setNetworkEnabled\|killswitch' crates/ui/src/routes/+layout.svelte | head
```

The handler is around line 62 (`async function toggleNetwork()`). The button is at line 107.

- [ ] **Step 4.2: Replace `toggleNetwork`**

Current pattern (approximate):
```svelte
<script lang="ts">
  import { setNetworkEnabled } from '$lib/api/client';
  let networkEnabled = $state(true);
  async function toggleNetwork() {
    await setNetworkEnabled(!networkEnabled);
    networkEnabled = !networkEnabled;
  }
</script>
```

Replace with:

```svelte
<script lang="ts">
  import { setNetworkEnabled } from '$lib/api/client';
  import { addToast } from '$lib/stores/toast';
  let networkEnabled = $state(true);

  async function toggleNetwork() {
    const previousState = networkEnabled;
    const newState = !previousState;
    try {
      await setNetworkEnabled(newState);
      networkEnabled = newState;
      const message = newState
        ? 'Reseau retabli.'
        : 'Reseau coupe. Toutes les connexions sont bloquees.';
      const variant = newState ? 'success' : 'warning';
      addToast(message, variant, 5000, {
        label: 'Annuler',
        handler: async () => {
          await setNetworkEnabled(previousState);
          networkEnabled = previousState;
        },
      });
    } catch (e) {
      addToast(`Erreur killswitch : ${e}`, 'error', 6000);
    }
  }
</script>
```

The button at line ~107 already calls `onclick={toggleNetwork}` — no markup change needed.

- [ ] **Step 4.3: Verify**

```bash
cd crates/ui && pnpm check 2>&1 | tail
```

Expected: 0 errors.

Manual check: launch `cd crates/ui && pnpm dev` (or `cargo tauri dev` if integrated), click killswitch, observe toast with "Annuler" button and 5 s progress bar. Click "Annuler" before expiry — state restored.

- [ ] **Step 4.4: Commit**

```bash
git add crates/ui/src/routes/+layout.svelte
git commit -m "feat(ui): killswitch en action immediate avec toast undo 5s"
```

---

## Task 5: Keyboard shortcuts on `DecisionPrompt`

**Files:**
- Modify: `crates/ui/src/lib/components/learning/DecisionPrompt.svelte`

- [ ] **Step 5.1: Inspect current component**

```bash
cat crates/ui/src/lib/components/learning/DecisionPrompt.svelte | head -80
```

Identify:
- The `onResponse` prop (callback invoked with the user's choice).
- The `DecisionAction` enum values used (`AllowOnce`, `BlockOnce`, `AlwaysAllow`, `AlwaysBlock`, `Ignore`, `CreateRule`...).
- The button markup.

- [ ] **Step 5.2: Add keyboard handler + `<kbd>` badges**

Inside the `<script>` block, add at the top (after existing imports):

```typescript
import { onMount } from 'svelte';
```

Inside the script body, add a keydown handler:

```typescript
function onKeydown(e: KeyboardEvent) {
  // Ne pas perturber les inputs (recherche, formulaire).
  // Don't disrupt input fields.
  if (
    e.target instanceof HTMLInputElement ||
    e.target instanceof HTMLTextAreaElement ||
    e.target instanceof HTMLSelectElement
  ) {
    return;
  }
  const key = e.key.toLowerCase();
  switch (key) {
    case 'enter':
    case 'a':
      onResponse(e.shiftKey ? 'AlwaysAllow' : 'AllowOnce');
      e.preventDefault();
      break;
    case 'b':
      onResponse(e.shiftKey ? 'AlwaysBlock' : 'BlockOnce');
      e.preventDefault();
      break;
    case 'i':
      onResponse('Ignore');
      e.preventDefault();
      break;
    case 'escape':
      // V0.2 : Esc = Ignore (le paquet est jete, le flux suivant repopup).
      // V0.2: Esc = Ignore (the packet is dropped; the next flow repops).
      onResponse('Ignore');
      e.preventDefault();
      break;
  }
}

onMount(() => {
  window.addEventListener('keydown', onKeydown);
  return () => window.removeEventListener('keydown', onKeydown);
});
```

The `onResponse` argument names must match the actual `DecisionAction` variant identifiers used by the codebase. Verify with `grep -n "AllowOnce\|BlockOnce\|AlwaysAllow\|AlwaysBlock\|Ignore" crates/ui/src/lib/components/learning/DecisionPrompt.svelte | head` — match exactly the strings the existing buttons send.

- [ ] **Step 5.3: Add `<kbd>` badges to buttons**

Locate each action button. Add a `<kbd>` after the label:

```svelte
<button onclick={() => onResponse('AllowOnce')} class="btn-allow">
  Autoriser une fois <kbd>A</kbd>
</button>
<button onclick={() => onResponse('AlwaysAllow')} class="btn-allow-always">
  Toujours autoriser <kbd>⇧A</kbd>
</button>
<button onclick={() => onResponse('BlockOnce')} class="btn-block">
  Bloquer une fois <kbd>B</kbd>
</button>
<button onclick={() => onResponse('AlwaysBlock')} class="btn-block-always">
  Toujours bloquer <kbd>⇧B</kbd>
</button>
<button onclick={() => onResponse('Ignore')} class="btn-ignore">
  Ignorer <kbd>I</kbd> / <kbd>Esc</kbd>
</button>
```

The exact button labels are in French and may already exist — preserve them, just append `<kbd>X</kbd>`.

Add CSS in the `<style>` block:

```css
kbd {
  display: inline-block;
  padding: 1px 6px;
  margin-left: 6px;
  font-size: 0.85em;
  font-family: var(--font-mono, monospace);
  background: var(--bg-tertiary, #2c2c2e);
  border: 1px solid var(--border-primary, #3a3a3c);
  border-radius: 3px;
  color: var(--text-secondary);
  vertical-align: middle;
}
```

- [ ] **Step 5.4: Verify**

```bash
cd crates/ui && pnpm check 2>&1 | tail
```

Expected: 0 errors.

Manual check: launch dev, trigger a popup decision, press `a` → AllowOnce fires; press `Shift+B` → AlwaysBlock; press `Esc` → Ignore.

- [ ] **Step 5.5: Commit**

```bash
git add crates/ui/src/lib/components/learning/DecisionPrompt.svelte
git commit -m "feat(ui): raccourcis clavier sur DecisionPrompt (a, b, shift+a/b, i, esc)"
```

---

## Task 6: Audit virtualization + debounce

**Files:**
- Modify: `crates/ui/src/routes/audit/+page.svelte`

- [ ] **Step 6.1: Inspect Table.svelte API**

```bash
cat crates/ui/src/lib/components/ui/Table.svelte | head -80
```

The component takes `columns: Column[]`, `rows: T[]`, `rowHeight?`, `maxHeight?`, `onrowclick?`, `renderCell?`. It implements virtual scroll internally.

- [ ] **Step 6.2: Refactor audit page**

Replace the pagination block in `crates/ui/src/routes/audit/+page.svelte` with a `<Table>` instance:

```svelte
<script lang="ts">
  import Table from '$lib/components/ui/Table.svelte';
  import { debounce } from '$lib/utils/debounce';
  import type { AuditEvent } from '$lib/types';
  // ... other imports preserved ...

  let allEvents = $state<AuditEvent[]>([]);
  let searchInput = $state('');
  let debouncedSearch = $state('');
  // Other filter states (severity, category, dateRange) preserved.

  const updateDebounced = debounce((v: string) => { debouncedSearch = v; }, 250);
  $effect(() => {
    updateDebounced(searchInput);
  });

  let filtered = $derived(applyFilters(allEvents, debouncedSearch, severity, category, dateRange));

  const columns = [
    { key: 'timestamp', label: 'Date', width: '180px', mono: true },
    { key: 'severity', label: 'Sevérité', width: '100px' },
    { key: 'category', label: 'Catégorie', width: '120px' },
    { key: 'description', label: 'Description' },
  ];
</script>

<!-- Filter bar preserved -->
<div class="filter-bar">
  <input bind:value={searchInput} placeholder="Rechercher..." />
  <!-- other filters -->
</div>

<Table {columns} rows={filtered} rowHeight={32} maxHeight="calc(100vh - 200px)" />
```

`applyFilters` is the existing filter logic — extract it from the previous pagination block. No behavior change to the filtering logic itself; just remove `currentPage`/`pageSize` and emit the full filtered list.

The export JSON action must use `filtered` (not the page) — verify:

```svelte
<button onclick={() => exportJson(filtered)}>Exporter</button>
```

- [ ] **Step 6.3: Verify**

```bash
cd crates/ui && pnpm check 2>&1 | tail
```

Expected: 0 errors.

Manual check: launch dev, navigate to /audit, scroll through audit events. With realistic data (NFQUEUE events), the page should scroll smoothly without lag.

- [ ] **Step 6.4: Commit**

```bash
git add crates/ui/src/routes/audit/+page.svelte
git commit -m "refactor(ui/audit): virtualisation via Table.svelte + debounce de la recherche"
```

---

## Task 7: Modal focus trap

**Files:**
- Modify: `crates/ui/src/lib/components/ui/Modal.svelte`

- [ ] **Step 7.1: Apply the action**

Inspect the current Modal:

```bash
cat crates/ui/src/lib/components/ui/Modal.svelte | head -40
```

Add the import and apply `use:focusTrap` on the dialog wrapper:

```svelte
<script lang="ts">
  import { focusTrap } from '$lib/actions/focus_trap';
  // ... existing imports ...
</script>

<div class="modal-backdrop" onclick={onclose} role="presentation">
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    aria-labelledby="modal-title"
    use:focusTrap
    onclick={(e) => e.stopPropagation()}
  >
    <slot />
  </div>
</div>
```

Adapt to the existing markup. The key change is adding `use:focusTrap` on the `[role="dialog"]` element.

- [ ] **Step 7.2: Verify**

```bash
cd crates/ui && pnpm check 2>&1 | tail
```

Expected: 0 errors.

Manual check: open a modal (e.g., delete-rule confirm), press Tab repeatedly — focus stays inside. Close the modal — focus returns to the element that opened it.

- [ ] **Step 7.3: Commit**

```bash
git add crates/ui/src/lib/components/ui/Modal.svelte
git commit -m "feat(ui): Modal applique focusTrap pour conformite WCAG 2.4.3"
```

---

## Task 8: Contrast fix (`--text-tertiary` and `--text-disabled`)

**Files:**
- Modify: `crates/ui/src/app.css`

- [ ] **Step 8.1: Update tokens**

Find the `:root { ... }` block. The current values are:
- `--text-secondary: #8e8e93;` (already 5.6:1 — good)
- `--text-tertiary: #636366;` (3.4:1 — fails WCAG AA)

Change to:
```css
  --text-secondary: #c7c7cc;       /* hierarchy bumped: secondary now lighter */
  --text-tertiary: #8e8e93;        /* WCAG AA passing: 4.6:1 vs --bg-primary */
  --text-disabled: #636366;        /* decorative only: dimmed icons, separators */
```

The hierarchy moves from { secondary 8e/tertiary 63 } to { secondary c7/tertiary 8e/disabled 63 }. The contrast hierarchy is preserved (secondary > tertiary > disabled).

- [ ] **Step 8.2: Audit each `--text-tertiary` usage**

Run: `grep -rn 'var(--text-tertiary)' crates/ui/src/ | head -30`

For each usage, decide:
- Is the text **functional** (user must read it: timestamp, label, value)? → keep `--text-tertiary`.
- Is it **decorative** (separator dot, dim icon, disabled state)? → migrate to `--text-disabled`.

This judgment requires looking at each call site. Default to keeping `--text-tertiary` if unsure (text remains readable).

Common candidates for migration to `--text-disabled`:
- Inactive icon strokes
- Bullet separators (·)
- Empty-state hint text in extreme corner cases (rare)

- [ ] **Step 8.3: Verify**

```bash
cd crates/ui && pnpm check 2>&1 | tail
```

Expected: 0 errors.

Visual check: launch dev, scan all pages. The hierarchy should look right (secondary clearly brighter than tertiary).

- [ ] **Step 8.4: Commit**

```bash
git add crates/ui/src/app.css
# plus any files where you migrated specific usages to --text-disabled
git commit -m "fix(ui): contraste WCAG AA pour --text-tertiary + nouveau token --text-disabled"
```

---

## Task 9: Debounce search on Connexions

**Files:**
- Modify: `crates/ui/src/routes/connections/+page.svelte`

(Audit page already done in Task 6.)

- [ ] **Step 9.1: Apply debounce pattern**

Open `crates/ui/src/routes/connections/+page.svelte`. Find the search input binding (likely `bind:value={searchValue}`). Replace with:

```svelte
<script lang="ts">
  import { debounce } from '$lib/utils/debounce';
  // ... existing imports ...

  let searchInput = $state('');
  let debouncedSearch = $state('');
  const updateDebounced = debounce((v: string) => { debouncedSearch = v; }, 250);
  $effect(() => {
    updateDebounced(searchInput);
  });

  let filteredConnections = $derived(
    applyFilters(connections, debouncedSearch, /* other filter states */)
  );
</script>

<input bind:value={searchInput} placeholder="Rechercher..." />
```

Replace any usage of the old `searchValue` with `debouncedSearch` (in the filter logic) and `searchInput` (in the input binding).

- [ ] **Step 9.2: Verify**

```bash
cd crates/ui && pnpm check 2>&1 | tail
```

Expected: 0 errors.

- [ ] **Step 9.3: Commit**

```bash
git add crates/ui/src/routes/connections/+page.svelte
git commit -m "refactor(ui/connections): debounce de la recherche (250ms)"
```

---

## Task 10: Toggles règles → `role="switch"` + `aria-checked`

**Files:**
- Modify: `crates/ui/src/routes/rules/+page.svelte`

- [ ] **Step 10.1: Find toggle markup**

```bash
grep -n 'aria-label.*Activer\|aria-label.*Desactiver\|toggle\|enabled' crates/ui/src/routes/rules/+page.svelte | head -10
```

Identify the toggle button(s) used to enable/disable rules.

- [ ] **Step 10.2: Add ARIA attributes**

Replace the toggle button:

```svelte
<button
  role="switch"
  aria-checked={rule.enabled}
  aria-label={rule.enabled ? `Désactiver ${rule.name}` : `Activer ${rule.name}`}
  onclick={() => toggleRule(rule.id, !rule.enabled)}
  class="rule-toggle"
  class:active={rule.enabled}
  type="button"
>
  <span class="track"><span class="thumb"></span></span>
</button>
```

The `class:active={rule.enabled}` and the inner `.track`/`.thumb` markup may already exist — preserve them. Only add `role="switch"` and `aria-checked={rule.enabled}`. Update `aria-label` to be dynamic.

- [ ] **Step 10.3: Verify**

```bash
cd crates/ui && pnpm check 2>&1 | tail
```

Expected: 0 errors.

Manual check: launch dev, open /rules, inspect a toggle in DevTools accessibility panel — should report "switch" with `aria-checked=true/false`.

- [ ] **Step 10.4: Commit**

```bash
git add crates/ui/src/routes/rules/+page.svelte
git commit -m "fix(ui/rules): toggles avec role=\"switch\" + aria-checked pour lecteurs d'ecran"
```

---

## Task 11: Sidebar mobile tap targets

**Files:**
- Modify: `crates/ui/src/lib/components/ui/Sidebar.svelte`

- [ ] **Step 11.1: Find mobile media query**

```bash
grep -n '@media\|max-width: 640' crates/ui/src/lib/components/ui/Sidebar.svelte
```

Identify the mobile `@media (max-width: 640px)` block (or similar).

- [ ] **Step 11.2: Update padding**

In the mobile media query block, ensure each `.sidebar-item` (or equivalent class) has:

```css
@media (max-width: 640px) {
  .sidebar-item,
  .nav-item {
    padding: 12px 8px;
    min-width: 44px;
    min-height: 44px;
    box-sizing: border-box;
  }
}
```

Adapt the selector to the actual class name in the file. The goal: tap target ≥ 44×44 px on mobile.

If the existing CSS sets a smaller padding (e.g., `padding: 4px`), the override above is sufficient. If the layout uses `flex-basis` or `gap` that constrains the size, adjust them too.

- [ ] **Step 11.3: Verify**

```bash
cd crates/ui && pnpm check 2>&1 | tail
```

Expected: 0 errors.

Manual check: launch dev, resize browser to ≤ 640 px, inspect each sidebar item — DevTools "Computed" tab should show `width ≥ 44px` and `height ≥ 44px`.

- [ ] **Step 11.4: Commit**

```bash
git add crates/ui/src/lib/components/ui/Sidebar.svelte
git commit -m "fix(ui/sidebar): tap targets >= 44x44 px sur mobile (WCAG 2.5.5)"
```

---

## Task 12: CHANGELOG section "UX & Accessibility"

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 12.1: Append section**

Under `## [0.2.0] - 2026-05-05`, after the existing subsections (Code Hygiene, Active Blocking, etc.), append:

```markdown
### UX & Accessibility

- **Killswitch immediat + toast undo 5 s** : plus de modal de confirmation paradoxal sur une action d'urgence ; un undo persistant 5 s couvre les mistaps mobile.
- **Raccourcis clavier popup decision** : `a`/`Enter` (autoriser une fois), `b` (bloquer une fois), `Shift+A` (toujours autoriser), `Shift+B` (toujours bloquer), `i` (ignorer), `Esc` (ignorer). Touches affichees via balises `<kbd>`.
- **Page Audit virtualisee** : remplacement de la pagination par `Table.svelte` virtual scroll. Scroll fluide sur ≥ 5000 evenements.
- **Modal focus trap** + restitution du focus a la fermeture (action Svelte `focusTrap`). Conformite WCAG 2.4.3.
- **Contraste WCAG AA** : `--text-tertiary` remonte de `#636366` (3.4:1) a `#8e8e93` (4.6:1). Nouveau token `--text-disabled` pour les usages decoratifs uniquement. `--text-secondary` passe a `#c7c7cc` pour preserver la hierarchie.
- **Debounce filtres** : recherche Connexions et Audit debouncee 250 ms — re-filtre uniquement apres pause de saisie.
- **Toggles regles** : `role="switch"` + `aria-checked` pour lecteurs d'ecran.
- **Sidebar mobile** : tap targets >= 44x44 px (WCAG 2.5.5).
- **Toast extensible** : nouveau champ `action?: { label, handler }` + barre de progression visuelle pour les durees > 0.

Nouveaux utilitaires : `crates/ui/src/lib/utils/debounce.ts`, `crates/ui/src/lib/actions/focus_trap.ts`.
```

- [ ] **Step 12.2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: section UX & Accessibility dans le CHANGELOG 0.2.0"
```

---

## Final verification

```bash
cd crates/ui && pnpm check 2>&1 | tail
cd /home/seb/Dev/SysWall && cargo clippy --workspace --exclude ui --all-targets -- -D warnings 2>&1 | tail
cargo test --workspace --exclude ui 2>&1 | grep result | tail
./system/tests/check-hardening.sh
```

Expected:
- `pnpm check`: 0 errors (warnings on existing code OK).
- `cargo clippy`: 0 warnings (no Rust changes in this sub-project).
- `cargo test`: 334+ tests pass (no Rust changes).
- Hardening check: OK.

---

## Self-Review

**Spec coverage:**
- Killswitch undo → Tasks 3, 4.
- Keyboard shortcuts popup → Task 5.
- Audit virtualization → Task 6.
- Focus trap → Tasks 2, 7.
- Contrast → Task 8.
- Debounce → Tasks 1, 6, 9.
- Toggle role → Task 10.
- Sidebar mobile → Task 11.
- CHANGELOG → Task 12.

All 8 audit findings + spec sections covered.

**Placeholder scan:** Each step has actual code. The "adapt to existing markup" notes are explicit invitations for the implementer to inspect (since UI components vary in shape) — not placeholders. The `applyFilters` function in Task 6 is reused from existing code; if it doesn't exist as a named function, the implementer extracts the inline filter expression into one.

**Type consistency:** `ToastAction { label, handler }` defined in Task 3, used in Task 4. `focusTrap` action in Task 2, applied in Task 7. `debounce` utility in Task 1, used in Tasks 6, 9.

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-05-ux-fixes-plan.md`.**

12 tasks, ~12 commits.

For sub-project D specifically, **Subagent-Driven recommended** — most tasks touch 1-2 files with clear strategy.
