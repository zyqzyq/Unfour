//! Shared Tauri composition layer.
//!
//! This crate owns everything in the app shell that is edition-independent:
//! the shared plugins, the command-bus setup, the managed [`AppState`], and the
//! `commands` adapters. Each edition's binary (core `apps/desktop`, Pro
//! `apps/desktop-pro`) supplies only its edition config, optional edition-only
//! handlers, and `generate_context!` — which are compile-time, per-binary
//! concerns — and delegates the rest to [`configure_core_app`].

pub mod commands;

use std::sync::{Arc, Mutex};
use tauri::{ipc::Channel, Manager, Runtime};
use unfour_command_bus::CommandBus;
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEdition {
    Community,
    Pro,
}

impl Default for AppEdition {
    fn default() -> Self {
        Self::Community
    }
}

impl AppEdition {
    fn diagnostics_edition(self) -> unfour_diag::Edition {
        match self {
            Self::Community => unfour_diag::Edition::Oss,
            Self::Pro => unfour_diag::Edition::Pro,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnfourAppConfig {
    pub edition: AppEdition,
    pub app_name: String,
    pub app_version: String,
    pub secret_store_namespace: String,
}

impl Default for UnfourAppConfig {
    fn default() -> Self {
        Self {
            edition: AppEdition::Community,
            app_name: "Unfour".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            secret_store_namespace: "unfour".to_string(),
        }
    }
}

/// Sink for live SSH terminal output. The frontend registers a Tauri IPC
/// [`Channel`] (via the `ssh_register_terminal_channel` command); the terminal
/// output callback then streams over it. Channels ride the same reliable
/// transport as commands, unlike the event system, which stalls under the
/// high-rate emit burst of a full-screen redraw on WebView2/Windows.
pub type TerminalChannelSlot = Arc<Mutex<Option<Channel<serde_json::Value>>>>;

pub struct AppState {
    pub command_bus: CommandBus,
    pub terminal_channel: TerminalChannelSlot,
    pub config: UnfourAppConfig,
    _logging_guard: Option<unfour_diag::LoggingGuard>,
}

/// Apply the shared plugins and command-bus setup to a Tauri builder.
///
/// The caller is responsible for the per-edition tail of the chain:
/// `.invoke_handler(unfour_app::generate_handlers![..])` and
/// `.run(tauri::generate_context!())`.
pub fn configure(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    configure_core_app(builder, UnfourAppConfig::default())
}

pub fn configure_core_app<R>(
    builder: tauri::Builder<R>,
    config: UnfourAppConfig,
) -> tauri::Builder<R>
where
    R: Runtime,
{
    builder
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let logging_guard = initialize_logging(&config);
            let secret_store_namespace = config.secret_store_namespace.clone();
            let command_bus = tauri::async_runtime::block_on(async {
                let paths = unfour_paths::initialize_unfour_storage()?;
                let db = LocalDb::connect_path(paths.database_path).await?;
                db.migrate().await?;
                CommandBus::from_db_with_secret_store(db, SecretStore::new(secret_store_namespace))
                    .await
            })?;

            let terminal_channel: TerminalChannelSlot = Arc::new(Mutex::new(None));

            #[cfg(feature = "ssh-native")]
            {
                let event_app = app.handle().clone();
                let channel_slot = terminal_channel.clone();
                command_bus.set_terminal_output_callback(std::sync::Arc::new(move |payload| {
                    use tauri::Emitter;
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload) {
                        // Prefer the IPC channel (reliable under burst). Fall back
                        // to the event system only until the frontend registers
                        // its channel.
                        let sent_via_channel = channel_slot
                            .lock()
                            .ok()
                            .and_then(|guard| {
                                guard.as_ref().map(|ch| ch.send(value.clone()).is_ok())
                            })
                            .unwrap_or(false);
                        if !sent_via_channel {
                            let _ = event_app.emit("ssh://terminal-data", value);
                        }
                    }
                }));
            }

            app.manage(AppState {
                command_bus,
                terminal_channel,
                config,
                _logging_guard: logging_guard,
            });

            Ok(())
        })
}

fn initialize_logging(config: &UnfourAppConfig) -> Option<unfour_diag::LoggingGuard> {
    let paths = unfour_paths::initialize_unfour_storage().ok()?;
    let mut logging_config = unfour_diag::LoggingConfig::oss_dev(paths.logs_dir);
    logging_config.app_name = config.app_name.clone();
    logging_config.edition = config.edition.diagnostics_edition();
    logging_config.version = config.app_version.clone();
    if !cfg!(debug_assertions) {
        logging_config.channel = unfour_diag::Channel::Stable;
        logging_config.package_kind = unfour_diag::PackageKind::Website;
        logging_config.log_level = "info".to_string();
    }
    unfour_diag::init_logging(logging_config).ok()
}

pub fn diagnostic_bundle_request(
    config: &UnfourAppConfig,
    paths: unfour_paths::UnfourPaths,
) -> unfour_diag::DiagnosticBundleRequest {
    let mut request =
        unfour_diag::DiagnosticBundleRequest::oss_dev(config.app_version.clone(), paths);
    request.app_name = config.app_name.clone();
    request.edition = config.edition.diagnostics_edition();
    if !cfg!(debug_assertions) {
        request.channel = unfour_diag::Channel::Stable;
        request.package_kind = unfour_diag::PackageKind::Website;
    }
    request
}

#[macro_export]
macro_rules! generate_handlers {
    ( $( $handler:path ),* $(,)? ) => {
        tauri::generate_handler![
            unfour_app::commands::export_diagnostics_bundle,
            unfour_app::commands::frontend_log,
            unfour_app::commands::open_diagnostics_dir,
            unfour_app::commands::open_log_dir,
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
            unfour_app::commands::api_collection_folders_list,
            unfour_app::commands::api_collection_folder_create,
            unfour_app::commands::api_collection_folder_rename,
            unfour_app::commands::api_collection_folder_delete,
            unfour_app::commands::api_collection_folder_move,
            unfour_app::commands::api_collection_folders_reorder,
            unfour_app::commands::api_request_move,
            unfour_app::commands::api_requests_reorder,
            unfour_app::commands::workspace_layout_get,
            unfour_app::commands::workspace_layout_update,
            unfour_app::commands::workspace_rename,
            unfour_app::commands::workspace_set_active,
            unfour_app::commands::workspace_update_environment,
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
            unfour_app::commands::database_connection_test_input,
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
            unfour_app::commands::ssh_connection_test,
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
            $( $handler ),*
        ]
    };
}
