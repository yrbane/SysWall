use thiserror::Error;

/// All domain-level errors.
/// Toutes les erreurs au niveau du domaine.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// Validation constraint violated.
    /// Contrainte de validation violée.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Entity not found.
    /// Entité introuvable.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Entity already exists (duplicate).
    /// L'entité existe déjà (doublon).
    #[error("Already exists: {0}")]
    AlreadyExists(String),

    /// Infrastructure failure (database, filesystem, etc.).
    /// Erreur d'infrastructure (base de données, système de fichiers, etc.).
    #[error("Infrastructure error: {0}")]
    Infrastructure(String),

    /// Operation not permitted for current context.
    /// Opération non autorisée dans le contexte actuel.
    #[error("Operation not permitted: {0}")]
    NotPermitted(String),
}
