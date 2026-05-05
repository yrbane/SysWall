mod verdict_broadcast;
pub use verdict_broadcast::VerdictBroadcasts;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use syswall_domain::entities::{
    AuditEvent, Connection, ConnectionSnapshot, ConnectionVerdict, Decision, DecisionId,
    EventCategory, PendingDecision, PendingDecisionId, PendingDecisionStatus, Severity,
};
use syswall_domain::errors::DomainError;
use syswall_domain::events::{DefaultPolicy, DomainEvent};
use syswall_domain::ports::{
    AuditRepository, DecisionRepository, EventBus, PendingDecisionRepository, RuleRepository,
    UserNotifier,
};
use syswall_domain::ports::interception::PacketVerdict;
use syswall_domain::services::PolicyEngine;

use crate::commands::RespondToDecisionCommand;

/// Configuration for the learning subsystem.
/// Configuration du sous-système d'apprentissage.
pub struct LearningConfig {
    /// Timeout in seconds before a pending decision expires.
    /// Délai en secondes avant qu'une décision en attente expire.
    pub prompt_timeout_secs: u64,

    /// Maximum number of pending decisions allowed in the queue.
    /// Nombre maximal de décisions en attente autorisées dans la file.
    pub max_pending_decisions: usize,
}

/// Service for managing the auto-learning flow (async, non-blocking).
/// Service de gestion du flux d'auto-apprentissage (asynchrone, non bloquant).
pub struct LearningService {
    pending_repo: Arc<dyn PendingDecisionRepository>,
    decision_repo: Arc<dyn DecisionRepository>,
    notifier: Arc<dyn UserNotifier>,
    event_bus: Arc<dyn EventBus>,
    audit_repo: Arc<dyn AuditRepository>,
    rule_repo: Arc<dyn RuleRepository>,
    default_policy: DefaultPolicy,
    verdict_broadcasts: Arc<VerdictBroadcasts>,
    config: LearningConfig,
}

impl LearningService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pending_repo: Arc<dyn PendingDecisionRepository>,
        decision_repo: Arc<dyn DecisionRepository>,
        notifier: Arc<dyn UserNotifier>,
        event_bus: Arc<dyn EventBus>,
        audit_repo: Arc<dyn AuditRepository>,
        rule_repo: Arc<dyn RuleRepository>,
        default_policy: DefaultPolicy,
        verdict_broadcasts: Arc<VerdictBroadcasts>,
        config: LearningConfig,
    ) -> Self {
        Self {
            pending_repo,
            decision_repo,
            notifier,
            event_bus,
            audit_repo,
            rule_repo,
            default_policy,
            verdict_broadcasts,
            config,
        }
    }

    /// Compute deduplication key from a connection snapshot.
    /// Calcule la clé de déduplication à partir d'un instantané de connexion.
    pub fn dedup_key(snapshot: &ConnectionSnapshot) -> String {
        format!(
            "{}:{}:{}:{}",
            snapshot.process_name.as_deref().unwrap_or("unknown"),
            snapshot.destination.ip,
            snapshot.destination.port,
            snapshot.protocol,
        )
    }

    /// Handle a connection that matched no rule and default policy is Ask.
    /// Creates a PendingDecision and notifies the UI. Does NOT block.
    ///
    /// Gère une connexion sans règle correspondante et politique par défaut Ask.
    /// Crée une PendingDecision et notifie l'interface. Ne bloque PAS.
    pub async fn handle_unknown_connection(
        &self,
        snapshot: ConnectionSnapshot,
    ) -> Result<(), DomainError> {
        let key = Self::dedup_key(&snapshot);

        // Debounce: skip if same key already pending
        if self.pending_repo.find_by_dedup_key(&key).await?.is_some() {
            return Ok(());
        }

        // Check queue capacity
        let pending_count = self.pending_repo.list_pending().await?.len();
        if pending_count >= self.config.max_pending_decisions {
            tracing::warn!("Pending decision queue full ({}), dropping", pending_count);
            return Ok(());
        }

        let pending = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: snapshot,
            requested_at: Utc::now(),
            expires_at: Utc::now() + Duration::seconds(self.config.prompt_timeout_secs as i64),
            deduplication_key: key,
            status: PendingDecisionStatus::Pending,
        };

        self.pending_repo.create(&pending).await?;
        let _ = self
            .event_bus
            .publish(DomainEvent::DecisionRequired(pending.clone()))
            .await;
        self.notifier.notify_decision_required(&pending).await?;

        Ok(())
    }

    /// Resolve a pending decision when the user responds.
    /// Résout une décision en attente lorsque l'utilisateur répond.
    pub async fn resolve_decision(
        &self,
        cmd: RespondToDecisionCommand,
    ) -> Result<Decision, DomainError> {
        let pending_list = self.pending_repo.list_pending().await?;
        let pending = pending_list
            .iter()
            .find(|p| p.id == cmd.pending_decision_id)
            .ok_or_else(|| {
                DomainError::NotFound(format!(
                    "PendingDecision {:?}",
                    cmd.pending_decision_id
                ))
            })?;

        if pending.status != PendingDecisionStatus::Pending {
            return Err(DomainError::Validation(
                "Decision is no longer pending".to_string(),
            ));
        }

        let decision = Decision {
            id: DecisionId::new(),
            pending_decision_id: cmd.pending_decision_id,
            connection_snapshot: pending.connection_snapshot.clone(),
            action: cmd.action,
            granularity: cmd.granularity,
            decided_at: Utc::now(),
            generated_rule: None,
        };

        self.decision_repo.save(&decision).await?;
        self.pending_repo.resolve(&cmd.pending_decision_id).await?;

        // Publier le verdict aux abonnés en attente.
        // Publish the verdict to all waiting subscribers.
        use syswall_domain::entities::DecisionAction;
        let verdict = match decision.action {
            DecisionAction::AllowOnce | DecisionAction::AlwaysAllow => PacketVerdict::Accept,
            DecisionAction::BlockOnce | DecisionAction::AlwaysBlock | DecisionAction::Ignore => {
                PacketVerdict::Drop
            }
            DecisionAction::CreateRule => PacketVerdict::Accept,
        };
        self.verdict_broadcasts
            .publish_and_remove(cmd.pending_decision_id, verdict)
            .await;

        let _ = self
            .event_bus
            .publish(DomainEvent::DecisionResolved(decision.clone()))
            .await;

        Ok(decision)
    }

    /// Expire overdue pending decisions.
    /// Expire les décisions en attente dépassées.
    pub async fn expire_overdue(&self) -> Result<Vec<PendingDecision>, DomainError> {
        let expired = self.pending_repo.expire_overdue().await?;
        for pd in &expired {
            let _ = self
                .event_bus
                .publish(DomainEvent::DecisionExpired(pd.id))
                .await;
        }
        Ok(expired)
    }

    /// Get all currently pending decisions.
    /// Récupère toutes les décisions actuellement en attente.
    pub async fn get_pending_decisions(&self) -> Result<Vec<PendingDecision>, DomainError> {
        self.pending_repo.list_pending().await
    }

    /// Attendre le verdict du broadcast pour une PendingDecision donnée.
    /// Wait for the broadcast verdict for a given PendingDecision.
    async fn wait_for_verdict(
        &self,
        id: PendingDecisionId,
    ) -> Result<PacketVerdict, DomainError> {
        use std::time::Duration as StdDuration;

        let mut rx = self.verdict_broadcasts.subscribe(id).await;
        match tokio::time::timeout(StdDuration::from_secs(28), rx.recv()).await {
            Ok(Ok(verdict)) => Ok(verdict),
            Ok(Err(_)) => Ok(PacketVerdict::Drop),
            Err(_) => {
                // Timeout : audit + Drop (le kernel droppera de toute façon à ~30s).
                // Timeout: audit + Drop (kernel will drop on its own at ~30s anyway).
                let event = AuditEvent::new(
                    Severity::Warning,
                    EventCategory::Decision,
                    "decision timeout: kernel will drop packet",
                )
                .with_metadata("decision_id", id.as_uuid().to_string());
                let _ = self.audit_repo.append(&event).await;
                Ok(PacketVerdict::Drop)
            }
        }
    }

    /// Gérer une connexion en attente de décision (debounce + wait).
    /// Handle a connection pending a user decision (debounce + wait).
    async fn pending_verdict_for(
        &self,
        conn: &Connection,
    ) -> Result<PacketVerdict, DomainError> {
        let snapshot = conn.snapshot();
        let dedup_key = Self::dedup_key(&snapshot);

        // Debounce : réutilise une PendingDecision encore active.
        // Debounce: re-use an existing active PendingDecision.
        if let Some(existing) = self.pending_repo.find_by_dedup_key(&dedup_key).await?
            && existing.is_pending()
        {
            return self.wait_for_verdict(existing.id).await;
        }

        // Vérifie la capacité de la file.
        // Check queue capacity.
        let pending_count = self.pending_repo.list_pending().await?.len();
        if pending_count >= self.config.max_pending_decisions {
            tracing::warn!("Pending decision queue full ({}), dropping packet", pending_count);
            return Ok(PacketVerdict::Drop);
        }

        // Création d'une nouvelle PendingDecision.
        // Create a new PendingDecision.
        let pending = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: snapshot,
            requested_at: Utc::now(),
            expires_at: Utc::now() + Duration::seconds(self.config.prompt_timeout_secs as i64),
            deduplication_key: dedup_key,
            status: PendingDecisionStatus::Pending,
        };

        self.pending_repo.create(&pending).await?;
        let _ = self
            .event_bus
            .publish(DomainEvent::DecisionRequired(pending.clone()))
            .await;
        self.notifier.notify_decision_required(&pending).await?;

        self.wait_for_verdict(pending.id).await
    }
}

#[async_trait]
impl syswall_domain::ports::interception::PacketDecisionHandler for LearningService {
    async fn decide(
        &self,
        connection: &Connection,
    ) -> Result<PacketVerdict, DomainError> {
        let rules = self.rule_repo.list_enabled_ordered().await?;
        let evaluation = PolicyEngine::evaluate(connection, &rules, self.default_policy);
        match evaluation.verdict {
            ConnectionVerdict::Allowed => Ok(PacketVerdict::Accept),
            ConnectionVerdict::Blocked | ConnectionVerdict::Ignored | ConnectionVerdict::Unknown => {
                Ok(PacketVerdict::Drop)
            }
            ConnectionVerdict::PendingDecision => self.pending_verdict_for(connection).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::*;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn test_snapshot() -> ConnectionSnapshot {
        ConnectionSnapshot {
            protocol: Protocol::Tcp,
            source: SocketAddress::new("10.0.0.1".parse().unwrap(), Port::new(5000).unwrap()),
            destination: SocketAddress::new("8.8.8.8".parse().unwrap(), Port::new(443).unwrap()),
            direction: Direction::Outbound,
            process_name: Some("curl".to_string()),
            process_path: None,
            user: Some("seb".to_string()),
            hostname: None,
        }
    }

    fn setup() -> (
        LearningService,
        Arc<FakePendingDecisionRepository>,
        Arc<FakeUserNotifier>,
    ) {
        let pending_repo = Arc::new(FakePendingDecisionRepository::new());
        let decision_repo = Arc::new(FakeDecisionRepository::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let notifier = Arc::new(FakeUserNotifier::new());
        let audit_repo = Arc::new(FakeAuditRepository::new());
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let verdict_broadcasts = Arc::new(VerdictBroadcasts::new());

        let config = LearningConfig {
            prompt_timeout_secs: 60,
            max_pending_decisions: 50,
        };

        let service = LearningService::new(
            pending_repo.clone(),
            decision_repo,
            notifier.clone(),
            event_bus,
            audit_repo,
            rule_repo,
            DefaultPolicy::Ask,
            verdict_broadcasts,
            config,
        );

        (service, pending_repo, notifier)
    }

    #[tokio::test]
    async fn handle_unknown_creates_pending_decision() {
        let (service, pending_repo, notifier) = setup();
        service
            .handle_unknown_connection(test_snapshot())
            .await
            .unwrap();

        let pending = pending_repo.list_pending().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].status, PendingDecisionStatus::Pending);

        // Verify notifier was called
        let notifs = notifier.decision_notifications.lock().unwrap();
        assert_eq!(notifs.len(), 1);
    }

    #[tokio::test]
    async fn debounce_same_connection() {
        let (service, pending_repo, _) = setup();

        // First call creates pending
        service
            .handle_unknown_connection(test_snapshot())
            .await
            .unwrap();

        // Second call with same snapshot is deduplicated
        service
            .handle_unknown_connection(test_snapshot())
            .await
            .unwrap();

        let pending = pending_repo.list_pending().await.unwrap();
        assert_eq!(pending.len(), 1); // Only one, not two
    }

    #[tokio::test]
    async fn resolve_decision_marks_resolved() {
        let (service, pending_repo, _) = setup();
        service
            .handle_unknown_connection(test_snapshot())
            .await
            .unwrap();

        let pending = pending_repo.list_pending().await.unwrap();
        let pending_id = pending[0].id;

        let cmd = RespondToDecisionCommand {
            pending_decision_id: pending_id,
            action: DecisionAction::AllowOnce,
            granularity: DecisionGranularity::AppOnly,
        };

        let decision = service.resolve_decision(cmd).await.unwrap();
        assert_eq!(decision.action, DecisionAction::AllowOnce);

        // Verify pending is now resolved (list_pending returns only Pending)
        let remaining = pending_repo.list_pending().await.unwrap();
        assert_eq!(remaining.len(), 0);
    }

    #[tokio::test]
    async fn expire_overdue_marks_expired() {
        let (service, pending_repo, _) = setup();

        // Manually create an already-expired pending decision
        let expired_pending = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            requested_at: Utc::now() - Duration::minutes(10),
            expires_at: Utc::now() - Duration::minutes(1),
            deduplication_key: "test:expired".to_string(),
            status: PendingDecisionStatus::Pending,
        };
        pending_repo.create(&expired_pending).await.unwrap();

        let expired = service.expire_overdue().await.unwrap();
        assert_eq!(expired.len(), 1);

        let pending = pending_repo.list_pending().await.unwrap();
        assert_eq!(pending.len(), 0);
    }
}

#[cfg(test)]
mod handler_tests {
    use super::*;
    use crate::fakes::*;
    use std::sync::Arc;
    use std::time::Duration;
    use syswall_domain::entities::{
        Connection, ConnectionId, ConnectionState, ConnectionVerdict as ConnVerdict,
        DecisionAction, DecisionGranularity, ProcessInfo, RuleCriteria,
        RuleEffect, RuleId, RuleScope, RuleSource, SystemUser,
    };
    use syswall_domain::events::DefaultPolicy;
    use syswall_domain::ports::interception::{PacketDecisionHandler, PacketVerdict};
    use syswall_domain::value_objects::{Direction, Port, Protocol, RulePriority, SocketAddress};

    use crate::commands::RespondToDecisionCommand;

    fn dummy_connection() -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol: Protocol::Tcp,
            source: SocketAddress::new(
                "10.0.0.1".parse().unwrap(),
                Port::new(5000).unwrap(),
            ),
            destination: SocketAddress::new(
                "8.8.8.8".parse().unwrap(),
                Port::new(443).unwrap(),
            ),
            direction: Direction::Outbound,
            state: ConnectionState::New,
            process: Some(ProcessInfo {
                pid: 1234,
                name: "curl".to_string(),
                path: None,
                cmdline: None,
                icon: None,
            }),
            user: Some(SystemUser {
                uid: 1000,
                name: "seb".to_string(),
            }),
            bytes_sent: 0,
            bytes_received: 0,
            started_at: chrono::Utc::now(),
            verdict: ConnVerdict::Unknown,
            matched_rule: None,
            remote_hostname: None,
        }
    }

    fn make_rule(effect: RuleEffect) -> syswall_domain::entities::Rule {
        syswall_domain::entities::Rule {
            id: RuleId::new(),
            name: "test rule".to_string(),
            priority: RulePriority::new(10),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect,
            scope: RuleScope::Permanent,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            source: RuleSource::Manual,
        }
    }

    fn make_service(
        rules: Vec<syswall_domain::entities::Rule>,
        default_policy: DefaultPolicy,
    ) -> (
        Arc<LearningService>,
        Arc<FakePendingDecisionRepository>,
        Arc<FakeAuditRepository>,
    ) {
        let pending_repo = Arc::new(FakePendingDecisionRepository::new());
        let decision_repo = Arc::new(FakeDecisionRepository::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let notifier = Arc::new(FakeUserNotifier::new());
        let audit_repo = Arc::new(FakeAuditRepository::new());
        let rule_repo = Arc::new(FakeRuleRepository::with_rules(rules));
        let verdict_broadcasts = Arc::new(VerdictBroadcasts::new());

        let config = LearningConfig {
            prompt_timeout_secs: 60,
            max_pending_decisions: 50,
        };

        let service = Arc::new(LearningService::new(
            pending_repo.clone(),
            decision_repo,
            notifier,
            event_bus,
            audit_repo.clone(),
            rule_repo,
            default_policy,
            verdict_broadcasts,
            config,
        ));

        (service, pending_repo, audit_repo)
    }

    #[tokio::test]
    async fn decide_existing_allow_rule_returns_accept() {
        let rule = make_rule(RuleEffect::Allow);
        let (service, pending_repo, _audit) = make_service(vec![rule], DefaultPolicy::Block);
        let conn = dummy_connection();
        let verdict = service.decide(&conn).await.unwrap();
        assert_eq!(verdict, PacketVerdict::Accept);
        assert_eq!(pending_repo.count_pending().await, 0);
    }

    #[tokio::test]
    async fn decide_no_rule_default_block_returns_drop() {
        let (service, pending_repo, _audit) = make_service(vec![], DefaultPolicy::Block);
        let conn = dummy_connection();
        let verdict = service.decide(&conn).await.unwrap();
        assert_eq!(verdict, PacketVerdict::Drop);
        assert_eq!(pending_repo.count_pending().await, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn decide_pending_creates_decision_and_waits_for_user() {
        let (service, pending_repo, _audit) = make_service(vec![], DefaultPolicy::Ask);
        let conn = dummy_connection();
        let service_clone = service.clone();
        let conn_clone = conn.clone();

        let task = tokio::spawn(async move { service_clone.decide(&conn_clone).await });

        // Laisser le spawn s'exécuter et créer une PendingDecision.
        // Yield so the spawn runs and creates a PendingDecision.
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(1)).await;

        let pds = pending_repo.snapshot_pending().await;
        let pd = pds.into_iter().next().expect("PendingDecision devrait exister");

        let cmd = RespondToDecisionCommand {
            pending_decision_id: pd.id,
            action: DecisionAction::AllowOnce,
            granularity: DecisionGranularity::AppOnly,
        };
        service.resolve_decision(cmd).await.unwrap();

        let verdict = task.await.unwrap().unwrap();
        assert_eq!(verdict, PacketVerdict::Accept);
    }

    #[tokio::test(start_paused = true)]
    async fn decide_pending_dedup_attaches_to_existing() {
        let (service, pending_repo, _audit) = make_service(vec![], DefaultPolicy::Ask);
        let conn = dummy_connection();

        let s1 = service.clone();
        let c1 = conn.clone();
        let s2 = service.clone();
        let c2 = conn.clone();

        let t1 = tokio::spawn(async move { s1.decide(&c1).await });
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(1)).await;

        let t2 = tokio::spawn(async move { s2.decide(&c2).await });
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(1)).await;

        // Une seule PendingDecision doit exister grâce au debounce.
        // Only one PendingDecision should exist thanks to debounce.
        assert_eq!(pending_repo.count_pending().await, 1);

        let pds = pending_repo.snapshot_pending().await;
        let pd = pds.into_iter().next().expect("PendingDecision devrait exister");

        let cmd = RespondToDecisionCommand {
            pending_decision_id: pd.id,
            action: DecisionAction::AllowOnce,
            granularity: DecisionGranularity::AppOnly,
        };
        service.resolve_decision(cmd).await.unwrap();

        let v1 = t1.await.unwrap().unwrap();
        let v2 = t2.await.unwrap().unwrap();
        assert_eq!(v1, v2);
        assert_eq!(v1, PacketVerdict::Accept);
    }

    #[tokio::test(start_paused = true)]
    async fn decide_pending_timeout_returns_drop_with_audit() {
        let (service, _pending_repo, audit) = make_service(vec![], DefaultPolicy::Ask);
        let conn = dummy_connection();
        let service_clone = service.clone();
        let conn_clone = conn.clone();

        let task = tokio::spawn(async move { service_clone.decide(&conn_clone).await });

        // Avancer le temps au-delà du timeout de 28s.
        // Advance time past the 28s wait_for_verdict timeout.
        tokio::time::advance(Duration::from_secs(30)).await;
        tokio::task::yield_now().await;

        let verdict = task.await.unwrap().unwrap();
        assert_eq!(verdict, PacketVerdict::Drop);

        let events = audit.snapshot().await;
        assert!(
            events.iter().any(|e| {
                e.severity == syswall_domain::entities::Severity::Warning
                    && e.category == syswall_domain::entities::EventCategory::Decision
                    && e.description.contains("timeout")
            }),
            "Un événement d'audit Warning/Decision/timeout devrait être présent"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn block_once_resolves_to_drop() {
        let (service, pending_repo, _audit) = make_service(vec![], DefaultPolicy::Ask);
        let conn = dummy_connection();
        let service_clone = service.clone();
        let conn_clone = conn.clone();

        let task = tokio::spawn(async move { service_clone.decide(&conn_clone).await });
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(1)).await;

        let pds = pending_repo.snapshot_pending().await;
        let pd = pds.into_iter().next().expect("PendingDecision devrait exister");

        let cmd = RespondToDecisionCommand {
            pending_decision_id: pd.id,
            action: DecisionAction::BlockOnce,
            granularity: DecisionGranularity::AppOnly,
        };
        service.resolve_decision(cmd).await.unwrap();

        let verdict = task.await.unwrap().unwrap();
        assert_eq!(verdict, PacketVerdict::Drop);
    }

    #[tokio::test(start_paused = true)]
    async fn ignore_resolves_to_drop() {
        let (service, pending_repo, _audit) = make_service(vec![], DefaultPolicy::Ask);
        let conn = dummy_connection();
        let service_clone = service.clone();
        let conn_clone = conn.clone();

        let task = tokio::spawn(async move { service_clone.decide(&conn_clone).await });
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(1)).await;

        let pds = pending_repo.snapshot_pending().await;
        let pd = pds.into_iter().next().expect("PendingDecision devrait exister");

        let cmd = RespondToDecisionCommand {
            pending_decision_id: pd.id,
            action: DecisionAction::Ignore,
            granularity: DecisionGranularity::AppOnly,
        };
        service.resolve_decision(cmd).await.unwrap();

        let verdict = task.await.unwrap().unwrap();
        assert_eq!(verdict, PacketVerdict::Drop);
    }
}
