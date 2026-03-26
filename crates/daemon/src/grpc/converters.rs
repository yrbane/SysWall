/// Proto <-> domain type converters for gRPC services.
/// Convertisseurs de types proto <-> domaine pour les services gRPC.

use chrono::Utc;
use syswall_app::commands::{CreateRuleCommand, RespondToDecisionCommand};
use syswall_domain::entities::{
    AuditEvent, AuditStats, DecisionAction, DecisionGranularity, EventCategory, PendingDecision,
    PendingDecisionId, PendingDecisionStatus, Rule, RuleCriteria, RuleEffect, RuleScope,
    RuleSource, Severity,
};
use syswall_domain::errors::DomainError;
use syswall_domain::events::{DomainEvent, FirewallStatus};
use syswall_domain::ports::AuditFilters;
use syswall_proto::syswall::{
    AuditEventMessage, AuditLogRequest, CreateRuleRequest, DashboardStatsResponse,
    DecisionResponseRequest, DomainEventMessage, PendingDecisionMessage, RuleMessage,
    StatusResponse,
};

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

/// Map a DomainError to the appropriate tonic status code.
/// Convertit une DomainError vers le code de statut tonic approprié.
pub fn domain_error_to_status(e: DomainError) -> tonic::Status {
    match e {
        DomainError::Validation(msg) => tonic::Status::invalid_argument(msg),
        DomainError::NotFound(msg) => tonic::Status::not_found(msg),
        DomainError::AlreadyExists(msg) => tonic::Status::already_exists(msg),
        DomainError::Infrastructure(msg) => tonic::Status::internal(msg),
        DomainError::NotPermitted(msg) => tonic::Status::permission_denied(msg),
    }
}

// ---------------------------------------------------------------------------
// Rule conversions
// ---------------------------------------------------------------------------

/// Convert a domain Rule to a proto RuleMessage.
/// Convertit une Rule du domaine en RuleMessage proto.
pub fn rule_to_proto(rule: &Rule) -> RuleMessage {
    let effect = match rule.effect {
        RuleEffect::Allow => "allow",
        RuleEffect::Block => "block",
        RuleEffect::Ask => "ask",
        RuleEffect::Observe => "observe",
    };

    let source = match rule.source {
        RuleSource::Manual => "manual",
        RuleSource::AutoLearning => "auto_learning",
        RuleSource::Import => "import",
        RuleSource::System => "system",
    };

    RuleMessage {
        id: rule.id.as_uuid().to_string(),
        name: rule.name.clone(),
        priority: rule.priority.value(),
        enabled: rule.enabled,
        criteria_json: serde_json::to_string(&rule.criteria).unwrap_or_default(),
        effect: effect.to_string(),
        scope_json: serde_json::to_string(&rule.scope).unwrap_or_default(),
        source: source.to_string(),
        created_at: rule.created_at.to_rfc3339(),
        updated_at: rule.updated_at.to_rfc3339(),
    }
}

/// Convert a proto CreateRuleRequest to a domain CreateRuleCommand.
/// Convertit une CreateRuleRequest proto en CreateRuleCommand du domaine.
pub fn proto_to_create_rule_cmd(req: &CreateRuleRequest) -> Result<CreateRuleCommand, tonic::Status> {
    if req.name.is_empty() {
        return Err(tonic::Status::invalid_argument("Rule name must not be empty"));
    }

    let criteria: RuleCriteria = serde_json::from_str(&req.criteria_json)
        .map_err(|e| tonic::Status::invalid_argument(format!("Invalid criteria_json: {}", e)))?;

    let effect = parse_rule_effect(&req.effect)?;

    let scope: RuleScope = serde_json::from_str(&req.scope_json)
        .map_err(|e| tonic::Status::invalid_argument(format!("Invalid scope_json: {}", e)))?;

    let source = parse_rule_source(&req.source)?;

    Ok(CreateRuleCommand {
        name: req.name.clone(),
        priority: req.priority,
        criteria,
        effect,
        scope,
        source,
    })
}

// ---------------------------------------------------------------------------
// PendingDecision conversions
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// DomainEvent conversion
// ---------------------------------------------------------------------------

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
    };

    DomainEventMessage {
        event_type: event_type.to_string(),
        payload_json,
        timestamp: Utc::now().to_rfc3339(),
    }
}

// ---------------------------------------------------------------------------
// FirewallStatus conversion
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Audit conversions
// ---------------------------------------------------------------------------

/// Convert a domain AuditEvent to a proto AuditEventMessage.
/// Convertit un AuditEvent du domaine en AuditEventMessage proto.
pub fn audit_event_to_proto(event: &AuditEvent) -> AuditEventMessage {
    AuditEventMessage {
        id: event.id.as_uuid().to_string(),
        timestamp: event.timestamp.to_rfc3339(),
        severity: serde_json::to_string(&event.severity)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string(),
        category: serde_json::to_string(&event.category)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string(),
        description: event.description.clone(),
        metadata_json: serde_json::to_string(&event.metadata).unwrap_or_default(),
    }
}

/// Convert a proto AuditLogRequest to domain AuditFilters.
/// Convertit une AuditLogRequest proto en AuditFilters du domaine.
pub fn proto_to_audit_filters(req: &AuditLogRequest) -> AuditFilters {
    AuditFilters {
        severity: if req.severity.is_empty() {
            None
        } else {
            parse_severity(&req.severity).ok()
        },
        category: if req.category.is_empty() {
            None
        } else {
            parse_event_category(&req.category).ok()
        },
        search: if req.search.is_empty() {
            None
        } else {
            Some(req.search.clone())
        },
        from: if req.from.is_empty() {
            None
        } else {
            chrono::DateTime::parse_from_rfc3339(&req.from)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        },
        to: if req.to.is_empty() {
            None
        } else {
            chrono::DateTime::parse_from_rfc3339(&req.to)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        },
    }
}

/// Convert domain AuditStats to a proto DashboardStatsResponse.
/// Convertit des AuditStats du domaine en DashboardStatsResponse proto.
pub fn audit_stats_to_proto(stats: &AuditStats) -> DashboardStatsResponse {
    DashboardStatsResponse {
        total_events: stats.total,
        by_category: stats.by_category.clone(),
        by_severity: stats.by_severity.clone(),
    }
}

/// Parse a string to a Severity enum.
/// Analyse une chaîne vers l'énumération Severity.
fn parse_severity(s: &str) -> Result<Severity, tonic::Status> {
    match s {
        "Debug" => Ok(Severity::Debug),
        "Info" => Ok(Severity::Info),
        "Warning" => Ok(Severity::Warning),
        "Error" => Ok(Severity::Error),
        "Critical" => Ok(Severity::Critical),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown severity: '{}'. Expected: Debug, Info, Warning, Error, Critical",
            s
        ))),
    }
}

/// Parse a string to an EventCategory enum.
/// Analyse une chaîne vers l'énumération EventCategory.
fn parse_event_category(s: &str) -> Result<EventCategory, tonic::Status> {
    match s {
        "Connection" => Ok(EventCategory::Connection),
        "Rule" => Ok(EventCategory::Rule),
        "Decision" => Ok(EventCategory::Decision),
        "System" => Ok(EventCategory::System),
        "Config" => Ok(EventCategory::Config),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown category: '{}'. Expected: Connection, Rule, Decision, System, Config",
            s
        ))),
    }
}

// ---------------------------------------------------------------------------
// String <-> Enum parsing helpers
// ---------------------------------------------------------------------------

/// Parse a string to a RuleEffect enum.
/// Analyse une chaîne vers l'énumération RuleEffect.
fn parse_rule_effect(s: &str) -> Result<RuleEffect, tonic::Status> {
    match s {
        "allow" => Ok(RuleEffect::Allow),
        "block" => Ok(RuleEffect::Block),
        "ask" => Ok(RuleEffect::Ask),
        "observe" => Ok(RuleEffect::Observe),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown rule effect: '{}'. Expected: allow, block, ask, observe",
            s
        ))),
    }
}

/// Parse a string to a RuleSource enum.
/// Analyse une chaîne vers l'énumération RuleSource.
fn parse_rule_source(s: &str) -> Result<RuleSource, tonic::Status> {
    match s {
        "manual" => Ok(RuleSource::Manual),
        "auto_learning" => Ok(RuleSource::AutoLearning),
        "import" => Ok(RuleSource::Import),
        "system" => Ok(RuleSource::System),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown rule source: '{}'. Expected: manual, auto_learning, import, system",
            s
        ))),
    }
}

/// Parse a string to a DecisionAction enum.
/// Analyse une chaîne vers l'énumération DecisionAction.
fn parse_decision_action(s: &str) -> Result<DecisionAction, tonic::Status> {
    match s {
        "allow_once" => Ok(DecisionAction::AllowOnce),
        "block_once" => Ok(DecisionAction::BlockOnce),
        "always_allow" => Ok(DecisionAction::AlwaysAllow),
        "always_block" => Ok(DecisionAction::AlwaysBlock),
        "create_rule" => Ok(DecisionAction::CreateRule),
        "ignore" => Ok(DecisionAction::Ignore),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown decision action: '{}'. Expected: allow_once, block_once, always_allow, always_block, create_rule, ignore",
            s
        ))),
    }
}

/// Parse a string to a DecisionGranularity enum.
/// Analyse une chaîne vers l'énumération DecisionGranularity.
fn parse_decision_granularity(s: &str) -> Result<DecisionGranularity, tonic::Status> {
    match s {
        "app_only" => Ok(DecisionGranularity::AppOnly),
        "app_and_ip" => Ok(DecisionGranularity::AppAndIp),
        "app_and_port" => Ok(DecisionGranularity::AppAndPort),
        "app_and_domain" => Ok(DecisionGranularity::AppAndDomain),
        "app_and_protocol" => Ok(DecisionGranularity::AppAndProtocol),
        "full" => Ok(DecisionGranularity::Full),
        _ => Err(tonic::Status::invalid_argument(format!(
            "Unknown decision granularity: '{}'. Expected: app_only, app_and_ip, app_and_port, app_and_domain, app_and_protocol, full",
            s
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    fn test_snapshot() -> ConnectionSnapshot {
        ConnectionSnapshot {
            protocol: Protocol::Tcp,
            source: SocketAddress::new("10.0.0.1".parse().unwrap(), Port::new(5000).unwrap()),
            destination: SocketAddress::new("8.8.8.8".parse().unwrap(), Port::new(443).unwrap()),
            direction: Direction::Outbound,
            process_name: Some("curl".to_string()),
            process_path: None,
            user: Some("seb".to_string()),
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
    fn rule_to_proto_all_fields() {
        let rule = test_rule();
        let msg = rule_to_proto(&rule);

        assert_eq!(msg.id, rule.id.as_uuid().to_string());
        assert_eq!(msg.name, "Test rule");
        assert_eq!(msg.priority, 10);
        assert!(msg.enabled);
        assert_eq!(msg.effect, "allow");
        assert_eq!(msg.source, "manual");
        assert!(!msg.criteria_json.is_empty());
        assert!(!msg.scope_json.is_empty());
        assert!(!msg.created_at.is_empty());
        assert!(!msg.updated_at.is_empty());
    }

    #[test]
    fn create_rule_request_valid() {
        let req = CreateRuleRequest {
            name: "Block SSH".to_string(),
            priority: 5,
            criteria_json: serde_json::to_string(&RuleCriteria::default()).unwrap(),
            effect: "block".to_string(),
            scope_json: serde_json::to_string(&RuleScope::Permanent).unwrap(),
            source: "manual".to_string(),
        };

        let cmd = proto_to_create_rule_cmd(&req).unwrap();
        assert_eq!(cmd.name, "Block SSH");
        assert_eq!(cmd.priority, 5);
        assert_eq!(cmd.effect, RuleEffect::Block);
        assert_eq!(cmd.source, RuleSource::Manual);
    }

    #[test]
    fn create_rule_request_invalid_json() {
        let req = CreateRuleRequest {
            name: "Bad".to_string(),
            priority: 1,
            criteria_json: "not json".to_string(),
            effect: "allow".to_string(),
            scope_json: "\"Permanent\"".to_string(),
            source: "manual".to_string(),
        };

        let err = proto_to_create_rule_cmd(&req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn create_rule_request_empty_name() {
        let req = CreateRuleRequest {
            name: String::new(),
            priority: 1,
            criteria_json: "{}".to_string(),
            effect: "allow".to_string(),
            scope_json: "\"Permanent\"".to_string(),
            source: "manual".to_string(),
        };

        let err = proto_to_create_rule_cmd(&req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
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

    #[test]
    fn domain_event_connection_detected() {
        use syswall_domain::entities::*;

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

    #[test]
    fn error_mapping_validation() {
        let status = domain_error_to_status(DomainError::Validation("bad".to_string()));
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn error_mapping_not_found() {
        let status = domain_error_to_status(DomainError::NotFound("missing".to_string()));
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[test]
    fn error_mapping_already_exists() {
        let status = domain_error_to_status(DomainError::AlreadyExists("dup".to_string()));
        assert_eq!(status.code(), tonic::Code::AlreadyExists);
    }

    #[test]
    fn error_mapping_infrastructure() {
        let status = domain_error_to_status(DomainError::Infrastructure("db down".to_string()));
        assert_eq!(status.code(), tonic::Code::Internal);
    }

    #[test]
    fn error_mapping_not_permitted() {
        let status = domain_error_to_status(DomainError::NotPermitted("nope".to_string()));
        assert_eq!(status.code(), tonic::Code::PermissionDenied);
    }

    #[test]
    fn audit_event_to_proto_all_fields() {
        use syswall_domain::entities::{AuditEvent, EventCategory, Severity};
        let event = AuditEvent::new(Severity::Warning, EventCategory::Rule, "Test audit event")
            .with_metadata("key", "value");
        let msg = audit_event_to_proto(&event);

        assert_eq!(msg.id, event.id.as_uuid().to_string());
        assert_eq!(msg.severity, "Warning");
        assert_eq!(msg.category, "Rule");
        assert_eq!(msg.description, "Test audit event");
        assert!(msg.metadata_json.contains("key"));
    }

    #[test]
    fn proto_to_audit_filters_empty_strings() {
        let req = AuditLogRequest {
            severity: String::new(),
            category: String::new(),
            search: String::new(),
            from: String::new(),
            to: String::new(),
            offset: 0,
            limit: 50,
        };
        let filters = proto_to_audit_filters(&req);
        assert!(filters.severity.is_none());
        assert!(filters.category.is_none());
        assert!(filters.search.is_none());
        assert!(filters.from.is_none());
        assert!(filters.to.is_none());
    }

    #[test]
    fn proto_to_audit_filters_with_values() {
        let req = AuditLogRequest {
            severity: "Error".to_string(),
            category: "System".to_string(),
            search: "nftables".to_string(),
            from: "2026-01-01T00:00:00Z".to_string(),
            to: "2026-12-31T23:59:59Z".to_string(),
            offset: 0,
            limit: 50,
        };
        let filters = proto_to_audit_filters(&req);
        assert_eq!(filters.severity, Some(Severity::Error));
        assert_eq!(filters.category, Some(EventCategory::System));
        assert_eq!(filters.search, Some("nftables".to_string()));
        assert!(filters.from.is_some());
        assert!(filters.to.is_some());
    }

    #[test]
    fn audit_stats_to_proto_maps_all_fields() {
        use std::collections::HashMap;
        let stats = AuditStats {
            total: 42,
            by_category: {
                let mut m = HashMap::new();
                m.insert("Rule".to_string(), 20);
                m.insert("System".to_string(), 22);
                m
            },
            by_severity: {
                let mut m = HashMap::new();
                m.insert("Info".to_string(), 30);
                m.insert("Error".to_string(), 12);
                m
            },
        };
        let msg = audit_stats_to_proto(&stats);
        assert_eq!(msg.total_events, 42);
        assert_eq!(*msg.by_category.get("Rule").unwrap(), 20);
        assert_eq!(*msg.by_severity.get("Error").unwrap(), 12);
    }

    #[test]
    fn parse_all_rule_effects() {
        assert_eq!(parse_rule_effect("allow").unwrap(), RuleEffect::Allow);
        assert_eq!(parse_rule_effect("block").unwrap(), RuleEffect::Block);
        assert_eq!(parse_rule_effect("ask").unwrap(), RuleEffect::Ask);
        assert_eq!(parse_rule_effect("observe").unwrap(), RuleEffect::Observe);
        assert!(parse_rule_effect("bad").is_err());
    }

    #[test]
    fn parse_all_rule_sources() {
        assert_eq!(parse_rule_source("manual").unwrap(), RuleSource::Manual);
        assert_eq!(parse_rule_source("auto_learning").unwrap(), RuleSource::AutoLearning);
        assert_eq!(parse_rule_source("import").unwrap(), RuleSource::Import);
        assert_eq!(parse_rule_source("system").unwrap(), RuleSource::System);
        assert!(parse_rule_source("bad").is_err());
    }

    #[test]
    fn parse_all_decision_actions() {
        assert_eq!(parse_decision_action("allow_once").unwrap(), DecisionAction::AllowOnce);
        assert_eq!(parse_decision_action("block_once").unwrap(), DecisionAction::BlockOnce);
        assert_eq!(parse_decision_action("always_allow").unwrap(), DecisionAction::AlwaysAllow);
        assert_eq!(parse_decision_action("always_block").unwrap(), DecisionAction::AlwaysBlock);
        assert_eq!(parse_decision_action("create_rule").unwrap(), DecisionAction::CreateRule);
        assert_eq!(parse_decision_action("ignore").unwrap(), DecisionAction::Ignore);
        assert!(parse_decision_action("bad").is_err());
    }

    #[test]
    fn parse_all_decision_granularities() {
        assert_eq!(parse_decision_granularity("app_only").unwrap(), DecisionGranularity::AppOnly);
        assert_eq!(parse_decision_granularity("app_and_ip").unwrap(), DecisionGranularity::AppAndIp);
        assert_eq!(parse_decision_granularity("app_and_port").unwrap(), DecisionGranularity::AppAndPort);
        assert_eq!(parse_decision_granularity("app_and_domain").unwrap(), DecisionGranularity::AppAndDomain);
        assert_eq!(parse_decision_granularity("app_and_protocol").unwrap(), DecisionGranularity::AppAndProtocol);
        assert_eq!(parse_decision_granularity("full").unwrap(), DecisionGranularity::Full);
        assert!(parse_decision_granularity("bad").is_err());
    }
}
