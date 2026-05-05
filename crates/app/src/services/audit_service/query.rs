//! Audit read side: filtering, stats, export.
//! Cote lecture audit : filtrage, statistiques, export.

use std::sync::Arc;

use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::entities::{AuditEvent, AuditStats};
use syswall_domain::ports::{AuditFilters, AuditRepository};

use super::{AuditService, ExportFormat};

impl AuditService {
    /// Query audit events with optional filters and pagination.
    /// Interroge les événements d'audit avec filtres et pagination optionnels.
    pub async fn query_events(
        &self,
        filters: &AuditFilters,
        pagination: &Pagination,
    ) -> Result<Vec<AuditEvent>, DomainError> {
        self.audit_repo.query(filters, pagination).await
    }

    /// Count audit events matching the given filters.
    /// Compte les événements d'audit correspondant aux filtres donnés.
    pub async fn count_events(&self, filters: &AuditFilters) -> Result<u64, DomainError> {
        self.audit_repo.count(filters).await
    }

    /// Get aggregated statistics for a time range.
    /// Obtient les statistiques agrégées pour une plage temporelle.
    pub async fn get_stats(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<AuditStats, DomainError> {
        self.audit_repo.get_stats(from, to).await
    }

    /// Export audit events matching the given filters as bytes in the specified format.
    /// Exporte les événements d'audit correspondant aux filtres donnés en octets dans le format spécifié.
    pub async fn export_events(
        &self,
        filters: &AuditFilters,
        _format: ExportFormat,
    ) -> Result<Vec<u8>, DomainError> {
        // Hard limit of 100,000 events to prevent unbounded memory usage
        let pagination = Pagination {
            offset: 0,
            limit: 100_000,
        };
        let events = self.audit_repo.query(filters, &pagination).await?;

        serde_json::to_vec_pretty(&events)
            .map_err(|e| DomainError::Infrastructure(format!("JSON serialization failed: {}", e)))
    }

    /// Get a reference to the underlying audit repository.
    /// Obtient une référence vers le dépôt d'audit sous-jacent.
    pub fn repo(&self) -> &Arc<dyn AuditRepository> {
        &self.audit_repo
    }
}
