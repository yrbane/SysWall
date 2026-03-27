/// DNS resolver with LRU cache for reverse IP lookups.
/// Résolveur DNS avec cache LRU pour les recherches IP inverses.
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use lru::LruCache;
use std::num::NonZeroUsize;
use tokio::sync::Mutex;

use syswall_domain::errors::DomainError;
use syswall_domain::ports::DnsResolver as DnsResolverPort;

/// Cached entry for a DNS lookup result.
/// Entrée mise en cache pour un résultat de recherche DNS.
struct CacheEntry {
    hostname: Option<String>,
    inserted_at: Instant,
}

/// LRU-cached reverse DNS resolver backed by the OS resolver.
/// Résolveur DNS inverse avec cache LRU, s'appuyant sur le résolveur OS.
pub struct DnsResolver {
    cache: Arc<Mutex<LruCache<IpAddr, CacheEntry>>>,
    ttl: Duration,
}

impl DnsResolver {
    /// Create a new resolver with the given LRU capacity and TTL in seconds.
    /// Crée un nouveau résolveur avec la capacité LRU et le TTL en secondes donnés.
    pub fn new(capacity: usize, ttl_secs: u64) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(4096).unwrap());
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(cap))),
            ttl: Duration::from_secs(ttl_secs),
        }
    }
}

impl Default for DnsResolver {
    fn default() -> Self {
        Self::new(4096, 300)
    }
}

#[async_trait]
impl DnsResolverPort for DnsResolver {
    /// Resolve the hostname for the given IP, using cache when possible.
    /// Résout le nom d'hôte pour l'IP donnée, en utilisant le cache si possible.
    async fn resolve(&self, ip: IpAddr) -> Result<Option<String>, DomainError> {
        // Check cache first
        // Vérification du cache en premier
        {
            let mut cache = self.cache.lock().await;
            if let Some(entry) = cache.get(&ip) {
                if entry.inserted_at.elapsed() < self.ttl {
                    return Ok(entry.hostname.clone());
                }
            }
        }

        // Perform blocking reverse DNS lookup
        // Effectue la recherche DNS inverse bloquante
        let hostname = tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&ip).ok())
            .await
            .map_err(|e| DomainError::Infrastructure(format!("DNS spawn_blocking failed: {e}")))?;

        // Store in cache (including negative results)
        // Stocke dans le cache (y compris les résultats négatifs)
        {
            let mut cache = self.cache.lock().await;
            cache.put(
                ip,
                CacheEntry {
                    hostname: hostname.clone(),
                    inserted_at: Instant::now(),
                },
            );
        }

        Ok(hostname)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syswall_domain::ports::DnsResolver as DnsResolverPort;

    #[tokio::test]
    async fn resolve_loopback_returns_some_or_none() {
        // Loopback may or may not have a reverse DNS entry depending on the OS.
        // Le loopback peut ou non avoir une entrée DNS inverse selon le système.
        let resolver = DnsResolver::default();
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        let result = resolver.resolve(ip).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn resolve_uses_cache_on_second_call() {
        let resolver = DnsResolver::default();
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        // First call — populates cache
        // Premier appel — remplit le cache
        let first = resolver.resolve(ip).await.unwrap();
        // Second call — should hit cache
        // Deuxième appel — doit utiliser le cache
        let second = resolver.resolve(ip).await.unwrap();
        assert_eq!(first, second);
    }
}
