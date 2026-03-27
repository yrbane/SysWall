use std::sync::Arc;

use chrono::{Duration, Utc};
use syswall_domain::entities::{
    ConnectionSnapshot, Decision, DecisionId, PendingDecision, PendingDecisionId,
    PendingDecisionStatus,
};
use syswall_domain::errors::DomainError;
use syswall_domain::events::DomainEvent;
use syswall_domain::ports::{
    DecisionRepository, EventBus, PendingDecisionRepository, UserNotifier,
};

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
    config: LearningConfig,
}

impl LearningService {
    pub fn new(
        pending_repo: Arc<dyn PendingDecisionRepository>,
        decision_repo: Arc<dyn DecisionRepository>,
        notifier: Arc<dyn UserNotifier>,
        event_bus: Arc<dyn EventBus>,
        config: LearningConfig,
    ) -> Self {
        Self {
            pending_repo,
            decision_repo,
            notifier,
            event_bus,
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

        // If the action creates a permanent rule, do it via RuleService
        // (Rule creation from decisions will be fully wired in sub-project 4)

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

        let config = LearningConfig {
            prompt_timeout_secs: 60,
            max_pending_decisions: 50,
        };

        let service = LearningService::new(
            pending_repo.clone(),
            decision_repo,
            notifier.clone(),
            event_bus,
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
