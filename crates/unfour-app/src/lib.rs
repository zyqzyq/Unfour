//! Shared Tauri composition layer.
//!
//! This crate owns the shared plugins, command-bus setup, managed [`AppState`],
//! and core command adapters. The single desktop binary may prepare a command
//! bus with capability hooks before delegating the remaining Tauri composition
//! to [`configure_core_app_with_extensions`].

pub mod commands;

use std::sync::{Arc, Mutex};
use tauri::{ipc::Channel, Manager, Runtime};
use unfour_command_bus::{CommandBus, CommandBusExtensions};
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;

/// The single, shared release channel type. Only two channels exist
/// project-wide: `Test` (pre-release / local dev) and `Stable` (formal
/// release). The channel is decided at build time by the host binary and is
/// never inferred from `debug_assertions` or the cargo profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseChannel {
    Test,
    Stable,
}

impl ReleaseChannel {
    /// Stable API value surfaced to the frontend and diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Stable => "stable",
        }
    }

    fn diagnostics_channel(self) -> unfour_diag::Channel {
        match self {
            Self::Test => unfour_diag::Channel::Test,
            Self::Stable => unfour_diag::Channel::Stable,
        }
    }
}

/// The final single-repository distribution model. Standard artifacts are
/// published byte-for-byte to GitHub Releases and Cloudflare R2; Microsoft
/// Store builds use MSIX and delegate update authority to the Store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppDistribution {
    Standard,
    MicrosoftStore,
}

impl AppDistribution {
    /// Stable API value surfaced to the frontend and diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::MicrosoftStore => "microsoft-store",
        }
    }

    fn diagnostics_distribution(self) -> unfour_diag::Distribution {
        match self {
            Self::Standard => unfour_diag::Distribution::Standard,
            Self::MicrosoftStore => unfour_diag::Distribution::MicrosoftStore,
        }
    }
}

/// The OS keychain service namespace for stored credentials. This is a stable
/// internal constant, not a configurable field: changing it would orphan every
/// existing user credential. It is intentionally not part of
/// [`UnfourAppConfig`] so no host binary can accidentally override it.
const SECRET_STORE_NAMESPACE: &str = "unfour";

/// Compile-time application identity. This is the single source of truth for
/// the About page, logging, and diagnostic bundles. Every field is supplied by
/// the host binary at build time; nothing is inferred at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnfourAppConfig {
    pub app_name: String,
    pub app_version: String,
    pub channel: ReleaseChannel,
    pub distribution: AppDistribution,
    pub commit: Option<String>,
}

impl Default for UnfourAppConfig {
    fn default() -> Self {
        Self {
            app_name: "Unfour".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            channel: ReleaseChannel::Test,
            distribution: AppDistribution::Standard,
            commit: None,
        }
    }
}

/// Sink for live SSH terminal output. The frontend registers a Tauri IPC
/// [`Channel`] (via the `ssh_register_terminal_channel` command); the terminal
/// output callback then streams over it. Channels ride the same reliable
/// transport as commands, unlike the event system, which stalls under the
/// high-rate emit burst of a full-screen redraw on WebView2/Windows.
pub type TerminalChannelSlot = Arc<Mutex<Option<Channel<serde_json::Value>>>>;
pub type SftpTransferChannelSlot = Arc<Mutex<Option<Channel<serde_json::Value>>>>;
pub type TaskRunChannelSlot = Arc<Mutex<Option<Channel<serde_json::Value>>>>;

pub struct AppState {
    pub command_bus: CommandBus,
    pub terminal_channel: TerminalChannelSlot,
    pub sftp_transfer_channel: SftpTransferChannelSlot,
    pub task_run_channel: TaskRunChannelSlot,
    pub config: UnfourAppConfig,
    _logging_guard: Option<unfour_diag::LoggingGuard>,
}

#[derive(Clone, Default)]
pub struct UnfourAppExtensions {
    pub command_bus: CommandBusExtensions,
    prepared_command_bus: Option<CommandBus>,
}

impl UnfourAppExtensions {
    /// Use a command bus that was initialized by the desktop composition root.
    /// This supports startup sequences that must run unified migrations and
    /// attach transactional hooks before background services begin.
    pub fn with_prepared_command_bus(command_bus: CommandBus) -> Self {
        Self {
            command_bus: CommandBusExtensions::default(),
            prepared_command_bus: Some(command_bus),
        }
    }
}

/// Apply the shared plugins and command-bus setup to a Tauri builder.
///
/// The caller is responsible for the desktop-specific tail of the chain:
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
    configure_core_app_with_extensions(builder, config, UnfourAppExtensions::default())
}

pub fn configure_core_app_with_extensions<R>(
    builder: tauri::Builder<R>,
    config: UnfourAppConfig,
    extensions: UnfourAppExtensions,
) -> tauri::Builder<R>
where
    R: Runtime,
{
    let prepared_command_bus = extensions.prepared_command_bus.clone();
    builder
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(move |app| {
            let logging_guard = initialize_logging(&config);
            let command_bus = match prepared_command_bus.clone() {
                Some(command_bus) => command_bus,
                None => tauri::async_runtime::block_on(async {
                    let paths = unfour_paths::initialize_unfour_storage()?;
                    let db = LocalDb::connect_path(paths.database_path).await?;
                    db.migrate().await?;
                    CommandBus::from_db_with_secret_store_and_extensions(
                        db,
                        SecretStore::new(SECRET_STORE_NAMESPACE),
                        extensions.command_bus.clone(),
                    )
                    .await
                })?,
            };

            let terminal_channel: TerminalChannelSlot = Arc::new(Mutex::new(None));
            let sftp_transfer_channel: SftpTransferChannelSlot = Arc::new(Mutex::new(None));
            let task_run_channel: TaskRunChannelSlot = Arc::new(Mutex::new(None));

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

                let transfer_channel_slot = sftp_transfer_channel.clone();
                command_bus.set_sftp_transfer_callback(std::sync::Arc::new(move |payload| {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload) {
                        if let Ok(guard) = transfer_channel_slot.lock() {
                            if let Some(channel) = guard.as_ref() {
                                // Retry immediately if the IPC buffer rejects a frame.
                                for _ in 0..3 {
                                    if channel.send(value.clone()).is_ok() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }));

                let task_run_channel_slot = task_run_channel.clone();
                command_bus.set_task_run_callback(std::sync::Arc::new(move |payload| {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload) {
                        if let Ok(guard) = task_run_channel_slot.lock() {
                            if let Some(channel) = guard.as_ref() {
                                let _ = channel.send(value);
                            }
                        }
                    }
                }));
            }

            app.manage(AppState {
                command_bus,
                terminal_channel,
                sftp_transfer_channel,
                task_run_channel,
                config,
                _logging_guard: logging_guard,
            });

            Ok(())
        })
}

fn initialize_logging(config: &UnfourAppConfig) -> Option<unfour_diag::LoggingGuard> {
    let paths = unfour_paths::initialize_unfour_storage().ok()?;
    let mut logging_config = unfour_diag::LoggingConfig::unified_dev(paths.logs_dir);
    logging_config.app_name = config.app_name.clone();
    logging_config.version = config.app_version.clone();
    // Release identity comes straight from the build-time config, never from the
    // cargo profile. `debug_assertions` is allowed to influence only the log
    // verbosity, which `unified_dev` already derives from it.
    logging_config.channel = config.channel.diagnostics_channel();
    logging_config.distribution = config.distribution.diagnostics_distribution();
    logging_config.commit = config.commit.clone();
    unfour_diag::init_logging(logging_config).ok()
}

pub fn diagnostic_bundle_request(
    config: &UnfourAppConfig,
    paths: unfour_paths::UnfourPaths,
) -> unfour_diag::DiagnosticBundleRequest {
    let mut request =
        unfour_diag::DiagnosticBundleRequest::unified_dev(config.app_version.clone(), paths);
    request.app_name = config.app_name.clone();
    request.channel = config.channel.diagnostics_channel();
    request.distribution = config.distribution.diagnostics_distribution();
    request.commit = config.commit.clone();
    request
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    fn paths() -> unfour_paths::UnfourPaths {
        let root = std::env::temp_dir().join(format!(
            "unfour-app-identity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        unfour_paths::UnfourPaths {
            product_data_dir: root.clone(),
            database_path: root.join("unfour.sqlite"),
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            backups_dir: root.join("backups"),
            logs_dir: root.join("logs"),
            diagnostics_dir: root.join("diagnostics"),
        }
    }

    #[test]
    fn unified_default_identity_is_standard_test() {
        let config = UnfourAppConfig::default();
        assert_eq!(config.distribution, AppDistribution::Standard);
        assert_eq!(config.channel, ReleaseChannel::Test);
        assert_eq!(config.app_name, "Unfour");
        assert_eq!(config.distribution.as_str(), "standard");
        assert_eq!(config.channel.as_str(), "test");
    }

    #[test]
    fn secret_store_namespace_is_the_stable_internal_constant() {
        assert_eq!(SECRET_STORE_NAMESPACE, "unfour");
    }

    #[test]
    fn diagnostic_request_uses_config_identity_for_both_channels() {
        for channel in [ReleaseChannel::Test, ReleaseChannel::Stable] {
            let config = UnfourAppConfig {
                app_name: "Unfour".to_string(),
                app_version: "9.9.9".to_string(),
                channel,
                distribution: AppDistribution::Standard,
                commit: Some("abc123".to_string()),
            };
            let request = diagnostic_bundle_request(&config, paths());
            assert_eq!(request.channel, channel.diagnostics_channel());
            assert_eq!(request.distribution, unfour_diag::Distribution::Standard);
            assert_eq!(request.version, "9.9.9");
            assert_eq!(request.commit.as_deref(), Some("abc123"));
        }
    }
}

#[macro_export]
macro_rules! generate_handlers {
    ( $( $handler:path ),* $(,)? ) => {
        tauri::generate_handler![
            unfour_app::commands::export_diagnostics_bundle,
            unfour_app::commands::frontend_log,
            unfour_app::commands::get_app_info,
            unfour_app::commands::mcp_binary_path,
            unfour_app::commands::open_diagnostics_dir,
            unfour_app::commands::open_log_dir,
            unfour_app::commands::system_health,
            unfour_app::commands::workspace_create,
            unfour_app::commands::workspace_delete,
            unfour_app::commands::workspace_list,
            unfour_app::commands::workspace_variables_list,
            unfour_app::commands::workspace_variables_replace,
            unfour_app::commands::workspace_variable_create,
            unfour_app::commands::workspace_variable_update,
            unfour_app::commands::workspace_variable_delete,
            unfour_app::commands::workspace_variables_resolve,
            unfour_app::commands::workspace_environments_list,
            unfour_app::commands::workspace_environment_create,
            unfour_app::commands::workspace_environment_update,
            unfour_app::commands::workspace_environment_update_metadata,
            unfour_app::commands::workspace_environments_reorder,
            unfour_app::commands::workspace_environment_delete,
            unfour_app::commands::workspace_environment_set_active,
            unfour_app::commands::workspace_environment_variable_create,
            unfour_app::commands::workspace_environment_variable_update,
            unfour_app::commands::workspace_environment_variables_replace,
            unfour_app::commands::workspace_environment_variable_delete,
            unfour_app::commands::api_environments_list,
            unfour_app::commands::api_environment_create,
            unfour_app::commands::api_environment_update,
            unfour_app::commands::api_environment_delete,
            unfour_app::commands::api_environment_activate,
            unfour_app::commands::api_collection_list,
            unfour_app::commands::api_collection_export,
            unfour_app::commands::api_collection_import,
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
            unfour_app::commands::workspace_update_mcp_policy,
            unfour_app::commands::workspace_set_default,
            unfour_app::commands::api_history_detail,
            unfour_app::commands::api_history_list,
            unfour_app::commands::api_request_delete,
            unfour_app::commands::api_request_duplicate,
            unfour_app::commands::api_request_save,
            unfour_app::commands::api_request_update,
            unfour_app::commands::api_saved_requests,
            unfour_app::commands::api_send_request,
            unfour_app::commands::api_send_request_v2,
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
            unfour_app::commands::ssh_command_history_list,
            unfour_app::commands::ssh_session_input,
            unfour_app::commands::ssh_register_terminal_channel,
            unfour_app::commands::ssh_register_sftp_transfer_channel,
            unfour_app::commands::ssh_sftp_cancel_transfer,
            unfour_app::commands::ssh_sftp_create_directory,
            unfour_app::commands::ssh_sftp_delete,
            unfour_app::commands::ssh_sftp_download,
            unfour_app::commands::ssh_sftp_list_directory,
            unfour_app::commands::ssh_sftp_open,
            unfour_app::commands::ssh_sftp_rename,
            unfour_app::commands::ssh_sftp_stat,
            unfour_app::commands::ssh_sftp_transfers_list,
            unfour_app::commands::ssh_sftp_upload,
            unfour_app::commands::ssh_session_log_export,
            unfour_app::commands::ssh_session_reconnect_cancel,
            unfour_app::commands::ssh_session_resize,
            unfour_app::commands::ssh_sessions_list,
            unfour_app::commands::ssh_task_delete,
            unfour_app::commands::ssh_task_duplicate,
            unfour_app::commands::ssh_task_get,
            unfour_app::commands::ssh_task_run_cancel,
            unfour_app::commands::ssh_task_run,
            unfour_app::commands::ssh_task_run_log_read,
            unfour_app::commands::ssh_task_runs_clear,
            unfour_app::commands::ssh_task_runs_list,
            unfour_app::commands::ssh_task_save,
            unfour_app::commands::ssh_tasks_list,
            unfour_app::commands::ssh_tasks_reorder,
            unfour_app::commands::ssh_register_task_run_channel,
            $( $handler ),*
        ]
    };
}
