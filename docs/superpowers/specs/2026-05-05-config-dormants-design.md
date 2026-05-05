# Spec — Sous-projet C.1 : Câblage des champs config dormants

> Date : 2026-05-05
> Branche cible : `main`
> Cycle : spec → plan → exécution
> Pré-requis : sous-projets A, B, C.2, D, E complétés

## Contexte

L'audit fonctionnel du 2026-05-04 a relevé que 8 champs de la config TOML sont déclarés mais signalés `never read` par le compilateur Rust :

- `daemon.watchdog_interval_secs`
- `daemon.log_dir`
- `database.journal_retention_days`
- `learning.enabled`
- `learning.default_timeout_action`
- `learning.overflow_action`
- `ui.theme`
- `ui.refresh_interval_ms`

Le sous-projet C.1 conclut le cycle d'audit en câblant les 5 champs réellement utiles et en supprimant les 3 champs sans valeur ajoutée (YAGNI).

## Objectifs

- Câbler `watchdog_interval_secs` via `sd_notify(WATCHDOG=1)` périodique → systemd détecte un démon zombie et le redémarre.
- Câbler `journal_retention_days` via une tâche périodique qui purge les `audit_events` plus anciens que N jours.
- Câbler `learning.default_timeout_action` → l'action appliquée quand `wait_for_verdict` expire (actuellement hardcodé `Drop`).
- Câbler `learning.overflow_action` → l'action appliquée quand le nombre de `PendingDecision` dépasse `max_pending_decisions`.
- Câbler `learning.enabled` → si `false`, les flux sans règle retombent sur `default_policy` au lieu de créer une `PendingDecision`.
- Supprimer `daemon.log_dir` (systemd `LogsDirectory=syswall` + journald s'en chargent déjà).
- Supprimer `ui.theme` (SysWall est dark-only par design — la couleur est imposée par les tokens CSS, pas configurable).
- Supprimer `ui.refresh_interval_ms` (l'UI utilise des streams gRPC, pas du polling).

Hors-scope :
- Ajout de nouveaux champs config (le scope est uniquement de câbler/nettoyer l'existant).
- Migration de format TOML (les utilisateurs existants n'ont pas encore de config v0.2 en wild).

## Décisions de conception

### C.1.A — Watchdog systemd

Approche : tâche tokio périodique qui appelle `nix::sys::stat::stat` ou `libsystemd::daemon::notify_watchdog`. Mais `nix` n'expose pas `sd_notify`. Solution : utiliser le crate `libsystemd = "0.7"` (workspace dep, ajouté pour ce câblage) ou écrire un wrapper minimal autour du socket `NOTIFY_SOCKET`.

**Décision** : ajouter `libsystemd = "0.7"` à `crates/daemon/Cargo.toml`. La tâche s'exécute à `WatchdogSec/2` (recommandation systemd) et envoie `WATCHDOG=1` au socket `$NOTIFY_SOCKET`. Si la variable n'existe pas (lancement hors systemd), la tâche se loggue un warn et ne fait rien (mode dégradé).

`watchdog_interval_secs` du daemon TOML est utilisé pour calculer l'intervalle d'envoi (= valeur / 2). Il doit être cohérent avec le `WatchdogSec=15` du service unit (ajouté dans le sous-projet A — à vérifier).

Si `WatchdogSec` n'est pas défini dans le service unit, l'envoi `WATCHDOG=1` est sans effet (systemd l'ignore). Sécuritaire.

### C.1.B — Rotation journal d'audit

Approche : tâche tokio quotidienne (`tokio::time::interval(Duration::from_secs(86400))`) qui appelle `audit_repo.delete_before(cutoff)` où `cutoff = Utc::now() - Duration::days(retention)`.

La méthode `delete_before` doit exister sur `AuditRepository` — vérifier ; sinon l'ajouter avec un `DELETE FROM audit_events WHERE timestamp < ?` paramétré.

L'intervalle 24 h n'est pas configurable (YAGNI) ; seule la rétention en jours l'est.

### C.1.C — `learning.enabled`

Dans `LearningService::pending_verdict_for`, premier check :

```rust
if !self.config.enabled {
    // Apprentissage désactivé : retomber sur la default policy.
    // Learning disabled: fall back to default policy.
    return Ok(match self.default_policy {
        DefaultPolicy::Allow => PacketVerdict::Accept,
        DefaultPolicy::Block | DefaultPolicy::Ask => PacketVerdict::Drop,
    });
}
```

Quand le learning est disabled mais que `default_policy = Ask`, on droppe (équivalent à `Block` à l'usage : si tu désactives le learning mais demandes "ask", on ne peut rien demander, donc on ferme).

### C.1.D — `learning.default_timeout_action`

Valeurs valides : `"allow"`, `"block"`. Default : `"block"`.

Dans `wait_for_verdict`, sur expiration :

```rust
Err(_) => {
    let event = AuditEvent::new(/* timeout audit existant */);
    let _ = self.audit_repo.append(&event).await;
    Ok(match self.config.default_timeout_action.as_str() {
        "allow" => PacketVerdict::Accept,
        _ => PacketVerdict::Drop,  // tout autre valeur => block (sécuritaire)
    })
}
```

Le mapping est conservateur : tout sauf `"allow"` = `Drop`. Aucune validation au démarrage (les valeurs invalides sont silencieusement traitées comme `block`). Bonus : un log `warn!` au démarrage si la valeur n'est pas `"allow"` ni `"block"`.

### C.1.E — `learning.overflow_action`

Valeurs valides : `"allow"`, `"block"`. Default : `"block"`.

Avant la création d'une nouvelle `PendingDecision`, check :

```rust
let pending_count = self.pending_repo.count_pending().await?;
if pending_count >= self.config.max_pending_decisions {
    let event = AuditEvent::new(
        Severity::Warning,
        EventCategory::Decision,
        format!("queue overflow: max_pending_decisions={} atteint", self.config.max_pending_decisions),
    );
    let _ = self.audit_repo.append(&event).await;
    return Ok(match self.config.overflow_action.as_str() {
        "allow" => PacketVerdict::Accept,
        _ => PacketVerdict::Drop,
    });
}
```

### C.1.F — Suppressions YAGNI

Trois champs supprimés du `crates/daemon/src/config.rs` et de `config/default.toml` :

- `daemon.log_dir`
- `ui.theme`
- `ui.refresh_interval_ms`

Pour chacun :
- Retirer le champ de la struct.
- Retirer la valeur du `default.toml`.
- `serde` ignore les champs inconnus par défaut → un utilisateur avec un vieux config.toml ne casse pas (le champ est simplement ignoré).
- Retirer toute référence dans le code (probablement aucune puisque marqué `never read`).

CHANGELOG documente la suppression.

## Architecture

### Files modifiés / créés

| Fichier | Changement |
|---|---|
| `crates/daemon/Cargo.toml` | Ajout `libsystemd = "0.7"` |
| `crates/daemon/src/config.rs` | Suppressions `log_dir`/`theme`/`refresh_interval_ms` ; lecture des autres champs |
| `crates/daemon/src/watchdog.rs` (nouveau) | Tâche périodique `notify_watchdog` |
| `crates/daemon/src/main.rs` ou `bootstrap.rs` | Lance la tâche watchdog |
| `crates/app/src/services/journal_rotation.rs` (nouveau) | Service de rotation périodique |
| `crates/app/src/services/mod.rs` | Export |
| `crates/daemon/src/bootstrap.rs` | Lance `JournalRotationService` |
| `crates/app/src/services/learning_service/mod.rs` | Câblage `enabled`/`default_timeout_action`/`overflow_action` |
| `crates/domain/src/ports/repositories.rs` | Si `delete_before` manque sur `AuditRepository`, l'ajouter |
| `crates/infra/src/persistence/audit_repository/writes.rs` | Implémentation `delete_before` si nouveau |
| `config/default.toml` | Suppressions YAGNI ; valeurs des champs câblés |
| `CHANGELOG.md` | Section "Config câblage" |

### `WatchdogService`

`crates/daemon/src/watchdog.rs` :

```rust
use std::time::Duration;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Lance une tâche tokio qui notifie systemd périodiquement (sd_notify WATCHDOG=1).
/// Spawns a tokio task that periodically notifies systemd (sd_notify WATCHDOG=1).
///
/// `interval_secs` est la valeur `daemon.watchdog_interval_secs` du TOML.
/// La fréquence d'envoi effective est `interval_secs / 2` pour respecter la marge systemd.
pub fn spawn_watchdog(interval_secs: u64, cancel: CancellationToken) {
    if std::env::var("NOTIFY_SOCKET").is_err() {
        warn!(target: "watchdog", "NOTIFY_SOCKET absent — pas lance par systemd, watchdog desactive");
        return;
    }
    let send_interval = Duration::from_secs(interval_secs.max(2) / 2);
    info!(target: "watchdog", interval_secs = ?send_interval, "watchdog systemd actif");
    tokio::spawn(async move {
        let mut ticker = interval(send_interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = libsystemd::daemon::notify(false, &[libsystemd::daemon::NotifyState::Watchdog]) {
                        debug!(target: "watchdog", "echec notify: {e}");
                    }
                }
                _ = cancel.cancelled() => {
                    info!(target: "watchdog", "watchdog termine proprement");
                    return;
                }
            }
        }
    });
}
```

### `JournalRotationService`

`crates/app/src/services/journal_rotation.rs` :

```rust
use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use syswall_domain::ports::repositories::AuditRepository;

const ROTATION_INTERVAL_SECS: u64 = 86400; // 24 h, non configurable (YAGNI)

/// Service de rotation du journal d'audit : supprime quotidiennement les events
/// plus anciens que `retention_days`.
/// Audit journal rotation service: deletes events older than `retention_days` daily.
pub fn spawn_journal_rotation(
    audit_repo: Arc<dyn AuditRepository>,
    retention_days: u32,
    cancel: CancellationToken,
) {
    if retention_days == 0 {
        warn!(target: "rotation", "journal_retention_days = 0 -> rotation desactivee");
        return;
    }
    info!(target: "rotation", retention_days, "rotation du journal d'audit active");
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(ROTATION_INTERVAL_SECS));
        ticker.tick().await; // skip the first immediate tick — laisser le boot finir
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let cutoff = Utc::now() - ChronoDuration::days(retention_days as i64);
                    match audit_repo.delete_before(cutoff).await {
                        Ok(deleted) => info!(target: "rotation", deleted, ?cutoff, "rotation effectuee"),
                        Err(e) => warn!(target: "rotation", "echec rotation: {e}"),
                    }
                }
                _ = cancel.cancelled() => {
                    info!(target: "rotation", "rotation terminee proprement");
                    return;
                }
            }
        }
    });
}
```

### Port `AuditRepository::delete_before` (si manquant)

Vérifier dans `crates/domain/src/ports/repositories.rs`. Si la méthode existe → bien. Sinon ajouter :

```rust
#[async_trait]
pub trait AuditRepository: Send + Sync {
    // ... existing methods ...

    /// Supprime tous les events strictement antérieurs à `cutoff`. Retourne le nombre supprimé.
    /// Deletes all events strictly before `cutoff`. Returns count deleted.
    async fn delete_before(&self, cutoff: chrono::DateTime<chrono::Utc>) -> Result<u64, DomainError>;
}
```

Avec implémentation SQLite :

```sql
DELETE FROM audit_events WHERE timestamp < ?
```

`rusqlite::Connection::execute("DELETE ...", params![cutoff.to_rfc3339()])?` retourne `usize`, à caster en `u64`.

Ajouter aussi à `FakeAuditRepository` une implémentation simple pour les tests.

## Tests

`crates/app/src/services/learning_service/mod.rs` (tests existants étendus) :

```rust
#[tokio::test]
async fn decide_learning_disabled_with_default_block_returns_drop() {
    // FakeRuleRepository empty + DefaultPolicy::Ask + learning.enabled = false.
    // Expect: PacketVerdict::Drop, no PendingDecision created.
}

#[tokio::test]
async fn decide_learning_disabled_with_default_allow_returns_accept() {
    // Same as above but DefaultPolicy::Allow.
    // Expect: PacketVerdict::Accept, no PendingDecision created.
}

#[tokio::test(start_paused = true)]
async fn decide_pending_overflow_with_block_action_drops() {
    // Pre-fill pending_repo with max_pending_decisions decisions.
    // New flow → expect Drop + audit event of severity Warning.
}

#[tokio::test(start_paused = true)]
async fn decide_pending_overflow_with_allow_action_accepts() {
    // Same but overflow_action = "allow".
}

#[tokio::test(start_paused = true)]
async fn timeout_with_default_timeout_action_allow_returns_accept() {
    // Pending decision created, no resolve, advance 30s, default_timeout_action = "allow".
    // Expect: PacketVerdict::Accept.
}
```

5 tests nouveaux. Le test existant `decide_pending_timeout_returns_drop_with_audit` reste valide (son `default_timeout_action` est `"block"` par défaut).

`crates/app/src/services/journal_rotation.rs` (smoke test) :

```rust
#[tokio::test(start_paused = true)]
async fn rotation_calls_delete_before_with_cutoff() {
    let fake_audit = Arc::new(FakeAuditRepository::new());
    let cancel = CancellationToken::new();
    spawn_journal_rotation(fake_audit.clone(), 30, cancel.clone());
    tokio::time::advance(Duration::from_secs(86_400 + 1)).await;
    tokio::task::yield_now().await;
    cancel.cancel();
    // FakeAuditRepository tracks delete_before calls.
    assert_eq!(fake_audit.delete_before_call_count(), 1);
}
```

Pas de test pour le watchdog (il dépend de systemd — couvert manuellement).

## Risques & mitigations

| Risque | Probabilité | Mitigation |
|---|---|---|
| `libsystemd` ne compile pas hors Linux | Inexistant pour SysWall | SysWall est Linux-only — confirmé par les capabilities |
| `delete_before` lent sur grosses tables | Faible | Index `idx_audit_timestamp` à vérifier ; sinon ajouter en migration |
| Suppression de `theme` casse une lecture côté UI | Faible | Vérifier qu'aucun fichier `.svelte`/`.ts` ne lit `theme` ; si oui, hardcoder `'dark'` côté UI |
| Watchdog spam de `Watchdog` quand `NOTIFY_SOCKET` absent | Moyen | Check au démarrage et early-return (déjà dans le code ci-dessus) |
| Tests overflow nécessitent un fake spécifique | Faible | Ajouter `FakePendingDecisionRepository::set_pending_count(n)` pour pré-remplir |

## Critères de succès

- [ ] `cargo check -p syswall-daemon` ne reporte plus de `never read` sur les 5 champs câblés.
- [ ] `cargo test --workspace --exclude ui` ≥ 339 tests pass (334 baseline + 5 nouveaux).
- [ ] `cargo clippy --workspace --exclude ui --all-targets -- -D warnings` reste à 0.
- [ ] `daemon.log_dir`, `ui.theme`, `ui.refresh_interval_ms` retirés de la struct config et de `default.toml`.
- [ ] CHANGELOG section "Config câblage" sous V0.2.

## Plan d'exécution (commits ciblés)

| # | Étape | Type |
|---|---|---|
| 1 | Suppression YAGNI : `daemon.log_dir`, `ui.theme`, `ui.refresh_interval_ms` | refactor |
| 2 | `AuditRepository::delete_before` (port + SQLite + Fake) si manquant | feat |
| 3 | `WatchdogService` (`crates/daemon/src/watchdog.rs`) + bootstrap | feat |
| 4 | `JournalRotationService` (`crates/app/src/services/journal_rotation.rs`) + bootstrap + tests | feat |
| 5 | `learning.enabled` câblé + 2 tests | feat |
| 6 | `learning.overflow_action` câblé + 2 tests | feat |
| 7 | `learning.default_timeout_action` câblé + 1 test | feat |
| 8 | CHANGELOG + verifications finales | docs |

8 commits estimés.

## Hors-scope

- Ajout de nouveaux champs config.
- Reformulation des autres champs config qui sont déjà utilisés.
- UI feedback sur les watchdog/rotation events (les audits suffisent).

---

*Spec rédigée le 2026-05-05.*
