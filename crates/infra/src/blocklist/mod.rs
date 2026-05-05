use std::collections::HashSet;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use chrono::Utc;
use tracing::{info, warn};

use syswall_domain::entities::Blocklist;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::BlocklistChecker;

/// Implémentation basée sur des fichiers texte (un domaine/IP par ligne).
/// File-based implementation (one domain/IP per line).
pub struct FileBlocklistRepository {
    directory: PathBuf,
    state: RwLock<BlocklistState>,
}

struct BlocklistState {
    domains: HashSet<String>,
    ips: HashSet<IpAddr>,
    lists: Vec<Blocklist>,
}

impl FileBlocklistRepository {
    /// Charge les blocklists depuis un répertoire.
    /// Load blocklists from a directory.
    pub fn new(directory: &Path) -> Result<Self, DomainError> {
        let repo = Self {
            directory: directory.to_path_buf(),
            state: RwLock::new(BlocklistState {
                domains: HashSet::new(),
                ips: HashSet::new(),
                lists: Vec::new(),
            }),
        };
        repo.reload()?;
        Ok(repo)
    }

    /// Parse un fichier de blocklist et retourne les entrées.
    /// Parse a blocklist file and return entries.
    fn parse_file(path: &Path) -> (HashSet<String>, HashSet<IpAddr>) {
        let mut domains = HashSet::new();
        let mut ips = HashSet::new();

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Impossible de lire la blocklist {}: {}", path.display(), e);
                return (domains, ips);
            }
        };

        for line in content.lines() {
            let trimmed = line.trim();
            // Ignorer les lignes vides et les commentaires
            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Tenter de parser comme IP d'abord
            // Try parsing as IP first
            if let Ok(ip) = trimmed.parse::<IpAddr>() {
                ips.insert(ip);
            } else {
                // Sinon c'est un domaine — normaliser en minuscules
                // Otherwise it's a domain — normalize to lowercase
                domains.insert(trimmed.to_lowercase());
            }
        }

        (domains, ips)
    }
}

impl BlocklistChecker for FileBlocklistRepository {
    fn is_blocked_domain(&self, domain: &str) -> bool {
        let state = self.state.read().expect("RwLock jamais empoisonné / never poisoned");
        let lower = domain.to_lowercase();
        // Vérification exacte et par suffixe (sous-domaine)
        // Exact match and suffix match (subdomain)
        if state.domains.contains(&lower) {
            return true;
        }
        // Vérifier si un domaine parent est bloqué (ex: ads.example.com bloqué si example.com est dans la liste)
        // Check if parent domain is blocked
        let parts: Vec<&str> = lower.split('.').collect();
        for i in 1..parts.len() {
            let parent = parts[i..].join(".");
            if state.domains.contains(&parent) {
                return true;
            }
        }
        false
    }

    fn is_blocked_ip(&self, ip: IpAddr) -> bool {
        let state = self.state.read().expect("RwLock jamais empoisonné / never poisoned");
        state.ips.contains(&ip)
    }

    fn list_blocklists(&self) -> Vec<Blocklist> {
        let state = self.state.read().expect("RwLock jamais empoisonné / never poisoned");
        state.lists.clone()
    }

    fn reload(&self) -> Result<(), DomainError> {
        if !self.directory.exists() {
            info!(
                "Répertoire de blocklists inexistant, création : {}",
                self.directory.display()
            );
            std::fs::create_dir_all(&self.directory).map_err(|e| {
                DomainError::Infrastructure(format!(
                    "Impossible de créer le répertoire blocklists: {}",
                    e
                ))
            })?;
            return Ok(());
        }

        let mut all_domains = HashSet::new();
        let mut all_ips = HashSet::new();
        let mut lists = Vec::new();

        let entries = std::fs::read_dir(&self.directory).map_err(|e| {
            DomainError::Infrastructure(format!("Impossible de lire le répertoire blocklists: {}", e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "txt") {
                let (domains, ips) = Self::parse_file(&path);
                let entry_count = domains.len() + ips.len();

                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                lists.push(Blocklist {
                    id: name.clone(),
                    name,
                    path: path.clone(),
                    enabled: true,
                    entry_count,
                    last_loaded: Utc::now(),
                });

                info!(
                    "Blocklist chargée : {} ({} entrées)",
                    path.display(),
                    entry_count
                );

                all_domains.extend(domains);
                all_ips.extend(ips);
            }
        }

        info!(
            "Blocklists rechargées : {} domaines, {} IPs depuis {} fichiers",
            all_domains.len(),
            all_ips.len(),
            lists.len()
        );

        let mut state = self.state.write().expect("RwLock jamais empoisonné / never poisoned");
        state.domains = all_domains;
        state.ips = all_ips;
        state.lists = lists;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_blocklist(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn loads_domains_from_file() {
        let dir = TempDir::new().unwrap();
        create_test_blocklist(
            dir.path(),
            "test.txt",
            "# Commentaire\nexample.com\nads.tracker.com\n\n",
        );

        let repo = FileBlocklistRepository::new(dir.path()).unwrap();
        assert!(repo.is_blocked_domain("example.com"));
        assert!(repo.is_blocked_domain("ads.tracker.com"));
        assert!(!repo.is_blocked_domain("google.com"));
    }

    #[test]
    fn blocks_subdomains() {
        let dir = TempDir::new().unwrap();
        create_test_blocklist(dir.path(), "test.txt", "example.com\n");

        let repo = FileBlocklistRepository::new(dir.path()).unwrap();
        assert!(repo.is_blocked_domain("sub.example.com"));
        assert!(repo.is_blocked_domain("deep.sub.example.com"));
        assert!(!repo.is_blocked_domain("notexample.com"));
    }

    #[test]
    fn loads_ips_from_file() {
        let dir = TempDir::new().unwrap();
        create_test_blocklist(dir.path(), "ips.txt", "1.2.3.4\n10.0.0.1\n");

        let repo = FileBlocklistRepository::new(dir.path()).unwrap();
        assert!(repo.is_blocked_ip("1.2.3.4".parse().unwrap()));
        assert!(!repo.is_blocked_ip("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn case_insensitive_domain_check() {
        let dir = TempDir::new().unwrap();
        create_test_blocklist(dir.path(), "test.txt", "Example.COM\n");

        let repo = FileBlocklistRepository::new(dir.path()).unwrap();
        assert!(repo.is_blocked_domain("example.com"));
        assert!(repo.is_blocked_domain("EXAMPLE.COM"));
    }

    #[test]
    fn list_blocklists_returns_loaded() {
        let dir = TempDir::new().unwrap();
        create_test_blocklist(dir.path(), "domains.txt", "a.com\nb.com\n");
        create_test_blocklist(dir.path(), "ips.txt", "1.1.1.1\n");

        let repo = FileBlocklistRepository::new(dir.path()).unwrap();
        let lists = repo.list_blocklists();
        assert_eq!(lists.len(), 2);
    }

    #[test]
    fn reload_picks_up_new_entries() {
        let dir = TempDir::new().unwrap();
        create_test_blocklist(dir.path(), "test.txt", "old.com\n");

        let repo = FileBlocklistRepository::new(dir.path()).unwrap();
        assert!(repo.is_blocked_domain("old.com"));
        assert!(!repo.is_blocked_domain("new.com"));

        // Ajouter une entrée et recharger
        // Add an entry and reload
        create_test_blocklist(dir.path(), "test.txt", "old.com\nnew.com\n");
        repo.reload().unwrap();

        assert!(repo.is_blocked_domain("new.com"));
    }

    #[test]
    fn empty_directory_works() {
        let dir = TempDir::new().unwrap();
        let repo = FileBlocklistRepository::new(dir.path()).unwrap();
        assert!(!repo.is_blocked_domain("anything.com"));
        assert_eq!(repo.list_blocklists().len(), 0);
    }
}
