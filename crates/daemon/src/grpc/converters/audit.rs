//! Conversions entre AuditEvent / AuditStats (domaine) et leurs équivalents proto.
//! Conversions between AuditEvent / AuditStats (domain) and their proto equivalents.

use syswall_domain::entities::{AuditEvent, AuditStats};
use syswall_domain::ports::AuditFilters;
use syswall_proto::syswall::{AuditEventMessage, AuditLogRequest, DashboardStatsResponse};

use super::parsers::{parse_event_category, parse_severity};

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

#[cfg(test)]
mod tests {
    use super::*;
    use syswall_domain::entities::{AuditEvent, EventCategory, Severity};
    use syswall_proto::syswall::AuditLogRequest;

    #[test]
    fn audit_event_to_proto_all_fields() {
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
}
