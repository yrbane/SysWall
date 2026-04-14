//! Commande de détails processus — lit /proc/<pid>/ pour obtenir des infos détaillées.
//! Process details command — reads /proc/<pid>/ for detailed information.

use std::path::PathBuf;

/// Informations détaillées sur un processus.
/// Detailed process information.
#[derive(serde::Serialize)]
pub struct ProcessDetails {
    pub pid: u32,
    pub name: String,
    pub exe: String,
    pub cmdline: String,
    pub cwd: String,
    pub user: String,
    pub uid: u32,
    pub state: String,
    pub threads: u32,
    pub memory_rss_kb: u64,
    pub open_fds: u32,
    pub start_time: String,
    pub ports: Vec<PortInfo>,
    pub environ: Vec<String>,
}

/// Information sur un port ouvert par le processus.
/// Information about a port opened by the process.
#[derive(serde::Serialize)]
pub struct PortInfo {
    pub protocol: String,
    pub local_port: u16,
    pub remote: String,
    pub state: String,
}

/// Récupère les détails d'un processus à partir de son PID.
/// Get process details from its PID.
#[tauri::command]
pub async fn get_process_details(pid: u32) -> Result<ProcessDetails, String> {
    let proc_path = PathBuf::from(format!("/proc/{}", pid));
    if !proc_path.exists() {
        return Err(format!("Processus {} introuvable", pid));
    }

    // Nom et état depuis /proc/<pid>/status
    // Name and state from /proc/<pid>/status
    let status_content = std::fs::read_to_string(proc_path.join("status"))
        .map_err(|e| format!("Impossible de lire le status: {}", e))?;

    let mut name = String::new();
    let mut state = String::new();
    let mut uid: u32 = 0;
    let mut threads: u32 = 0;
    let mut rss_pages: u64 = 0;

    for line in status_content.lines() {
        if let Some(val) = line.strip_prefix("Name:") {
            name = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("State:") {
            state = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("Uid:") {
            uid = val.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("Threads:") {
            threads = val.trim().parse().unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("VmRSS:") {
            rss_pages = val.trim().split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
        }
    }

    // Chemin de l'exécutable
    // Executable path
    let exe = std::fs::read_link(proc_path.join("exe"))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "--".to_string());

    // Ligne de commande
    // Command line
    let cmdline = std::fs::read(proc_path.join("cmdline"))
        .map(|b| String::from_utf8_lossy(&b).replace('\0', " ").trim().to_string())
        .unwrap_or_default();

    // Répertoire de travail
    // Working directory
    let cwd = std::fs::read_link(proc_path.join("cwd"))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "--".to_string());

    // Nom d'utilisateur depuis l'UID
    // Username from UID
    let user = nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(uid))
        .ok()
        .flatten()
        .map(|u| u.name)
        .unwrap_or_else(|| uid.to_string());

    // Nombre de fichiers ouverts
    // Open file descriptor count
    let open_fds = std::fs::read_dir(proc_path.join("fd"))
        .map(|entries| entries.count() as u32)
        .unwrap_or(0);

    // Variables d'environnement (les 10 premières)
    // Environment variables (first 10)
    let environ = std::fs::read(proc_path.join("environ"))
        .map(|b| {
            String::from_utf8_lossy(&b)
                .split('\0')
                .filter(|s| !s.is_empty())
                .take(10)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Ports ouverts par ce processus via /proc/net/tcp + /proc/net/udp
    // Open ports by this process via ss
    let ports = get_process_ports(pid);

    // Heure de démarrage approximative depuis /proc/<pid>/stat
    // Approximate start time from /proc/<pid>/stat
    let start_time = get_start_time(pid);

    Ok(ProcessDetails {
        pid,
        name,
        exe,
        cmdline,
        cwd,
        user,
        uid,
        state,
        threads,
        memory_rss_kb: rss_pages,
        open_fds,
        start_time,
        ports,
        environ,
    })
}

/// Récupère les ports ouverts par un processus via ss.
/// Get ports opened by a process via ss.
fn get_process_ports(pid: u32) -> Vec<PortInfo> {
    let mut ports = Vec::new();
    let pid_str = format!("pid={}", pid);

    for (args, protocol) in &[(&["-tnp"][..], "TCP"), (&["-unp"][..], "UDP")] {
        let output = match std::process::Command::new("/usr/bin/ss")
            .args(*args)
            .output()
        {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => continue,
        };

        for line in output.lines().skip(1) {
            if !line.contains(&pid_str) {
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 5 {
                continue;
            }
            // State LocalAddr:Port PeerAddr:Port
            let state = fields[0].to_string();
            let local = fields[3];
            let remote = fields[4].to_string();

            let local_port = local
                .rsplit(':')
                .next()
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(0);

            ports.push(PortInfo {
                protocol: protocol.to_string(),
                local_port,
                remote,
                state,
            });
        }
    }
    ports
}

/// Récupère l'heure de démarrage approximative du processus.
/// Get approximate process start time.
fn get_start_time(pid: u32) -> String {
    // Utilise la date de création de /proc/<pid>
    // Use /proc/<pid> creation time
    let metadata = match std::fs::metadata(format!("/proc/{}", pid)) {
        Ok(m) => m,
        Err(_) => return "--".to_string(),
    };

    metadata
        .modified()
        .ok()
        .map(|t| {
            let datetime: chrono::DateTime<chrono::Local> = t.into();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        })
        .unwrap_or_else(|| "--".to_string())
}
