use async_trait::async_trait;

use syswall_domain::entities::Connection;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::{ConnectionEventStream, ConnectionMonitor};

/// In-memory fake connection monitor for testing (empty streams; optional
/// preseeded active connections snapshot).
/// Moniteur de connexion factice en mémoire pour les tests (flux vides ;
/// instantané de connexions actives pré-rempli en option).
#[derive(Default)]
pub struct FakeConnectionMonitor {
    /// Connections returned by `get_active_connections`.
    /// Connexions retournées par `get_active_connections`.
    active_connections: Vec<Connection>,
}

impl FakeConnectionMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a fake monitor whose snapshot returns the given connections.
    /// Construit un moniteur factice dont l'instantané retourne les connexions données.
    pub fn with_connections(active_connections: Vec<Connection>) -> Self {
        Self { active_connections }
    }
}

#[async_trait]
impl ConnectionMonitor for FakeConnectionMonitor {
    async fn stream_events(&self) -> Result<ConnectionEventStream, DomainError> {
        Ok(Box::pin(futures::stream::empty()))
    }

    async fn get_active_connections(&self) -> Result<Vec<Connection>, DomainError> {
        Ok(self.active_connections.clone())
    }
}
