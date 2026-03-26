use async_trait::async_trait;

use crate::entities::{
    AuditEvent, Decision, EventCategory, PendingDecision, PendingDecisionId, Rule, RuleEffect,
    RuleId, RuleSource, Severity,
};
use crate::errors::DomainError;
use crate::events::Pagination;
use crate::value_objects::Direction;

/// Filters for querying rules.
/// Filtres pour la recherche de règles.
#[derive(Debug, Clone, Default)]
pub struct RuleFilters {
    pub source: Option<RuleSource>,
    pub effect: Option<RuleEffect>,
    pub enabled: Option<bool>,
    pub direction: Option<Direction>,
    pub search: Option<String>,
}

/// Filters for querying audit events.
/// Filtres pour la recherche d'événements d'audit.
#[derive(Debug, Clone, Default)]
pub struct AuditFilters {
    pub severity: Option<Severity>,
    pub category: Option<EventCategory>,
    pub search: Option<String>,
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
}

/// Repository for firewall rules.
/// Dépôt pour les règles de pare-feu.
#[async_trait]
pub trait RuleRepository: Send + Sync {
    async fn save(&self, rule: &Rule) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: &RuleId) -> Result<Option<Rule>, DomainError>;
    async fn find_all(
        &self,
        filters: &RuleFilters,
        pagination: &Pagination,
    ) -> Result<Vec<Rule>, DomainError>;
    async fn delete(&self, id: &RuleId) -> Result<(), DomainError>;
    async fn list_enabled_ordered(&self) -> Result<Vec<Rule>, DomainError>;
}

/// Repository for audit events.
/// Dépôt pour les événements d'audit.
#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn append(&self, event: &AuditEvent) -> Result<(), DomainError>;
    async fn query(
        &self,
        filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError>;
    async fn count(&self, filters: &AuditFilters) -> Result<u64, DomainError>;
}

/// Repository for resolved decisions.
/// Dépôt pour les décisions résolues.
#[async_trait]
pub trait DecisionRepository: Send + Sync {
    async fn save(&self, decision: &Decision) -> Result<(), DomainError>;
}

/// Repository for pending decisions awaiting user response.
/// Dépôt pour les décisions en attente de réponse utilisateur.
#[async_trait]
pub trait PendingDecisionRepository: Send + Sync {
    async fn create(&self, request: &PendingDecision) -> Result<(), DomainError>;
    async fn list_pending(&self) -> Result<Vec<PendingDecision>, DomainError>;
    async fn resolve(&self, id: &PendingDecisionId) -> Result<(), DomainError>;
    async fn expire_overdue(&self) -> Result<Vec<PendingDecision>, DomainError>;
    async fn find_by_dedup_key(&self, key: &str) -> Result<Option<PendingDecision>, DomainError>;
}
