//! Résout les icônes d'applications depuis les fichiers .desktop et les thèmes d'icônes.
//! Suit le thème GTK actif du système (ex: Papirus, Adwaita) avec chaîne d'héritage.
//!
//! Resolves application icons from .desktop files and icon themes.
//! Follows the active GTK icon theme (e.g., Papirus, Adwaita) with inheritance chain.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing::{debug, info};

/// Résolveur d'icônes avec cache pour les applications Linux desktop.
/// Cached icon resolver for Linux desktop applications.
pub struct IconResolver {
    cache: Mutex<HashMap<String, Option<String>>>,
    desktop_index: HashMap<String, String>,
    /// Chaîne de thèmes d'icônes à chercher (thème actif + héritage + hicolor)
    /// Icon theme chain to search (active theme + inheritance + hicolor)
    theme_chain: Vec<String>,
}

impl Default for IconResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl IconResolver {
    /// Construit un nouveau résolveur en détectant le thème GTK et en indexant les .desktop.
    /// Build a new resolver by detecting the GTK theme and indexing .desktop files.
    pub fn new() -> Self {
        let theme_chain = Self::detect_theme_chain();
        info!("IconResolver: thème d'icônes détecté : {:?}", theme_chain);

        let desktop_index = Self::index_desktop_files();
        debug!(
            "IconResolver: {} entrées desktop indexées",
            desktop_index.len()
        );

        Self {
            cache: Mutex::new(HashMap::new()),
            desktop_index,
            theme_chain,
        }
    }

    /// Résout le chemin d'une icône pour un nom d'exécutable ou un chemin complet.
    /// Resolve an icon path for an executable name or full path.
    pub fn resolve(&self, exe_name: &str, exe_path: Option<&Path>) -> Option<String> {
        if let Some(cached) = self
            .cache
            .lock()
            .expect("Mutex jamais empoisonné / never poisoned")
            .get(exe_name)
        {
            return cached.clone();
        }

        let result = self.resolve_inner(exe_name, exe_path);

        self.cache
            .lock()
            .expect("Mutex jamais empoisonné / never poisoned")
            .insert(exe_name.to_string(), result.clone());

        result
    }

    fn resolve_inner(&self, exe_name: &str, exe_path: Option<&Path>) -> Option<String> {
        // 1. Chercher par nom d'exécutable dans l'index desktop
        // 1. Search by executable name in desktop index
        if let Some(icon_name) = self.desktop_index.get(exe_name)
            && let Some(path) = self.find_icon_file(icon_name)
        {
            return Some(path);
        }

        // 2. Chercher par basename du chemin de l'exécutable
        // 2. Search by basename of executable path
        if let Some(path) = exe_path
            && let Some(basename) = path.file_name().and_then(|n| n.to_str())
            && basename != exe_name
            && let Some(icon_name) = self.desktop_index.get(basename)
            && let Some(path) = self.find_icon_file(icon_name)
        {
            return Some(path);
        }

        // 3. Essayer le nom de l'exécutable directement comme nom d'icône
        // 3. Try the executable name directly as icon name
        if let Some(path) = self.find_icon_file(exe_name) {
            return Some(path);
        }

        // 4. Essayer en minuscules
        // 4. Try lowercase
        let lower = exe_name.to_lowercase();
        if lower != exe_name
            && let Some(path) = self.find_icon_file(&lower)
        {
            return Some(path);
        }

        None
    }

    /// Détecte le thème d'icônes GTK actif et construit la chaîne d'héritage.
    /// Detect the active GTK icon theme and build the inheritance chain.
    fn detect_theme_chain() -> Vec<String> {
        let mut chain = Vec::new();

        // Détecter le thème actif via gsettings
        // Detect active theme via gsettings
        let active_theme = Self::detect_active_theme();

        if let Some(ref theme) = active_theme {
            Self::build_inheritance_chain(theme, &mut chain);
        }

        // Toujours hicolor en dernier fallback
        // Always hicolor as last fallback
        if !chain.contains(&"hicolor".to_string()) {
            chain.push("hicolor".to_string());
        }

        chain
    }

    /// Détecte le thème d'icônes actif. Quand on tourne en root (daemon systemd),
    /// gsettings ne fonctionne pas — on lit les fichiers de config GTK des utilisateurs.
    ///
    /// Detect active icon theme. When running as root (systemd daemon),
    /// gsettings doesn't work — we read users' GTK config files instead.
    fn detect_active_theme() -> Option<String> {
        // 1. Variable d'environnement explicite
        // 1. Explicit environment variable
        if let Ok(theme) = std::env::var("GTK_ICON_THEME") {
            return Some(theme);
        }

        // 2. Essayer gsettings (fonctionne si session D-Bus disponible)
        // 2. Try gsettings (works if D-Bus session is available)
        if let Ok(output) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "icon-theme"])
            .output()
            && output.status.success()
        {
            let theme = String::from_utf8_lossy(&output.stdout)
                .trim()
                .trim_matches('\'')
                .to_string();
            // Ignorer "Adwaita" si on est root — c'est probablement le défaut, pas le choix de l'utilisateur
            // Ignore "Adwaita" if running as root — it's likely the default, not the user's choice
            if !(theme.is_empty() || theme == "Adwaita" && nix::unistd::getuid().is_root()) {
                return Some(theme);
            }
        }

        // 3. Scanner les fichiers settings.ini des utilisateurs réels (UID >= 1000)
        //    C'est la méthode fiable quand le daemon tourne en root via systemd
        // 3. Scan real users' settings.ini (UID >= 1000)
        //    This is the reliable method when daemon runs as root via systemd
        if let Ok(entries) = std::fs::read_dir("/home") {
            for entry in entries.flatten() {
                let settings_path = entry.path().join(".config/gtk-3.0/settings.ini");
                if let Ok(content) = std::fs::read_to_string(&settings_path) {
                    for line in content.lines() {
                        if let Some(theme) = line.strip_prefix("gtk-icon-theme-name=") {
                            let theme = theme.trim().to_string();
                            if !theme.is_empty() {
                                debug!(
                                    "Thème d'icônes trouvé dans {}: {}",
                                    settings_path.display(),
                                    theme
                                );
                                return Some(theme);
                            }
                        }
                    }
                }
                // Essayer aussi gtk-4.0
                // Also try gtk-4.0
                let settings_path_4 = entry.path().join(".config/gtk-4.0/settings.ini");
                if let Ok(content) = std::fs::read_to_string(&settings_path_4) {
                    for line in content.lines() {
                        if let Some(theme) = line.strip_prefix("gtk-icon-theme-name=") {
                            let theme = theme.trim().to_string();
                            if !theme.is_empty() {
                                return Some(theme);
                            }
                        }
                    }
                }
            }
        }

        // 4. Lire le config du processus courant (fallback)
        // 4. Read current process config (fallback)
        if let Some(config_dir) = dirs::config_dir() {
            let settings_path = config_dir.join("gtk-3.0/settings.ini");
            if let Ok(content) = std::fs::read_to_string(settings_path) {
                for line in content.lines() {
                    if let Some(theme) = line.strip_prefix("gtk-icon-theme-name=") {
                        return Some(theme.trim().to_string());
                    }
                }
            }
        }

        // 5. Fallback : Adwaita (défaut GNOME)
        // 5. Fallback: Adwaita (GNOME default)
        Some("Adwaita".to_string())
    }

    /// Construit la chaîne d'héritage en lisant Inherits= dans index.theme.
    /// Build inheritance chain by reading Inherits= from index.theme.
    fn build_inheritance_chain(theme: &str, chain: &mut Vec<String>) {
        if chain.contains(&theme.to_string()) {
            return; // Éviter les boucles / Avoid loops
        }
        chain.push(theme.to_string());

        // Lire Inherits= depuis index.theme
        // Read Inherits= from index.theme
        let index_path = format!("/usr/share/icons/{}/index.theme", theme);
        if let Ok(content) = std::fs::read_to_string(&index_path) {
            for line in content.lines() {
                if let Some(inherits) = line.strip_prefix("Inherits=") {
                    for parent in inherits.split(',') {
                        let parent = parent.trim();
                        if !parent.is_empty() {
                            Self::build_inheritance_chain(parent, chain);
                        }
                    }
                    break;
                }
            }
        }
    }

    /// Cherche un fichier icône dans la chaîne de thèmes.
    /// Search for an icon file in the theme chain.
    fn find_icon_file(&self, icon_name: &str) -> Option<String> {
        // Si c'est déjà un chemin absolu, le retourner directement
        // If it's already an absolute path, return it directly
        if icon_name.starts_with('/') {
            if Path::new(icon_name).exists() {
                return Some(icon_name.to_string());
            }
            return None;
        }

        let preferred_sizes = [
            "48x48", "64x64", "32x32", "128x128", "256x256", "scalable", "24x24", "22x22", "16x16",
        ];
        let extensions = ["svg", "png", "xpm"];

        // Chercher dans chaque thème de la chaîne
        // Search in each theme in the chain
        for theme in &self.theme_chain {
            for size in &preferred_sizes {
                for ext in &extensions {
                    let path = format!(
                        "/usr/share/icons/{}/{}/apps/{}.{}",
                        theme, size, icon_name, ext
                    );
                    if Path::new(&path).exists() {
                        return Some(path);
                    }
                }
            }

            // Certains thèmes utilisent des sous-dossiers différents
            // Some themes use different subdirectory structures
            for category in &["apps", "categories", "mimetypes"] {
                for ext in &extensions {
                    let path = format!(
                        "/usr/share/icons/{}/48x48/{}/{}.{}",
                        theme, category, icon_name, ext
                    );
                    if Path::new(&path).exists() {
                        return Some(path);
                    }
                }
            }
        }

        // Fallback : pixmaps
        // Fallback: pixmaps
        for ext in &extensions {
            let path = format!("/usr/share/pixmaps/{}.{}", icon_name, ext);
            if Path::new(&path).exists() {
                return Some(path);
            }
        }

        None
    }

    /// Indexe tous les fichiers .desktop pour mapper nom d'exécutable → nom d'icône.
    /// Index all .desktop files to map executable name → icon name.
    fn index_desktop_files() -> HashMap<String, String> {
        let mut index = HashMap::new();

        let dirs = [
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
            dirs::data_dir().unwrap_or_default().join("applications"),
        ];

        for dir in &dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                        continue;
                    }

                    if let Some((exec_names, icon)) = Self::parse_desktop_file(&path) {
                        for name in exec_names {
                            index.insert(name, icon.clone());
                        }
                    }
                }
            }
        }

        index
    }

    /// Parse un fichier .desktop et retourne (noms d'exécutables, nom d'icône).
    /// Parse a .desktop file, returning (executable names, icon name).
    fn parse_desktop_file(path: &Path) -> Option<(Vec<String>, String)> {
        let content = std::fs::read_to_string(path).ok()?;

        let mut icon = None;
        let mut exec = None;
        let mut in_desktop_entry = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed == "[Desktop Entry]" {
                in_desktop_entry = true;
                continue;
            }
            if trimmed.starts_with('[') {
                in_desktop_entry = false;
                continue;
            }

            if !in_desktop_entry {
                continue;
            }

            if let Some(value) = trimmed.strip_prefix("Icon=") {
                icon = Some(value.trim().to_string());
            }
            if let Some(value) = trimmed.strip_prefix("Exec=") {
                let parts: Vec<&str> = value.split_whitespace().collect();
                for part in &parts {
                    let p = Path::new(part);
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        if name == "env" || name.contains('=') {
                            continue;
                        }
                        exec = Some(name.to_string());
                        break;
                    }
                }
            }
        }

        let icon = icon?;
        let exec = exec?;

        let mut names = vec![exec];
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            names.push(stem.to_string());
        }

        Some((names, icon))
    }
}
