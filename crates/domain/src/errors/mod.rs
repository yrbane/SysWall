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

    /// Anti-lockout triggered: connectivity was lost after a ruleset apply, rules rolled back.
    /// Anti-lockout déclenché : la connectivité a été perdue après un apply, règles annulées.
    #[error(
        "Anti-lockout triggered: {rolled_back_count} rule change(s) rolled back due to lost connectivity"
    )]
    AntilockoutTriggered { rolled_back_count: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antilockout_triggered_displays_count() {
        let err = DomainError::AntilockoutTriggered {
            rolled_back_count: 3,
        };
        assert_eq!(
            err.to_string(),
            "Anti-lockout triggered: 3 rule change(s) rolled back due to lost connectivity"
        );
    }
}
