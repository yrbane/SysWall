//! Notifie systemd periodiquement (sd_notify WATCHDOG=1) si lance sous systemd.
//! Periodically notifies systemd (sd_notify WATCHDOG=1) when launched under systemd.
//!
//! Utilise `sd-notify` (UnixDatagram raw) plutot que `libsystemd` (C FFI) :
//! pas de dependance systeme `-lsystemd` requise, compilation garantie en sandbox.
//! Uses `sd-notify` (raw UnixDatagram) instead of `libsystemd` (C FFI):
//! no system `-lsystemd` dependency required, guaranteed to compile in sandbox.

use std::time::Duration;

use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Lance la tache watchdog. Si NOTIFY_SOCKET est absent (pas systemd), no-op.
/// Spawns the watchdog task. If NOTIFY_SOCKET is absent (no systemd), no-op.
pub fn spawn_watchdog(interval_secs: u64, cancel: CancellationToken) {
    if std::env::var("NOTIFY_SOCKET").is_err() {
        warn!(
            target: "watchdog",
            "NOTIFY_SOCKET absent — pas lance par systemd, watchdog desactive"
        );
        return;
    }
    // Frequence d'envoi = interval / 2 (recommandation systemd : pinger a la moitie de WatchdogSec)
    // Send frequency = interval / 2 (systemd recommendation: ping at half of WatchdogSec)
    let send_interval = Duration::from_secs(interval_secs.max(2) / 2);
    info!(
        target: "watchdog",
        ?send_interval,
        "watchdog systemd actif"
    );
    tokio::spawn(async move {
        let mut ticker = interval(send_interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = sd_notify::notify(false, &[sd_notify::NotifyState::Watchdog]) {
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
