//! Event stream subscriber — listens to daemon gRPC events and emits Tauri events.

use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use syswall_proto::syswall::SubscribeRequest;

use crate::grpc_client::GrpcClient;

/// Map gRPC event_type strings to Tauri event names.
fn map_event_name(event_type: &str) -> &str {
    match event_type {
        "connection_detected" => "syswall://connection-detected",
        "connection_updated" => "syswall://connection-updated",
        "connection_closed" => "syswall://connection-closed",
        "rule_created" => "syswall://rule-created",
        "rule_updated" => "syswall://rule-updated",
        "rule_deleted" => "syswall://rule-deleted",
        "rule_matched" => "syswall://rule-matched",
        "decision_required" => "syswall://decision-required",
        "decision_resolved" => "syswall://decision-resolved",
        "decision_expired" => "syswall://decision-expired",
        "firewall_status_changed" => "syswall://status-changed",
        "system_error" => "syswall://system-error",
        other => {
            warn!("Unknown event type: {}", other);
            "syswall://unknown"
        }
    }
}

/// Payload emitted to the frontend for each event.
#[derive(Clone, serde::Serialize)]
pub struct EventPayload {
    pub event_type: String,
    pub payload_json: String,
    pub timestamp: String,
}

/// Subscribe to the daemon event stream and forward events to the Tauri frontend.
pub async fn subscribe_and_forward(
    app_handle: AppHandle,
    client: Arc<Mutex<Option<GrpcClient>>>,
) {
    loop {
        let events_client = {
            let guard = client.lock().await;
            match guard.as_ref() {
                Some(c) => c.events.clone(),
                None => {
                    warn!("No gRPC client available, waiting before retry...");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            }
        };

        info!("Subscribing to daemon event stream...");

        let stream_result = events_client
            .clone()
            .subscribe_events(SubscribeRequest {})
            .await;

        match stream_result {
            Ok(response) => {
                let mut stream = response.into_inner();
                info!("Event stream connected");

                loop {
                    match stream.message().await {
                        Ok(Some(msg)) => {
                            let event_name = map_event_name(&msg.event_type);
                            let payload = EventPayload {
                                event_type: msg.event_type.clone(),
                                payload_json: msg.payload_json.clone(),
                                timestamp: msg.timestamp.clone(),
                            };

                            if let Err(e) = app_handle.emit(event_name, &payload) {
                                error!("Failed to emit event {}: {}", event_name, e);
                            }
                        }
                        Ok(None) => {
                            warn!("Event stream ended, will reconnect...");
                            break;
                        }
                        Err(e) => {
                            error!("Event stream error: {}, will reconnect...", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to subscribe to events: {}", e);
            }
        }

        // Wait before reconnecting
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
