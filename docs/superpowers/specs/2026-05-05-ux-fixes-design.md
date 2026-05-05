# Spec — Sous-projet D : UX bloquants

> Date : 2026-05-05
> Branche cible : `main`
> Cycle : spec → plan → exécution TDD → commits incrémentaux
> Pré-requis : sous-projets A, B, C.2 complétés

## Contexte

L'audit UX du 2026-05-04 (`docs/audit-2026-05-04.md`) et le sous-projet C.2 (NFQUEUE block-while-pending, qui rend les popups de décision réellement bloquants) exposent 8 frictions UX critiques :

1. **Killswitch sans confirmation** (`crates/ui/src/routes/+layout.svelte:308`) — un clic coupe ou réactive le réseau, sans modal ni undo. Risque mistap mobile élevé.
2. **Pas de raccourcis clavier sur popup décision** (`DecisionPrompt.svelte`) — sans `Enter`/`Esc`/chiffres. Avec NFQUEUE actif, l'utilisateur voit beaucoup plus de popups : la friction clavier devient bloquante.
3. **Audit pagine au lieu de virtualiser** (`audit/+page.svelte:215-260`) alors que `Table.svelte` virtualisé existe déjà. Anti-pattern sur firewall produisant des milliers d'événements (encore plus avec NFQUEUE).
4. **Modal sans focus trap** (`Modal.svelte`) ni restitution focus à la fermeture. WCAG 2.4.3 violation.
5. **Contraste `--text-tertiary #636366` ≈ 3.4:1** sur `--bg-primary #1c1c1e`. Sous WCAG AA 4.5:1 pour texte normal.
6. **Filtres Connexions/Audit non debouncés** — re-filtre à chaque keystroke, latence perçue sur gros volumes.
7. **Toggle règles sans `role="switch"` + `aria-checked`** (`rules/+page.svelte`) — non identifié comme interrupteur par lecteurs d'écran.
8. **Sidebar mobile tap targets ~6 px de padding** — sous WCAG 2.5.5 (44×44 px minimum).

## Objectifs

- Rendre l'UI de SysWall utilisable et accessible avec NFQUEUE actif (haute fréquence de popups).
- Atteindre WCAG 2.1 AA sur les points soulevés (contraste, focus trap, role="switch", tap targets).
- Pas de friction inutile sur les actions d'urgence (killswitch).
- Tests Vitest pour la logique UI (raccourcis, debounce, undo timer).

Hors-scope (différé en sous-projets ultérieurs) :
- **RuleForm prévisualisation** (`RuleForm.svelte`) — feature, pas friction. Sous-projet ultérieur.
- **i18n réelle** (un seul `lib/i18n/fr.ts`, pas de framework `svelte-i18n`/`paraglide`) — refactor important, sous-projet dédié.
- **Sidebar mobile non scrollable horizontalement** avec 6 items + badge — cosmétique, pas bloquant.
- **`StatCard:hover` mort** + autres polish hover/transition — sous-projet E (design polish).

## Décisions de conception (validées avec l'utilisateur)

- **Killswitch** : action immédiate + toast undo persistant 5 s (option 2 du brainstorming). Le toast affiche un compte à rebours visuel et un bouton **Annuler** qui restitue l'état précédent. Si l'utilisateur clique « Annuler » dans la fenêtre, on appelle `setNetworkEnabled(previous_state)` et on ferme le toast. Si la fenêtre expire sans action, le toast se ferme silencieusement (l'action est confirmée).
- **Raccourcis popup décision** : `a` ou `Enter` = autoriser une fois, `b` = bloquer une fois, `Shift+A` = toujours autoriser, `Shift+B` = toujours bloquer, `i` = ignorer, `Esc` = différer (replace le popup au fond, le réintroduit après timeout). Touches affichées dans les boutons via balises `<kbd>`.
- **Virtualisation Audit** : `audit/+page.svelte` réutilise le composant `Table.svelte` (qui implémente déjà du virtual scroll avec sticky header + buffer). Suppression de la pagination par pages classique (les filtres deviennent des reducers sur le dataset).
- **Modal focus trap** : nouvel utilitaire `focusTrap` (action Svelte) appliqué à `<Modal>`. Stocke l'élément actif au montage, fait `firstFocusable.focus()`, intercepte `Tab`/`Shift+Tab` pour boucler, restitue le focus au démontage.
- **Contraste** : remonter `--text-tertiary` de `#636366` à `#8e8e93` (déjà valeur de `--text-secondary`). Vérifier que ça ne dégrade pas le contraste secondaire (qui passerait au-dessus de `--text-tertiary`). Alternativement, introduire une nouvelle variable `--text-quaternary` pour les usages décoratifs (icônes désactivées) et conserver `--text-tertiary` à 4.5:1.
- **Debounce filtres** : utilitaire `debounce(fn, 250)` appliqué aux `searchValue` de Connexions et Audit. Le `$effect` reste mais lit la valeur debounced au lieu de la valeur instantanée.
- **Toggle règles** : remplacer la `<button>` actuelle par un `<button role="switch" aria-checked={enabled}>`. Pas de changement visuel.
- **Sidebar mobile** : remonter le padding des items à minimum 12 px vertical + 8 px horizontal pour atteindre 44×44 px de tap target. Conserver les icônes 24 px et le label 11 px.

## Architecture

### D.1 — Killswitch toast undo

`crates/ui/src/routes/+layout.svelte` (handler du killswitch) :

```typescript
async function handleKillswitch() {
    const previousState = networkEnabled;
    const newState = !previousState;
    await setNetworkEnabled(newState);
    networkEnabled = newState;

    let undoTriggered = false;
    addToast({
        message: newState
            ? "Réseau rétabli."
            : "Réseau coupé. Toutes les connexions sont bloquées.",
        kind: newState ? 'success' : 'warning',
        duration: 5000,
        action: {
            label: 'Annuler',
            handler: async () => {
                undoTriggered = true;
                await setNetworkEnabled(previousState);
                networkEnabled = previousState;
            },
        },
    });
}
```

`crates/ui/src/lib/stores/toast.ts` ajoute le support de l'action :

```typescript
export interface ToastAction {
    label: string;
    handler: () => void | Promise<void>;
}

export interface Toast {
    id: string;
    message: string;
    kind: 'info' | 'success' | 'warning' | 'error';
    duration: number;     // ms ; 0 = persistent
    action?: ToastAction; // bouton optionnel
}
```

`crates/ui/src/lib/components/Toast.svelte` rend le bouton + une barre de progression visuelle pour le timer 5 s.

### D.2 — Raccourcis popup décision

`crates/ui/src/lib/components/learning/DecisionPrompt.svelte` :

```svelte
<script lang="ts">
    import { onMount } from 'svelte';
    let { decision, onResponse }: Props = $props();

    function onKeydown(e: KeyboardEvent) {
        if (e.target instanceof HTMLInputElement) return; // ne pas perturber les inputs
        const key = e.key.toLowerCase();
        switch (key) {
            case 'enter':
            case 'a':
                if (e.shiftKey) onResponse('AlwaysAllow');
                else onResponse('AllowOnce');
                e.preventDefault();
                break;
            case 'b':
                if (e.shiftKey) onResponse('AlwaysBlock');
                else onResponse('BlockOnce');
                e.preventDefault();
                break;
            case 'i':
                onResponse('Ignore');
                e.preventDefault();
                break;
            case 'escape':
                onResponse('Defer'); // future variant; pour V0.2 = équivalent Ignore
                e.preventDefault();
                break;
        }
    }

    onMount(() => {
        window.addEventListener('keydown', onKeydown);
        return () => window.removeEventListener('keydown', onKeydown);
    });
</script>

<button onclick={() => onResponse('AllowOnce')}>
    Autoriser une fois <kbd>A</kbd>
</button>
<button onclick={() => onResponse('BlockOnce')}>
    Bloquer une fois <kbd>B</kbd>
</button>
<!-- etc -->
```

Le `'Defer'` action n'existe pas encore dans `DecisionAction` ; pour cette V0.2 on mappe `Esc` à `Ignore` (le paquet est jeté, mais le flux suivant retombera sur la default policy → re-popup). Documenté.

Une nouvelle variable CSS `--kbd-bg`/`--kbd-color` pour styliser `<kbd>` cohérent avec le design system.

### D.3 — Virtualisation Audit

`crates/ui/src/routes/audit/+page.svelte` :
- Suppression de la logique `pageSize` / `currentPage` / `paginate(events, page)`.
- Remplacement par `<Table data={filteredEvents} columns={columns} rowHeight={32}>` (le composant `Table.svelte` virtualise déjà).
- Filtres (sévérité, catégorie, dates, recherche texte) deviennent des reducers sur le dataset complet : `let filteredEvents = $derived(applyFilters(rawEvents, filters));`.
- L'export JSON utilise `filteredEvents` (pas la page courante).

`Table.svelte` doit être inspecté pour confirmer qu'il accepte des colonnes typées et un `rowHeight`. Si l'API diffère, adapter.

### D.4 — Focus trap Modal

`crates/ui/src/lib/actions/focus_trap.ts` (nouveau) :

```typescript
import type { Action } from 'svelte/action';

export const focusTrap: Action<HTMLElement> = (node) => {
    const previouslyFocused = document.activeElement as HTMLElement | null;

    const focusables = () =>
        node.querySelectorAll<HTMLElement>(
            'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'
        );

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

`crates/ui/src/lib/components/ui/Modal.svelte` applique l'action :

```svelte
<div class="modal" use:focusTrap role="dialog" aria-modal="true">
    <slot />
</div>
```

### D.5 — Fix contraste

`crates/ui/src/app.css` :

```css
:root {
    /* ... existing tokens ... */
    --text-secondary: #aeaeb2;       /* déjà 5.6:1 vs #1c1c1e — bon */
    --text-tertiary: #8e8e93;        /* remonté de #636366 à #8e8e93 — 4.6:1 vs #1c1c1e — WCAG AA passé */
    --text-disabled: #636366;        /* nouveau : pour les usages décoratifs uniquement (pas de texte critique) */
}
```

Auditer chaque usage de `--text-tertiary` et déterminer s'il doit rester `--text-tertiary` (texte lisible) ou devenir `--text-disabled` (icône fade, séparateur). Critère : si l'utilisateur doit pouvoir lire la valeur, c'est `--text-tertiary` ; sinon `--text-disabled`.

### D.6 — Debounce filtres

`crates/ui/src/lib/utils/debounce.ts` (nouveau) :

```typescript
export function debounce<F extends (...args: never[]) => void>(fn: F, delayMs: number): F {
    let timer: ReturnType<typeof setTimeout> | null = null;
    return ((...args: Parameters<F>) => {
        if (timer) clearTimeout(timer);
        timer = setTimeout(() => fn(...args), delayMs);
    }) as F;
}
```

`crates/ui/src/routes/connections/+page.svelte` et `audit/+page.svelte` :

```typescript
let searchInput = $state('');
let debouncedSearch = $state('');
const updateDebounced = debounce((v: string) => { debouncedSearch = v; }, 250);
$effect(() => updateDebounced(searchInput));

let filtered = $derived(
    rawData.filter(item => matchesFilter(item, debouncedSearch, otherFilters))
);
```

### D.7 — Toggle règles `role="switch"`

`crates/ui/src/routes/rules/+page.svelte` :

```svelte
<button
    role="switch"
    aria-checked={rule.enabled}
    aria-label={rule.enabled ? `Désactiver ${rule.name}` : `Activer ${rule.name}`}
    onclick={() => toggleRule(rule.id, !rule.enabled)}
    class="toggle"
    class:active={rule.enabled}
>
    <span class="track"><span class="thumb"></span></span>
</button>
```

### D.8 — Sidebar mobile tap targets

`crates/ui/src/lib/components/ui/Sidebar.svelte` (CSS pour `@media (max-width: 640px)`) :

```css
@media (max-width: 640px) {
    .sidebar-item {
        padding: 12px 8px;       /* 12*2 + 24 (icone) = 48px de hauteur, > 44 WCAG */
        min-width: 44px;
        min-height: 44px;
    }
}
```

## Tests

### Unit (Vitest dans `crates/ui/`)

`crates/ui/src/lib/utils/debounce.test.ts` :
- `debounce_calls_after_delay` (avec `vi.useFakeTimers()`)
- `debounce_resets_on_repeated_calls`
- `debounce_passes_args`

`crates/ui/src/lib/actions/focus_trap.test.ts` :
- `focus_trap_focuses_first_element_on_mount`
- `focus_trap_loops_tab_at_last_element`
- `focus_trap_loops_shift_tab_at_first_element`
- `focus_trap_restores_previous_focus_on_destroy`

### Component (Vitest + jsdom)

`crates/ui/src/lib/components/learning/DecisionPrompt.test.ts` :
- `enter_key_triggers_allow_once`
- `b_key_triggers_block_once`
- `shift_a_triggers_always_allow`
- `i_key_triggers_ignore`
- `escape_triggers_ignore_for_v0_2`
- `keys_inside_input_do_not_trigger_actions`

`crates/ui/src/lib/components/ui/Toast.test.ts` :
- `toast_renders_action_button_when_provided`
- `clicking_action_button_calls_handler`
- `toast_auto_dismisses_after_duration`

### Manual checks

- Sur l'UI : vérifier killswitch undo (clic, attendre 2 s, cliquer Annuler → état restauré).
- Audit page : charger 5000 audit events (fake DB) et scroller — pas de lag, sticky header visible.
- Modal : ouvrir avec un input externe focused, fermer, vérifier que le focus revient à l'input.
- DevTools accessibility tree : vérifier `role="switch"` + `aria-checked` sur les toggles règles.

## Risques & mitigations

| Risque | Probabilité | Mitigation |
|---|---|---|
| `Table.svelte` virtual scroll a une API incompatible avec audit data | Moyenne | Inspecter `Table.svelte` avant la migration. Si l'API limite, adapter l'audit data au format attendu, ne pas modifier `Table.svelte` (utilisée ailleurs). |
| Le focus trap interfère avec les composants imbriqués (popover dans modal) | Faible | Tests d'intégration manuels ; documenter la limite (un seul focus trap actif à la fois). |
| Remonter `--text-tertiary` casse le contraste secondaire | Faible | Auditer les usages avant. La valeur `#8e8e93` est identique à `--text-secondary` actuel — ce qui peut casser la hiérarchie visuelle. **Décision** : `--text-secondary` passe à `#c7c7cc` (encore plus clair) pour préserver la hiérarchie. À valider sur écran. |
| Raccourcis clavier conflictent avec inputs ailleurs | Moyenne | Le handler vérifie `e.target instanceof HTMLInputElement` avant d'agir. |
| Toast undo non visible si plusieurs toasts empilés | Faible | Le toast undo a `priority: 'high'` ou `kind: 'warning'` qui l'épingle en tête de pile. À implémenter dans `toast.ts`. |

## Critères de succès

- [ ] Killswitch : clic → action immédiate + toast undo 5 s avec barre de progression. Annuler restaure l'état.
- [ ] Popup décision : raccourcis `a`, `Enter`, `b`, `Shift+A`, `Shift+B`, `i`, `Esc` fonctionnels. Touches affichées dans les boutons.
- [ ] Page Audit : virtualisation via `Table.svelte` ; scrolling fluide sur ≥ 5000 lignes.
- [ ] `Modal` : focus trap actif (Tab boucle), focus restitué à la fermeture.
- [ ] Tous les usages de `--text-tertiary` passent WCAG AA (≥ 4.5:1) ou sont migrés vers `--text-disabled`.
- [ ] Filtres Connexions et Audit debouncés (250 ms).
- [ ] Toggles règles ont `role="switch"` + `aria-checked`.
- [ ] Sidebar mobile : tap targets ≥ 44×44 px.
- [ ] Tests Vitest verts (au moins 18 nouveaux tests).
- [ ] `pnpm check` 0 erreur (TypeScript + Svelte).
- [ ] CHANGELOG section "UX & Accessibility" sous V0.2.

## Plan d'exécution (commits ciblés)

| # | Étape | Type |
|---|---|---|
| 1 | Utilitaire `debounce` + tests Vitest | feat |
| 2 | Action `focus_trap` + tests Vitest | feat |
| 3 | Toast extension `action` + barre progression + tests | feat |
| 4 | Killswitch action immédiate + toast undo 5 s | feat |
| 5 | Raccourcis clavier `DecisionPrompt` + balises `<kbd>` + tests | feat |
| 6 | Migration Audit page → `Table.svelte` virtualisé | refactor |
| 7 | Application `focusTrap` dans `Modal` | feat |
| 8 | Fix contraste tokens (`--text-tertiary`, `--text-secondary`, `--text-disabled`) | fix |
| 9 | Debounce search Connexions + Audit | refactor |
| 10 | Toggles règles → `role="switch"` + `aria-checked` | fix |
| 11 | Sidebar mobile tap targets ≥ 44×44 | fix |
| 12 | CHANGELOG section UX & Accessibility | docs |

12 commits estimés.

## Hors-scope explicite

- RuleForm prévisualisation (feature, sous-projet ultérieur).
- i18n réelle (refactor, sous-projet dédié).
- Sidebar mobile scroll horizontal (cosmétique).
- Polish hover/transition (sous-projet E).
- Confirmation killswitch (rejetée — option 2 retenue).

---

*Spec rédigée le 2026-05-05. À approuver avant transition vers writing-plans.*
