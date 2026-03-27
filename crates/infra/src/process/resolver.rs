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
    /// Socket table: local_port → PID, refreshed lazily
    socket_table: std::sync::Mutex<SocketTableCache>,
}

struct SocketTableCache {
    table: std::collections::HashMap<u16, u32>,
    last_refresh: std::time::Instant,
    ttl: std::time::Duration,
}

impl SocketTableCache {
    fn new(ttl: std::time::Duration) -> Self {
        Self {
            table: std::collections::HashMap::new(),
            last_refresh: std::time::Instant::now() - ttl - ttl, // force first refresh
            ttl,
        }
    }

    fn is_stale(&self) -> bool {
        self.last_refresh.elapsed() > self.ttl
    }
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
            socket_table: std::sync::Mutex::new(SocketTableCache::new(
                std::time::Duration::from_secs(2),
            )),
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

    /// Refresh the socket table by running `ss -tnp -unp` and parsing all entries.
    /// Returns a map of local_port → PID for all sockets with known processes.
    ///
    /// Rafraîchit la table des sockets via `ss -tnp -unp`.
    fn refresh_socket_table() -> Option<std::collections::HashMap<u16, u32>> {
        let output = std::process::Command::new("ss")
            .args(["-tnp", "-unp", "state", "all"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut table = std::collections::HashMap::new();

        for line in stdout.lines().skip(1) {
            // Extract local port and PID
            // Format: STATE  RECV SEND  LOCAL:PORT  REMOTE:PORT  users:(("name",pid=NNN,fd=N))
            let pid = match Self::parse_ss_pid(line) {
                Some(p) => p,
                None => continue,
            };

            let local_port = match Self::parse_ss_local_port(line) {
                Some(p) => p,
                None => continue,
            };

            table.insert(local_port, pid);
        }

        Some(table)
    }

    /// Extract local port from an ss output line.
    fn parse_ss_local_port(line: &str) -> Option<u16> {
        // Fields are whitespace-separated. Local address is field 4 (0-indexed: 3)
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            return None;
        }
        let local_addr = fields[3];
        // Port is after the last ':'
        let port_str = local_addr.rsplit(':').next()?;
        port_str.parse().ok()
    }

    /// Extract PID from an ss output line.
    fn parse_ss_pid(line: &str) -> Option<u32> {
        let pid_start = line.find("pid=")?;
        let after_pid = &line[pid_start + 4..];
        let pid_str: String = after_pid.chars().take_while(|c| c.is_ascii_digit()).collect();
        pid_str.parse().ok().filter(|&p| p > 0)
    }

    /// Parse ss output to extract PID.
    /// Example line: `ESTAB 0 0 192.168.1.159:443 8.8.8.8:12345 users:(("firefox",pid=1234,fd=22))`
    fn parse_ss_output(output: &str) -> Option<u32> {
        for line in output.lines().skip(1) {
            // Look for pid=NNNN in the line
            if let Some(pid_start) = line.find("pid=") {
                let after_pid = &line[pid_start + 4..];
                let pid_str: String = after_pid.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if pid > 0 {
                        return Some(pid);
                    }
                }
            }
        }
        None
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

    /// Resolve process info by connection 5-tuple using a lazily-refreshed socket table.
    /// The table is rebuilt from `ss -tnp -unp` if older than 2 seconds.
    ///
    /// Résout les informations du processus par 5-tuple via une table de sockets rafraîchie à la demande.
    async fn resolve_by_connection(
        &self,
        protocol: Protocol,
        local_ip: std::net::IpAddr,
        local_port: u16,
        remote_ip: std::net::IpAddr,
        remote_port: u16,
    ) -> Result<Option<ProcessInfo>, DomainError> {
        // Refresh socket table if stale
        let needs_refresh = self
            .socket_table
            .lock()
            .map(|cache| cache.is_stale())
            .unwrap_or(true);

        if needs_refresh {
            let new_table =
                tokio::task::spawn_blocking(|| Self::refresh_socket_table())
                    .await
                    .map_err(|e| {
                        DomainError::Infrastructure(format!("spawn_blocking failed: {}", e))
                    })?;

            if let Some(table) = new_table {
                let count = table.len();
                if let Ok(mut cache) = self.socket_table.lock() {
                    cache.table = table;
                    cache.last_refresh = std::time::Instant::now();
                }
                debug!("Socket table refreshed: {} entries", count);
            }
        }

        // Look up PID from socket table
        let pid = self
            .socket_table
            .lock()
            .ok()
            .and_then(|cache| cache.table.get(&local_port).copied());

        match pid {
            Some(pid) => self.resolve(pid).await,
            None => {
                debug!(
                    "No process in socket table for port {} ({}:{} -> {}:{} {:?})",
                    local_port, local_ip, local_port, remote_ip, remote_port, protocol
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
