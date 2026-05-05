use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use syswall_domain::entities::PendingDecisionId;
use syswall_domain::ports::interception::PacketVerdict;

/// Manages broadcast channels keyed by `PendingDecisionId`. Multiple captured packets
/// awaiting the same decision subscribe to the same channel; resolution publishes
/// the verdict to all subscribers and removes the channel.
/// Gère les canaux broadcast indexés par `PendingDecisionId`. Plusieurs paquets
/// capturés attendant la même décision s'abonnent au même canal ; la résolution
/// publie le verdict à tous les abonnés et retire le canal.
#[derive(Debug, Default)]
pub struct VerdictBroadcasts {
    inner: Arc<Mutex<HashMap<PendingDecisionId, broadcast::Sender<PacketVerdict>>>>,
}

impl VerdictBroadcasts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe a receiver to an existing broadcast (creating if absent).
    /// Abonne un receveur à un broadcast existant (création si absent).
    pub async fn subscribe(&self, id: PendingDecisionId) -> broadcast::Receiver<PacketVerdict> {
        let mut map = self.inner.lock().await;
        let sender = map
            .entry(id)
            .or_insert_with(|| broadcast::channel(64).0);
        sender.subscribe()
    }

    /// Publish a verdict to all subscribers and remove the broadcast.
    /// Publie un verdict à tous les abonnés et retire le broadcast.
    pub async fn publish_and_remove(&self, id: PendingDecisionId, verdict: PacketVerdict) {
        let mut map = self.inner.lock().await;
        if let Some(sender) = map.remove(&id) {
            // Best-effort send: receivers may have already dropped (timeouts).
            // Envoi best-effort : les receveurs peuvent avoir deja ete abandonnes.
            let _ = sender.send(verdict);
        }
    }

    /// Number of active broadcasts (for tests/observability).
    /// Nombre de broadcasts actifs (pour tests/observabilité).
    pub async fn active_count(&self) -> usize {
        self.inner.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_propagates_verdict_to_subscribers() {
        let broadcasts = VerdictBroadcasts::new();
        let id = PendingDecisionId::new();
        let mut rx1 = broadcasts.subscribe(id).await;
        let mut rx2 = broadcasts.subscribe(id).await;
        broadcasts
            .publish_and_remove(id, PacketVerdict::Accept)
            .await;
        assert_eq!(rx1.recv().await.unwrap(), PacketVerdict::Accept);
        assert_eq!(rx2.recv().await.unwrap(), PacketVerdict::Accept);
        assert_eq!(broadcasts.active_count().await, 0);
    }

    #[tokio::test]
    async fn publish_unknown_id_is_noop() {
        let broadcasts = VerdictBroadcasts::new();
        let id = PendingDecisionId::new();
        broadcasts.publish_and_remove(id, PacketVerdict::Drop).await;
        assert_eq!(broadcasts.active_count().await, 0);
    }
}
