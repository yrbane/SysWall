//! Connection commands — snapshot of active connections for UI seeding.
//! Commandes de connexion — instantané des connexions actives pour amorcer l'UI.

use tauri::State;

use crate::grpc_client::GrpcState;

/// A domain event message describing an active connection.
/// Mirrors the proto `DomainEventMessage` (event_type = "connection_detected").
///
/// Un message d'événement de domaine décrivant une connexion active.
/// Reflète le `DomainEventMessage` proto (event_type = "connection_detected").
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ConnectionEventResult {
    pub event_type: String,
    pub payload_json: String,
    pub timestamp: String,
}

/// Fetch a snapshot of the currently active connections from the daemon.
/// Each entry is a "connection_detected" event so the frontend can reuse its
/// existing event-rendering logic to seed the store.
///
/// Récupère un instantané des connexions actuellement actives depuis le démon.
/// Chaque entrée est un événement "connection_detected" afin que le frontend
/// réutilise sa logique de rendu d'événements pour amorcer le store.
#[tauri::command]
pub async fn get_active_connections(
    state: State<'_, GrpcState>,
) -> Result<Vec<ConnectionEventResult>, String> {
    let mut client = state.get_client().await?;

    let connections = client.get_active_connections().await?;

    let result = connections
        .into_iter()
        .map(|c| ConnectionEventResult {
            event_type: c.event_type,
            payload_json: c.payload_json,
            timestamp: c.timestamp,
        })
        .collect();

    Ok(result)
}
