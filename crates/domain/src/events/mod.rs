use serde::{Deserialize, Serialize};

use crate::entities::{
    Connection, ConnectionId, ConnectionState, ConnectionVerdict, Decision, PendingDecision,
    PendingDecisionId, Rule, RuleId, Severity,
};

/// All domain events flowing through the EventBus.
/// Tous les événements du domaine transitant par l'EventBus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    ConnectionDetected(Connection),
    ConnectionUpdated {
        id: ConnectionId,
        state: ConnectionState,
    },
    ConnectionClosed(ConnectionId),
    RuleCreated(Rule),
    RuleUpdated(Rule),
    RuleDeleted(RuleId),
    RuleMatched {
        connection_id: ConnectionId,
        rule_id: RuleId,
        verdict: ConnectionVerdict,
    },
    DecisionRequired(PendingDecision),
    DecisionResolved(Decision),
    DecisionExpired(PendingDecisionId),
    FirewallStatusChanged(FirewallStatus),
    SystemError {
        message: String,
        severity: Severity,
    },
}

/// Overall firewall status.
/// État global du pare-feu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallStatus {
    pub enabled: bool,
    pub active_rules_count: u32,
    pub nftables_synced: bool,
    pub uptime_secs: u64,
    pub version: String,
}

/// Pagination parameters for list queries.
/// Paramètres de pagination pour les requêtes de liste.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub offset: u64,
    pub limit: u64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 50,
        }
    }
}

/// Notification sent to the UI (non-blocking, informational).
/// Notification envoyée à l'interface utilisateur (non bloquante, informative).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub message: String,
    pub severity: Severity,
}

/// Default policy when no rules match.
/// Politique par défaut lorsqu'aucune règle ne correspond.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefaultPolicy {
    Ask,
    Allow,
    Block,
}
