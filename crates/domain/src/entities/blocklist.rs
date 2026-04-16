use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Une blocklist de domaines ou d'adresses IP.
/// A blocklist of domains or IP addresses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blocklist {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub entry_count: usize,
    pub last_loaded: DateTime<Utc>,
}
