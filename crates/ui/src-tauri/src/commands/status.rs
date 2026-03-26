//! Status command — get firewall status.

use tauri::State;

use syswall_proto::syswall::Empty;

use crate::grpc_client::GrpcState;

/// Response type for the frontend (serializable).
#[derive(serde::Serialize)]
pub struct StatusResult {
    pub enabled: bool,
    pub active_rules_count: u32,
    pub nftables_synced: bool,
    pub uptime_secs: u64,
    pub version: String,
}

/// Get the current firewall status from the daemon.
#[tauri::command]
pub async fn get_status(state: State<'_, GrpcState>) -> Result<StatusResult, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .get_status(Empty {})
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    let status = response.into_inner();

    Ok(StatusResult {
        enabled: status.enabled,
        active_rules_count: status.active_rules_count,
        nftables_synced: status.nftables_synced,
        uptime_secs: status.uptime_secs,
        version: status.version,
    })
}
