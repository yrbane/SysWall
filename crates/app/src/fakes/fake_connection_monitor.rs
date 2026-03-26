use async_trait::async_trait;

use syswall_domain::entities::Connection;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::{ConnectionEventStream, ConnectionMonitor};

/// In-memory fake connection monitor for testing (returns empty streams).
/// Moniteur de connexion factice en mémoire pour les tests (retourne des flux vides).
pub struct FakeConnectionMonitor;

impl FakeConnectionMonitor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ConnectionMonitor for FakeConnectionMonitor {
    async fn stream_events(&self) -> Result<ConnectionEventStream, DomainError> {
        Ok(Box::pin(futures::stream::empty()))
    }

    async fn get_active_connections(&self) -> Result<Vec<Connection>, DomainError> {
        Ok(vec![])
    }
}
