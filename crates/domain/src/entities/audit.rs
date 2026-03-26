use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for an audit event.
/// Identifiant unique d'un événement d'audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(Uuid);

impl EventId {
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

/// Event severity level.
/// Niveau de sévérité d'un événement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

/// Category of audit event.
/// Catégorie d'événement d'audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventCategory {
    Connection,
    Rule,
    Decision,
    System,
    Config,
}

/// A journal entry in the audit log.
/// Une entrée dans le journal d'audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: EventId,
    pub timestamp: DateTime<Utc>,
    pub severity: Severity,
    pub category: EventCategory,
    pub description: String,
    pub metadata: HashMap<String, String>,
}

impl AuditEvent {
    /// Create a new audit event with the given severity, category, and description.
    /// Crée un nouvel événement d'audit avec la sévérité, catégorie et description données.
    pub fn new(
        severity: Severity,
        category: EventCategory,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: EventId::new(),
            timestamp: Utc::now(),
            severity,
            category,
            description: description.into(),
            metadata: HashMap::new(),
        }
    }

    /// Add a key-value metadata pair (builder pattern).
    /// Ajoute une paire clé-valeur de métadonnées (patron constructeur).
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_event_builder() {
        let event = AuditEvent::new(Severity::Info, EventCategory::Rule, "Rule created")
            .with_metadata("rule_id", "abc-123")
            .with_metadata("rule_name", "Block SSH");

        assert_eq!(event.severity, Severity::Info);
        assert_eq!(event.category, EventCategory::Rule);
        assert_eq!(event.metadata.get("rule_id").unwrap(), "abc-123");
        assert_eq!(event.metadata.len(), 2);
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Debug < Severity::Info);
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
        assert!(Severity::Error < Severity::Critical);
    }
}
