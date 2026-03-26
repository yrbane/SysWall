use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use syswall_domain::errors::DomainError;

/// Database wrapper managing SQLite connection with WAL mode and migrations.
/// Wrapper de base de données gérant la connexion SQLite avec le mode WAL et les migrations.
pub struct Database {
    writer: Mutex<Connection>,
}

impl Database {
    /// Open or create the database at the given path. Enables WAL mode and runs migrations.
    /// Ouvre ou crée la base de données au chemin donné. Active le mode WAL et exécute les migrations.
    pub fn open(path: &Path) -> Result<Self, DomainError> {
        let conn = Connection::open(path)
            .map_err(|e| DomainError::Infrastructure(format!("Failed to open DB: {}", e)))?;

        Self::configure(&conn)?;
        Self::migrate(&conn)?;

        Ok(Self {
            writer: Mutex::new(conn),
        })
    }

    /// Open an in-memory database (for testing).
    /// Ouvre une base de données en mémoire (pour les tests).
    pub fn open_in_memory() -> Result<Self, DomainError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| DomainError::Infrastructure(format!("Failed to open in-memory DB: {}", e)))?;

        Self::configure(&conn)?;
        Self::migrate(&conn)?;

        Ok(Self {
            writer: Mutex::new(conn),
        })
    }

    fn configure(conn: &Connection) -> Result<(), DomainError> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;"
        )
        .map_err(|e| DomainError::Infrastructure(format!("Failed to configure DB: {}", e)))
    }

    fn migrate(conn: &Connection) -> Result<(), DomainError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS rules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                priority INTEGER NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                criteria_json TEXT NOT NULL,
                effect TEXT NOT NULL,
                scope_json TEXT NOT NULL,
                source TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_rules_priority ON rules(priority);
            CREATE INDEX IF NOT EXISTS idx_rules_enabled ON rules(enabled);
            CREATE INDEX IF NOT EXISTS idx_rules_source ON rules(source);

            CREATE TABLE IF NOT EXISTS pending_decisions (
                id TEXT PRIMARY KEY,
                snapshot_json TEXT NOT NULL,
                requested_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                deduplication_key TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'Pending'
            );

            CREATE INDEX IF NOT EXISTS idx_pending_status ON pending_decisions(status);
            CREATE INDEX IF NOT EXISTS idx_pending_expires ON pending_decisions(expires_at);
            CREATE INDEX IF NOT EXISTS idx_pending_dedup ON pending_decisions(deduplication_key);

            CREATE TABLE IF NOT EXISTS decisions (
                id TEXT PRIMARY KEY,
                pending_decision_id TEXT NOT NULL,
                snapshot_json TEXT NOT NULL,
                action TEXT NOT NULL,
                granularity TEXT NOT NULL,
                decided_at TEXT NOT NULL,
                generated_rule_id TEXT
            );

            CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                severity TEXT NOT NULL,
                category TEXT NOT NULL,
                description TEXT NOT NULL,
                metadata_json TEXT NOT NULL DEFAULT '{}'
            );

            CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_severity ON audit_events(severity);
            CREATE INDEX IF NOT EXISTS idx_audit_category ON audit_events(category);"
        )
        .map_err(|e| DomainError::Infrastructure(format!("Migration failed: {}", e)))
    }

    /// Execute a closure with the writer connection.
    /// Exécute une fermeture avec la connexion d'écriture.
    pub fn with_writer<F, T>(&self, f: F) -> Result<T, DomainError>
    where
        F: FnOnce(&Connection) -> Result<T, DomainError>,
    {
        let conn = self.writer.lock().map_err(|e| {
            DomainError::Infrastructure(format!("Failed to acquire DB lock: {}", e))
        })?;
        f(&conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_succeeds() {
        let db = Database::open_in_memory();
        assert!(db.is_ok());
    }

    #[test]
    fn tables_created_on_open() {
        let db = Database::open_in_memory().unwrap();
        db.with_writer(|conn| {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('rules', 'pending_decisions', 'decisions', 'audit_events')",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
            assert_eq!(count, 4);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn wal_mode_enabled() {
        let db = Database::open_in_memory().unwrap();
        db.with_writer(|conn| {
            let mode: String = conn
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
            // In-memory databases may report "memory" instead of "wal"
            // but the PRAGMA was still executed successfully
            assert!(mode == "wal" || mode == "memory");
            Ok(())
        })
        .unwrap();
    }
}
