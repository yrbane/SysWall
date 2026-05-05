# Spec — Sous-projet B : Hygiène du code SysWall

> Date : 2026-05-05
> Branche cible : `main`
> Cycle : spec → plan d'implémentation → exécution TDD → commits incrémentaux
> Pré-requis : sous-projet A (renforcement sécurité) complété (HEAD = `ffa3ddb`)

## Contexte

L'audit du 2026-05-04 (`docs/audit-2026-05-04.md`) et la review finale du sous-projet A (`docs/superpowers/plans/2026-05-05-security-hardening-plan.md`) ont identifié 6 axes d'hygiène :

1. **455 `unwrap()` en code de prod** (infra 269, app 120, daemon 31, domain 29, ebpf 6). Sur un démon firewall, une panic = perte de protection réseau.
2. **Incohérence de version** : `Cargo.toml` workspace = `0.1.0` mais paquets `system/arch/syswall-0.2.0-1...zst` = `0.2.0`.
3. **5 god-modules > 600 LOC** : `crates/daemon/src/grpc/converters.rs` (780), `crates/domain/src/services/policy_engine.rs` (639), `crates/infra/src/persistence/audit_repository.rs` (634), `crates/app/src/services/audit_service.rs` (625), `crates/infra/src/nftables/translator.rs` (611) et `adapter.rs` (592).
4. **24 warnings clippy `infra`** : `collapsible_if`, `Default impl manquant`, `redundant closures`, `unused imports`, `is_empty manquant`. Bloquent le `-D warnings` workspace.
5. **Dépendance Cargo `infra → app`** déclarée dans `crates/infra/Cargo.toml` mais inutilisée par le code source (vérifié en review finale du sous-projet A). Violation hexagonale au niveau Cargo.
6. **CI `cargo clippy --workspace -- -D warnings` non activé** : régressions silencieuses sur les warnings actuels.

## Objectifs

- Éradiquer les `unwrap()` en prod par crate, en remplacement par `?` (propagation), `expect("...")` documenté (cas infaillible), ou refactor d'API si l'unwrap signale un design fragile.
- Activer `#![deny(clippy::unwrap_used, clippy::expect_used)]` crate par crate après nettoyage.
- Aligner la version workspace à `0.2.0` (cohérent avec les paquets système, le tag git suivra à la demande de l'utilisateur).
- Scinder les 5 god-modules en sous-modules par responsabilité claire.
- Fixer les 24 warnings clippy infra (sans `#[allow(...)]` global, sauf justification documentée).
- Activer `cargo clippy --workspace --all-targets -- -D warnings` en CI.
- Supprimer la dépendance `syswall-app` de `crates/infra/Cargo.toml` après vérification qu'elle n'est pas utilisée.

Hors-scope :
- Refactorisation des architectures (le sous-projet n'est pas un redesign — uniquement de l'hygiène).
- Sous-projets C/D/E (fonctionnel, UX, design polish).
- Migration vers `cargo audit` / `cargo deny` (souhaitable mais hors hygiène brute, traité en sous-projet C).

## Décisions de conception

### B.1 — Stratégie d'éradication des `unwrap()`

**Approche hybride graduelle par crate** (option 3 du brainstorming) :

1. Crate par crate dans cet ordre : `domain` (29) → `app` (120) → `daemon` (31) → `infra` (269) → `ebpf` (6).
2. Pour chaque crate :
   - Audit de chaque occurrence : classifier en **infaillible** (logique pure, parsing de littéral, lock non-poisonable) ou **fragile** (IO, parsing externe, conversion potentiellement lossy).
   - **Infaillible** → `expect("raison technique en français")`. Le commentaire explique pourquoi le panic ne peut pas survenir (invariant maintenu par X, garanti par Y).
   - **Fragile** → propager via `?`, ajouter le variant `DomainError` si besoin (rare).
   - **Tests** → garder `unwrap()`/`expect()` autorisés dans les modules `#[cfg(test)]` (les tests doivent pouvoir signaler un échec brut).
3. Une fois tous les `unwrap()` traités dans une crate, ajouter en tête du `lib.rs` (ou `main.rs` pour daemon) :
   ```rust
   #![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
   ```
   Le `cfg_attr(not(test))` garde les tests permissifs.
4. **Domain abrite aussi quelques unwrap dans des tests** — vérifier qu'ils sont bien sous `#[cfg(test)]`. Aucun `clippy::unwrap_used` ne devrait fail dans les tests.
5. **Stratégie d'`expect()`** : les messages d'`expect()` doivent expliquer **pourquoi** c'est infaillible, pas **quoi** est attendu. Bons exemples :
   - `expect("hashmap construit ligne 12 avec cette clé garantie")`
   - `expect("Mutex jamais poisoné: pas de unwind dans le critical section")`
   - `expect("&'static str provient d'un literal compile-time")`
   Mauvais exemples (à proscrire) :
   - `expect("should not fail")`
   - `expect("must succeed")`

### B.2 — Bump de version 0.1.0 → 0.2.0

- Modifier `Cargo.toml` racine, ligne `version = "0.1.0"` → `version = "0.2.0"`.
- Vérifier que tous les `Cargo.toml` des crates du workspace utilisent `version.workspace = true` (sinon mettre à jour).
- Vérifier les paquets : `system/arch/syswall-0.2.0-...pkg.tar.zst` existe déjà → cohérent post-bump.
- **Pas de tag git** dans ce sous-projet (l'utilisateur taggera quand il publiera).
- Mettre à jour le CHANGELOG : la section `[0.2.0] - 2026-05-XX` actuelle devient datée explicitement (date du dernier commit du sous-projet B).

### B.3 — Scission des god-modules

Pour chaque module, **respecter une découpe par responsabilité** (single responsibility), pas par taille arbitraire. Cible : aucun fichier > 400 LOC dans le résultat.

| Module | Taille | Découpe proposée |
|---|---|---|
| `crates/daemon/src/grpc/converters.rs` (780) | 780 | 1 fichier par domaine d'entité : `converters/rule.rs`, `converters/decision.rs`, `converters/audit.rs`, `converters/connection.rs`, `converters/error.rs`, `converters/mod.rs` (re-exports) |
| `crates/domain/src/services/policy_engine.rs` (639) | 639 | `policy_engine/mod.rs` (orchestration), `policy_engine/matcher.rs` (matching critères), `policy_engine/evaluator.rs` (résolution priorité + default policy), `policy_engine/scoring.rs` si présent |
| `crates/infra/src/persistence/audit_repository.rs` (634) | 634 | `audit_repository/mod.rs` (impl trait), `audit_repository/queries.rs` (SELECT + filtres), `audit_repository/writes.rs` (INSERT batch), `audit_repository/migration.rs` (création tables) |
| `crates/app/src/services/audit_service.rs` (625) | 625 | CQRS-light : `audit_service/mod.rs` (struct + new), `audit_service/command.rs` (record/append batch), `audit_service/query.rs` (filtre/recherche/stats/export) |
| `crates/infra/src/nftables/translator.rs` (611) + `adapter.rs` (592) | 1203 | `translator/mod.rs` (orchestration), `translator/criteria.rs` (criteria → nft expression), `translator/action.rs` (RuleAction → nft verdict), `translator/system_rules.rs` (whitelist DNS/DHCP/NTP/loopback). `adapter.rs` reste si < 600 après extraction du `perform_rollback_static` (fait en sous-projet A) — sinon split en `adapter/{mod,apply,rollback,whitelist}.rs`. |

**Tests** : déplacer les tests existants dans le sous-module qui correspond. Si un test couvre plusieurs sous-modules, le mettre dans le module orchestrateur (`mod.rs`).

**API publique** : préserver l'API existante via re-export depuis `mod.rs` (`pub use submodule::Type;`) — aucun call site ne doit casser.

### B.4 — Clippy `infra` propre

24 warnings actuels. Approche fix par fix, pas de `#[allow]` global. Catégories typiques :

- `collapsible_if` → fusion des `if x { if y { ... } }` en `if x && y { ... }`.
- `Default` manquant sur `*::new()` sans args → ajouter `impl Default { fn default() -> Self { Self::new() } }`.
- `redundant_closure` → remplacer `|x| f(x)` par `f`.
- `is_empty` manquant sur types avec `len()` → ajouter `pub fn is_empty(&self) -> bool { self.len() == 0 }`.
- `unused_imports` (warn `unused import: warn`) → supprimer.

**Cas particulier** : `dns/snooper.rs:31` flag un `unused import: warn`. Soit l'import est mort, soit il est utilisé sous une feature non activée (vérifier avec `--all-features`).

### B.5 — Suppression dépendance `infra → app`

Inspecter `crates/infra/Cargo.toml` :
1. Confirmer que `syswall-app` est listé en `[dependencies]`.
2. `grep -r 'syswall_app\|use syswall_app' crates/infra/src/` doit retourner 0 résultats.
3. Si propre : supprimer la ligne, lancer `cargo check -p syswall-infra` pour confirmer pas de breakage.
4. Si dépendance utilisée (improbable mais possible via macro ou type ré-exporté) : documenter l'usage et différer la suppression à un sous-projet ultérieur.

### B.6 — CI `clippy --workspace --all-targets -- -D warnings`

Une fois B.1, B.4, B.5 livrés :
- Modifier `.github/workflows/ci.yml` job `clippy` (ou créer si absent) :
  ```yaml
  - name: cargo clippy
    run: cargo clippy --workspace --all-targets -- -D warnings
  ```
- Fixer les warnings résiduels qui apparaîtraient sur `--all-targets` (tests, examples, benches).

## Architecture (déjà en place)

Aucun changement structurel. Toutes les modifs sont :
- internes à un module (split de fichier),
- mécaniques (unwrap → ? / expect),
- de configuration (Cargo, CI).

## Plan d'exécution (commits ciblés)

| # | Étape | Crate | Type | Commits estimés |
|---|---|---|---|---|
| 1 | Bump version 0.1.0 → 0.2.0 + CHANGELOG date | workspace | docs+meta | 1 |
| 2 | Suppression dep `infra → app` | infra | chore | 1 |
| 3 | Éradication unwrap `domain` (29) + `deny` activé | domain | refactor | 2-3 |
| 4 | Split `policy_engine.rs` | domain | refactor | 1 |
| 5 | Éradication unwrap `app` (120) + `deny` activé | app | refactor | 4-6 |
| 6 | Split `audit_service.rs` (CQRS) | app | refactor | 1 |
| 7 | Éradication unwrap `daemon` (31) + `deny` activé | daemon | refactor | 2 |
| 8 | Split `converters.rs` par entité | daemon | refactor | 1 |
| 9 | Fix 24 warnings clippy infra | infra | fix | 1-2 |
| 10 | Éradication unwrap `infra` (269) + `deny` activé | infra | refactor | 6-10 |
| 11 | Split `audit_repository.rs` | infra | refactor | 1 |
| 12 | Split `translator.rs` (+ éventuel split `adapter.rs`) | infra | refactor | 1-2 |
| 13 | Éradication unwrap `ebpf` (6) + `deny` activé | ebpf | refactor | 1 |
| 14 | Activer `clippy --workspace --all-targets -D warnings` en CI | CI | feat | 1 |
| 15 | Documentation : CHANGELOG section "Code Hygiene", README pas de changement | docs | docs | 1 |

**Total estimé** : 25-35 commits atomiques. Chaque commit doit compiler et passer les tests à lui seul.

## Risques & mitigations

| Risque | Probabilité | Mitigation |
|---|---|---|
| Un `unwrap()` jugé infaillible cache un vrai bug | Moyenne | Pour chaque `expect()`, le message doit citer l'invariant. Si on ne peut pas justifier, c'est qu'il faut propager. |
| Split de fichier introduit des erreurs d'imports/visibilité | Faible | `cargo check` après chaque split. Tests existants doivent passer sans modification. |
| Suppression `infra → app` casse une feature cachée | Très faible | `grep` exhaustif avant suppression. `cargo check --all-features` après. |
| Le bump `0.2.0` crée un conflit avec un `Cargo.lock` existant | Faible | `cargo update -p syswall*` après bump pour rafraîchir. |
| Activer `deny(unwrap_used)` casse les builds en aval | Inexistant | Le `deny` ne s'applique qu'au crate qui le déclare, pas aux consommateurs. |

## Critères de succès

- [ ] `grep -rn 'unwrap()\|\.expect(' crates/{domain,app,daemon,infra,ebpf}/src/ | grep -v test` retourne 0 occurrences sauf `expect()` documentés.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
- [ ] `cargo test --workspace --exclude syswall-ui` retourne ≥ 308 tests pass, 0 fail.
- [ ] Aucun fichier `.rs` > 500 LOC dans `crates/{domain,app,daemon,infra}/src/` sauf justification documentée en commentaire de tête.
- [ ] `Cargo.toml` racine `version = "0.2.0"`.
- [ ] `crates/infra/Cargo.toml` ne dépend plus de `syswall-app`.
- [ ] `.github/workflows/ci.yml` lance `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] CHANGELOG entry pour la section "Code Hygiene" dans la version 0.2.0.

## Hors-scope explicite

- Tests d'intégration nouveaux (le sous-projet ne change pas le comportement, donc les tests existants suffisent).
- Refactor de `connection_service.rs` ou autres services < 600 LOC (sauf si une scission > 600 émerge accidentellement).
- Suppression des warnings de l'UI Tauri (sous-projet D).
- Optimisations de perf (micro ou macro).

---

*Spec rédigée le 2026-05-05. À approuver avant transition vers writing-plans.*
