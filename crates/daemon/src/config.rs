use serde::Deserialize;
use std::path::{Path, PathBuf};

use syswall_domain::errors::DomainError;
use syswall_domain::events::DefaultPolicy;

/// Top-level SysWall daemon configuration.
/// Configuration principale du démon SysWall.
// Les champs de configuration sont désérialisés depuis TOML ; tous doivent être présents
// même si certains ne sont pas encore lus dans le code courant.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SysWallConfig {
    pub config_version: u32,
    pub daemon: DaemonConfig,
    pub database: DatabaseConfig,
    pub firewall: FirewallConfig,
    pub monitoring: MonitoringConfig,
    pub learning: LearningConfig,
    pub ui: UiConfig,
    #[serde(default)]
    pub ebpf: EbpfConfig,
    #[serde(default)]
    pub antilockout: Option<AntilockoutConfig>,
    #[serde(default)]
    pub nfqueue: Option<NfqueueConfig>,
}

/// Daemon runtime configuration (socket, logging, watchdog).
/// Configuration d'exécution du démon (socket, journalisation, chien de garde).
#[derive(Debug, Deserialize)]
pub struct DaemonConfig {
    pub socket_path: PathBuf,
    /// Niveau de log applique par tracing-subscriber via RUST_LOG ou la valeur par defaut.
    /// Log level applied by tracing-subscriber via RUST_LOG or the default value.
    #[allow(dead_code)]
    pub log_level: String,
    pub watchdog_interval_secs: u64,
}

/// Database configuration (path, retention, audit batching).
/// Configuration de la base de données (chemin, rétention, mise en lot des audits).
#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub journal_retention_days: u32,
    pub audit_batch_size: usize,
    pub audit_flush_interval_secs: u64,
}

/// Firewall configuration (default policy, nftables).
/// Configuration du pare-feu (politique par défaut, nftables).
#[derive(Debug, Deserialize)]
pub struct FirewallConfig {
    pub default_policy: DefaultPolicyConfig,
    pub nftables_table_name: String,
    #[serde(default = "default_nft_path")]
    pub nft_binary_path: std::path::PathBuf,
    #[serde(default = "default_nft_timeout")]
    pub nft_command_timeout_secs: u64,
    #[serde(default = "default_nft_max_output")]
    pub nft_max_output_bytes: usize,
    #[serde(default)]
    pub use_fake: bool,
}

fn default_nft_path() -> std::path::PathBuf {
    std::path::PathBuf::from("/usr/sbin/nft")
}

fn default_nft_timeout() -> u64 {
    5
}

fn default_nft_max_output() -> usize {
    1_048_576
}

/// Default policy enum as read from configuration.
/// Énumération de la politique par défaut lue depuis la configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DefaultPolicyConfig {
    Ask,
    Allow,
    Block,
}

impl From<&DefaultPolicyConfig> for DefaultPolicy {
    fn from(config: &DefaultPolicyConfig) -> Self {
        match config {
            DefaultPolicyConfig::Ask => DefaultPolicy::Ask,
            DefaultPolicyConfig::Allow => DefaultPolicy::Allow,
            DefaultPolicyConfig::Block => DefaultPolicy::Block,
        }
    }
}

/// Connection monitoring configuration (buffers, cache TTL, event bus).
/// Configuration du suivi des connexions (tampons, TTL du cache, bus d'événements).
#[derive(Debug, Deserialize)]
pub struct MonitoringConfig {
    pub conntrack_buffer_size: usize,
    pub process_cache_ttl_secs: u64,
    #[serde(default = "default_cache_capacity")]
    pub process_cache_capacity: usize,
    pub event_bus_capacity: usize,
    #[serde(default = "default_conntrack_path")]
    pub conntrack_binary_path: std::path::PathBuf,
    #[serde(default = "default_conntrack_protocols")]
    pub conntrack_protocols: Vec<String>,
    #[serde(default = "default_dns_cache_capacity")]
    pub dns_cache_capacity: usize,
    #[serde(default = "default_dns_cache_ttl")]
    pub dns_cache_ttl_secs: u64,
    #[serde(default)]
    pub use_fake: bool,
    /// Fenêtre de fusion des événements ConnectionDetected (ms). 0 = désactivé.
    /// Merge window for ConnectionDetected events (ms). 0 = disabled.
    #[serde(default = "default_event_merge_window")]
    pub event_merge_window_ms: u64,
}

fn default_event_merge_window() -> u64 {
    100
}

fn default_cache_capacity() -> usize {
    1024
}

fn default_dns_cache_capacity() -> usize {
    4096
}

fn default_dns_cache_ttl() -> u64 {
    300
}

fn default_conntrack_path() -> std::path::PathBuf {
    std::path::PathBuf::from("/usr/sbin/conntrack")
}

fn default_conntrack_protocols() -> Vec<String> {
    vec!["tcp".to_string(), "udp".to_string()]
}

/// Learning mode configuration (debounce, timeouts, overflow).
/// Configuration du mode d'apprentissage (anti-rebond, délais, débordement).
#[derive(Debug, Deserialize)]
pub struct LearningConfig {
    pub enabled: bool,
    /// Anti-rebond cote UI — non utilise par le daemon (traite par le frontend).
    /// UI-side debounce — not used by the daemon (handled by the frontend).
    #[allow(dead_code)]
    pub debounce_window_secs: u64,
    pub prompt_timeout_secs: u64,
    pub default_timeout_action: String,
    pub max_pending_decisions: usize,
    pub overflow_action: String,
}

/// UI configuration (locale only — theme and refresh are managed externally).
/// Configuration de l'interface utilisateur (locale uniquement).
#[derive(Debug, Deserialize)]
pub struct UiConfig {
    /// Locale de l'interface — reservee a une future internationalisation.
    /// UI locale — reserved for future internationalization.
    #[allow(dead_code)]
    pub locale: String,
}

/// Anti-lockout watchdog configuration.
/// Configuration de la sentinelle anti-lockout.
#[derive(Debug, Clone, Deserialize)]
pub struct AntilockoutConfig {
    #[serde(default = "default_antilockout_enabled")]
    pub enabled: bool,
    #[serde(default = "default_antilockout_endpoints")]
    pub endpoints: Vec<String>,
    #[serde(default = "default_antilockout_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_antilockout_probe_interval")]
    pub probe_interval_secs: u64,
    #[serde(default = "default_antilockout_per_endpoint_timeout")]
    pub per_endpoint_timeout_secs: u64,
}

fn default_antilockout_enabled() -> bool {
    true
}
fn default_antilockout_endpoints() -> Vec<String> {
    vec!["1.1.1.1:53".into(), "[2606:4700:4700::1111]:53".into()]
}
fn default_antilockout_timeout() -> u64 {
    30
}
fn default_antilockout_probe_interval() -> u64 {
    5
}
fn default_antilockout_per_endpoint_timeout() -> u64 {
    2
}

impl Default for AntilockoutConfig {
    fn default() -> Self {
        Self {
            enabled: default_antilockout_enabled(),
            endpoints: default_antilockout_endpoints(),
            timeout_secs: default_antilockout_timeout(),
            probe_interval_secs: default_antilockout_probe_interval(),
            per_endpoint_timeout_secs: default_antilockout_per_endpoint_timeout(),
        }
    }
}

/// Configuration eBPF (activation, taille du ring buffer).
/// eBPF configuration (activation, ring buffer size).
#[derive(Debug, Deserialize)]
pub struct EbpfConfig {
    /// Activer la capture eBPF des PID (fallback procfs si désactivé ou indisponible).
    /// Enable eBPF PID capture (falls back to procfs if disabled or unavailable).
    #[serde(default = "default_ebpf_enabled")]
    pub enabled: bool,
}

fn default_ebpf_enabled() -> bool {
    true
}

impl Default for EbpfConfig {
    fn default() -> Self {
        Self {
            enabled: default_ebpf_enabled(),
        }
    }
}

/// NFQUEUE-based active blocking configuration.
/// Configuration du blocage actif via NFQUEUE.
#[derive(Debug, Clone, Deserialize)]
pub struct NfqueueConfig {
    #[serde(default = "default_nfq_enabled")]
    pub enabled: bool,
    #[serde(default = "default_nfq_queue_num")]
    pub queue_num: u16,
    #[serde(default = "default_nfq_max_queued")]
    pub max_queued: u32,
    #[serde(default = "default_nfq_overflow_policy")]
    pub overflow_policy: String, // "block" or "accept"
}

fn default_nfq_enabled() -> bool {
    true
}
fn default_nfq_queue_num() -> u16 {
    0
}
fn default_nfq_max_queued() -> u32 {
    1024
}
fn default_nfq_overflow_policy() -> String {
    "block".to_string()
}

impl Default for NfqueueConfig {
    fn default() -> Self {
        Self {
            enabled: default_nfq_enabled(),
            queue_num: default_nfq_queue_num(),
            max_queued: default_nfq_max_queued(),
            overflow_policy: default_nfq_overflow_policy(),
        }
    }
}

impl SysWallConfig {
    /// Load config from a TOML file. Falls back to defaults if the file doesn't exist.
    /// Charge la configuration depuis un fichier TOML. Retourne une erreur si le fichier est invalide.
    pub fn load(path: &Path) -> Result<Self, DomainError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| DomainError::Infrastructure(format!("Failed to read config: {}", e)))?;
        Self::from_toml(&content)
    }

    /// Parse config from a TOML string.
    /// Analyse la configuration depuis une chaîne TOML.
    pub fn from_toml(content: &str) -> Result<Self, DomainError> {
        toml::from_str(content)
            .map_err(|e| DomainError::Validation(format!("Invalid config: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CONFIG: &str = r#"
config_version = 1

[daemon]
socket_path = "/var/run/syswall/syswall.sock"
log_level = "info"
watchdog_interval_secs = 15

[database]
path = "/var/lib/syswall/syswall.db"
journal_retention_days = 30
audit_batch_size = 100
audit_flush_interval_secs = 2

[firewall]
default_policy = "ask"
nftables_table_name = "syswall"
nft_binary_path = "/usr/sbin/nft"
nft_command_timeout_secs = 5
nft_max_output_bytes = 1048576
use_fake = true

[monitoring]
conntrack_buffer_size = 4096
process_cache_ttl_secs = 5
process_cache_capacity = 1024
event_bus_capacity = 4096
conntrack_binary_path = "/usr/sbin/conntrack"
conntrack_protocols = ["tcp", "udp"]
use_fake = true

[learning]
enabled = true
debounce_window_secs = 5
prompt_timeout_secs = 60
default_timeout_action = "block"
max_pending_decisions = 50
overflow_action = "block"

[ui]
locale = "fr"

[ebpf]
enabled = true
"#;

    #[test]
    fn parse_valid_config() {
        let config = SysWallConfig::from_toml(TEST_CONFIG).unwrap();
        assert_eq!(config.config_version, 1);
        assert_eq!(config.daemon.log_level, "info");
        assert!(matches!(
            config.firewall.default_policy,
            DefaultPolicyConfig::Ask
        ));
        assert_eq!(config.learning.prompt_timeout_secs, 60);
        assert_eq!(config.ui.locale, "fr");
    }

    #[test]
    fn invalid_toml_returns_error() {
        let result = SysWallConfig::from_toml("not valid toml {{{");
        assert!(result.is_err());
    }

    #[test]
    fn default_policy_conversion() {
        let policy: DefaultPolicy = (&DefaultPolicyConfig::Ask).into();
        assert_eq!(policy, DefaultPolicy::Ask);
    }

    const ANTILOCKOUT_CONFIG: &str = r#"
[antilockout]
enabled = true
endpoints = ["1.1.1.1:53", "[2606:4700:4700::1111]:53"]
timeout_secs = 30
probe_interval_secs = 5
per_endpoint_timeout_secs = 2
"#;

    #[test]
    fn parse_antilockout_section() {
        let full = format!("{}\n{}", TEST_CONFIG, ANTILOCKOUT_CONFIG);
        let config = SysWallConfig::from_toml(&full).unwrap();
        let al = config.antilockout.as_ref().unwrap();
        assert!(al.enabled);
        assert_eq!(al.endpoints.len(), 2);
        assert_eq!(al.timeout_secs, 30);
        assert_eq!(al.probe_interval_secs, 5);
        assert_eq!(al.per_endpoint_timeout_secs, 2);
    }

    #[test]
    fn antilockout_section_is_optional() {
        let config = SysWallConfig::from_toml(TEST_CONFIG).unwrap();
        // None means "use defaults at bootstrap time"
        assert!(config.antilockout.is_none());
    }

    #[test]
    fn antilockout_default_values() {
        let cfg = AntilockoutConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.timeout_secs, 30);
        assert_eq!(cfg.probe_interval_secs, 5);
        assert_eq!(cfg.per_endpoint_timeout_secs, 2);
        assert_eq!(cfg.endpoints.len(), 2);
        assert_eq!(cfg.endpoints[0], "1.1.1.1:53");
    }

    const NFQ_TOML: &str = r#"
[nfqueue]
enabled = true
queue_num = 7
max_queued = 2048
overflow_policy = "accept"
"#;

    #[test]
    fn parse_nfqueue_section() {
        let full = format!("{}\n{}", TEST_CONFIG, NFQ_TOML);
        let cfg = SysWallConfig::from_toml(&full).unwrap();
        let nq = cfg.nfqueue.as_ref().unwrap();
        assert!(nq.enabled);
        assert_eq!(nq.queue_num, 7);
        assert_eq!(nq.max_queued, 2048);
        assert_eq!(nq.overflow_policy, "accept");
    }

    #[test]
    fn nfqueue_section_is_optional() {
        let cfg = SysWallConfig::from_toml(TEST_CONFIG).unwrap();
        assert!(cfg.nfqueue.is_none());
    }

    #[test]
    fn nfqueue_default_values() {
        let cfg = NfqueueConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.queue_num, 0);
        assert_eq!(cfg.max_queued, 1024);
        assert_eq!(cfg.overflow_policy, "block");
    }
}
