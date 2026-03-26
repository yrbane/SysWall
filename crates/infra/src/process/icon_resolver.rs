/// Resolves application icons from .desktop files and icon themes.
/// Résout les icônes d'applications à partir des fichiers .desktop et des thèmes d'icônes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing::debug;

/// Cached icon resolver for Linux desktop applications.
/// Résolveur d'icônes avec cache pour les applications Linux desktop.
pub struct IconResolver {
    /// Maps executable name -> icon file path
    cache: Mutex<HashMap<String, Option<String>>>,
    /// Pre-indexed: executable name -> desktop Icon= value
    desktop_index: HashMap<String, String>,
}

impl IconResolver {
    /// Build a new resolver by indexing all .desktop files.
    /// Construit un nouveau résolveur en indexant tous les fichiers .desktop.
    pub fn new() -> Self {
        let desktop_index = Self::index_desktop_files();
        debug!(
            "IconResolver: indexed {} desktop entries",
            desktop_index.len()
        );
        Self {
            cache: Mutex::new(HashMap::new()),
            desktop_index,
        }
    }

    /// Resolve an icon path for an executable name (e.g., "firefox") or full path.
    /// Returns the path to a PNG/SVG icon file, or None.
    ///
    /// Résout le chemin d'une icône pour un nom d'exécutable ou un chemin complet.
    /// Retourne le chemin vers un fichier PNG/SVG, ou None.
    pub fn resolve(&self, exe_name: &str, exe_path: Option<&Path>) -> Option<String> {
        // Check cache
        if let Some(cached) = self.cache.lock().unwrap().get(exe_name) {
            return cached.clone();
        }

        let result = self.resolve_inner(exe_name, exe_path);

        // Cache the result
        self.cache
            .lock()
            .unwrap()
            .insert(exe_name.to_string(), result.clone());

        result
    }

    fn resolve_inner(&self, exe_name: &str, exe_path: Option<&Path>) -> Option<String> {
        // 1. Try direct match by executable name in desktop index
        if let Some(icon_name) = self.desktop_index.get(exe_name) {
            if let Some(path) = Self::find_icon_file(icon_name) {
                return Some(path);
            }
        }

        // 2. Try by basename of exe_path
        if let Some(path) = exe_path {
            if let Some(basename) = path.file_name().and_then(|n| n.to_str()) {
                if basename != exe_name {
                    if let Some(icon_name) = self.desktop_index.get(basename) {
                        if let Some(path) = Self::find_icon_file(icon_name) {
                            return Some(path);
                        }
                    }
                }
            }
        }

        // 3. Try the exe_name directly as an icon name
        if let Some(path) = Self::find_icon_file(exe_name) {
            return Some(path);
        }

        None
    }

    /// Scan .desktop files and build executable name -> Icon= mapping.
    fn index_desktop_files() -> HashMap<String, String> {
        let mut index = HashMap::new();

        let dirs = [
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
            dirs::data_dir()
                .unwrap_or_default()
                .join("applications"),
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
                // Extract the binary name from the Exec= line
                // e.g., "Exec=/usr/bin/firefox %u" -> "firefox"
                // e.g., "Exec=env VAR=1 steam %U" -> "steam"
                let parts: Vec<&str> = value.trim().split_whitespace().collect();
                for part in &parts {
                    let p = Path::new(part);
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        // Skip env vars and common wrappers
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

        // Also derive the desktop file basename as an alternative name
        // e.g., "org.mozilla.firefox.desktop" -> "org.mozilla.firefox"
        let mut names = vec![exec];
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            names.push(stem.to_string());
        }

        Some((names, icon))
    }

    /// Find an icon file by icon name, searching common icon theme paths.
    /// Preferred sizes: 48x48, 64x64, 128x128, scalable, then anything.
    fn find_icon_file(icon_name: &str) -> Option<String> {
        // If it's already an absolute path, return it
        if icon_name.starts_with('/') {
            if Path::new(icon_name).exists() {
                return Some(icon_name.to_string());
            }
            return None;
        }

        let themes = ["hicolor", "Adwaita", "breeze"];
        let preferred_sizes = ["48x48", "64x64", "128x128", "32x32", "256x256", "scalable"];
        let extensions = ["png", "svg", "xpm"];

        for theme in &themes {
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
        }

        // Try pixmaps as fallback
        for ext in &extensions {
            let path = format!("/usr/share/pixmaps/{}.{}", icon_name, ext);
            if Path::new(&path).exists() {
                return Some(path);
            }
        }

        None
    }
}
