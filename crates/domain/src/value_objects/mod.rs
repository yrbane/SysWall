use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;

use crate::errors::DomainError;

// --- Port ---

/// Network port (1-65535). Rejects 0.
/// Port réseau (1-65535). Rejette 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Port(u16);

impl Port {
    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::Validation(
                "Port must be between 1 and 65535".to_string(),
            ));
        }
        Ok(Self(value))
    }

    pub fn value(&self) -> u16 {
        self.0
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// --- RulePriority ---

/// Rule evaluation priority. Lower value = higher priority.
/// System rules use 0. User rules start at 1.
///
/// Priorité d'évaluation des règles. Valeur basse = priorité haute.
/// Les règles système utilisent 0. Les règles utilisateur commencent à 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RulePriority(u32);

impl RulePriority {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn system() -> Self {
        Self(0)
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for RulePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// --- ExecutablePath ---

/// Validated absolute path to an executable.
/// Chemin absolu validé vers un exécutable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutablePath(PathBuf);

impl ExecutablePath {
    pub fn new(path: PathBuf) -> Result<Self, DomainError> {
        if !path.is_absolute() {
            return Err(DomainError::Validation(format!(
                "Executable path must be absolute, got: {}",
                path.display()
            )));
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }
}

impl fmt::Display for ExecutablePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

// --- Protocol ---

/// Network protocol.
/// Protocole réseau.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
    Other(u8),
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::Tcp => write!(f, "TCP"),
            Protocol::Udp => write!(f, "UDP"),
            Protocol::Icmp => write!(f, "ICMP"),
            Protocol::Other(n) => write!(f, "OTHER({})", n),
        }
    }
}

// --- Direction ---

/// Traffic direction relative to the host.
/// Direction du trafic par rapport à l'hôte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Inbound,
    Outbound,
}

// --- SocketAddress ---

/// IP address + port combination.
/// Combinaison adresse IP + port.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SocketAddress {
    pub ip: IpAddr,
    pub port: Port,
}

impl SocketAddress {
    pub fn new(ip: IpAddr, port: Port) -> Self {
        Self { ip, port }
    }
}

impl fmt::Display for SocketAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_valid_creates_successfully() {
        assert!(Port::new(80).is_ok());
        assert_eq!(Port::new(80).unwrap().value(), 80);
    }

    #[test]
    fn port_zero_rejected() {
        assert!(Port::new(0).is_err());
    }

    #[test]
    fn port_max_valid() {
        assert!(Port::new(65535).is_ok());
    }

    #[test]
    fn port_one_valid() {
        assert!(Port::new(1).is_ok());
    }

    #[test]
    fn rule_priority_ordering() {
        let p0 = RulePriority::system();
        let p1 = RulePriority::new(1);
        let p100 = RulePriority::new(100);
        assert!(p0 < p1);
        assert!(p1 < p100);
    }

    #[test]
    fn rule_priority_system_is_zero() {
        assert_eq!(RulePriority::system().value(), 0);
    }

    #[test]
    fn executable_path_absolute_valid() {
        assert!(ExecutablePath::new(PathBuf::from("/usr/bin/firefox")).is_ok());
    }

    #[test]
    fn executable_path_relative_rejected() {
        assert!(ExecutablePath::new(PathBuf::from("bin/firefox")).is_err());
    }
}
