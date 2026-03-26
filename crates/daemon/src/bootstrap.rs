use std::sync::Arc;
use std::time::Duration;

use tracing::info;

use syswall_app::fakes::{
    FakeConnectionMonitor, FakeFirewallEngine, FakeProcessResolver, FakeUserNotifier,
};
use syswall_app::services::audit_service::AuditService;
use syswall_app::services::connection_service::ConnectionService;
use syswall_app::services::learning_service::{
    LearningConfig as AppLearningConfig, LearningService,
};
use syswall_app::services::rule_service::RuleService;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::{ConnectionMonitor, FirewallEngine, ProcessResolver};
use syswall_infra::conntrack::{ConntrackConfig, ConntrackMonitorAdapter};
use syswall_infra::event_bus::TokioBroadcastEventBus;
use syswall_infra::nftables::{NftablesConfig, NftablesFirewallAdapter};
use syswall_infra::persistence::audit_repository::SqliteAuditRepository;
use syswall_infra::persistence::decision_repository::SqliteDecisionRepository;
use syswall_infra::persistence::pending_decision_repository::SqlitePendingDecisionRepository;
use syswall_infra::persistence::rule_repository::SqliteRuleRepository;
use syswall_infra::persistence::Database;
use syswall_infra::process::{ProcfsConfig, ProcfsProcessResolver};

use crate::config::SysWallConfig;

/// All the wired-up services, ready to use.
/// Tous les services assembles, prets a l'emploi.
pub struct AppContext {
    pub rule_service: Arc<RuleService>,
    pub connection_service: Arc<ConnectionService>,
    pub learning_service: Arc<LearningService>,
    pub audit_service: Arc<AuditService>,
    pub event_bus: Arc<TokioBroadcastEventBus>,
    /// Connection monitor for the Supervisor to start streaming.
    /// Moniteur de connexion pour que le Superviseur demarre le streaming.
    pub connection_monitor: Arc<dyn ConnectionMonitor>,
    /// Firewall engine for sync_all_rules at startup.
    /// Moteur de pare-feu pour sync_all_rules au demarrage.
    pub firewall: Arc<dyn FirewallEngine>,
    /// Rule repository reference for whitelist creation.
    /// Reference au depot de regles pour la creation de la liste blanche.
    pub rule_repo: Arc<SqliteRuleRepository>,
}

/// Wire up all dependencies and return the application context.
/// Assemble toutes les dependances et retourne le contexte applicatif.
pub fn bootstrap(config: &SysWallConfig) -> Result<AppContext, DomainError> {
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

    // Firewall engine -- real or fake based on config
    // Moteur de pare-feu -- reel ou factice selon la configuration
    let firewall: Arc<dyn FirewallEngine> = if config.firewall.use_fake {
        info!("Using FakeFirewallEngine (use_fake = true)");
        Arc::new(FakeFirewallEngine::new())
    } else {
        info!("Using NftablesFirewallAdapter");
        Arc::new(NftablesFirewallAdapter::new(NftablesConfig {
            table_name: config.firewall.nftables_table_name.clone(),
            nft_binary_path: config.firewall.nft_binary_path.clone(),
            command_timeout: Duration::from_secs(config.firewall.nft_command_timeout_secs),
            max_output_bytes: config.firewall.nft_max_output_bytes,
        })?)
    };

    // Process resolver -- real or fake based on config
    // Resolveur de processus -- reel ou factice selon la configuration
    let process_resolver: Arc<dyn ProcessResolver> = if config.monitoring.use_fake {
        info!("Using FakeProcessResolver (use_fake = true)");
        Arc::new(FakeProcessResolver::new())
    } else {
        info!("Using ProcfsProcessResolver");
        Arc::new(ProcfsProcessResolver::new(ProcfsConfig {
            cache_capacity: config.monitoring.process_cache_capacity,
            cache_ttl: Duration::from_secs(config.monitoring.process_cache_ttl_secs),
        })?)
    };

    // Connection monitor -- real or fake based on config
    // Moniteur de connexion -- reel ou factice selon la configuration
    let connection_monitor: Arc<dyn ConnectionMonitor> = if config.monitoring.use_fake {
        info!("Using FakeConnectionMonitor (use_fake = true)");
        Arc::new(FakeConnectionMonitor::new())
    } else {
        info!("Using ConntrackMonitorAdapter");
        Arc::new(ConntrackMonitorAdapter::new(ConntrackConfig {
            binary_path: config.monitoring.conntrack_binary_path.clone(),
            protocols: config.monitoring.conntrack_protocols.clone(),
            buffer_size: config.monitoring.conntrack_buffer_size,
        })?)
    };

    let notifier = Arc::new(FakeUserNotifier::new());

    // Application services
    let rule_service = Arc::new(RuleService::new(
        rule_repo.clone(),
        firewall.clone(),
        event_bus.clone(),
    ));

    let default_policy = (&config.firewall.default_policy).into();

    let connection_service = Arc::new(ConnectionService::new(
        process_resolver,
        rule_repo.clone(),
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
        connection_monitor,
        firewall,
        rule_repo,
    })
}
