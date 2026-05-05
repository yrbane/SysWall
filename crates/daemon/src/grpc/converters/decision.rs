//! Conversions entre PendingDecision (domaine) et son équivalent proto.
//! Conversions between PendingDecision (domain) and its proto equivalent.

use syswall_app::commands::RespondToDecisionCommand;
use syswall_domain::entities::{PendingDecision, PendingDecisionId, PendingDecisionStatus};
use syswall_proto::syswall::{DecisionResponseRequest, PendingDecisionMessage};

use super::parsers::{parse_decision_action, parse_decision_granularity};

/// Convert a domain PendingDecision to a proto PendingDecisionMessage.
/// Convertit une PendingDecision du domaine en PendingDecisionMessage proto.
pub fn pending_decision_to_proto(pd: &PendingDecision) -> PendingDecisionMessage {
    let status = match pd.status {
        PendingDecisionStatus::Pending => "pending",
        PendingDecisionStatus::Resolved => "resolved",
        PendingDecisionStatus::Expired => "expired",
        PendingDecisionStatus::Cancelled => "cancelled",
    };

    PendingDecisionMessage {
        id: pd.id.as_uuid().to_string(),
        snapshot_json: serde_json::to_string(&pd.connection_snapshot).unwrap_or_default(),
        requested_at: pd.requested_at.to_rfc3339(),
        expires_at: pd.expires_at.to_rfc3339(),
        status: status.to_string(),
    }
}

/// Convert a proto DecisionResponseRequest to a domain RespondToDecisionCommand.
/// Convertit une DecisionResponseRequest proto en RespondToDecisionCommand du domaine.
// tonic::Status est imposé par l'API gRPC ; taille inévitable.
#[allow(clippy::result_large_err)]
pub fn proto_to_respond_cmd(
    req: &DecisionResponseRequest,
) -> Result<RespondToDecisionCommand, tonic::Status> {
    let uuid = uuid::Uuid::parse_str(&req.pending_decision_id)
        .map_err(|e| tonic::Status::invalid_argument(format!("Invalid UUID: {}", e)))?;

    let action = parse_decision_action(&req.action)?;
    let granularity = parse_decision_granularity(&req.granularity)?;

    Ok(RespondToDecisionCommand {
        pending_decision_id: PendingDecisionId::from_uuid(uuid),
        action,
        granularity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

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

    fn test_pending_decision() -> PendingDecision {
        PendingDecision {
            id: PendingDecisionId::new(),
            connection_snapshot: test_snapshot(),
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            deduplication_key: "curl:8.8.8.8:443:tcp".to_string(),
            status: PendingDecisionStatus::Pending,
        }
    }

    #[test]
    fn pending_decision_to_proto_all_fields() {
        let pd = test_pending_decision();
        let msg = pending_decision_to_proto(&pd);

        assert_eq!(msg.id, pd.id.as_uuid().to_string());
        assert_eq!(msg.status, "pending");
        assert!(!msg.snapshot_json.is_empty());
        assert!(!msg.requested_at.is_empty());
        assert!(!msg.expires_at.is_empty());
    }

    #[test]
    fn respond_cmd_valid() {
        let pd_id = PendingDecisionId::new();
        let req = DecisionResponseRequest {
            pending_decision_id: pd_id.as_uuid().to_string(),
            action: "allow_once".to_string(),
            granularity: "app_only".to_string(),
        };

        let cmd = proto_to_respond_cmd(&req).unwrap();
        assert_eq!(cmd.pending_decision_id, pd_id);
        assert_eq!(cmd.action, DecisionAction::AllowOnce);
        assert_eq!(cmd.granularity, DecisionGranularity::AppOnly);
    }

    #[test]
    fn respond_cmd_invalid_uuid() {
        let req = DecisionResponseRequest {
            pending_decision_id: "not-a-uuid".to_string(),
            action: "allow_once".to_string(),
            granularity: "app_only".to_string(),
        };

        let err = proto_to_respond_cmd(&req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn respond_cmd_invalid_action() {
        let req = DecisionResponseRequest {
            pending_decision_id: uuid::Uuid::new_v4().to_string(),
            action: "nope".to_string(),
            granularity: "app_only".to_string(),
        };

        let err = proto_to_respond_cmd(&req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }
}
