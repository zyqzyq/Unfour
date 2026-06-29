// Tauri's resource selects Common Controls v6 before the Windows test harness starts.
#[cfg(all(test, target_os = "windows"))]
#[link(name = "resource", kind = "static")]
unsafe extern "C" {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "unfour=info,tauri=info".into()),
        )
        .try_init();

    // Shared plugins, command-bus setup and AppState live in `unfour-app`.
    // This binary owns only the per-edition handler list and Tauri context.
    unfour_app::configure(tauri::Builder::default())
        .invoke_handler(tauri::generate_handler![
            unfour_app::commands::system_health,
            unfour_app::commands::workspace_create,
            unfour_app::commands::workspace_delete,
            unfour_app::commands::workspace_list,
            unfour_app::commands::api_environments_list,
            unfour_app::commands::api_environment_create,
            unfour_app::commands::api_environment_update,
            unfour_app::commands::api_environment_delete,
            unfour_app::commands::api_environment_activate,
            unfour_app::commands::api_collection_list,
            unfour_app::commands::api_collection_create,
            unfour_app::commands::api_collection_rename,
            unfour_app::commands::api_collection_delete,
            unfour_app::commands::api_collection_add_folder,
            unfour_app::commands::api_request_move,
            unfour_app::commands::workspace_layout_get,
            unfour_app::commands::workspace_layout_update,
            unfour_app::commands::workspace_rename,
            unfour_app::commands::workspace_set_active,
            unfour_app::commands::api_history_detail,
            unfour_app::commands::api_history_list,
            unfour_app::commands::api_request_delete,
            unfour_app::commands::api_request_duplicate,
            unfour_app::commands::api_request_save,
            unfour_app::commands::api_request_update,
            unfour_app::commands::api_saved_requests,
            unfour_app::commands::api_send_request,
            unfour_app::commands::credential_create,
            unfour_app::commands::credential_delete,
            unfour_app::commands::credential_inspect,
            unfour_app::commands::credential_rotate,
            unfour_app::commands::database_connection_delete,
            unfour_app::commands::database_connection_save,
            unfour_app::commands::database_catalogs_list,
            unfour_app::commands::database_connection_test,
            unfour_app::commands::database_connections_list,
            unfour_app::commands::database_query_execute,
            unfour_app::commands::database_row_mutate,
            unfour_app::commands::database_query_history_clear,
            unfour_app::commands::database_query_history_list,
            unfour_app::commands::database_query_history_record,
            unfour_app::commands::database_saved_sql_delete,
            unfour_app::commands::database_saved_sql_list,
            unfour_app::commands::database_saved_sql_save,
            unfour_app::commands::database_schema_get,
            unfour_app::commands::database_table_browse,
            unfour_app::commands::database_table_structure,
            unfour_app::commands::ssh_connection_delete,
            unfour_app::commands::ssh_connection_save,
            unfour_app::commands::ssh_connections_list,
            unfour_app::commands::ssh_host_key_get,
            unfour_app::commands::ssh_host_key_list,
            unfour_app::commands::ssh_host_key_reset,
            unfour_app::commands::ssh_known_hosts_export,
            unfour_app::commands::ssh_known_hosts_import,
            unfour_app::commands::ssh_session_close,
            unfour_app::commands::ssh_session_connect,
            unfour_app::commands::ssh_session_history,
            unfour_app::commands::ssh_session_input,
            unfour_app::commands::ssh_register_terminal_channel,
            unfour_app::commands::ssh_session_log_export,
            unfour_app::commands::ssh_session_reconnect_cancel,
            unfour_app::commands::ssh_session_resize,
            unfour_app::commands::ssh_sessions_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Unfour");
}
