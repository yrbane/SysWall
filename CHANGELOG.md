# Changelog

Toutes les modifications notables seront documentees ici.
All notable changes documented here.

## [0.3.1] - 2026-07-15 · « Déploiement en une commande »

### Fixed / Corrige

- **Le daemon ne démarrait pas sous systemd** : le unit ne pointait pas vers la config, le daemon cherchait alors `config/default.toml` relatif au cwd (`/` sous systemd) et crashait. Ajout de `Environment=SYSWALL_CONFIG=/etc/syswall/config.toml`. / The daemon failed to start under systemd because the unit did not reference the config; it looked for `config/default.toml` relative to cwd (`/`) and crashed. Added the absolute config path.
- **`User=syswall` sans utilisateur** : le unit tourne en `User=syswall` mais les installeurs ne créaient que le groupe (le seul groupe fait échouer `systemctl start`). `install.sh` et `system/arch/syswall.install` créent désormais l'utilisateur système dédié (`useradd --system`). / The unit runs as `User=syswall` but installers only created the group, so `systemctl start` failed. Both installers now create the dedicated system user.
- **`install.sh` cassé par le build UI** : le `cd crates/ui && … && cd ../..` laissait le cwd dans `crates/ui` quand le build UI échouait (exception `set -e` sur liste `&&`), faisant planter la copie du daemon. Racine résolue en absolu, build UI isolé dans un sous-shell non bloquant, appel direct du CLI `tauri build --no-bundle` (le `--` de `npm run` avalait le flag et le bundling AppImage, qui exige le réseau via linuxdeploy, tournait quand même), chemin du binaire UI corrigé (`target/release/ui`), et authentification sudo demandée en amont (échec rapide plutôt qu'après plusieurs minutes de build). / `install.sh` was broken by the UI build: a failed UI build left the cwd inside `crates/ui`, breaking the daemon copy. Root resolved absolutely, UI build isolated in a non-fatal subshell, tauri CLI called directly with `--no-bundle`, UI binary path fixed, and sudo authenticated upfront.

### Added / Ajoute

- **Installation en une seule commande** : `system/install.sh` détecte la distribution (`/etc/os-release`), refuse proprement les systèmes sans systemd, signale le paquet natif sur la famille Arch, puis **installe ET démarre** le service (`systemctl restart` + statut). / One-command install: `install.sh` detects the distribution, cleanly refuses non-systemd systems, hints the native package on the Arch family, then installs AND starts the service.
- **Garde-fou CI** `system/tests/check-service-config.sh` : vérifie la cohérence unit/installeurs (`SYSWALL_CONFIG`, utilisateur `syswall`, détection distro, démarrage du service). Câblé au job `hardening-check`. / CI guard test asserting unit/installer consistency, wired into the `hardening-check` job.

## [0.3.0] - 2026-05-05

Version de stabilisation post-V0.2 : finition technique, hardening CI, polish.

### Added / Ajoute

- **Action `Defer` reelle sur popup decision** : nouvelle variante `DecisionAction::Defer { duration_secs: u64 }` qui snooze la `dedup_key` en memoire pendant N secondes (1..=86400). Les nouveaux flux matchant retombent sur Drop sans popup, puis la decision se re-declenche apres expiration. Raccourci UI `Esc` -> `defer:300` (5 min). Parser gRPC `defer:N` avec validation des bornes.
- **`VerdictWaitError` typed** : remplace le silent fallback `Ok(Drop)` dans `wait_for_verdict` par 3 variantes typees (`Timeout`, `ChannelClosed`, `ChannelLagged { missed }`). Audit dedie par variante avec severity adaptee (Warning timeout, Error channel) et metadata `wait_error` machine-readable.
- **Benches Criterion** : `crates/infra/benches/nfq_parser_bench.rs` (parsing IPv4/IPv6 TCP/UDP : ~160-465 ns/paquet) et `crates/app/benches/dedup_key_bench.rs` (~360 ns/call). Le bench `policy_bench.rs` preexiste.
- **Real gRPC test harness** : `crates/daemon/tests/grpc_limits_test.rs` n'est plus un squelette. `message_over_1mib_is_rejected` envoie 2 MiB et verifie `OutOfRange` ; `small_message_is_accepted` valide le happy path. `concurrency_limit_is_enforced` reste TODO V0.4.
- **`cargo deny` en CI** : nouveau `deny.toml` (whitelist licenses MIT/Apache-2/BSD/ISC/Zlib/MPL-2/Unicode-3.0/CC0/0BSD), 17 RUSTSEC ignorees ligne par ligne pour les unmaintained transitives Tauri/GTK/wry. Job `deny` dans CI via `EmbarkStudios/cargo-deny-action@v2`.
- **Jobs CI dedie** : `grpc-integration` (env `SYSWALL_TEST_GRPC=1`, sans privileges) et `nfqueue-smoke` (sudo + modprobe nfnetlink_queue, `continue-on-error: true` car le runner peut ne pas avoir CAP_NET_ADMIN).
- **Property tests PolicyEngine** : 6 invariants verifies par proptest (politique par defaut, no-panic, coherence de la regle matchee, first-match-wins, isolation des familles IP, bornes de ports inclusives) dans `crates/domain/tests/policy_engine_proptest.rs`.
- **Fuzzing cargo-fuzz** : 3 cibles libFuzzer sur les surfaces d'entree non fiables — JSON criteria/scope/rule (`crates/domain/fuzz`), config TOML et converter gRPC `CreateRuleRequest` avec entrees Arbitrary biaisees (`crates/daemon/fuzz`). Job CI `fuzz-smoke` (60 s/cible, nightly).
- **Lib daemon** : `crates/daemon/src/lib.rs` expose `config` et `grpc` pour les tests d'integration et le fuzzing (le binaire reste inchange).

### Changed / Modifie

- **Version workspace** bumpee `0.2.0 -> 0.3.0`. La nouvelle variante `DecisionAction::Defer { .. }` est techniquement breaking pour les clients gRPC qui font un match exhaustif, justifiant le bump minor.
- **Tests sortis des god-files infra** : `crates/infra/src/nftables/translator/mod.rs` 511 -> 113 LOC (production-only) ; `crates/infra/src/nftables/adapter/mod.rs` 717 -> 606 LOC. Tests deplaces dans `tests.rs` siblings via `#[cfg(test)] mod tests;` (preserve la visibilite `pub(super)`).
- **License declaree** explicitement sur tous les crates du workspace (via `license.workspace = true`) plus `crates/ebpf-prog` et `crates/ui/src-tauri` (license = "MIT" direct car hors workspace ou config differente).

### Documentation

- **`docs/roadmap-2026-2027.md`** : roadmap V0.3 -> V1.0 sur 6-9 mois (Stabilization, UX+i18n, Packaging, Ecosysteme).
- **GitHub Release V0.2.0** publiee avec corps structure (highlights, prerequis, CHANGELOG complet).

### Notes

V0.3.6 (logo final designe pro) reste externe : le logo SVG livre en V0.2.0 est un placeholder propre (bouclier + filet d'interception) ; la finalisation par un graphiste reste a programmer.

---

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

### Config cablage (sous-projet C.1)

Cinq champs config TOML auparavant signales `never read` par le compilateur sont desormais utilises :

- **`daemon.watchdog_interval_secs`** : envoi periodique de `sd_notify(WATCHDOG=1)` a systemd. Si `NOTIFY_SOCKET` est absent (lance hors systemd), no-op silencieux. Frequence d'envoi = `interval_secs / 2` (recommandation systemd).
- **`database.journal_retention_days`** : tache de rotation tokio quotidienne qui supprime les `audit_events` anterieurs a `Utc::now() - retention_days`. `0` desactive la rotation. Nouvelle methode `AuditRepository::delete_before(cutoff)`.
- **`learning.enabled`** : `false` desactive completement la creation de `PendingDecision` ; les flux sans regle retombent sur `default_policy`.
- **`learning.default_timeout_action`** : action appliquee apres expiration d'un verdict NFQUEUE (`"allow"` ou `"block"`, defaut `"block"`).
- **`learning.overflow_action`** : action appliquee quand `max_pending_decisions` est atteint (`"allow"` ou `"block"`, defaut `"block"`). Audit `Severity::Warning, Category::Decision` sur saturation.

Trois champs supprimes (YAGNI — sans valeur ajoutee) :

- `daemon.log_dir` (gere par systemd `LogsDirectory=syswall` + journald).
- `ui.theme` (SysWall est dark-only par design).
- `ui.refresh_interval_ms` (l'UI utilise des streams gRPC, pas du polling).

Nouvelle dependance Cargo : `sd-notify = "0.4"` (UnixDatagram raw, pas de -lsystemd requis).
