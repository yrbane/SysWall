use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use syswall_domain::entities::{ProcessInfo, SystemUser};
use syswall_domain::errors::DomainError;
use syswall_domain::ports::ProcessResolver;
use syswall_domain::value_objects::ExecutablePath;

use syswall_domain::value_objects::Protocol;

use super::cache::ProcessCache;
use super::proc_parser::{parse_cmdline_opt, parse_proc_net_tcp, parse_proc_net_tcp6, parse_proc_net_udp, parse_proc_status, ProcNetEntry};

/// Configuration for the ProcfsProcessResolver.
/// Configuration pour le ProcfsProcessResolver.
#[derive(Debug, Clone)]
pub struct ProcfsConfig {
    pub cache_capacity: usize,
    pub cache_ttl: Duration,
}

impl Default for ProcfsConfig {
    fn default() -> Self {
        Self {
            cache_capacity: 1024,
            cache_ttl: Duration::from_secs(5),
        }
    }
}

/// Real /proc-based process resolver.
/// Resolveur de processus reel base sur /proc.
pub struct ProcfsProcessResolver {
    cache: ProcessCache,
    icon_resolver: super::icon_resolver::IconResolver,
}

impl ProcfsProcessResolver {
    /// Create a new resolver. Verifies /proc is accessible.
    /// Cree un nouveau resolveur. Verifie que /proc est accessible.
    pub fn new(config: ProcfsConfig) -> Result<Self, DomainError> {
        if !Path::new("/proc").exists() {
            return Err(DomainError::Infrastructure(
                "/proc is not accessible".to_string(),
            ));
        }

        Ok(Self {
            cache: ProcessCache::new(config.cache_capacity, config.cache_ttl),
            icon_resolver: super::icon_resolver::IconResolver::new(),
        })
    }

    /// Read process info from /proc/<pid>/.
    /// Lit les informations du processus depuis /proc/<pid>/.
    fn read_process_info(pid: u32) -> Option<(ProcessInfo, Option<SystemUser>)> {
        let proc_path = PathBuf::from(format!("/proc/{}", pid));
        if !proc_path.exists() {
            return None;
        }

        // Read executable path
        let exe_path = std::fs::read_link(proc_path.join("exe"))
            .ok()
            .and_then(|p| {
                let path_str = p.to_string_lossy().to_string();
                // Strip " (deleted)" suffix if present
                let clean_path = if path_str.ends_with(" (deleted)") {
                    PathBuf::from(&path_str[..path_str.len() - 10])
                } else {
                    p
                };
                ExecutablePath::new(clean_path).ok()
            });

        // Read cmdline
        let cmdline = std::fs::read(proc_path.join("cmdline"))
            .ok()
            .and_then(|bytes| parse_cmdline_opt(&bytes));

        // Read status for name and UID
        let status_content = std::fs::read_to_string(proc_path.join("status")).ok()?;
        let status = parse_proc_status(&status_content)?;

        let user = nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(status.uid))
            .ok()
            .flatten()
            .map(|u| SystemUser {
                uid: status.uid,
                name: u.name,
            });

        Some((
            ProcessInfo {
                pid,
                name: status.name,
                path: exe_path,
                cmdline,
                icon: None, // Resolved later by icon_resolver
            },
            user,
        ))
    }

    /// Find socket inode by matching a connection 5-tuple against /proc/net/tcp and /proc/net/udp.
    /// Trouve l'inode de socket en comparant un 5-tuple de connexion avec /proc/net/tcp et /proc/net/udp.
    fn find_inode_by_connection(
        protocol: Protocol,
        local_ip: std::net::IpAddr,
        local_port: u16,
        remote_ip: std::net::IpAddr,
        remote_port: u16,
    ) -> Option<u64> {
        let entries = match protocol {
            Protocol::Tcp => {
                let mut entries = Vec::new();
                if let Ok(content) = std::fs::read_to_string("/proc/net/tcp") {
                    entries.extend(parse_proc_net_tcp(&content));
                }
                if let Ok(content) = std::fs::read_to_string("/proc/net/tcp6") {
                    entries.extend(parse_proc_net_tcp6(&content));
                }
                entries
            }
            Protocol::Udp => {
                let mut entries = Vec::new();
                if let Ok(content) = std::fs::read_to_string("/proc/net/udp") {
                    entries.extend(parse_proc_net_udp(&content));
                }
                // /proc/net/udp6 uses same format
                if let Ok(content) = std::fs::read_to_string("/proc/net/udp6") {
                    entries.extend(parse_proc_net_tcp6(&content)); // same parser
                }
                entries
            }
            _ => return None,
        };

        // Try exact match first (local+remote)
        for entry in &entries {
            if entry.local_ip == local_ip
                && entry.local_port == local_port
                && entry.remote_ip == remote_ip
                && entry.remote_port == remote_port
                && entry.inode != 0
            {
                return Some(entry.inode);
            }
        }

        // Fallback: match by local port only (for listening sockets or when remote is 0.0.0.0)
        for entry in &entries {
            if entry.local_port == local_port && entry.inode != 0 {
                return Some(entry.inode);
            }
        }

        None
    }

    /// Scan /proc/[0-9]*/fd/* to find which PID owns a socket inode.
    /// Parcourt /proc/[0-9]*/fd/* pour trouver quel PID possede un inode de socket.
    fn find_pid_by_inode(inode: u64) -> Option<u32> {
        let target = format!("socket:[{}]", inode);

        let proc_dir = match std::fs::read_dir("/proc") {
            Ok(d) => d,
            Err(_) => return None,
        };

        for entry in proc_dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Only numeric directories (PIDs)
            if !name_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            let fd_dir = entry.path().join("fd");
            let fd_entries = match std::fs::read_dir(&fd_dir) {
                Ok(d) => d,
                Err(_) => continue,
            };

            for fd_entry in fd_entries.flatten() {
                if std::fs::read_link(fd_entry.path())
                    .ok()
                    .is_some_and(|link| link.to_string_lossy() == target)
                {
                    return name_str.parse().ok();
                }
            }
        }

        None
    }
}

#[async_trait]
impl ProcessResolver for ProcfsProcessResolver {
    /// Resolve process info by PID.
    /// Resout les informations du processus par PID.
    async fn resolve(&self, pid: u32) -> Result<Option<ProcessInfo>, DomainError> {
        // Check cache first
        if let Some((info, _)) = self.cache.get_by_pid(pid) {
            return Ok(Some(info));
        }

        // Read from /proc in a blocking task
        let result = tokio::task::spawn_blocking(move || Self::read_process_info(pid))
            .await
            .map_err(|e| {
                DomainError::Infrastructure(format!("spawn_blocking failed: {}", e))
            })?;

        if let Some((mut info, ref user)) = result {
            // Resolve icon
            info.icon = self.icon_resolver.resolve(
                &info.name,
                info.path.as_ref().map(|p| p.as_path()),
            );
            self.cache
                .insert_pid(pid, info.clone(), user.clone());
            return Ok(Some(info));
        }

        Ok(None)
    }

    /// Resolve process info by connection 5-tuple.
    /// Resout les informations du processus par 5-tuple de connexion.
    async fn resolve_by_connection(
        &self,
        protocol: Protocol,
        local_ip: std::net::IpAddr,
        local_port: u16,
        remote_ip: std::net::IpAddr,
        remote_port: u16,
    ) -> Result<Option<ProcessInfo>, DomainError> {
        // Find inode from /proc/net/tcp or /proc/net/udp
        let inode = tokio::task::spawn_blocking(move || {
            Self::find_inode_by_connection(protocol, local_ip, local_port, remote_ip, remote_port)
        })
        .await
        .map_err(|e| DomainError::Infrastructure(format!("spawn_blocking failed: {}", e)))?;

        match inode {
            Some(inode) => self.resolve_by_socket(inode).await,
            None => {
                debug!(
                    "No socket inode found for {}:{} -> {}:{} ({:?})",
                    local_ip, local_port, remote_ip, remote_port, protocol
                );
                Ok(None)
            }
        }
    }

    /// Resolve process info by socket inode.
    /// Resout les informations du processus par inode de socket.
    async fn resolve_by_socket(&self, inode: u64) -> Result<Option<ProcessInfo>, DomainError> {
        // Check inode cache first
        if let Some((info, _)) = self.cache.get_by_inode(inode) {
            return Ok(Some(info));
        }

        // Find PID by scanning /proc in a blocking task
        let pid = tokio::task::spawn_blocking(move || Self::find_pid_by_inode(inode))
            .await
            .map_err(|e| {
                DomainError::Infrastructure(format!("spawn_blocking failed: {}", e))
            })?;

        let pid = match pid {
            Some(p) => p,
            None => {
                debug!("No PID found for socket inode {}", inode);
                return Ok(None);
            }
        };

        // Resolve the PID
        let result = tokio::task::spawn_blocking(move || Self::read_process_info(pid))
            .await
            .map_err(|e| {
                DomainError::Infrastructure(format!("spawn_blocking failed: {}", e))
            })?;

        if let Some((mut info, ref user)) = result {
            // Resolve icon
            info.icon = self.icon_resolver.resolve(
                &info.name,
                info.path.as_ref().map(|p| p.as_path()),
            );
            self.cache
                .insert_inode(inode, info.clone(), user.clone());
            self.cache
                .insert_pid(pid, info.clone(), user.clone());
            return Ok(Some(info));
        }

        Ok(None)
    }
}
