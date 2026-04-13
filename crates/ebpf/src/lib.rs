pub mod events;

use std::net::IpAddr;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tracing::{debug, info};

use syswall_domain::entities::ProcessInfo;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::ProcessResolver;
use syswall_domain::value_objects::{ExecutablePath, Protocol};

use events::SocketEvent;

/// Associe (protocole, port_local) → PID pour des recherches O(1).
/// Maps (protocol, local_port) → PID for O(1) lookups.
type SocketMap = DashMap<(u8, u16), u32>;

// Chemin vers le programme BPF compilé, embarqué à la compilation.
// Path to compiled BPF program, embedded at build time.
const BPF_PROG_BYTES: &[u8] = include_bytes!(
    "../../ebpf-prog/target/bpfel-unknown-none/release/syswall-ebpf-prog"
);

/// Résolveur de processus basé sur eBPF.
/// Charge un programme BPF qui hooke inet_sock_set_state et capture le PID par socket.
///
/// eBPF-based process resolver.
/// Loads a BPF program hooking inet_sock_set_state to capture PID per socket.
pub struct EbpfProcessResolver {
    socket_map: Arc<SocketMap>,
    _drain_handle: tokio::task::JoinHandle<()>,
}

impl EbpfProcessResolver {
    /// Tente de charger le programme eBPF. Retourne Err en cas d'échec.
    /// Attempt to load the eBPF program. Returns Err if loading fails.
    pub fn try_new() -> Result<Self, DomainError> {
        let mut ebpf = aya::Ebpf::load(BPF_PROG_BYTES).map_err(|e| {
            DomainError::Infrastructure(format!(
                "Échec du chargement du programme eBPF: {}",
                e
            ))
        })?;

        // Initialisation des logs eBPF (optionnel, best-effort)
        // Initialize eBPF logging (optional, best-effort)
        if let Err(e) = aya_log::EbpfLogger::init(&mut ebpf) {
            debug!("eBPF logger init skipped: {}", e);
        }

        // Attachement au tracepoint
        // Attach to tracepoint
        use aya::programs::TracePoint;
        let program: &mut TracePoint = ebpf
            .program_mut("inet_sock_set_state")
            .ok_or_else(|| {
                DomainError::Infrastructure(
                    "Programme BPF 'inet_sock_set_state' introuvable".into(),
                )
            })?
            .try_into()
            .map_err(|e: aya::programs::ProgramError| {
                DomainError::Infrastructure(format!("Erreur de type du programme BPF: {}", e))
            })?;

        program.load().map_err(|e| {
            DomainError::Infrastructure(format!(
                "Échec du chargement du tracepoint BPF: {}",
                e
            ))
        })?;
        program
            .attach("sock", "inet_sock_set_state")
            .map_err(|e| {
                DomainError::Infrastructure(format!(
                    "Échec de l'attachement au tracepoint BPF: {}",
                    e
                ))
            })?;

        info!("Tracepoint eBPF inet_sock_set_state attaché avec succès");

        // Configuration du lecteur ring buffer
        // Setup ring buffer reader
        let socket_map: Arc<SocketMap> = Arc::new(DashMap::new());
        let map_clone = socket_map.clone();

        let ring_buf = aya::maps::RingBuf::try_from(
            ebpf.take_map("EVENTS").ok_or_else(|| {
                DomainError::Infrastructure("Map BPF 'EVENTS' introuvable".into())
            })?,
        )
        .map_err(|e| {
            DomainError::Infrastructure(format!("Erreur de création du RingBuf: {}", e))
        })?;

        let drain_handle = tokio::spawn(async move {
            Self::drain_ring_buffer(ring_buf, map_clone).await;
        });

        Ok(Self {
            socket_map,
            _drain_handle: drain_handle,
        })
    }

    /// Draine les événements du ring buffer et met à jour la table de sockets.
    /// Drains events from ring buffer and updates socket map.
    async fn drain_ring_buffer(
        mut ring_buf: aya::maps::RingBuf<aya::maps::MapData>,
        map: Arc<SocketMap>,
    ) {
        info!("Démarrage du drainage du ring buffer eBPF");
        loop {
            while let Some(event_data) = ring_buf.next() {
                if event_data.len() < std::mem::size_of::<SocketEvent>() {
                    continue;
                }
                let event: SocketEvent = unsafe {
                    std::ptr::read_unaligned(event_data.as_ptr() as *const SocketEvent)
                };

                if event.pid > 0 && event.sport > 0 {
                    map.insert((event.protocol, event.sport), event.pid);
                    debug!(
                        "eBPF: proto={} port={} pid={} ({})",
                        event.protocol,
                        event.sport,
                        event.pid,
                        event.comm_str()
                    );
                }
            }
            // Cède le contrôle pour éviter le busy-looping
            // Yield to avoid busy-looping
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    fn protocol_to_u8(protocol: Protocol) -> u8 {
        match protocol {
            Protocol::Tcp => 6,  // IPPROTO_TCP
            Protocol::Udp => 17, // IPPROTO_UDP
            _ => 0,
        }
    }
}

#[async_trait]
impl ProcessResolver for EbpfProcessResolver {
    async fn resolve(&self, pid: u32) -> Result<Option<ProcessInfo>, DomainError> {
        let result = tokio::task::spawn_blocking(move || read_process_info_from_proc(pid))
            .await
            .map_err(|e| DomainError::Infrastructure(format!("spawn_blocking: {}", e)))?;
        Ok(result)
    }

    async fn resolve_by_socket(&self, _inode: u64) -> Result<Option<ProcessInfo>, DomainError> {
        // Non applicable pour le résolveur eBPF
        // Not applicable for eBPF resolver
        Ok(None)
    }

    async fn resolve_by_connection(
        &self,
        protocol: Protocol,
        _local_ip: IpAddr,
        local_port: u16,
        _remote_ip: IpAddr,
        _remote_port: u16,
    ) -> Result<Option<ProcessInfo>, DomainError> {
        let proto = Self::protocol_to_u8(protocol);
        if let Some(entry) = self.socket_map.get(&(proto, local_port)) {
            let pid = *entry;
            return self.resolve(pid).await;
        }
        Ok(None)
    }
}

/// Combine eBPF (rapide, niveau kernel) avec un fallback procfs.
/// Combines eBPF (fast, kernel-level) with procfs fallback.
pub struct HybridProcessResolver {
    ebpf: Option<EbpfProcessResolver>,
    fallback: Arc<dyn ProcessResolver>,
}

impl HybridProcessResolver {
    pub fn new(ebpf: Option<EbpfProcessResolver>, fallback: Arc<dyn ProcessResolver>) -> Self {
        Self { ebpf, fallback }
    }
}

#[async_trait]
impl ProcessResolver for HybridProcessResolver {
    async fn resolve(&self, pid: u32) -> Result<Option<ProcessInfo>, DomainError> {
        if let Some(ref ebpf) = self.ebpf {
            if let Ok(Some(info)) = ebpf.resolve(pid).await {
                return Ok(Some(info));
            }
        }
        self.fallback.resolve(pid).await
    }

    async fn resolve_by_socket(&self, inode: u64) -> Result<Option<ProcessInfo>, DomainError> {
        self.fallback.resolve_by_socket(inode).await
    }

    async fn resolve_by_connection(
        &self,
        protocol: Protocol,
        local_ip: IpAddr,
        local_port: u16,
        remote_ip: IpAddr,
        remote_port: u16,
    ) -> Result<Option<ProcessInfo>, DomainError> {
        // Essai eBPF d'abord (rapide, O(1))
        // Try eBPF first (fast, O(1))
        if let Some(ref ebpf) = self.ebpf {
            if let Ok(Some(info)) = ebpf
                .resolve_by_connection(protocol, local_ip, local_port, remote_ip, remote_port)
                .await
            {
                return Ok(Some(info));
            }
        }
        // Fallback vers procfs (3-tier)
        // Fallback to procfs (3-tier)
        self.fallback
            .resolve_by_connection(protocol, local_ip, local_port, remote_ip, remote_port)
            .await
    }
}

/// Lit les informations du processus depuis /proc/<pid>.
/// Reads process info from /proc/<pid>.
fn read_process_info_from_proc(pid: u32) -> Option<ProcessInfo> {
    let proc_path = std::path::PathBuf::from(format!("/proc/{}", pid));
    if !proc_path.exists() {
        return None;
    }

    let exe_path = std::fs::read_link(proc_path.join("exe"))
        .ok()
        .and_then(|p| {
            let s = p.to_string_lossy().to_string();
            let clean = if s.ends_with(" (deleted)") {
                std::path::PathBuf::from(&s[..s.len() - 10])
            } else {
                p
            };
            ExecutablePath::new(clean).ok()
        });

    let cmdline = std::fs::read(proc_path.join("cmdline")).ok().and_then(|b| {
        let s = String::from_utf8_lossy(&b)
            .replace('\0', " ")
            .trim()
            .to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    });

    let status = std::fs::read_to_string(proc_path.join("status")).ok()?;
    let name = status
        .lines()
        .find(|l| l.starts_with("Name:"))
        .map(|l| l.trim_start_matches("Name:").trim().to_string())?;

    Some(ProcessInfo {
        pid,
        name,
        path: exe_path,
        cmdline,
        icon: None, // Résolu par IconResolver dans la couche daemon
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faux résolveur pour les tests.
    /// Fake resolver for testing.
    struct FakeResolver;

    #[async_trait]
    impl ProcessResolver for FakeResolver {
        async fn resolve(&self, pid: u32) -> Result<Option<ProcessInfo>, DomainError> {
            Ok(Some(ProcessInfo {
                pid,
                name: "fake".to_string(),
                path: None,
                cmdline: None,
                icon: None,
            }))
        }
        async fn resolve_by_socket(
            &self,
            _: u64,
        ) -> Result<Option<ProcessInfo>, DomainError> {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn hybrid_falls_back_when_no_ebpf() {
        let hybrid = HybridProcessResolver::new(None, Arc::new(FakeResolver));
        let result = hybrid.resolve(1234).await.unwrap();
        assert_eq!(result.unwrap().name, "fake");
    }

    #[tokio::test]
    async fn hybrid_resolve_by_connection_falls_back() {
        let hybrid = HybridProcessResolver::new(None, Arc::new(FakeResolver));
        // Sans eBPF, resolve_by_connection utilise le fallback
        // Without eBPF, resolve_by_connection uses the fallback
        let result = hybrid
            .resolve_by_connection(
                Protocol::Tcp,
                "127.0.0.1".parse().unwrap(),
                12345,
                "93.184.216.34".parse().unwrap(),
                443,
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn hybrid_resolve_by_socket_delegates_to_fallback() {
        let hybrid = HybridProcessResolver::new(None, Arc::new(FakeResolver));
        let result = hybrid.resolve_by_socket(999).await.unwrap();
        assert!(result.is_none()); // FakeResolver retourne None pour socket
    }

    #[test]
    fn read_process_info_from_proc_reads_current() {
        // Teste la lecture de notre propre processus
        // Tests reading our own process info
        let pid = std::process::id();
        let info = read_process_info_from_proc(pid);
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.pid, pid);
        assert!(!info.name.is_empty());
    }

    #[test]
    fn read_process_info_returns_none_for_invalid_pid() {
        let info = read_process_info_from_proc(999_999_999);
        assert!(info.is_none());
    }
}
