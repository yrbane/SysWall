use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use syswall_app::fakes::{
    FakeConnectionMonitor, FakeFirewallEngine, FakeProcessResolver, FakeUserNotifier,
};
use syswall_app::services::antilockout_guard::{AntilockoutConfig as GuardConfig, AntilockoutGuard};
use syswall_app::services::audit_service::AuditService;
use syswall_app::services::connection_service::ConnectionService;
use syswall_app::services::learning_service::{
    LearningConfig as AppLearningConfig, LearningService, VerdictBroadcasts,
};
use syswall_app::services::rule_service::RuleService;
use syswall_domain::ports::connectivity::LockoutGuard;
use syswall_domain::ports::{ConnectionMonitor, FirewallEngine, ProcessResolver};
use syswall_infra::conntrack::{ConntrackConfig, ConntrackMonitorAdapter};
use syswall_infra::connectivity::TcpProbe;
use syswall_infra::dns::DnsResolver as InfraDnsResolver;
use syswall_infra::event_bus::TokioBroadcastEventBus;
use syswall_infra::nftables::{NftablesConfig, NftablesFirewallAdapter};
use syswall_infra::persistence::audit_repository::SqliteAuditRepository;
use syswall_infra::persistence::decision_repository::SqliteDecisionRepository;
use syswall_infra::persistence::pending_decision_repository::SqlitePendingDecisionRepository;
use syswall_infra::persistence::rule_repository::SqliteRuleRepository;
use syswall_infra::persistence::Database;
use syswall_ebpf::{EbpfProcessResolver, HybridProcessResolver};
use syswall_infra::process::{ProcfsConfig, ProcfsProcessResolver};

use crate::config::SysWallConfig;
use crate::startup_error::StartupError;

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
pub fn bootstrap(config: &SysWallConfig) -> Result<AppContext, StartupError> {
    // Database
    let db = Arc::new(
        Database::open(&config.database.path)
            .map_err(|e| StartupError::InfrastructureInit(e.to_string()))?,
    );

    // Repositories
    let rule_repo = Arc::new(SqliteRuleRepository::new(db.clone()));
    let pending_repo = Arc::new(SqlitePendingDecisionRepository::new(db.clone()));
    let decision_repo = Arc::new(SqliteDecisionRepository::new(db.clone()));
    let audit_repo = Arc::new(SqliteAuditRepository::new(db.clone()));

    // Event bus (avec fusion optionnelle des événements ConnectionDetected)
    // Event bus (with optional ConnectionDetected event merging)
    let event_bus = if config.monitoring.event_merge_window_ms > 0 {
        Arc::new(TokioBroadcastEventBus::with_merge_window(
            config.monitoring.event_bus_capacity,
            Duration::from_millis(config.monitoring.event_merge_window_ms),
        ))
    } else {
        Arc::new(TokioBroadcastEventBus::new(
            config.monitoring.event_bus_capacity,
        ))
    };

    // Firewall engine -- real or fake based on config
    // Moteur de pare-feu -- reel ou factice selon la configuration
    let firewall: Arc<dyn FirewallEngine> = if config.firewall.use_fake {
        info!("Using FakeFirewallEngine (use_fake = true)");
        Arc::new(FakeFirewallEngine::new())
    } else {
        info!("Using NftablesFirewallAdapter");
        let nft_adapter = NftablesFirewallAdapter::new(NftablesConfig {
            table_name: config.firewall.nftables_table_name.clone(),
            nft_binary_path: config.firewall.nft_binary_path.clone(),
            command_timeout: Duration::from_secs(config.firewall.nft_command_timeout_secs),
            max_output_bytes: config.firewall.nft_max_output_bytes,
        })
        .map_err(|e| StartupError::InfrastructureInit(e.to_string()))?;

        // Injection du guard anti-lockout dans l'adaptateur nftables
        // Inject the anti-lockout guard into the nftables adapter
        let al_cfg = config
            .antilockout
            .clone()
            .unwrap_or_default();

        let nft_adapter = if al_cfg.enabled {
            let endpoints: Result<Vec<SocketAddr>, _> = al_cfg
                .endpoints
                .iter()
                .map(|s| s.parse::<SocketAddr>())
                .collect();
            let endpoints = endpoints.map_err(|e| {
                StartupError::ConfigInvalid(format!("antilockout.endpoints parse error: {e}"))
            })?;
            let probe = Arc::new(
                TcpProbe::new(endpoints, Duration::from_secs(al_cfg.per_endpoint_timeout_secs))
                    .map_err(|e| StartupError::ConfigInvalid(format!("antilockout: {e}")))?,
            );
            let guard = Arc::new(AntilockoutGuard::new(
                probe,
                audit_repo.clone(),
                GuardConfig {
                    timeout: Duration::from_secs(al_cfg.timeout_secs),
                    probe_interval: Duration::from_secs(al_cfg.probe_interval_secs),
                },
                event_bus.clone(),
            ));
            nft_adapter.with_lockout_guard(guard as Arc<dyn LockoutGuard>)
        } else {
            nft_adapter
        };

        Arc::new(nft_adapter)
    };

    // Résolveur de processus -- fake, ou hybrid (eBPF + procfs)
    // Process resolver -- fake, or hybrid (eBPF + procfs)
    let process_resolver: Arc<dyn ProcessResolver> = if config.monitoring.use_fake {
        info!("Using FakeProcessResolver (use_fake = true)");
        Arc::new(FakeProcessResolver::new())
    } else {
        let procfs = Arc::new(
            ProcfsProcessResolver::new(ProcfsConfig {
                cache_capacity: config.monitoring.process_cache_capacity,
                cache_ttl: Duration::from_secs(config.monitoring.process_cache_ttl_secs),
            })
            .map_err(|e| StartupError::InfrastructureInit(e.to_string()))?,
        );

        // Tente de charger eBPF, fallback silencieux vers procfs seul
        // Try eBPF, silent fallback to procfs-only
        let ebpf = if config.ebpf.enabled {
            match EbpfProcessResolver::try_new() {
                Ok(e) => {
                    info!("Résolveur eBPF chargé avec succès");
                    Some(e)
                }
                Err(e) => {
                    warn!("eBPF indisponible, fallback vers procfs: {}", e);
                    None
                }
            }
        } else {
            info!("eBPF désactivé par configuration");
            None
        };

        info!(
            "Using HybridProcessResolver (ebpf={})",
            ebpf.is_some()
        );
        Arc::new(HybridProcessResolver::new(ebpf, procfs))
    };

    // Connection monitor -- real or fake based on config
    // Moniteur de connexion -- reel ou factice selon la configuration
    let connection_monitor: Arc<dyn ConnectionMonitor> = if config.monitoring.use_fake {
        info!("Using FakeConnectionMonitor (use_fake = true)");
        Arc::new(FakeConnectionMonitor::new())
    } else {
        info!("Using ConntrackMonitorAdapter");
        Arc::new(
            ConntrackMonitorAdapter::new(ConntrackConfig {
                binary_path: config.monitoring.conntrack_binary_path.clone(),
                protocols: config.monitoring.conntrack_protocols.clone(),
                buffer_size: config.monitoring.conntrack_buffer_size,
            })
            .map_err(|e| StartupError::InfrastructureInit(e.to_string()))?,
        )
    };

    let notifier = Arc::new(FakeUserNotifier::new());

    // DNS resolver (LRU cache, capacity 4096, TTL 300s)
    // Résolveur DNS (cache LRU, capacité 4096, TTL 300s)
    let dns_resolver = Arc::new(InfraDnsResolver::new(
        config.monitoring.dns_cache_capacity,
        config.monitoring.dns_cache_ttl_secs,
    ));

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
        dns_resolver,
    ));

    let verdict_broadcasts = Arc::new(VerdictBroadcasts::new());
    let learning_service = Arc::new(LearningService::new(
        pending_repo,
        decision_repo,
        notifier,
        event_bus.clone(),
        audit_repo.clone(),
        rule_repo.clone(),
        default_policy,
        verdict_broadcasts,
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
