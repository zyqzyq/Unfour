mod commands;

// Tauri's resource selects Common Controls v6 before the Windows test harness starts.
#[cfg(all(test, target_os = "windows"))]
#[link(name = "resource", kind = "static")]
unsafe extern "C" {}

use tauri::Manager;
use unfour_command_bus::CommandBus;
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;

pub struct AppState {
    pub command_bus: CommandBus,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "unfour_workspace=info,tauri=info".into()),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let command_bus = tauri::async_runtime::block_on(async {
                let db = LocalDb::connect(&app_handle).await?;
                db.migrate().await?;
                CommandBus::from_db_with_secret_store(db, SecretStore::new("unfour-workspace"))
                    .await
            })?;

            #[cfg(feature = "ssh-native")]
            {
                let event_app = app_handle.clone();
                command_bus.set_terminal_output_callback(std::sync::Arc::new(move |payload| {
                    use tauri::Emitter;
                    if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&payload) {
                        let _ = event_app.emit("ssh://terminal-data", payload);
                    }
                }));
            }

            app.manage(AppState { command_bus });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system_health,
            commands::workspace_create,
            commands::workspace_delete,
            commands::workspace_list,
            commands::api_environments_list,
            commands::api_environment_create,
            commands::api_environment_update,
            commands::api_environment_delete,
            commands::api_environment_activate,
            commands::api_collection_list,
            commands::api_collection_create,
            commands::api_collection_rename,
            commands::api_collection_delete,
            commands::api_collection_add_folder,
            commands::api_request_move,
            commands::workspace_layout_get,
            commands::workspace_layout_update,
            commands::workspace_rename,
            commands::workspace_set_active,
            commands::api_history_detail,
            commands::api_history_list,
            commands::api_request_delete,
            commands::api_request_duplicate,
            commands::api_request_save,
            commands::api_request_update,
            commands::api_saved_requests,
            commands::api_send_request,
            commands::credential_create,
            commands::credential_delete,
            commands::credential_inspect,
            commands::credential_rotate,
            commands::database_connection_delete,
            commands::database_connection_save,
            commands::database_connection_test,
            commands::database_connections_list,
            commands::database_query_execute,
            commands::database_schema_get,
            commands::database_table_browse,
            commands::ssh_connection_delete,
            commands::ssh_connection_save,
            commands::ssh_connections_list,
            commands::ssh_host_key_get,
            commands::ssh_host_key_list,
            commands::ssh_host_key_reset,
            commands::ssh_known_hosts_export,
            commands::ssh_known_hosts_import,
            commands::ssh_session_close,
            commands::ssh_session_connect,
            commands::ssh_session_history,
            commands::ssh_session_input,
            commands::ssh_session_log_export,
            commands::ssh_session_reconnect_cancel,
            commands::ssh_session_resize,
            commands::ssh_sessions_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Unfour Workspace");
}
