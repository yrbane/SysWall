//! SysWall UI — Tauri application entry point.

mod commands;
mod grpc_client;
mod streams;

use grpc_client::{GrpcClient, GrpcState};
use tracing::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let grpc_state = GrpcState::new();
    let client_arc = grpc_state.client.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(grpc_state)
        .invoke_handler(tauri::generate_handler![
            commands::status::get_status,
            commands::rules::list_rules,
            commands::rules::create_rule,
            commands::rules::delete_rule,
            commands::rules::toggle_rule,
            commands::decisions::list_pending_decisions,
            commands::decisions::respond_to_decision,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let client = client_arc.clone();

            // Spawn background task: connect to daemon and subscribe to events
            tauri::async_runtime::spawn(async move {
                info!("Attempting initial connection to daemon...");

                match GrpcClient::connect(None).await {
                    Ok(grpc_client) => {
                        info!("Initial daemon connection successful");
                        let mut guard = client.lock().await;
                        *guard = Some(grpc_client);
                        drop(guard);

                        // Start event stream forwarding
                        streams::subscribe_and_forward(app_handle, client).await;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Initial daemon connection failed: {}. Will retry on demand.",
                            e
                        );
                        // Event stream will retry in its loop
                        streams::subscribe_and_forward(app_handle, client).await;
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
