//! Conversions entre DomainEvent / FirewallStatus (domaine) et leurs équivalents proto.
//! Conversions between DomainEvent / FirewallStatus (domain) and their proto equivalents.

use chrono::Utc;
use syswall_domain::events::{DomainEvent, FirewallStatus};
use syswall_proto::syswall::{DomainEventMessage, StatusResponse};

/// Convert a domain DomainEvent to a proto DomainEventMessage.
/// Convertit un DomainEvent du domaine en DomainEventMessage proto.
pub fn domain_event_to_proto(event: &DomainEvent) -> DomainEventMessage {
    let (event_type, payload_json) = match event {
        DomainEvent::ConnectionDetected(conn) => (
            "connection_detected",
            serde_json::to_string(conn).unwrap_or_default(),
        ),
        DomainEvent::ConnectionUpdated { id, state } => (
            "connection_updated",
            serde_json::json!({ "id": id, "state": state }).to_string(),
        ),
        DomainEvent::ConnectionClosed(id) => (
            "connection_closed",
            serde_json::to_string(id).unwrap_or_default(),
        ),
        DomainEvent::RuleCreated(rule) => (
            "rule_created",
            serde_json::to_string(rule).unwrap_or_default(),
        ),
        DomainEvent::RuleUpdated(rule) => (
            "rule_updated",
            serde_json::to_string(rule).unwrap_or_default(),
        ),
        DomainEvent::RuleDeleted(id) => (
            "rule_deleted",
            serde_json::to_string(id).unwrap_or_default(),
        ),
        DomainEvent::RuleMatched {
            connection_id,
            rule_id,
            verdict,
        } => (
            "rule_matched",
            serde_json::json!({
                "connection_id": connection_id,
                "rule_id": rule_id,
                "verdict": verdict,
            })
            .to_string(),
        ),
        DomainEvent::DecisionRequired(pd) => (
            "decision_required",
            serde_json::to_string(pd).unwrap_or_default(),
        ),
        DomainEvent::DecisionResolved(d) => (
            "decision_resolved",
            serde_json::to_string(d).unwrap_or_default(),
        ),
        DomainEvent::DecisionExpired(id) => (
            "decision_expired",
            serde_json::to_string(id).unwrap_or_default(),
        ),
        DomainEvent::FirewallStatusChanged(status) => (
            "firewall_status_changed",
            serde_json::to_string(status).unwrap_or_default(),
        ),
        DomainEvent::SystemError { message, severity } => (
            "system_error",
            serde_json::json!({ "message": message, "severity": severity }).to_string(),
        ),
        DomainEvent::AntilockoutTriggered { rolled_back_count } => (
            "antilockout_triggered",
            serde_json::json!({ "rolled_back_count": rolled_back_count }).to_string(),
        ),
    };

    DomainEventMessage {
        event_type: event_type.to_string(),
        payload_json,
        timestamp: Utc::now().to_rfc3339(),
    }
}

/// Convert a domain FirewallStatus to a proto StatusResponse.
/// Convertit un FirewallStatus du domaine en StatusResponse proto.
pub fn status_to_proto(status: &FirewallStatus) -> StatusResponse {
    StatusResponse {
        enabled: status.enabled,
        active_rules_count: status.active_rules_count,
        nftables_synced: status.nftables_synced,
        uptime_secs: status.uptime_secs,
        version: status.version.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn test_rule() -> Rule {
        Rule {
            id: RuleId::new(),
            name: "Test rule".to_string(),
            priority: RulePriority::new(10),
            enabled: true,
            criteria: RuleCriteria::default(),
            effect: RuleEffect::Allow,
            scope: RuleScope::Permanent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source: RuleSource::Manual,
        }
    }

    fn test_pending_decision() -> PendingDecision {
        PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: ConnectionSnapshot {
                protocol: Protocol::Tcp,
                source: SocketAddress::new("10.0.0.1".parse().unwrap(), Port::new(5000).unwrap()),
                destination: SocketAddress::new(
                    "8.8.8.8".parse().unwrap(),
                    Port::new(443).unwrap(),
                ),
                direction: Direction::Outbound,
                process_name: Some("curl".to_string()),
                process_path: None,
                user: Some("seb".to_string()),
                hostname: None,
            },
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            deduplication_key: "curl:8.8.8.8:443:tcp".to_string(),
            status: PendingDecisionStatus::Pending,
        }
    }

    #[test]
    fn domain_event_connection_detected() {
        let conn = Connection {
            id: ConnectionId::new(),
            protocol: Protocol::Tcp,
            source: SocketAddress::new("10.0.0.1".parse().unwrap(), Port::new(5000).unwrap()),
            destination: SocketAddress::new("8.8.8.8".parse().unwrap(), Port::new(443).unwrap()),
            direction: Direction::Outbound,
            state: ConnectionState::New,
            process: None,
            user: None,
            bytes_sent: 0,
            bytes_received: 0,
            started_at: Utc::now(),
            verdict: ConnectionVerdict::Unknown,
            matched_rule: None,
            remote_hostname: None,
        };
        let event = DomainEvent::ConnectionDetected(conn);
        let msg = domain_event_to_proto(&event);

        assert_eq!(msg.event_type, "connection_detected");
        assert!(!msg.payload_json.is_empty());
        assert!(!msg.timestamp.is_empty());
    }

    #[test]
    fn domain_event_rule_created() {
        let event = DomainEvent::RuleCreated(test_rule());
        let msg = domain_event_to_proto(&event);
        assert_eq!(msg.event_type, "rule_created");
    }

    #[test]
    fn domain_event_decision_required() {
        let event = DomainEvent::DecisionRequired(test_pending_decision());
        let msg = domain_event_to_proto(&event);
        assert_eq!(msg.event_type, "decision_required");
    }

    #[test]
    fn domain_event_system_error() {
        let event = DomainEvent::SystemError {
            message: "test error".to_string(),
            severity: Severity::Error,
        };
        let msg = domain_event_to_proto(&event);
        assert_eq!(msg.event_type, "system_error");
        assert!(msg.payload_json.contains("test error"));
    }

    #[test]
    fn status_to_proto_maps_all_fields() {
        let status = FirewallStatus {
            enabled: true,
            active_rules_count: 42,
            nftables_synced: true,
            uptime_secs: 3600,
            version: "0.1.0".to_string(),
        };
        let msg = status_to_proto(&status);

        assert!(msg.enabled);
        assert_eq!(msg.active_rules_count, 42);
        assert!(msg.nftables_synced);
        assert_eq!(msg.uptime_secs, 3600);
        assert_eq!(msg.version, "0.1.0");
    }
}
