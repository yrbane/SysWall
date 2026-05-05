//! Conversions entre DomainError et tonic::Status.
//! Conversions between DomainError and tonic::Status.

use syswall_domain::errors::DomainError;

/// Map a DomainError to the appropriate tonic status code.
/// Convertit une DomainError vers le code de statut tonic approprié.
pub fn domain_error_to_status(e: DomainError) -> tonic::Status {
    match e {
        DomainError::Validation(msg) => tonic::Status::invalid_argument(msg),
        DomainError::NotFound(msg) => tonic::Status::not_found(msg),
        DomainError::AlreadyExists(msg) => tonic::Status::already_exists(msg),
        DomainError::Infrastructure(msg) => tonic::Status::internal(msg),
        DomainError::NotPermitted(msg) => tonic::Status::permission_denied(msg),
        // FAILED_PRECONDITION : etat systeme (perte de connectivite) qui empeche l'operation, pas un conflit transitoire.
        // FAILED_PRECONDITION: system state (connectivity loss) prevented the operation — not a transient conflict.
        DomainError::AntilockoutTriggered { .. } => {
            tonic::Status::failed_precondition(e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_mapping_validation() {
        let status = domain_error_to_status(DomainError::Validation("bad".to_string()));
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn error_mapping_not_found() {
        let status = domain_error_to_status(DomainError::NotFound("missing".to_string()));
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[test]
    fn error_mapping_already_exists() {
        let status = domain_error_to_status(DomainError::AlreadyExists("dup".to_string()));
        assert_eq!(status.code(), tonic::Code::AlreadyExists);
    }

    #[test]
    fn error_mapping_infrastructure() {
        let status = domain_error_to_status(DomainError::Infrastructure("db down".to_string()));
        assert_eq!(status.code(), tonic::Code::Internal);
    }

    #[test]
    fn error_mapping_not_permitted() {
        let status = domain_error_to_status(DomainError::NotPermitted("nope".to_string()));
        assert_eq!(status.code(), tonic::Code::PermissionDenied);
    }
}
