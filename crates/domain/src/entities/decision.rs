use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::connection::ConnectionSnapshot;
use super::rule::RuleId;

/// Unique identifier for a pending decision.
/// Identifiant unique d'une décision en attente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PendingDecisionId(Uuid);

impl Default for PendingDecisionId {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingDecisionId {
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

/// Status of a pending decision.
/// Statut d'une décision en attente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PendingDecisionStatus {
    Pending,
    Resolved,
    Expired,
    Cancelled,
}

/// A decision request waiting for user response.
/// Une demande de décision en attente de la réponse de l'utilisateur.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDecision {
    pub id: PendingDecisionId,
    pub connection_snapshot: ConnectionSnapshot,
    pub requested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub deduplication_key: String,
    pub status: PendingDecisionStatus,
}

impl PendingDecision {
    /// Check if the decision request has expired.
    /// Vérifie si la demande de décision a expiré.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if the decision is still pending.
    /// Vérifie si la décision est toujours en attente.
    pub fn is_pending(&self) -> bool {
        self.status == PendingDecisionStatus::Pending
    }
}

/// Unique identifier for a resolved decision.
/// Identifiant unique d'une décision résolue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecisionId(Uuid);

impl Default for DecisionId {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionId {
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

/// What the user decided to do.
/// Ce que l'utilisateur a décidé de faire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionAction {
    AllowOnce,
    BlockOnce,
    AlwaysAllow,
    AlwaysBlock,
    CreateRule,
    Ignore,
}

/// Granularity of the rule generated from a decision.
/// Granularité de la règle générée à partir d'une décision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionGranularity {
    AppOnly,
    AppAndIp,
    AppAndPort,
    AppAndDomain,
    AppAndProtocol,
    Full,
}

/// A resolved auto-learning decision.
/// Une décision d'auto-apprentissage résolue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: DecisionId,
    pub pending_decision_id: PendingDecisionId,
    pub connection_snapshot: ConnectionSnapshot,
    pub action: DecisionAction,
    pub granularity: DecisionGranularity,
    pub decided_at: DateTime<Utc>,
    pub generated_rule: Option<RuleId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::*;

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

    #[test]
    fn pending_decision_expired_when_past_deadline() {
        let pd = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            requested_at: Utc::now() - chrono::Duration::minutes(10),
            expires_at: Utc::now() - chrono::Duration::minutes(1),
            deduplication_key: "curl:8.8.8.8:443:tcp".to_string(),
            status: PendingDecisionStatus::Pending,
        };
        assert!(pd.is_expired());
        assert!(pd.is_pending());
    }

    #[test]
    fn pending_decision_not_expired_when_future_deadline() {
        let pd = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            deduplication_key: "curl:8.8.8.8:443:tcp".to_string(),
            status: PendingDecisionStatus::Pending,
        };
        assert!(!pd.is_expired());
    }

    #[test]
    fn resolved_decision_not_pending() {
        let pd = PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            deduplication_key: "test".to_string(),
            status: PendingDecisionStatus::Resolved,
        };
        assert!(!pd.is_pending());
    }
}
