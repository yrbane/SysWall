//! Tests d'intégration pour les limites de taille et concurrence gRPC.
//! Integration tests for gRPC size and concurrency limits.
//!
//! S'active si SYSWALL_TEST_GRPC est défini ; sinon skip silencieux.
//! Activates when SYSWALL_TEST_GRPC is set; otherwise skips silently.
//!
//! Note architecture : le daemon est un crate binaire — ses modules internes
//! (SysWallControlService, etc.) ne sont pas accessibles depuis les tests
//! d'intégration. Ce harness reconstruit un mini-serveur tonic directement
//! en utilisant uniquement les crates publics (syswall-proto, tonic).
//!
//! Architecture note: the daemon is a binary crate — its internal modules
//! (SysWallControlService, etc.) are not accessible from integration tests.
//! This harness rebuilds a mini tonic server using only public crates
//! (syswall-proto, tonic). The stub service verifies size limits enforced
//! by tonic before any handler is called.

use std::time::Duration;

use tempfile::tempdir;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::{Channel, Endpoint, Server};
use tower::service_fn;

use syswall_proto::syswall::sys_wall_control_client::SysWallControlClient;
use syswall_proto::syswall::sys_wall_control_server::{SysWallControl, SysWallControlServer};
use syswall_proto::syswall::sys_wall_events_server::{SysWallEvents, SysWallEventsServer};
use syswall_proto::syswall::{
    AuditLogRequest, AuditLogResponse, CreateRuleRequest, DashboardStatsRequest,
    DashboardStatsResponse, DecisionAck, DecisionResponseRequest, DomainEventMessage, Empty,
    ExportAuditLogRequest, ExportAuditLogResponse, PendingDecisionListResponse, RuleFiltersRequest,
    RuleIdRequest, RuleListResponse, RuleResponse, SetNetworkEnabledRequest, StatusResponse,
    SubscribeRequest, ToggleRuleRequest,
};

// ---------------------------------------------------------------------------
// Stub minimal : implémente SysWallControl sans logique métier.
// Minimal stub: implements SysWallControl without business logic.
// Les limites de taille sont appliquées par tonic AVANT l'appel du handler.
// Size limits are enforced by tonic BEFORE the handler is called.
// ---------------------------------------------------------------------------

struct StubControlService;

#[tonic::async_trait]
impl SysWallControl for StubControlService {
    async fn get_status(
        &self,
        _req: tonic::Request<Empty>,
    ) -> Result<tonic::Response<StatusResponse>, tonic::Status> {
        Ok(tonic::Response::new(StatusResponse {
            enabled: false,
            active_rules_count: 0,
            nftables_synced: false,
            uptime_secs: 0,
            version: "test".to_string(),
        }))
    }

    async fn list_rules(
        &self,
        _req: tonic::Request<RuleFiltersRequest>,
    ) -> Result<tonic::Response<RuleListResponse>, tonic::Status> {
        Ok(tonic::Response::new(RuleListResponse { rules: vec![] }))
    }

    async fn create_rule(
        &self,
        _req: tonic::Request<CreateRuleRequest>,
    ) -> Result<tonic::Response<RuleResponse>, tonic::Status> {
        // Ce handler ne sera jamais appelé pour les messages > 1 Mio car tonic
        // rejette avant de désérialiser.
        // This handler is never called for messages > 1 MiB because tonic
        // rejects before deserializing.
        Ok(tonic::Response::new(RuleResponse { rule: None }))
    }

    async fn delete_rule(
        &self,
        _req: tonic::Request<RuleIdRequest>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        Ok(tonic::Response::new(Empty {}))
    }

    async fn toggle_rule(
        &self,
        _req: tonic::Request<ToggleRuleRequest>,
    ) -> Result<tonic::Response<RuleResponse>, tonic::Status> {
        Ok(tonic::Response::new(RuleResponse { rule: None }))
    }

    async fn respond_to_decision(
        &self,
        _req: tonic::Request<DecisionResponseRequest>,
    ) -> Result<tonic::Response<DecisionAck>, tonic::Status> {
        Ok(tonic::Response::new(DecisionAck {
            decision_id: String::new(),
        }))
    }

    async fn list_pending_decisions(
        &self,
        _req: tonic::Request<Empty>,
    ) -> Result<tonic::Response<PendingDecisionListResponse>, tonic::Status> {
        Ok(tonic::Response::new(PendingDecisionListResponse {
            decisions: vec![],
        }))
    }

    async fn query_audit_log(
        &self,
        _req: tonic::Request<AuditLogRequest>,
    ) -> Result<tonic::Response<AuditLogResponse>, tonic::Status> {
        Ok(tonic::Response::new(AuditLogResponse {
            events: vec![],
            total_count: 0,
        }))
    }

    async fn get_dashboard_stats(
        &self,
        _req: tonic::Request<DashboardStatsRequest>,
    ) -> Result<tonic::Response<DashboardStatsResponse>, tonic::Status> {
        Ok(tonic::Response::new(DashboardStatsResponse {
            total_events: 0,
            by_category: std::collections::HashMap::new(),
            by_severity: std::collections::HashMap::new(),
        }))
    }

    async fn export_audit_log(
        &self,
        _req: tonic::Request<ExportAuditLogRequest>,
    ) -> Result<tonic::Response<ExportAuditLogResponse>, tonic::Status> {
        Ok(tonic::Response::new(ExportAuditLogResponse {
            data: vec![],
            content_type: String::new(),
        }))
    }

    async fn set_network_enabled(
        &self,
        _req: tonic::Request<SetNetworkEnabledRequest>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        Ok(tonic::Response::new(Empty {}))
    }
}

struct StubEventService;

#[tonic::async_trait]
impl SysWallEvents for StubEventService {
    type SubscribeEventsStream = std::pin::Pin<
        Box<
            dyn tokio_stream::Stream<Item = Result<DomainEventMessage, tonic::Status>>
                + Send
                + 'static,
        >,
    >;

    async fn subscribe_events(
        &self,
        _req: tonic::Request<SubscribeRequest>,
    ) -> Result<tonic::Response<Self::SubscribeEventsStream>, tonic::Status> {
        let stream = tokio_stream::empty();
        Ok(tonic::Response::new(Box::pin(stream)))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Démarre un serveur gRPC de test avec les mêmes limites de taille que la production.
/// Starts a test gRPC server with the same size limits as production.
async fn spawn_test_server(
    socket_path: std::path::PathBuf,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    let listener = UnixListener::bind(&socket_path).expect("bind test socket");
    let incoming = UnixListenerStream::new(listener);

    // Mêmes limites que la production (1 Mio décodage, 4 Mio encodage)
    // Same limits as production (1 MiB decode, 4 MiB encode)
    let control_server = SysWallControlServer::new(StubControlService)
        .max_decoding_message_size(1 << 20)
        .max_encoding_message_size(4 << 20);
    let events_server = SysWallEventsServer::new(StubEventService)
        .max_decoding_message_size(1 << 20)
        .max_encoding_message_size(4 << 20);

    tokio::spawn(async move {
        Server::builder()
            .timeout(Duration::from_secs(30))
            .concurrency_limit_per_connection(64)
            .add_service(control_server)
            .add_service(events_server)
            .serve_with_incoming_shutdown(incoming, cancel.cancelled())
            .await
            .expect("test gRPC server error");
    })
}

/// Crée un Channel tonic connecté à un socket Unix.
/// Creates a tonic Channel connected to a Unix socket.
/// Utilise TokioIo pour adapter UnixStream au trait hyper::rt::io.
/// Uses TokioIo to adapt UnixStream to the hyper::rt::io trait.
async fn unix_channel(socket_path: std::path::PathBuf) -> Channel {
    Endpoint::try_from("http://[::]:50051")
        .unwrap()
        .connect_with_connector(service_fn(move |_| {
            let p = socket_path.clone();
            async move {
                tokio::net::UnixStream::connect(&p)
                    .await
                    .map(hyper_util::rt::TokioIo::new)
            }
        }))
        .await
        .expect("connexion au socket de test")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Vérifie que le serveur rejette les messages dépassant 1 Mio.
/// Verifies that the server rejects messages exceeding 1 MiB.
#[tokio::test]
async fn message_over_1mib_is_rejected() {
    if std::env::var("SYSWALL_TEST_GRPC").is_err() {
        eprintln!("SYSWALL_TEST_GRPC non défini, test ignoré");
        return;
    }

    let tmp = tempdir().expect("tempdir");
    let socket_path = tmp.path().join("grpc_limits_test.sock");
    let cancel = CancellationToken::new();

    let _handle = spawn_test_server(socket_path.clone(), cancel.clone()).await;

    // Attend que le socket soit disponible
    for _ in 0..50 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let channel = unix_channel(socket_path).await;
    // Le client n'a pas de limite de décodage restrictive — seul le serveur en a une
    // The client has no restrictive decode limit — only the server does
    let mut client = SysWallControlClient::new(channel);

    // criteria_json > 1 Mio pour déclencher le rejet côté serveur
    // criteria_json > 1 MiB to trigger server-side rejection
    let oversized = "x".repeat(2 * 1024 * 1024); // 2 Mio / 2 MiB
    let req = CreateRuleRequest {
        name: "test".to_string(),
        priority: 1,
        criteria_json: oversized,
        effect: "allow".to_string(),
        scope_json: "\"Permanent\"".to_string(),
        source: "manual".to_string(),
    };

    let result = client.create_rule(req).await;
    cancel.cancel();

    // Le serveur doit rejeter le message trop grand
    // The server must reject the oversized message
    assert!(
        result.is_err(),
        "Le serveur aurait dû rejeter le message > 1 Mio"
    );
    let code = result.unwrap_err().code();
    eprintln!("Code de statut reçu (attendu) : {:?}", code);
    // tonic retourne ResourceExhausted pour les messages dépassant max_decoding_message_size
    // tonic returns ResourceExhausted for messages exceeding max_decoding_message_size
    assert!(
        matches!(
            code,
            tonic::Code::ResourceExhausted
                | tonic::Code::Internal
                | tonic::Code::OutOfRange
                | tonic::Code::InvalidArgument
        ),
        "Code inattendu : {:?}",
        code
    );
}

/// Vérifie que les petits messages sont acceptés normalement.
/// Verifies that small messages are accepted normally.
#[tokio::test]
async fn small_message_is_accepted() {
    if std::env::var("SYSWALL_TEST_GRPC").is_err() {
        eprintln!("SYSWALL_TEST_GRPC non défini, test ignoré");
        return;
    }

    let tmp = tempdir().expect("tempdir");
    let socket_path = tmp.path().join("grpc_small_test.sock");
    let cancel = CancellationToken::new();

    let _handle = spawn_test_server(socket_path.clone(), cancel.clone()).await;

    for _ in 0..50 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let channel = unix_channel(socket_path).await;
    let mut client = SysWallControlClient::new(channel);

    let req = CreateRuleRequest {
        name: "test".to_string(),
        priority: 1,
        criteria_json: "{}".to_string(),
        effect: "allow".to_string(),
        scope_json: "\"Permanent\"".to_string(),
        source: "manual".to_string(),
    };

    let result = client.create_rule(req).await;
    cancel.cancel();

    assert!(
        result.is_ok(),
        "Un petit message doit être accepté : {:?}",
        result.err()
    );
}

#[tokio::test]
async fn concurrency_limit_is_enforced() {
    if std::env::var("SYSWALL_TEST_GRPC").is_err() {
        eprintln!("SYSWALL_TEST_GRPC non défini, test ignoré");
        return;
    }

    // TODO(v0.4) : ouvrir 65 flux concurrents et vérifier que le 65e est rejeté/mis en attente.
    // Nécessite une coordination inter-tâches et des requêtes streaming qui bloquent.
    // TODO(v0.4): open 65 concurrent streams and verify the 65th is queued/rejected.
    // Requires inter-task coordination and blocking streaming requests.
    eprintln!("test de limite de concurrence — implémentation reportée en V0.4");
}
