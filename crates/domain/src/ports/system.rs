use async_trait::async_trait;
use futures::Stream;
use std::net::IpAddr;
use std::pin::Pin;

use crate::entities::{Blocklist, Connection, ProcessInfo, Rule, RuleId};
use crate::errors::DomainError;
use crate::events::FirewallStatus;

/// Stream of connection events from the monitoring subsystem.
/// Flux d'événements de connexion provenant du sous-système de surveillance.
pub type ConnectionEventStream =
    Pin<Box<dyn Stream<Item = Result<Connection, DomainError>> + Send>>;

/// Adapter for the underlying firewall engine (e.g., nftables).
/// Adaptateur pour le moteur de pare-feu sous-jacent (ex. nftables).
#[async_trait]
pub trait FirewallEngine: Send + Sync {
    async fn apply_rule(&self, rule: &Rule) -> Result<(), DomainError>;
    async fn remove_rule(&self, rule_id: &RuleId) -> Result<(), DomainError>;
    async fn sync_all_rules(&self, rules: &[Rule]) -> Result<(), DomainError>;
    async fn get_status(&self) -> Result<FirewallStatus, DomainError>;

    /// Coupe tout le trafic réseau (kill-switch).
    /// Cut all network traffic (kill-switch).
    async fn drop_all(&self) -> Result<(), DomainError> {
        Err(DomainError::Infrastructure("drop_all not supported".into()))
    }

    /// Rétablit le trafic réseau après un kill-switch.
    /// Restore network traffic after kill-switch.
    async fn remove_drop_all(&self) -> Result<(), DomainError> {
        Err(DomainError::Infrastructure(
            "remove_drop_all not supported".into(),
        ))
    }
}

/// Adapter for monitoring network connections (e.g., conntrack).
/// Adaptateur pour la surveillance des connexions réseau (ex. conntrack).
#[async_trait]
pub trait ConnectionMonitor: Send + Sync {
    async fn stream_events(&self) -> Result<ConnectionEventStream, DomainError>;
    async fn get_active_connections(&self) -> Result<Vec<Connection>, DomainError>;
}

/// Performs reverse DNS resolution for IP addresses.
/// Effectue la résolution DNS inverse pour les adresses IP.
#[async_trait]
pub trait DnsResolver: Send + Sync {
    /// Resolve the hostname for the given IP address, returning None on failure.
    /// Résout le nom d'hôte pour l'adresse IP donnée, retourne None en cas d'échec.
    async fn resolve(&self, ip: IpAddr) -> Result<Option<String>, DomainError>;
}

/// Resolves process information from PIDs or socket inodes.
/// Résout les informations de processus à partir des PID ou des inodes de socket.
#[async_trait]
pub trait ProcessResolver: Send + Sync {
    async fn resolve(&self, pid: u32) -> Result<Option<ProcessInfo>, DomainError>;
    async fn resolve_by_socket(&self, inode: u64) -> Result<Option<ProcessInfo>, DomainError>;

    /// Resolve process info by connection 5-tuple. Default returns None.
    /// Not all resolvers support this -- only ProcfsProcessResolver implements it.
    ///
    /// Résout les informations du processus par 5-tuple de connexion.
    /// Tous les résolveurs ne le supportent pas -- seul ProcfsProcessResolver l'implémente.
    async fn resolve_by_connection(
        &self,
        _protocol: crate::value_objects::Protocol,
        _local_ip: std::net::IpAddr,
        _local_port: u16,
        _remote_ip: std::net::IpAddr,
        _remote_port: u16,
    ) -> Result<Option<ProcessInfo>, DomainError> {
        Ok(None)
    }
}

/// Vérifie si un domaine ou une IP est dans une blocklist.
/// Checks if a domain or IP is in a blocklist.
pub trait BlocklistChecker: Send + Sync {
    /// Vérifie si le domaine est bloqué. / Check if domain is blocked.
    fn is_blocked_domain(&self, domain: &str) -> bool;
    /// Vérifie si l'IP est bloquée. / Check if IP is blocked.
    fn is_blocked_ip(&self, ip: IpAddr) -> bool;
    /// Retourne la liste des blocklists chargées. / Return loaded blocklists.
    fn list_blocklists(&self) -> Vec<Blocklist>;
    /// Recharge les blocklists depuis le disque. / Reload blocklists from disk.
    fn reload(&self) -> Result<(), DomainError>;
}
