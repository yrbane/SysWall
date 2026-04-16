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

/// Lit un fichier icône et retourne un data URI (base64).
/// Permet d'afficher des icônes système (ex: Papirus) dans WebKit
/// sans avoir besoin de permissions filesystem Tauri.
///
/// Read an icon file and return a data URI (base64).
/// Allows displaying system icons (e.g., Papirus) in WebKit
/// without needing Tauri filesystem permissions.
#[tauri::command]
pub async fn read_icon(path: String) -> Result<String, String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err("Fichier introuvable".into());
    }

    // Vérifier que c'est bien un fichier d'icône (sécurité)
    // Verify it's an icon file (security)
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let mime = match ext {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "xpm" => "image/x-xpixmap",
        _ => return Err("Format non supporté".into()),
    };

    // Vérifier que le chemin est dans un répertoire d'icônes connu (sécurité)
    // Verify path is in a known icon directory (security)
    let path_str = file_path.to_string_lossy();
    if !path_str.starts_with("/usr/share/icons/")
        && !path_str.starts_with("/usr/share/pixmaps/")
        && !path_str.starts_with("/usr/local/share/icons/")
    {
        return Err("Chemin non autorisé".into());
    }

    let data = std::fs::read(file_path).map_err(|e| format!("Lecture impossible: {}", e))?;
    let b64 = base64_encode(&data);

    Ok(format!("data:{};base64,{}", mime, b64))
}

/// Encodage base64 minimal sans dépendance externe.
/// Minimal base64 encoding without external dependency.
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
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
