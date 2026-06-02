mod ai_reserved;
mod app_error;
mod audit_log;
mod command_bus;
mod commands;
mod local_db;
mod models;
mod services;
mod sync_reserved;

use command_bus::CommandBus;
use tauri::Manager;

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
            let command_bus = tauri::async_runtime::block_on(CommandBus::new(app_handle))?;
            app.manage(AppState { command_bus });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system_health,
            commands::workspace_create,
            commands::workspace_delete,
            commands::workspace_list,
            commands::workspace_environment_get,
            commands::workspace_environment_update,
            commands::workspace_layout_get,
            commands::workspace_layout_update,
            commands::workspace_rename,
            commands::workspace_set_active,
            commands::api_history_detail,
            commands::api_history_list,
            commands::api_request_save,
            commands::api_saved_requests,
            commands::api_send_request,
            commands::database_connection_delete,
            commands::database_connection_save,
            commands::database_connection_test,
            commands::database_connections_list,
            commands::database_query_execute,
            commands::database_schema_get,
            commands::database_table_browse,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Unfour Workspace");
}
