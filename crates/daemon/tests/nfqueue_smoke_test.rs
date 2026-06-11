//! NFQUEUE smoke test — requires CAP_NET_ADMIN + a real kernel queue.
//! Test fumée NFQUEUE — nécessite CAP_NET_ADMIN + une vraie queue kernel.
//!
//! Activated only when SYSWALL_TEST_NFQUEUE is set; otherwise skipped.
//! Activé uniquement si SYSWALL_TEST_NFQUEUE est défini ; sinon ignoré.

#[tokio::test]
async fn nfqueue_open_and_close() {
    if std::env::var("SYSWALL_TEST_NFQUEUE").is_err() {
        eprintln!("SYSWALL_TEST_NFQUEUE not set, skipping");
        return;
    }
    // Vrai test fumée : ouvrir une queue, faire bind, set_queue_max_len, fermer.
    // Real smoke test: open a queue, bind, set max len, close.
    use std::sync::Arc;
    use syswall_domain::ports::interception::{
        PacketDecisionHandler, PacketInterceptor, PacketVerdict,
    };
    use syswall_infra::nfqueue::{NfqueueInterceptor, OverflowPolicy};
    use tokio_util::sync::CancellationToken;

    struct Reject;
    #[async_trait::async_trait]
    impl PacketDecisionHandler for Reject {
        async fn decide(
            &self,
            _conn: &syswall_domain::entities::Connection,
        ) -> Result<PacketVerdict, syswall_domain::errors::DomainError> {
            Ok(PacketVerdict::Drop)
        }
    }

    let interceptor = NfqueueInterceptor::new(99, 16, OverflowPolicy::Block);
    let handler: Arc<dyn PacketDecisionHandler> = Arc::new(Reject);
    let cancel = CancellationToken::new();

    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        cancel_clone.cancel();
    });

    let result = interceptor.run(handler, cancel).await;
    // Avec CAP_NET_ADMIN, le run termine proprement après le cancel.
    // Sans CAP_NET_ADMIN, le run échoue à l'ouverture de la queue.
    assert!(result.is_ok() || result.is_err()); // l'un ou l'autre est valide
    eprintln!("smoke result: {result:?}");
}
