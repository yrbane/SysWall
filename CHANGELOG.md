# Changelog

Toutes les modifications notables seront documentees ici.
All notable changes documented here.

## [0.2.0] - 2026-05-05

### Added / Ajoute

- **Anti-lockout 30 s** : annulation automatique des changements de regles si la connectivite externe est perdue dans les 30 s suivant l'apply (`AntilockoutGuard` + `TcpProbe`). Endpoints configurables dans `[antilockout] endpoints = [...]`.
- **Authentification peer SO_PEERCRED** sur le socket gRPC : seul `root` ou les membres du groupe systeme `syswall` peuvent ouvrir une session. Refus audites.
- **Categories d'audit** : `EventCategory::Antilockout`, `EventCategory::Authentication`.
- **Erreur domain** : `DomainError::AntilockoutTriggered { rolled_back_count }`.
- **CSP Tauri stricte** dans la fenetre UI (sans `unsafe-eval`).
- **Limites gRPC** : 1 MiB max decoding, 4 MiB max encoding, 64 streams concurrents par connexion, timeout 30 s.
- **Toast critique UI** sur evenement `AntilockoutTriggered`.
- **Scripts d'install unifies** dans `system/install/postinst.sh` (creent user/group syswall).

### Changed / Modifie

- **Service systemd durci** : `User=syswall` (utilisateur dedie), `AmbientCapabilities` (plus de root), `ProtectSystem=strict`, `RestrictAddressFamilies`, `SystemCallFilter`, `LockPersonality`, `NoNewPrivileges`, etc.
- **Demarrage daemon** : `panic!` remplace par `Result<(), StartupError>` + `exit(78)` (EX_CONFIG sysexits.h) pour les echecs au boot.

### Fixed / Corrige

- Le champ `firewall.rollback_timeout_secs` etait declare mais jamais lu (warning compilateur). Il est maintenant utilise par le guard anti-lockout.

### Security

- **Pre-V0.2** : tout binaire executable par un user du groupe `syswall` pouvait desactiver le pare-feu via le socket gRPC sans authentification. Resolu par `SO_PEERCRED`.
- **Pre-V0.2** : le daemon tournait en `User=root` sans aucune restriction (toute exploitation memoire = root complet). Resolu par utilisateur dedie + capability bounding + sandbox.

### Documentation

- README : section Securite renforcee en FR+EN.
- `crates/ui/CLAUDE.md` : procedure de verification manuelle de la CSP.
- `docs/superpowers/specs/2026-05-05-security-hardening-design.md` : spec de conception.
- `docs/superpowers/plans/2026-05-05-security-hardening-plan.md` : plan d'implementation TDD.
- `docs/audit-2026-05-04.md` : audit complet a l'origine de cette version.

### Code Hygiene

- **`unwrap()` en production eradiques** : ~63 occurrences en production remplacees par `?` (propagation) ou `expect("invariant en francais")` documentees. Les crates `domain`, `app`, `daemon`, `infra`, `ebpf` activent maintenant `#![cfg_attr(not(test), deny(clippy::unwrap_used))]`.
- **God-modules scindes** : `policy_engine` (matcher + evaluator), `audit_service` (command + query CQRS-light), `converters` (rule + decision + audit + connection + error + parsers + event), `audit_repository` (queries + writes + migration), `translator` (criteria + verdict + system_rules), `adapter` (apply + rollback + whitelist). Aucun module de production > 500 LOC sauf integration tests.
- **Version du workspace** : alignee a `0.2.0` (coherent avec les paquets systeme).
- **Dependance Cargo `infra -> app`** : retiree (etait inutilisee, violation hexagonale au niveau Cargo).
- **CI** : `cargo clippy --workspace --exclude ui --all-targets -- -D warnings` est maintenant un gate obligatoire.
- **24 warnings clippy `infra`** : tous corriges.
- **21 warnings clippy `daemon`** pre-existants : tous corriges.

### Active Blocking (NFQUEUE)

- **Nouveau port `PacketInterceptor`** + adapter `NfqueueInterceptor` (`crates/infra/src/nfqueue/`) : intercepte le premier paquet de chaque nouveau flux sortant via `nfnetlink_queue` et synchronise le verdict avec la decision utilisateur.
- **`LearningService` implemente `PacketDecisionHandler`** : evalue via `PolicyEngine`, gere la creation de `PendingDecision`, et attend (≤ 28 s) le verdict via `VerdictBroadcasts`.
- **Deduplication baked-in** : un seul popup par `(app, remote_ip, remote_port, protocol)` meme sous burst.
- **Regle nft `interception`** : `iif lo accept` puis `ct state new queue num 0 bypass` ajoutee au boot.
- **Mode degrade** : daemon demarre meme si NFQUEUE echoue.
- **Config `[nfqueue]`** : `enabled`, `queue_num`, `max_queued`, `overflow_policy`.
- **Limite documentee** : timeout 28 s par decision (kernel jette a 30 s) ; audit `Severity::Warning, Category::Decision` sur expiration.
- **Smoke test** gate par `SYSWALL_TEST_NFQUEUE` dans `crates/daemon/tests/nfqueue_smoke_test.rs`.

Dependances Cargo ajoutees : `nfq = "0.2.5"`, `etherparse = "0.20"` (workspace, dans `crates/infra/Cargo.toml`).

### UX & Accessibility

- **Killswitch immediat + toast undo 5 s** : plus de modal de confirmation paradoxal sur une action d'urgence ; un undo persistant 5 s couvre les mistaps mobile.
- **Raccourcis clavier popup decision** : `a`/`Enter` (autoriser une fois), `b` (bloquer une fois), `Shift+A` (toujours autoriser), `Shift+B` (toujours bloquer), `i` (ignorer), `Esc` (ignorer). Touches affichees via balises `<kbd>`.
- **Page Audit virtualisee** : remplacement de la pagination par `Table.svelte` virtual scroll. Scroll fluide sur >= 5000 evenements.
- **Modal focus trap** + restitution du focus a la fermeture (action Svelte `focusTrap`). Conformite WCAG 2.4.3.
- **Contraste WCAG AA** : `--text-tertiary` remonte de `#636366` (3.4:1) a `#8e8e93` (4.6:1). Nouveau token `--text-disabled` pour les usages decoratifs uniquement. `--text-secondary` passe a `#c7c7cc` pour preserver la hierarchie.
- **Debounce filtres** : recherche Connexions et Audit debouncee 250 ms.
- **Toggles regles** : `role="switch"` + `aria-checked` pour lecteurs d'ecran.
- **Sidebar mobile** : tap targets >= 44x44 px (WCAG 2.5.5).
- **Toast extensible** : nouveau champ `action?: { label, handler }` + barre de progression visuelle.

Nouveaux utilitaires : `crates/ui/src/lib/utils/debounce.ts`, `crates/ui/src/lib/actions/focus_trap.ts`.

### Design Polish

- **Direction artistique tranchee** : macOS Dark conserve, accent identitaire SysWall `#2cd4d4` (cyan turquoise) reserve aux moments-cles (logo, killswitch actif, filet d'interception).
- **Web fonts auto-hostees** : Inter Variable + JetBrains Mono dans `crates/ui/static/fonts/`. `font-display: swap`.
- **Logo SysWall SVG** : composant `SyswallLogo.svelte` (mark + wordmark) + `favicon.svg`.
- **Icones Lucide** : remplacement des emojis sidebar (LayoutDashboard, Network, Shield, BrainCircuit, Ban, ClipboardList, Settings).
- **Polish tableaux denses** : zebra-striping (`--bg-row-stripe`), sticky header shadow au scroll, `font-variant-numeric: tabular-nums` global, hover de ligne plus marque (`--bg-row-hover`).
- **`StatCard:hover`** : translation 1 px + ombre.
- **`Input`** : etats `error` (bordure rouge + helper text) et `disabled` (opacite 0.5).
- **Card** : prop `glow` retiree (YAGNI).
- **Pulsation killswitch** : 2 s ease-in-out cyan quand le reseau est actif. Desactivee sous `prefers-reduced-motion: reduce`.
- **Tokens** : `--accent-syswall`, `--accent-syswall-dim`, `--accent-syswall-glow`, `--bg-row-hover`, `--bg-row-stripe`, `--shadow-sticky-header`.
