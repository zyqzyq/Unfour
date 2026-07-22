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

/// The single, shared distribution/package kind type. Only two kinds exist
/// project-wide: `GitHub` (GitHub Releases) and `Website` (website downloads).
/// Community builds are always distributed as `GitHub`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    GitHub,
    Website,
}

impl PackageKind {
    /// Stable API value surfaced to the frontend and diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::Website => "website",
        }
    }

    fn diagnostics_package_kind(self) -> unfour_diag::PackageKind {
        match self {
            Self::GitHub => unfour_diag::PackageKind::GitHub,
            Self::Website => unfour_diag::PackageKind::Website,
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
    pub edition: AppEdition,
    pub app_name: String,
    pub app_version: String,
    pub channel: ReleaseChannel,
    pub package_kind: PackageKind,
    pub commit: Option<String>,
    /// The commit of the Unfour core this build is based on. When the host
    /// binary does not override it, this defaults to [`UnfourAppConfig::commit`]
    /// (the host's own build commit), so Community builds report a single,
    /// unified identity. Future Pro builds may set a distinct core commit.
    pub core_commit: Option<String>,
}

impl Default for UnfourAppConfig {
    fn default() -> Self {
        Self {
            edition: AppEdition::Community,
            app_name: "Unfour".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            channel: ReleaseChannel::Test,
            package_kind: PackageKind::GitHub,
            commit: None,
            core_commit: None,
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

/// Apply the shared plugins and command-bus setup to a Tauri builder.
///
/// The caller is responsible for the per-edition tail of the chain:
/// `.invoke_handler(unfour_app::generate_handlers![..])` and
/// `.run(tauri::generate_context!())`.
pub fn configure(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    configure_core_app(builder, UnfourAppConfig::default())
}

/// `core_commit` mirrors [`UnfourAppConfig::commit`] unless the host binary
/// overrides it. This keeps the baseline identity single-valued (Community)
/// while leaving room for Pro builds to report a distinct core commit later.
fn normalize_config(mut config: UnfourAppConfig) -> UnfourAppConfig {
    if config.core_commit.is_none() {
        config.core_commit = config.commit.clone();
    }
    config
}

pub fn configure_core_app<R>(
    builder: tauri::Builder<R>,
    config: UnfourAppConfig,
) -> tauri::Builder<R>
where
    R: Runtime,
{
    let config = normalize_config(config);
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
            let command_bus = tauri::async_runtime::block_on(async {
                let paths = unfour_paths::initialize_unfour_storage()?;
                let db = LocalDb::connect_path(paths.database_path).await?;
                db.migrate().await?;
                CommandBus::from_db_with_secret_store(db, SecretStore::new(SECRET_STORE_NAMESPACE))
                    .await
            })?;

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
    let mut logging_config = unfour_diag::LoggingConfig::oss_dev(paths.logs_dir);
    logging_config.app_name = config.app_name.clone();
    logging_config.edition = config.edition.diagnostics_edition();
    logging_config.version = config.app_version.clone();
    // Release identity comes straight from the build-time config, never from the
    // cargo profile. `debug_assertions` is allowed to influence only the log
    // verbosity, which `oss_dev` already derives from it.
    logging_config.channel = config.channel.diagnostics_channel();
    logging_config.package_kind = config.package_kind.diagnostics_package_kind();
    logging_config.commit = config.commit.clone();
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
    request.channel = config.channel.diagnostics_channel();
    request.package_kind = config.package_kind.diagnostics_package_kind();
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
    fn community_default_identity_is_community_github_test() {
        let config = UnfourAppConfig::default();
        assert_eq!(config.edition, AppEdition::Community);
        assert_eq!(config.package_kind, PackageKind::GitHub);
        assert_eq!(config.channel, ReleaseChannel::Test);
        assert_eq!(config.app_name, "Unfour");
        assert_eq!(config.package_kind.as_str(), "github");
        assert_eq!(config.channel.as_str(), "test");
    }

    #[test]
    fn secret_store_namespace_is_the_stable_internal_constant() {
        assert_eq!(SECRET_STORE_NAMESPACE, "unfour");
    }

    #[test]
    fn core_commit_defaults_to_commit_when_unset() {
        // No explicit core_commit -> mirrors commit (Community baseline).
        let config = UnfourAppConfig {
            edition: AppEdition::Community,
            app_name: "Unfour".to_string(),
            app_version: "1.0.0".to_string(),
            channel: ReleaseChannel::Test,
            package_kind: PackageKind::GitHub,
            commit: Some("deadbeef".to_string()),
            core_commit: None,
        };
        let normalized = normalize_config(config);
        assert_eq!(normalized.core_commit.as_deref(), Some("deadbeef"));
        assert_eq!(normalized.commit.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn core_commit_is_preserved_when_explicitly_set() {
        // A future Pro build may supply a distinct core commit; it must win.
        let config = UnfourAppConfig {
            edition: AppEdition::Pro,
            app_name: "Unfour".to_string(),
            app_version: "1.0.0".to_string(),
            channel: ReleaseChannel::Stable,
            package_kind: PackageKind::GitHub,
            commit: Some("prosha123".to_string()),
            core_commit: Some("coresha456".to_string()),
        };
        let normalized = normalize_config(config);
        assert_eq!(normalized.core_commit.as_deref(), Some("coresha456"));
        assert_eq!(normalized.commit.as_deref(), Some("prosha123"));
    }

    #[test]
    fn diagnostic_request_uses_config_identity_for_both_channels() {
        for channel in [ReleaseChannel::Test, ReleaseChannel::Stable] {
            let config = UnfourAppConfig {
                edition: AppEdition::Community,
                app_name: "Unfour".to_string(),
                app_version: "9.9.9".to_string(),
                channel,
                package_kind: PackageKind::GitHub,
                commit: Some("abc123".to_string()),
                core_commit: None,
            };
            let request = diagnostic_bundle_request(&config, paths());
            assert_eq!(request.channel, channel.diagnostics_channel());
            assert_eq!(request.package_kind, unfour_diag::PackageKind::GitHub);
            assert_eq!(request.edition, unfour_diag::Edition::Oss);
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
            unfour_app::commands::workspace_variables_resolve,
            unfour_app::commands::workspace_environments_list,
            unfour_app::commands::workspace_environment_create,
            unfour_app::commands::workspace_environment_update,
            unfour_app::commands::workspace_environment_delete,
            unfour_app::commands::workspace_environment_set_active,
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
            unfour_app::commands::ssh_register_task_run_channel,
            $( $handler ),*
        ]
    };
}
