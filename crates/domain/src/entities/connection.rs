use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::value_objects::{Direction, ExecutablePath, Protocol, SocketAddress};

/// Unique identifier for a connection.
/// Identifiant unique d'une connexion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(Uuid);

impl ConnectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

/// Runtime outcome for a connection.
/// Résultat à l'exécution pour une connexion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionVerdict {
    Unknown,
    PendingDecision,
    Allowed,
    Blocked,
    Ignored,
}

/// Connection lifecycle state.
/// État du cycle de vie d'une connexion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionState {
    New,
    Established,
    Related,
    Closing,
    Closed,
}

/// Information about the process that owns a connection.
/// Informations sur le processus propriétaire d'une connexion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub path: Option<ExecutablePath>,
    pub cmdline: Option<String>,
}

/// System user owning a process.
/// Utilisateur système propriétaire d'un processus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemUser {
    pub uid: u32,
    pub name: String,
}

/// Snapshot of connection state for decision records.
/// Instantané de l'état d'une connexion pour les enregistrements de décision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionSnapshot {
    pub protocol: Protocol,
    pub source: SocketAddress,
    pub destination: SocketAddress,
    pub direction: Direction,
    pub process_name: Option<String>,
    pub process_path: Option<ExecutablePath>,
    pub user: Option<String>,
}

/// A network connection observed by the system.
/// Une connexion réseau observée par le système.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: ConnectionId,
    pub protocol: Protocol,
    pub source: SocketAddress,
    pub destination: SocketAddress,
    pub direction: Direction,
    pub state: ConnectionState,
    pub process: Option<ProcessInfo>,
    pub user: Option<SystemUser>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub started_at: DateTime<Utc>,
    pub verdict: ConnectionVerdict,
    pub matched_rule: Option<super::rule::RuleId>,
}

impl Connection {
    /// Create a snapshot of this connection's current state.
    /// Crée un instantané de l'état actuel de cette connexion.
    pub fn snapshot(&self) -> ConnectionSnapshot {
        ConnectionSnapshot {
            protocol: self.protocol,
            source: self.source.clone(),
            destination: self.destination.clone(),
            direction: self.direction,
            process_name: self.process.as_ref().map(|p| p.name.clone()),
            process_path: self.process.as_ref().and_then(|p| p.path.clone()),
            user: self.user.as_ref().map(|u| u.name.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::Port;

    fn test_connection() -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol: Protocol::Tcp,
            source: SocketAddress::new(
                "192.168.1.100".parse().unwrap(),
                Port::new(45000).unwrap(),
            ),
            destination: SocketAddress::new(
                "93.184.216.34".parse().unwrap(),
                Port::new(443).unwrap(),
            ),
            direction: Direction::Outbound,
            state: ConnectionState::New,
            process: Some(ProcessInfo {
                pid: 1234,
                name: "firefox".to_string(),
                path: Some(ExecutablePath::new("/usr/bin/firefox".into()).unwrap()),
                cmdline: Some("firefox https://example.com".to_string()),
            }),
            user: Some(SystemUser {
                uid: 1000,
                name: "seb".to_string(),
            }),
            bytes_sent: 0,
            bytes_received: 0,
            started_at: Utc::now(),
            verdict: ConnectionVerdict::Unknown,
            matched_rule: None,
        }
    }

    #[test]
    fn connection_snapshot_captures_process_info() {
        let conn = test_connection();
        let snap = conn.snapshot();
        assert_eq!(snap.process_name, Some("firefox".to_string()));
        assert_eq!(snap.protocol, Protocol::Tcp);
        assert_eq!(snap.direction, Direction::Outbound);
    }

    #[test]
    fn connection_snapshot_handles_missing_process() {
        let mut conn = test_connection();
        conn.process = None;
        conn.user = None;
        let snap = conn.snapshot();
        assert_eq!(snap.process_name, None);
        assert_eq!(snap.user, None);
    }
}
