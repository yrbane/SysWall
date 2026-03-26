use std::sync::Arc;

use syswall_app::fakes::{FakeFirewallEngine, FakeProcessResolver, FakeUserNotifier};
use syswall_app::services::audit_service::AuditService;
use syswall_app::services::connection_service::ConnectionService;
use syswall_app::services::learning_service::{
    LearningConfig as AppLearningConfig, LearningService,
};
use syswall_app::services::rule_service::RuleService;
use syswall_infra::event_bus::TokioBroadcastEventBus;
use syswall_infra::persistence::audit_repository::SqliteAuditRepository;
use syswall_infra::persistence::decision_repository::SqliteDecisionRepository;
use syswall_infra::persistence::pending_decision_repository::SqlitePendingDecisionRepository;
use syswall_infra::persistence::rule_repository::SqliteRuleRepository;
use syswall_infra::persistence::Database;

use crate::config::SysWallConfig;

/// All the wired-up services, ready to use.
/// Tous les services assemblés, prêts à l'emploi.
pub struct AppContext {
    pub rule_service: Arc<RuleService>,
    pub connection_service: Arc<ConnectionService>,
    pub learning_service: Arc<LearningService>,
    pub audit_service: Arc<AuditService>,
    pub event_bus: Arc<TokioBroadcastEventBus>,
}

/// Wire up all dependencies and return the application context.
/// Assemble toutes les dépendances et retourne le contexte applicatif.
pub fn bootstrap(
    config: &SysWallConfig,
) -> Result<AppContext, syswall_domain::errors::DomainError> {
    // Database
    let db = Arc::new(Database::open(&config.database.path)?);

    // Repositories
    let rule_repo = Arc::new(SqliteRuleRepository::new(db.clone()));
    let pending_repo = Arc::new(SqlitePendingDecisionRepository::new(db.clone()));
    let decision_repo = Arc::new(SqliteDecisionRepository::new(db.clone()));
    let audit_repo = Arc::new(SqliteAuditRepository::new(db.clone()));

    // Event bus
    let event_bus = Arc::new(TokioBroadcastEventBus::new(
        config.monitoring.event_bus_capacity,
    ));

    // System adapters — stubs for foundations, replaced in sub-projects 2-3
    // Adaptateurs système — ébauches pour les fondations, remplacés dans les sous-projets 2-3
    let firewall = Arc::new(FakeFirewallEngine::new());
    let process_resolver = Arc::new(FakeProcessResolver::new());
    let notifier = Arc::new(FakeUserNotifier::new());

    // Application services
    let rule_service = Arc::new(RuleService::new(
        rule_repo.clone(),
        firewall,
        event_bus.clone(),
    ));

    let default_policy = (&config.firewall.default_policy).into();

    let connection_service = Arc::new(ConnectionService::new(
        process_resolver,
        rule_repo,
        event_bus.clone(),
        default_policy,
    ));

    let learning_service = Arc::new(LearningService::new(
        pending_repo,
        decision_repo,
        notifier,
        event_bus.clone(),
        AppLearningConfig {
            prompt_timeout_secs: config.learning.prompt_timeout_secs,
            max_pending_decisions: config.learning.max_pending_decisions,
        },
    ));

    let audit_service = Arc::new(AuditService::new(audit_repo));

    Ok(AppContext {
        rule_service,
        connection_service,
        learning_service,
        audit_service,
        event_bus,
    })
}
