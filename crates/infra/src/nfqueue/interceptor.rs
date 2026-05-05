//! Adapter NFQUEUE : lit les paquets, demande au handler un verdict, le transmet au kernel.
//! NFQUEUE adapter: read packets, ask handler for verdict, forward to kernel.
//!
//! Architecture sync/async :
//! - nfq 0.2.5 est entièrement synchrone (bloquant sur recv()).
//! - Message n'est pas Send (partage un Arc<Vec<u32>> lié au socket), donc on ne peut pas
//!   séparer recv() et verdict() dans des threads distincts.
//! - Solution : spawn_blocking contenant la boucle entière. Le handler async est appelé via
//!   tokio::runtime::Handle::block_on() depuis le thread bloquant.
//! - La queue tourne en mode non-bloquant (set_nonblocking) pour vérifier le cancel token.
//!
//! Sync/async architecture:
//! - nfq 0.2.5 is fully synchronous (blocking recv()).
//! - Message is not Send (shares an Arc<Vec<u32>> tied to the socket), so recv() and
//!   verdict() cannot be split across threads.
//! - Solution: spawn_blocking owns the entire loop. The async handler is called via
//!   tokio::runtime::Handle::block_on() from the blocking thread.
//! - The queue runs in non-blocking mode (set_nonblocking) to poll the cancel token.

use async_trait::async_trait;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use syswall_domain::errors::DomainError;
use syswall_domain::ports::interception::{
    PacketDecisionHandler, PacketInterceptor, PacketVerdict,
};

use super::parser::parse_packet;

/// Politique appliquée quand le canal de paquets est saturé.
/// Policy applied when the packet channel is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// Bloquer (DROP) le paquet si la file est pleine. / Block (DROP) if queue is full.
    Block,
    /// Accepter le paquet si la file est pleine. / Accept if queue is full.
    Accept,
}

pub struct NfqueueInterceptor {
    queue_num: u16,
    max_queued: u32,
    overflow: OverflowPolicy,
}

impl NfqueueInterceptor {
    pub fn new(queue_num: u16, max_queued: u32, overflow: OverflowPolicy) -> Self {
        Self { queue_num, max_queued, overflow }
    }
}

#[async_trait]
impl PacketInterceptor for NfqueueInterceptor {
    /// Lance la boucle d'interception jusqu'à l'annulation du token.
    /// Runs the interception loop until the cancel token fires.
    async fn run(
        &self,
        handler: Arc<dyn PacketDecisionHandler>,
        cancel: CancellationToken,
    ) -> Result<(), DomainError> {
        info!(
            target: "nfqueue",
            queue_num = self.queue_num,
            max_queued = self.max_queued,
            "démarrage de l'intercepteur NFQUEUE"
        );

        let queue_num = self.queue_num;
        let max_queued = self.max_queued;
        let overflow = self.overflow;

        // Lancer la boucle bloquante dans un thread dédié.
        // Run the blocking loop in a dedicated thread.
        let result = tokio::task::spawn_blocking(move || {
            run_blocking_loop(queue_num, max_queued, overflow, handler, cancel)
        })
        .await;

        match result {
            Ok(inner) => inner,
            Err(e) => Err(DomainError::Infrastructure(format!(
                "nfqueue task paniqué: {e}"
            ))),
        }
    }
}

/// Boucle bloquante principale : ouvre la queue, traite les paquets, envoie les verdicts.
/// Main blocking loop: opens the queue, processes packets, sends verdicts.
///
/// Appelée depuis spawn_blocking — s'exécute dans un thread OS dédié.
/// Called from spawn_blocking — executes in a dedicated OS thread.
fn run_blocking_loop(
    queue_num: u16,
    max_queued: u32,
    overflow: OverflowPolicy,
    handler: Arc<dyn PacketDecisionHandler>,
    cancel: CancellationToken,
) -> Result<(), DomainError> {
    // Récupérer le handle du runtime Tokio pour appeler le handler async.
    // Retrieve the Tokio runtime handle to call the async handler.
    let rt = tokio::runtime::Handle::current();

    let mut queue =
        nfq::Queue::open().map_err(|e| DomainError::Infrastructure(format!("nfq::open: {e}")))?;

    queue
        .bind(queue_num)
        .map_err(|e| DomainError::Infrastructure(format!("nfq::bind({}): {e}", queue_num)))?;

    queue
        .set_queue_max_len(queue_num, max_queued)
        .map_err(|e| DomainError::Infrastructure(format!("nfq::set_queue_max_len: {e}")))?;

    // Mode non-bloquant pour permettre la vérification du cancel token.
    // Non-blocking mode to allow cancel token polling.
    queue.set_nonblocking(true);

    info!(
        target: "nfqueue",
        queue_num,
        max_queued,
        "queue NFQUEUE ouverte et liée"
    );

    loop {
        if cancel.is_cancelled() {
            info!(target: "nfqueue", queue_num, "token annulé, fermeture de la queue");
            break;
        }

        let mut msg = match queue.recv() {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Pas de paquet disponible — céder le CPU brièvement avant de réessayer.
                // No packet available — yield briefly before retrying.
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
            Err(e) if e.raw_os_error() == Some(4) => {
                // EINTR (signal reçu) : réessayer.
                // EINTR (signal received): retry.
                continue;
            }
            Err(e) => {
                error!(target: "nfqueue", error = %e, "erreur recv nfq");
                return Err(DomainError::Infrastructure(format!("nfq::recv: {e}")));
            }
        };

        let payload = msg.get_payload().to_vec();

        // Parser le paquet brut en Connection domain.
        // Parse raw packet into a domain Connection.
        let connection = match parse_packet(&payload) {
            Ok(c) => c,
            Err(e) => {
                // Paquet non parseable (ICMP, ARP, …) : appliquer la politique d'overflow
                // ou accepter par défaut.
                // Unparseable packet (ICMP, ARP, …): apply overflow policy or accept by default.
                warn!(
                    target: "nfqueue",
                    error = %e,
                    "paquet non parseable — politique overflow appliquée"
                );
                let verdict = overflow_to_nfq_verdict(overflow);
                msg.set_verdict(verdict);
                if let Err(ve) = queue.verdict(msg) {
                    error!(target: "nfqueue", error = %ve, "erreur verdict nfq");
                }
                continue;
            }
        };

        // Appeler le handler async depuis le contexte bloquant.
        // Call async handler from blocking context.
        let verdict = match rt.block_on(handler.decide(&connection)) {
            Ok(PacketVerdict::Accept) => nfq::Verdict::Accept,
            Ok(PacketVerdict::Drop) => nfq::Verdict::Drop,
            Err(e) => {
                warn!(
                    target: "nfqueue",
                    error = %e,
                    "handler erreur — paquet accepté par défaut"
                );
                nfq::Verdict::Accept
            }
        };

        msg.set_verdict(verdict);
        if let Err(e) = queue.verdict(msg) {
            error!(target: "nfqueue", error = %e, "erreur verdict nfq");
            return Err(DomainError::Infrastructure(format!("nfq::verdict: {e}")));
        }
    }

    info!(target: "nfqueue", queue_num, "boucle d'interception terminée");
    Ok(())
}

/// Convertit la politique d'overflow en verdict nfq.
/// Converts overflow policy to an nfq verdict.
#[inline]
fn overflow_to_nfq_verdict(policy: OverflowPolicy) -> nfq::Verdict {
    match policy {
        OverflowPolicy::Accept => nfq::Verdict::Accept,
        OverflowPolicy::Block => nfq::Verdict::Drop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_policy_equality() {
        assert_eq!(OverflowPolicy::Block, OverflowPolicy::Block);
        assert_ne!(OverflowPolicy::Block, OverflowPolicy::Accept);
    }

    #[test]
    fn interceptor_constructs_cleanly() {
        let i = NfqueueInterceptor::new(0, 1024, OverflowPolicy::Block);
        assert_eq!(i.queue_num, 0);
        assert_eq!(i.max_queued, 1024);
        assert_eq!(i.overflow, OverflowPolicy::Block);
    }

    #[test]
    fn overflow_to_nfq_verdict_mapping() {
        assert_eq!(overflow_to_nfq_verdict(OverflowPolicy::Accept), nfq::Verdict::Accept);
        assert_eq!(overflow_to_nfq_verdict(OverflowPolicy::Block), nfq::Verdict::Drop);
    }
}
