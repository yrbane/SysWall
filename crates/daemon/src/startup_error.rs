use thiserror::Error;

/// Fatal startup failures emitted by `run()` and consumed by `main()`.
/// Erreurs fatales de démarrage émises par `run()` et consommées par `main()`.
#[derive(Debug, Error)]
pub enum StartupError {
    /// Configuration parse error or validation failure.
    /// Erreur d'analyse ou de validation de la configuration.
    #[error("syswall: configuration invalid: {0}")]
    ConfigInvalid(String),

    /// The system group `syswall` is missing — install scripts not run.
    /// Le groupe système `syswall` est absent — scripts d'installation non exécutés.
    #[error("syswall: missing system group 'syswall' (run install scripts)")]
    SyswallGroupMissing,

    /// Failed to chown the gRPC socket to the syswall group.
    /// Échec du chown du socket gRPC vers le groupe syswall.
    #[error("syswall: failed to chown socket {path}: {source}")]
    SocketChownFailed {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// Failed to bind the gRPC Unix socket.
    /// Échec du bind du socket Unix gRPC.
    #[error("syswall: failed to bind socket {path}: {source}")]
    SocketBindFailed {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// Generic infrastructure init failure (DB, eBPF load, etc.).
    /// Erreur d'initialisation infra (BD, chargement eBPF, etc.).
    #[error("syswall: infrastructure init failed: {0}")]
    InfrastructureInit(String),
}

impl StartupError {
    /// EX_CONFIG (sysexits.h) — configuration error or missing prerequisites.
    /// EX_CONFIG (sysexits.h) — erreur de configuration ou prérequis manquant.
    pub fn exit_code(&self) -> i32 {
        78
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_is_ex_config() {
        let err = StartupError::SyswallGroupMissing;
        assert_eq!(err.exit_code(), 78);
    }

    #[test]
    fn group_missing_displays_actionable_message() {
        let s = StartupError::SyswallGroupMissing.to_string();
        assert!(s.contains("syswall"));
        assert!(s.contains("install scripts"));
    }

    #[test]
    fn config_invalid_includes_inner_message() {
        let err = StartupError::ConfigInvalid("missing field x".into());
        let s = err.to_string();
        assert!(s.contains("configuration invalid"));
        assert!(s.contains("missing field x"));
    }

    #[test]
    fn socket_chown_failed_carries_path_and_source() {
        let err = StartupError::SocketChownFailed {
            path: "/run/syswall.sock".into(),
            source: std::io::Error::from_raw_os_error(13),
        };
        let s = err.to_string();
        assert!(s.contains("/run/syswall.sock"));
    }
}
