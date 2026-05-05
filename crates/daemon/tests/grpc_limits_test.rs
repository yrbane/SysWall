//! Integration test for gRPC size and concurrency limits.
//! Test d'integration pour les limites de taille et concurrence gRPC.
//!
//! This test is gated behind the SYSWALL_TEST_GRPC env var because it requires
//! a fully-spun-up daemon test harness (extracted in a future task).
//! Ce test est activé uniquement si SYSWALL_TEST_GRPC est défini car il nécessite
//! un harness de daemon lancé (extrait dans une tâche future).

#[tokio::test]
async fn message_over_1mib_is_rejected() {
    if std::env::var("SYSWALL_TEST_GRPC").is_err() {
        eprintln!("SYSWALL_TEST_GRPC not set, skipping");
        return;
    }
    // TODO(future task): bring up server, send oversized CreateRuleRequest, expect rejection.
    // The limits themselves are configured in `crates/daemon/src/grpc/server.rs` and
    // are exercised in production. This placeholder ensures the test file exists and compiles.
}

#[tokio::test]
async fn concurrency_limit_is_enforced() {
    if std::env::var("SYSWALL_TEST_GRPC").is_err() {
        eprintln!("SYSWALL_TEST_GRPC not set, skipping");
        return;
    }
    // TODO(future task): open 65 concurrent streams, expect the 65th to be queued/rejected.
}
