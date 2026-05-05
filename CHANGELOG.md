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
