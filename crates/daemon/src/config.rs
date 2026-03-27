use serde::Deserialize;
use std::path::{Path, PathBuf};

use syswall_domain::errors::DomainError;
use syswall_domain::events::DefaultPolicy;

/// Top-level SysWall daemon configuration.
/// Configuration principale du démon SysWall.
#[derive(Debug, Deserialize)]
pub struct SysWallConfig {
    pub config_version: u32,
    pub daemon: DaemonConfig,
    pub database: DatabaseConfig,
    pub firewall: FirewallConfig,
    pub monitoring: MonitoringConfig,
    pub learning: LearningConfig,
    pub ui: UiConfig,
}

/// Daemon runtime configuration (socket, logging, watchdog).
/// Configuration d'exécution du démon (socket, journalisation, chien de garde).
#[derive(Debug, Deserialize)]
pub struct DaemonConfig {
    pub socket_path: PathBuf,
    pub log_level: String,
    pub log_dir: PathBuf,
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

/// Firewall configuration (default policy, rollback, nftables).
/// Configuration du pare-feu (politique par défaut, retour arrière, nftables).
#[derive(Debug, Deserialize)]
pub struct FirewallConfig {
    pub default_policy: DefaultPolicyConfig,
    pub rollback_timeout_secs: u64,
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
    pub debounce_window_secs: u64,
    pub prompt_timeout_secs: u64,
    pub default_timeout_action: String,
    pub max_pending_decisions: usize,
    pub overflow_action: String,
}

/// UI configuration (locale, theme, refresh rate).
/// Configuration de l'interface utilisateur (locale, thème, fréquence de rafraîchissement).
#[derive(Debug, Deserialize)]
pub struct UiConfig {
    pub locale: String,
    pub theme: String,
    pub refresh_interval_ms: u64,
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
log_dir = "/var/log/syswall"
watchdog_interval_secs = 15

[database]
path = "/var/lib/syswall/syswall.db"
journal_retention_days = 30
audit_batch_size = 100
audit_flush_interval_secs = 2

[firewall]
default_policy = "ask"
rollback_timeout_secs = 30
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
theme = "dark"
refresh_interval_ms = 1000
"#;

    #[test]
    fn parse_valid_config() {
        let config = SysWallConfig::from_toml(TEST_CONFIG).unwrap();
        assert_eq!(config.config_version, 1);
        assert_eq!(config.daemon.log_level, "info");
        assert!(matches!(config.firewall.default_policy, DefaultPolicyConfig::Ask));
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
}
