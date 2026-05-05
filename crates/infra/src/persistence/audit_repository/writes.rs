//! Côté écriture du dépôt d'audit : insertion, batch, suppression.
//! Audit write side: insertion, batch, deletion.

use syswall_domain::entities::AuditEvent;
use syswall_domain::errors::DomainError;

use super::SqliteAuditRepository;

impl SqliteAuditRepository {
    /// Insère un événement d'audit.
    /// Insert a single audit event.
    pub(super) async fn run_append(
        &self,
        event: AuditEvent,
    ) -> Result<(), DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                conn.execute(
                    "INSERT INTO audit_events (id, timestamp, severity, category, description, metadata_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        event.id.as_uuid().to_string(),
                        event.timestamp.to_rfc3339(),
                        serde_json::to_string(&event.severity).expect("Severity sérialisable / serializable").trim_matches('"'),
                        serde_json::to_string(&event.category).expect("EventCategory sérialisable / serializable").trim_matches('"'),
                        event.description,
                        serde_json::to_string(&event.metadata).expect("Metadata sérialisable / serializable"),
                    ],
                )
                .map_err(|e| {
                    DomainError::Infrastructure(format!("Failed to append audit event: {}", e))
                })?;
                Ok(())
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    /// Insère un lot d'événements d'audit dans une transaction.
    /// Insert a batch of audit events in a single transaction.
    pub(super) async fn run_append_batch(
        &self,
        events: Vec<AuditEvent>,
    ) -> Result<(), DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let tx = conn.unchecked_transaction().map_err(|e| {
                    DomainError::Infrastructure(format!("Failed to start transaction: {}", e))
                })?;
                {
                    let mut stmt = tx
                        .prepare_cached(
                            "INSERT INTO audit_events (id, timestamp, severity, category, description, metadata_json)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        )
                        .map_err(|e| {
                            DomainError::Infrastructure(format!("Failed to prepare statement: {}", e))
                        })?;
                    for event in &events {
                        stmt.execute(rusqlite::params![
                            event.id.as_uuid().to_string(),
                            event.timestamp.to_rfc3339(),
                            serde_json::to_string(&event.severity)
                                .expect("Severity sérialisable / serializable")
                                .trim_matches('"'),
                            serde_json::to_string(&event.category)
                                .expect("EventCategory sérialisable / serializable")
                                .trim_matches('"'),
                            event.description,
                            serde_json::to_string(&event.metadata).expect("Metadata sérialisable / serializable"),
                        ])
                        .map_err(|e| {
                            DomainError::Infrastructure(format!(
                                "Failed to insert batch event: {}",
                                e
                            ))
                        })?;
                    }
                }
                tx.commit().map_err(|e| {
                    DomainError::Infrastructure(format!("Failed to commit batch: {}", e))
                })?;
                Ok(())
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }

    /// Supprime les événements antérieurs à une date.
    /// Delete events older than the given timestamp.
    pub(super) async fn run_delete_before(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, DomainError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.with_writer(|conn| {
                let deleted = conn
                    .execute(
                        "DELETE FROM audit_events WHERE timestamp < ?1",
                        rusqlite::params![before.to_rfc3339()],
                    )
                    .map_err(|e| {
                        DomainError::Infrastructure(format!(
                            "Failed to delete old audit events: {}",
                            e
                        ))
                    })?;
                Ok(deleted as u64)
            })
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("Spawn blocking failed: {}", e)))?
    }
}
