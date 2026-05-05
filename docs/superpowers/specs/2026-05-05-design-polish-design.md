# Spec — Sous-projet E : Design polish

> Date : 2026-05-05
> Branche cible : `main`
> Cycle : spec → plan → exécution
> Pré-requis : sous-projets A, B, C.2, D complétés

## Contexte

L'audit visuel du 2026-05-04 a relevé 8 finitions visuelles bloquant le passage du design system de **6/10** (squelette technique solide, finition d'amateur) à un niveau **8/10** (proche Little Snitch / GlassWire).

Le sous-projet D a déjà traité une partie du contraste (`--text-tertiary` WCAG AA), du focus trap, et des tap targets mobile. Restent les améliorations purement visuelles et identitaires.

## Objectifs

- Trancher la **direction artistique** : macOS Dark sobre + un accent identitaire cyan turquoise SysWall (option 3 du brainstorming).
- Remplacer les **emojis** de la sidebar par un set d'icônes vectorielles cohérent (**Lucide**).
- Embarquer une **web font** auto-hostée (`Inter` pour sans, `JetBrains Mono` pour mono) → fini la fallback chaotique macOS/Linux/Windows.
- Créer un **logo SysWall SVG** (mark + wordmark) + favicon multi-tailles.
- **Polir les tableaux denses** : zebra-striping subtil, sticky-header shadow au scroll, `font-variant-numeric: tabular-nums` sur ports/IPs/octets, hover de ligne plus visible.
- Réparer le **`StatCard:hover` mort** (border-color identique = aucun effet).
- Ajouter les **états `error` et `disabled`** au composant `Input`.
- Supprimer l'option **`glow` du Card** (jamais utilisée — YAGNI).

Hors-scope :
- Refactor architectural du système de design (déjà bon).
- Refonte de la palette macOS Dark (conservée).
- Animations complexes hors hover/focus.
- Mode clair (out of scope ; SysWall reste dark-only).

## Décisions de conception (validées avec l'utilisateur)

- **Direction artistique** : option 3 hybride — macOS Dark conservé + accent identitaire `--accent-syswall: #2cd4d4` (cyan turquoise). Cet accent sert UNIQUEMENT à :
  - la pilule killswitch quand le réseau est actif (pulsation très subtile : 2s ease-in-out, glow 6px à 0.4 alpha),
  - le tracé fin du logo SysWall,
  - le filet de séparation `interception chain` dans la vue Connexions (1px, indique "monitoring actif"),
  - les badges `Severity::Critical` (en alternance avec `--accent-red`).
  Pas d'usage décoratif. Le reste de l'UI reste sur la palette macOS System Colors existante.
- **Web fonts** : `Inter Variable` (file `.woff2` auto-hostée dans `crates/ui/static/fonts/`, weight range 100-900) pour `--font-sans`, `JetBrains Mono` (régulier 400 + bold 700) pour `--font-mono`. `font-display: swap` pour ne pas bloquer le rendu.
- **Icônes** : librairie `lucide-svelte` (pas `tabler` ni `heroicons`). Choix motivé : Lucide a 1500+ icônes consistantes, tree-shakable, déjà optimisées pour React/Svelte/Vue. Tabler est aussi bon mais moins connu en milieu Svelte.
- **Logo** : créé en SVG inline dans `crates/ui/src/lib/components/branding/SyswallLogo.svelte`. Mark = bouclier + filet cyan diagonal (évoque "interception"). Wordmark = "SysWall" en `Inter SemiBold`. Déclinaisons : 16/24/32/64/128/256 px (favicon), variant `mark-only` pour la sidebar collapsed.
- **Zebra-striping** : `tr:nth-child(even) { background: rgba(255,255,255,0.02); }` — très subtil, ne casse pas le dark theme.
- **Sticky header shadow** : `box-shadow: 0 1px 0 rgba(255,255,255,0.06)` quand le contenu scrollé > 0. Détecté via `IntersectionObserver` ou `scroll` event sur le conteneur de table.
- **Tabular nums** : `font-variant-numeric: tabular-nums` global sur `.mono` et sur cellules `<td>` contenant des ports/IPs/bytes.
- **Hover row** : passer de `--bg-hover` (0.06 alpha) à 0.10 alpha pour les rows de tableau.
- **Pas de mode "réduire animation"** spécifique nouveau : `prefers-reduced-motion` déjà géré dans `app.css`. La pulsation killswitch est désactivée sous cette préférence.

## Architecture

### E.1 — Tokens CSS étendus

`crates/ui/src/app.css` :

```css
:root {
    /* Existing macOS Dark palette preserved */

    /* Identity accent (sub-project E) */
    --accent-syswall: #2cd4d4;
    --accent-syswall-dim: rgba(44, 212, 212, 0.4);
    --accent-syswall-glow: 0 0 6px rgba(44, 212, 212, 0.4);

    /* Hover bumped for dense tables (sub-project E) */
    --bg-row-hover: rgba(255, 255, 255, 0.10);
    --bg-row-stripe: rgba(255, 255, 255, 0.02);

    /* Sticky-header shadow (sub-project E) */
    --shadow-sticky-header: 0 1px 0 rgba(255, 255, 255, 0.06);

    /* Web fonts (sub-project E) */
    --font-sans: 'Inter Variable', -apple-system, BlinkMacSystemFont, system-ui, sans-serif;
    --font-mono: 'JetBrains Mono', 'SF Mono', Menlo, Consolas, monospace;
}

/* Tabular nums on mono and table cells */
.mono, td, .number {
    font-variant-numeric: tabular-nums;
}

/* prefers-reduced-motion : neutralise la pulsation */
@media (prefers-reduced-motion: reduce) {
    .killswitch-active-pulse { animation: none !important; }
}
```

### E.2 — Web fonts auto-hostées

Télécharger via `curl` ou `wget` les fichiers `.woff2` :
- `Inter Variable` (latin) depuis https://rsms.me/inter/font-files/InterVariable.woff2
- `JetBrains Mono Regular` et `JetBrains Mono Bold` depuis https://github.com/JetBrains/JetBrainsMono/releases (latin uniquement, pas les ligatures custom — keep it simple)

Fichiers placés dans `crates/ui/static/fonts/` :
- `inter-variable.woff2`
- `jetbrains-mono-regular.woff2`
- `jetbrains-mono-bold.woff2`

`@font-face` déclarés en tête de `app.css` :

```css
@font-face {
    font-family: 'Inter Variable';
    src: url('/fonts/inter-variable.woff2') format('woff2-variations');
    font-weight: 100 900;
    font-style: normal;
    font-display: swap;
}

@font-face {
    font-family: 'JetBrains Mono';
    src: url('/fonts/jetbrains-mono-regular.woff2') format('woff2');
    font-weight: 400;
    font-style: normal;
    font-display: swap;
}

@font-face {
    font-family: 'JetBrains Mono';
    src: url('/fonts/jetbrains-mono-bold.woff2') format('woff2');
    font-weight: 700;
    font-style: normal;
    font-display: swap;
}
```

`font-display: swap` évite un flash sans texte au boot.

**Si le téléchargement échoue** dans l'env d'exécution (sandbox sans internet), créer les fichiers vides comme placeholders et documenter dans le CHANGELOG. Les fonts système feront le fallback (cf. les chaînes de fallback dans les tokens).

### E.3 — Logo SysWall SVG

`crates/ui/src/lib/components/branding/SyswallLogo.svelte` :

```svelte
<script lang="ts">
    interface Props {
        size?: number;          // px, default 24
        variant?: 'mark' | 'full';  // 'mark' = shield only, 'full' = mark + wordmark
        accent?: string;        // CSS color, default var(--accent-syswall)
    }
    let { size = 24, variant = 'mark', accent = 'var(--accent-syswall)' }: Props = $props();

    let width = $derived(variant === 'full' ? size * 4.5 : size);
</script>

<svg
    width={width}
    height={size}
    viewBox={variant === 'full' ? '0 0 108 24' : '0 0 24 24'}
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    aria-label="SysWall"
    role="img"
>
    <!-- Shield mark -->
    <path
        d="M12 2 L20 5 V12 C20 17 16.5 21 12 22 C7.5 21 4 17 4 12 V5 Z"
        stroke={accent}
        stroke-width="1.5"
        stroke-linejoin="round"
        fill="rgba(44, 212, 212, 0.06)"
    />
    <!-- Diagonal interception line -->
    <line
        x1="6"
        y1="14"
        x2="18"
        y2="6"
        stroke={accent}
        stroke-width="1.5"
        stroke-linecap="round"
    />

    {#if variant === 'full'}
        <!-- Wordmark "SysWall" -->
        <text
            x="28"
            y="17"
            font-family="Inter Variable, sans-serif"
            font-weight="600"
            font-size="14"
            fill="currentColor"
        >
            SysWall
        </text>
    {/if}
</svg>
```

`crates/ui/static/favicon.svg` (déclinaison statique 32×32 du mark seul, pour le navigateur) — reproduit le path ci-dessus.

`crates/ui/static/icon-{16,32,48,128,256}.png` : générés depuis le SVG. Si pas d'outil de conversion en local, conserver le SVG comme favicon principal (`<link rel="icon" type="image/svg+xml" href="/favicon.svg">` dans `app.html`).

Mise à jour de `app.html` pour pointer vers `favicon.svg` plutôt que `favicon.png`.

Suppression de `static/svelte.svg`, `static/tauri.svg`, `static/vite.svg` s'ils ne sont plus référencés.

### E.4 — Icônes Lucide

Installer `lucide-svelte` :
```bash
cd crates/ui && pnpm add lucide-svelte
```

Mapper les emojis sidebar actuels vers Lucide (audit visuel a noté `📊 🔗 🛡️ 🧠 🚫 📋 ⚙️`) :

| Vue | Emoji actuel | Icône Lucide |
|---|---|---|
| Dashboard | 📊 | `LayoutDashboard` |
| Connexions | 🔗 | `Network` |
| Règles | 🛡️ | `Shield` |
| Apprentissage | 🧠 | `BrainCircuit` |
| Blocklists | 🚫 | `Ban` |
| Audit | 📋 | `ClipboardList` |
| Paramètres | ⚙️ | `Settings` |

Sidebar item :
```svelte
<script lang="ts">
    import { LayoutDashboard, Network, Shield, BrainCircuit, Ban, ClipboardList, Settings } from 'lucide-svelte';
</script>

<a href="/dashboard">
    <LayoutDashboard size={20} strokeWidth={1.75} />
    <span>Tableau de bord</span>
</a>
```

`size={20}` cohérent avec la sidebar 180 px desktop. `strokeWidth={1.75}` pour un trait légèrement plus épais que le défaut 2 (équilibre lisibilité / élégance).

Auditer les autres usages d'emojis dans le codebase :
```bash
grep -rn '[\xF0-\xF4][\x80-\xBF][\x80-\xBF][\x80-\xBF]' crates/ui/src/ --include='*.svelte' --include='*.ts'
```

Remplacer chaque occurrence où l'emoji est utilisé comme icône fonctionnelle (pas comme contenu utilisateur). Les emojis dans `aria-label` sont gardés car ils n'apparaissent pas visuellement.

### E.5 — Polish tableaux denses

Modifier `crates/ui/src/lib/components/ui/Table.svelte` :

**Zebra-striping** :
```css
.body-row:nth-child(even) {
    background: var(--bg-row-stripe);
}
.body-row:hover {
    background: var(--bg-row-hover);
}
```

**Sticky header shadow** :
```svelte
<script>
    let scrolled = $state(false);
    function onScroll(e: Event) {
        const target = e.target as HTMLElement;
        scrolled = target.scrollTop > 0;
    }
</script>

<div class="table-container" onscroll={onScroll}>
    <div class="header-row" class:scrolled>...</div>
    ...
</div>

<style>
    .header-row.scrolled {
        box-shadow: var(--shadow-sticky-header);
    }
</style>
```

**Tabular nums** déjà géré globalement par `app.css` via `td { font-variant-numeric: tabular-nums; }` (E.1).

### E.6 — Réparer `StatCard:hover`

`crates/ui/src/lib/components/ui/StatCard.svelte` :

```css
.stat-card {
    transition: transform 150ms ease, border-color 150ms ease, box-shadow 150ms ease;
}
.stat-card:hover {
    transform: translateY(-1px);
    border-color: var(--border-secondary, rgba(255, 255, 255, 0.12));
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
}
```

Le translate de 1 px + l'élévation d'ombre donnent un retour visuel net sans casser la sobriété macOS.

### E.7 — États `error` et `disabled` sur Input

`crates/ui/src/lib/components/ui/Input.svelte` :

```svelte
<script lang="ts">
    interface Props {
        value: string;
        type?: string;
        placeholder?: string;
        error?: string;          // message d'erreur ; si présent → border rouge + helper text
        disabled?: boolean;
        // ... existing props ...
    }
    let { value = $bindable(''), error, disabled, ...rest }: Props = $props();
</script>

<div class="input-wrapper" class:has-error={!!error} class:is-disabled={disabled}>
    <input bind:value {disabled} aria-invalid={!!error} aria-describedby={error ? 'input-error' : undefined} {...rest} />
    {#if error}
        <span id="input-error" class="error-message" role="alert">{error}</span>
    {/if}
</div>

<style>
    .input-wrapper.has-error input {
        border-color: var(--accent-red);
    }
    .input-wrapper.has-error .error-message {
        color: var(--accent-red);
        font-size: var(--font-size-sm);
        margin-top: 4px;
        display: block;
    }
    .input-wrapper.is-disabled input {
        opacity: 0.5;
        cursor: not-allowed;
    }
</style>
```

L'API publique reste rétrocompatible : `error` et `disabled` sont optionnels.

### E.8 — Suppression `glow` mort sur Card

Inspecter `crates/ui/src/lib/components/ui/Card.svelte`. Si la prop `glow` existe et n'est jamais utilisée (vérifier via `grep -rn 'Card.*glow' crates/ui/src/`), la supprimer purement et simplement (YAGNI).

Si l'audit visuel s'est trompé et que `glow` est utilisé quelque part, conserver et noter dans le rapport.

### E.9 — Pulsation killswitch

`crates/ui/src/routes/+layout.svelte` (sur la pilule killswitch quand `networkEnabled === true`) :

```svelte
<button
    class="killswitch-pill"
    class:active={networkEnabled}
    class:killswitch-active-pulse={networkEnabled}
    ...
>
```

```css
.killswitch-pill.active {
    box-shadow: var(--accent-syswall-glow);
}
@keyframes killswitch-pulse {
    0%, 100% { box-shadow: 0 0 6px rgba(44, 212, 212, 0.4); }
    50% { box-shadow: 0 0 10px rgba(44, 212, 212, 0.6); }
}
.killswitch-active-pulse {
    animation: killswitch-pulse 2s ease-in-out infinite;
}
```

`@media (prefers-reduced-motion: reduce)` neutralise déjà l'animation (E.1).

## Tests

Pas de tests unitaires nouveaux pour ce sous-projet — c'est purement visuel/CSS.

**Validation manuelle** documentée dans `crates/ui/CLAUDE.md` :

```markdown
## Validation visuelle (sous-projet E)

Après modification du design system, vérifier :
1. Tous les emojis sidebar remplacés par des icônes Lucide.
2. Police Inter chargée (DevTools → Network → woff2). Pas de FOIT/FOUT visible.
3. Logo SysWall affiché en topbar et favicon visible dans l'onglet du navigateur.
4. Pilule killswitch pulse subtilement quand le réseau est actif.
5. Hover sur StatCard → léger lift + ombre.
6. Hover sur ligne de tableau → fond plus marqué qu'avant.
7. Zebra-striping subtil mais perceptible sur tableaux denses (audit, connexions).
8. `prefers-reduced-motion: reduce` (chrome://flags ou outil de devtools) → pulsation désactivée.
9. Accessibilité : Lighthouse audit ≥ 95 sur la page Dashboard.
```

`pnpm check` reste 0 erreur.

## Risques & mitigations

| Risque | Probabilité | Mitigation |
|---|---|---|
| Téléchargement web font échoue (sandbox sans internet) | Probable selon l'env | Documenter, créer placeholders, fallback chain en CSS suffit pour livrer. |
| `lucide-svelte` ajoute un bundle volumineux | Faible | Tree-shakable par défaut ; `pnpm build` et vérifier `dist/` reste raisonnable. |
| Glassmorphism + nouvelle pulsation = perf dégradée sur certains GPUs Linux | Faible | Profiler ; documenter que `prefers-reduced-motion` désactive la pulsation. |
| Logo SVG inline gonfle chaque page | Très faible | Le SVG fait ~500 octets ; négligeable. |
| Suppression `glow` casse un usage caché | Faible | Grep exhaustif avant suppression ; rollback facile. |
| Inter Variable conflit avec `Inter` non-variable installé localement | Très faible | Le `@font-face` nomme explicitement `Inter Variable` (différent du nom système `Inter`). |

## Critères de succès

- [ ] `--accent-syswall` défini + utilisé sur killswitch + logo + filet interception.
- [ ] `inter-variable.woff2`, `jetbrains-mono-regular.woff2`, `jetbrains-mono-bold.woff2` dans `crates/ui/static/fonts/`.
- [ ] `@font-face` déclarés en tête de `app.css`.
- [ ] `crates/ui/src/lib/components/branding/SyswallLogo.svelte` rendu via topbar + favicon.svg.
- [ ] `lucide-svelte` installé ; emojis sidebar remplacés ; aucun emoji visible comme icône fonctionnelle.
- [ ] `Table.svelte` : zebra-striping + sticky header shadow.
- [ ] `tabular-nums` global sur `td`.
- [ ] `StatCard:hover` produit un effet visuel net.
- [ ] `Input` : props `error` et `disabled` fonctionnelles.
- [ ] `Card.glow` supprimé (ou conservé si réellement utilisé).
- [ ] Pulsation killswitch active + désactivée sous `prefers-reduced-motion`.
- [ ] `pnpm check` 0 erreur.
- [ ] CHANGELOG section "Design Polish" sous V0.2.

## Plan d'exécution (commits ciblés)

| # | Étape | Type |
|---|---|---|
| 1 | Tokens étendus (`--accent-syswall`, `--bg-row-*`, `--shadow-sticky-header`, `--font-sans/mono`, `tabular-nums`) | feat |
| 2 | Web fonts auto-hostées (`@font-face` + fichiers `static/fonts/`) | feat |
| 3 | `SyswallLogo.svelte` + favicon.svg + intégration topbar | feat |
| 4 | `lucide-svelte` installé + emojis sidebar remplacés | feat |
| 5 | Audit & remplacement des autres emojis fonctionnels (si présents) | refactor |
| 6 | Zebra-striping + sticky header shadow dans `Table.svelte` | feat |
| 7 | Réparation `StatCard:hover` | fix |
| 8 | États `error` et `disabled` sur `Input` | feat |
| 9 | Suppression `glow` mort sur `Card` (YAGNI) | refactor |
| 10 | Pulsation killswitch + respect `prefers-reduced-motion` | feat |
| 11 | CHANGELOG section "Design Polish" + procédure de validation visuelle dans `crates/ui/CLAUDE.md` | docs |

11 commits estimés.

## Hors-scope explicite

- Mode clair (SysWall reste dark-only).
- Refonte de la palette macOS Dark.
- Storybook ou autre catalogue de composants.
- Animations complexes (transitions de page, hero, etc.).
- Logo final designé par un graphiste pro — le SVG livré est le placeholder identitaire propre, à raffiner ensuite.
- Tests unitaires CSS (Vitest non installé, hors scope).

---

*Spec rédigée le 2026-05-05. À approuver avant transition vers writing-plans.*
