//! Service de rotation du journal d'audit : supprime quotidiennement les events
//! plus anciens que `retention_days`.
//! Audit journal rotation service: deletes events older than `retention_days` daily.

use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use syswall_domain::ports::repositories::AuditRepository;

/// Intervalle de rotation : 24 h (non configurable — YAGNI, le daemon n'a pas besoin de moins).
/// Rotation interval: 24 h (not configurable — YAGNI, the daemon doesn't need less).
const ROTATION_INTERVAL_SECS: u64 = 86_400;

/// Lance une tache tokio qui purge quotidiennement les events plus anciens que `retention_days`.
/// Spawns a tokio task that purges events older than `retention_days` once a day.
pub fn spawn_journal_rotation(
    audit_repo: Arc<dyn AuditRepository>,
    retention_days: u32,
    cancel: CancellationToken,
) {
    if retention_days == 0 {
        warn!(
            target: "rotation",
            "journal_retention_days = 0 -> rotation desactivee"
        );
        return;
    }
    info!(
        target: "rotation",
        retention_days,
        "rotation du journal d'audit active"
    );
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(ROTATION_INTERVAL_SECS));
        // Ignore le premier tick immediat pour eviter une purge au demarrage.
        // Skip the first immediate tick to avoid a purge on startup.
        ticker.tick().await;
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let cutoff = Utc::now() - ChronoDuration::days(retention_days as i64);
                    match audit_repo.delete_before(cutoff).await {
                        Ok(deleted) => info!(
                            target: "rotation",
                            deleted,
                            cutoff = cutoff.to_rfc3339(),
                            "rotation effectuee"
                        ),
                        Err(e) => warn!(
                            target: "rotation",
                            "echec rotation: {e}"
                        ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::fake_audit_repository::FakeAuditRepository;

    #[tokio::test(start_paused = true)]
    async fn rotation_zero_retention_days_is_noop() {
        let fake = Arc::new(FakeAuditRepository::new());
        let cancel = CancellationToken::new();
        spawn_journal_rotation(fake.clone(), 0, cancel.clone());
        tokio::time::advance(Duration::from_secs(86_400 * 2)).await;
        cancel.cancel();
        assert_eq!(fake.delete_before_call_count(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn rotation_calls_delete_before_after_one_day() {
        let fake = Arc::new(FakeAuditRepository::new());
        let cancel = CancellationToken::new();
        spawn_journal_rotation(fake.clone(), 30, cancel.clone());
        // Tick initial (skip) : avance une premiere journee.
        // Initial tick (skip): advance one day.
        tokio::time::advance(Duration::from_secs(86_400 + 1)).await;
        // Laisse la tache consommer le tick de skip.
        // Let the task consume the skip tick.
        tokio::task::yield_now().await;
        // Avance d'une deuxieme journee pour declencher la vraie rotation.
        // Advance a second day to trigger actual rotation.
        tokio::time::advance(Duration::from_secs(86_400 + 1)).await;
        // Laisse la tache s'executer.
        // Let the task run.
        tokio::task::yield_now().await;
        cancel.cancel();
        assert_eq!(fake.delete_before_call_count(), 1);
    }
}
