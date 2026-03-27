use std::net::IpAddr;

use async_trait::async_trait;

use syswall_domain::errors::DomainError;
use syswall_domain::ports::DnsResolver;

/// In-memory fake DNS resolver for testing (always returns None).
/// Résolveur DNS factice en mémoire pour les tests (retourne toujours None).
pub struct FakeDnsResolver;

impl Default for FakeDnsResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeDnsResolver {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DnsResolver for FakeDnsResolver {
    async fn resolve(&self, _ip: IpAddr) -> Result<Option<String>, DomainError> {
        Ok(None)
    }
}
