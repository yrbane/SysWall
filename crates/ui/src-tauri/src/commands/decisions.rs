//! Decision commands — manage pending auto-learning decisions.

use tauri::State;

use syswall_proto::syswall::{DecisionResponseRequest, Empty};

use crate::grpc_client::GrpcState;

/// Serializable pending decision for the frontend.
#[derive(serde::Serialize, Clone)]
pub struct PendingDecisionResult {
    pub id: String,
    pub snapshot_json: String,
    pub requested_at: String,
    pub expires_at: String,
    pub status: String,
}

/// Input for responding to a decision.
#[derive(serde::Deserialize)]
pub struct DecisionResponseInput {
    pub pending_decision_id: String,
    pub action: String,
    pub granularity: String,
}

/// List all pending decisions.
#[tauri::command]
pub async fn list_pending_decisions(
    state: State<'_, GrpcState>,
) -> Result<Vec<PendingDecisionResult>, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .list_pending_decisions(Empty {})
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    let decisions = response
        .into_inner()
        .decisions
        .into_iter()
        .map(|d| PendingDecisionResult {
            id: d.id,
            snapshot_json: d.snapshot_json,
            requested_at: d.requested_at,
            expires_at: d.expires_at,
            status: d.status,
        })
        .collect();

    Ok(decisions)
}

/// Respond to a pending decision.
#[tauri::command]
pub async fn respond_to_decision(
    state: State<'_, GrpcState>,
    input: DecisionResponseInput,
) -> Result<String, String> {
    let mut client = state.get_client().await?;

    let response = client
        .control
        .respond_to_decision(DecisionResponseRequest {
            pending_decision_id: input.pending_decision_id,
            action: input.action,
            granularity: input.granularity,
        })
        .await
        .map_err(|e| format!("gRPC error: {}", e))?;

    Ok(response.into_inner().decision_id)
}
