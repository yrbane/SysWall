use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lru::LruCache;

use syswall_domain::entities::{ProcessInfo, SystemUser};

/// Entry in the process cache with insertion timestamp.
/// Entree dans le cache de processus avec horodatage d'insertion.
#[derive(Clone)]
struct CacheEntry {
    info: ProcessInfo,
    user: Option<SystemUser>,
    inserted_at: Instant,
}

/// LRU cache with TTL for process resolution results.
/// Cache LRU avec TTL pour les resultats de resolution de processus.
pub struct ProcessCache {
    pid_cache: Mutex<LruCache<u32, CacheEntry>>,
    inode_cache: Mutex<LruCache<u64, CacheEntry>>,
    ttl: Duration,
}

impl ProcessCache {
    /// Create a new cache with the given capacity and TTL.
    /// Cree un nouveau cache avec la capacite et le TTL donnes.
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap();
        Self {
            pid_cache: Mutex::new(LruCache::new(cap)),
            inode_cache: Mutex::new(LruCache::new(cap)),
            ttl,
        }
    }

    /// Get a cached entry by PID. Returns None if not found or stale.
    /// Retourne une entree mise en cache par PID. Retourne None si introuvable ou perimee.
    pub fn get_by_pid(&self, pid: u32) -> Option<(ProcessInfo, Option<SystemUser>)> {
        let mut cache = self.pid_cache.lock().unwrap();
        let entry = cache.get(&pid)?;
        if entry.inserted_at.elapsed() > self.ttl {
            cache.pop(&pid);
            return None;
        }
        Some((entry.info.clone(), entry.user.clone()))
    }

    /// Get a cached entry by socket inode. Returns None if not found or stale.
    /// Retourne une entree mise en cache par inode de socket. Retourne None si introuvable ou perimee.
    pub fn get_by_inode(&self, inode: u64) -> Option<(ProcessInfo, Option<SystemUser>)> {
        let mut cache = self.inode_cache.lock().unwrap();
        let entry = cache.get(&inode)?;
        if entry.inserted_at.elapsed() > self.ttl {
            cache.pop(&inode);
            return None;
        }
        Some((entry.info.clone(), entry.user.clone()))
    }

    /// Insert a process info entry by PID.
    /// Insere une entree d'info processus par PID.
    pub fn insert_pid(&self, pid: u32, info: ProcessInfo, user: Option<SystemUser>) {
        let mut cache = self.pid_cache.lock().unwrap();
        cache.put(
            pid,
            CacheEntry {
                info,
                user,
                inserted_at: Instant::now(),
            },
        );
    }

    /// Insert a process info entry by socket inode.
    /// Insere une entree d'info processus par inode de socket.
    pub fn insert_inode(&self, inode: u64, info: ProcessInfo, user: Option<SystemUser>) {
        let mut cache = self.inode_cache.lock().unwrap();
        cache.put(
            inode,
            CacheEntry {
                info,
                user,
                inserted_at: Instant::now(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_info(pid: u32) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: format!("proc-{}", pid),
            path: None,
            cmdline: None,
        }
    }

    #[test]
    fn cache_returns_fresh_entry() {
        let cache = ProcessCache::new(10, Duration::from_secs(5));
        let info = make_info(1);
        cache.insert_pid(1, info.clone(), None);
        let result = cache.get_by_pid(1);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0.name, "proc-1");
    }

    #[test]
    fn cache_evicts_stale_entry() {
        let cache = ProcessCache::new(10, Duration::from_millis(1));
        let info = make_info(1);
        cache.insert_pid(1, info, None);
        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.get_by_pid(1).is_none());
    }

    #[test]
    fn cache_respects_capacity() {
        let cache = ProcessCache::new(2, Duration::from_secs(60));
        cache.insert_pid(1, make_info(1), None);
        cache.insert_pid(2, make_info(2), None);
        cache.insert_pid(3, make_info(3), None);
        assert!(cache.get_by_pid(1).is_none());
        assert!(cache.get_by_pid(2).is_some());
        assert!(cache.get_by_pid(3).is_some());
    }

    #[test]
    fn inode_cache_independent_from_pid_cache() {
        let cache = ProcessCache::new(10, Duration::from_secs(5));
        let info = make_info(1);
        cache.insert_inode(99, info, None);
        assert!(cache.get_by_pid(1).is_none());
        assert!(cache.get_by_inode(99).is_some());
    }

    #[test]
    fn cache_stores_user_info() {
        let cache = ProcessCache::new(10, Duration::from_secs(5));
        let user = SystemUser {
            uid: 1000,
            name: "seb".to_string(),
        };
        cache.insert_pid(1, make_info(1), Some(user));
        let (_, u) = cache.get_by_pid(1).unwrap();
        assert_eq!(u.unwrap().name, "seb");
    }

    #[test]
    fn cache_miss_returns_none() {
        let cache = ProcessCache::new(10, Duration::from_secs(5));
        assert!(cache.get_by_pid(999).is_none());
        assert!(cache.get_by_inode(999).is_none());
    }
}
