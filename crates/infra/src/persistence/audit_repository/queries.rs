//! Côté requête du dépôt d'audit : filtrage, statistiques, lecture.
//! Audit query side: filtering, stats, retrieval.

use std::collections::HashMap;

use syswall_domain::entities::{AuditEvent, AuditStats, EventCategory, EventId, Severity};
use syswall_domain::errors::DomainError;
use syswall_domain::events::Pagination;
use syswall_domain::ports::AuditFilters;

use super::SqliteAuditRepository;

impl SqliteAuditRepository {
    /// Convertit une ligne SQLite en AuditEvent.
    /// Convert a SQLite row to AuditEvent.
    pub(super) fn row_to_audit_event(row: &rusqlite::Row) -> Result<AuditEvent, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let timestamp_str: String = row.get(1)?;
        let severity_str: String = row.get(2)?;
        let category_str: String = row.get(3)?;
        let description: String = row.get(4)?;
        let metadata_json: String = row.get(5)?;

        let severity = serde_json::from_str::<Severity>(&format!("\"{}\"", severity_str))
            .unwrap_or(Severity::Info);
        let category = serde_json::from_str::<EventCategory>(&format!("\"{}\"", category_str))
            .unwrap_or(EventCategory::System);

        Ok(AuditEvent {
            id: EventId::from_uuid(
                id_str.parse().expect("UUID stocké par notre code / stored by our code"),
            ),
            timestamp: timestamp_str
                .parse()
                .expect("RFC3339 stocké par notre code / stored by our code"),
            severity,
            category,
            description,
            metadata: serde_json::from_str(&metadata_json).unwrap_or_default(),
        })
    }

    /// Construit la clause WHERE et collecte les paramètres depuis AuditFilters.
    /// Build the WHERE clause and collect parameters from AuditFilters.
    pub(super) fn build_where_clause(
        filters: &AuditFilters,
    ) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
        let mut clauses: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref severity) = filters.severity {
            clauses.push("severity = ?".to_string());
            params.push(Box::new(
                serde_json::to_string(severity)
                    .expect("Severity sérialisable / serializable")
                    .trim_matches('"')
                    .to_string(),
            ));
        }
        if let Some(ref category) = filters.category {
            clauses.push("category = ?".to_string());
            params.push(Box::new(
                serde_json::to_string(category)
                    .expect("EventCategory sérialisable / serializable")
                    .trim_matches('"')
                    .to_string(),
            ));
        }
        if let Some(ref search) = filters.search {
            clauses.push("description LIKE ?".to_string());
            params.push(Box::new(format!("%{}%", search)));
        }
        if let Some(ref from) = filters.from {
            clauses.push("timestamp >= ?".to_string());
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(ref to) = filters.to {
            clauses.push("timestamp <= ?".to_string());
            params.push(Box::new(to.to_rfc3339()));
        }

        let where_sql = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };

        (where_sql, params)
    }

    /// Requête paginée avec filtres.
    /// Paginated query with filters.
    pub(super) async fn run_query(
        &self,
        filters: AuditFilters,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<AuditEvent>, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let (where_sql, mut params) = Self::build_where_clause(&filters);
                let sql = format!(
                    "SELECT id, timestamp, severity, category, description, metadata_json \
                     FROM audit_events{} ORDER BY timestamp DESC LIMIT ? OFFSET ?",
                    where_sql
                );
                params.push(Box::new(limit as i64));
                params.push(Box::new(offset as i64));

                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();

                let mut stmt = conn
                    .prepare(&sql)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                let events = stmt
                    .query_map(param_refs.as_slice(), Self::row_to_audit_event)
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(events)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    /// Compte les événements correspondant aux filtres.
    /// Count events matching the filters.
    pub(super) async fn run_count(
        &self,
        filters: AuditFilters,
    ) -> Result<u64, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let (where_sql, params) = Self::build_where_clause(&filters);
                let sql = format!("SELECT COUNT(*) FROM audit_events{}", where_sql);

                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();

                let count: i64 = conn
                    .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                Ok(count as u64)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    /// Calcule les statistiques agrégées pour une plage temporelle.
    /// Compute aggregated stats for a time range.
    pub(super) async fn run_get_stats(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<AuditStats, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let from_str = from.to_rfc3339();
                let to_str = to.to_rfc3339();

                // Total count in range
                let total: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM audit_events WHERE timestamp >= ?1 AND timestamp <= ?2",
                        rusqlite::params![from_str, to_str],
                        |row| row.get(0),
                    )
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

                // Counts per category
                let mut stmt = conn
                    .prepare(
                        "SELECT category, COUNT(*) FROM audit_events \
                         WHERE timestamp >= ?1 AND timestamp <= ?2 \
                         GROUP BY category",
                    )
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                let by_category: HashMap<String, u64> = stmt
                    .query_map(rusqlite::params![from_str, to_str], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
                    })
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                // Counts per severity
                let mut stmt = conn
                    .prepare(
                        "SELECT severity, COUNT(*) FROM audit_events \
                         WHERE timestamp >= ?1 AND timestamp <= ?2 \
                         GROUP BY severity",
                    )
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
                let by_severity: HashMap<String, u64> = stmt
                    .query_map(rusqlite::params![from_str, to_str], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
                    })
                    .map_err(|e| DomainError::Infrastructure(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(AuditStats {
                    total: total as u64,
                    by_category,
                    by_severity,
                })
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }
}
