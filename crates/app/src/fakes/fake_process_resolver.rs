use async_trait::async_trait;

use syswall_domain::entities::ProcessInfo;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::ProcessResolver;

/// In-memory fake process resolver for testing (always returns None).
/// Résolveur de processus factice en mémoire pour les tests (retourne toujours None).
pub struct FakeProcessResolver;

impl Default for FakeProcessResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeProcessResolver {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProcessResolver for FakeProcessResolver {
    async fn resolve(&self, _pid: u32) -> Result<Option<ProcessInfo>, DomainError> {
        Ok(None)
    }

    async fn resolve_by_socket(&self, _inode: u64) -> Result<Option<ProcessInfo>, DomainError> {
        Ok(None)
    }
}
