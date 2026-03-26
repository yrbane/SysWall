use async_trait::async_trait;

use crate::entities::{
    AuditEvent, AuditStats, Decision, EventCategory, PendingDecision, PendingDecisionId, Rule,
    RuleEffect, RuleId, RuleSource, Severity,
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

    /// Append multiple events in a single batch (transactional).
    /// Ajoute plusieurs événements en un seul lot (transactionnel).
    async fn append_batch(&self, events: &[AuditEvent]) -> Result<(), DomainError>;

    /// Delete all events with timestamp before the given cutoff. Returns count deleted.
    /// Supprime tous les événements antérieurs au seuil donné. Retourne le nombre supprimé.
    async fn delete_before(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, DomainError>;

    /// Get aggregated statistics for events in the given time range.
    /// Obtient les statistiques agrégées pour les événements dans la plage temporelle donnée.
    async fn get_stats(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<AuditStats, DomainError>;
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
