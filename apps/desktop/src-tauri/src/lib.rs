use std::sync::Arc;

use tokio::sync::mpsc;
use unfour_command_bus::{CommandBus, CommandBusExtensions, DEFAULT_SECRET_SERVICE};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;

mod account;
mod sync;

// Tauri's resource selects Common Controls v6 before the Windows test harness starts.
#[cfg(all(test, target_os = "windows"))]
#[link(name = "resource", kind = "static")]
unsafe extern "C" {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Community identity is stated explicitly here — the single, compile-time
    // source of truth. `channel` and `commit` are injected by `build.rs`
    // (see UNFOUR_RELEASE_CHANNEL / UNFOUR_BUILD_COMMIT); nothing is inferred
    // from the cargo profile or `debug_assertions` at runtime.
    let config = unfour_app::UnfourAppConfig {
        edition: unfour_app::AppEdition::Community,
        app_name: "Unfour".to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        package_kind: unfour_app::PackageKind::GitHub,
        channel: build_channel(),
        commit: build_commit(),
        // `core_commit` defaults to this same unified repository commit.
        core_commit: None,
    };

    let runtime = tauri::async_runtime::block_on(initialize_unified_runtime())
        .expect("error while initializing the Unfour desktop runtime");
    let UnifiedDesktopRuntime {
        command_bus,
        account_state,
        sync_state,
        sync_receiver,
    } = runtime;

    // The worker starts only after unified migrations have completed and the
    // command bus owns the SyncOutboxHook. Its first network attempt is gated
    // by account state and entitlement, so signed-out/offline startup remains
    // fully local-first.
    let background_sync = sync_state.service.clone();
    tauri::async_runtime::spawn(async move {
        background_sync.run_background(sync_receiver).await;
    });

    unfour_app::configure_core_app_with_extensions(
        tauri::Builder::default(),
        config,
        unfour_app::UnfourAppExtensions::with_prepared_command_bus(command_bus),
    )
    .plugin(tauri_plugin_deep_link::init())
    .manage(account_state)
    .manage(sync_state)
    .invoke_handler(unfour_app::generate_handlers![
        account::account_get_state,
        account::account_begin_sign_in,
        account::account_handle_deep_link,
        account::account_sign_out,
        account::account_open_upgrade,
        account::account_open_web_account,
        sync::cloud_sync_enable,
        sync::cloud_sync_disable,
        sync::cloud_sync_status,
        sync::cloud_sync_global_status,
        sync::cloud_sync_set_global_enabled,
        sync::cloud_sync_diagnostics,
        sync::cloud_sync_now,
        sync::cloud_sync_retry_dead_letter_current_local,
        sync::cloud_sync_use_remote_dead_letter,
        sync::cloud_sync_all,
        sync::cloud_sync_list_workspaces,
        sync::cloud_sync_download,
        sync::cloud_sync_conflicts,
        sync::cloud_sync_keep_local,
        sync::cloud_sync_use_remote
    ])
    .run(tauri::generate_context!())
    .expect("error while running Unfour");
}

struct UnifiedDesktopRuntime {
    command_bus: CommandBus,
    account_state: account::AccountAppState,
    sync_state: sync::SyncAppState,
    sync_receiver: mpsc::UnboundedReceiver<String>,
}

async fn initialize_unified_runtime() -> AppResult<UnifiedDesktopRuntime> {
    let paths = unfour_paths::initialize_unfour_storage()?;
    let db = LocalDb::connect_path(paths.database_path).await?;
    initialize_unified_runtime_with_db(
        db,
        env!("UNFOUR_ACCOUNT_API_URL"),
        env!("UNFOUR_ACCOUNT_WEB_URL"),
        env!("UNFOUR_ACCOUNT_ALLOW_LOOPBACK_HTTP") == "1",
    )
    .await
}

async fn initialize_unified_runtime_with_db(
    db: LocalDb,
    account_api_url: &str,
    account_web_url: &str,
    allow_loopback_http: bool,
) -> AppResult<UnifiedDesktopRuntime> {
    // One compatibility-aware migrator entry point owns both the historical
    // core schema and the migrated Cloud Sync schema.
    unfour_cloud_sync_storage::migrate(db.pool()).await?;

    // Service construction validates only compile-time origins. It does not
    // touch the network or require a signed-in account.
    let account_state =
        account::AccountAppState::new(account_api_url, account_web_url, allow_loopback_http)
            .map_err(|error| AppError::Config(error.code().to_string()))?;
    let account_service = account_state.service();

    let sync_access = sync::SyncAccessGate::default();
    let tokens = sync::AccountTokenProvider::new(account_service, sync_access.clone());
    let transport = unfour_cloud_sync::HttpSyncTransport::new(account_api_url, tokens)
        .map_err(|error| AppError::Config(error.code().to_string()))?;
    let (sync_service, sync_hook, sync_receiver) =
        unfour_cloud_sync::SyncRuntime::build(db.clone(), Arc::new(transport));
    let command_bus_extensions = CommandBusExtensions::new(vec![sync_hook]);
    let command_bus = CommandBus::from_db_with_secret_store_and_extensions(
        db,
        SecretStore::new(DEFAULT_SECRET_SERVICE),
        command_bus_extensions,
    )
    .await?;

    Ok(UnifiedDesktopRuntime {
        command_bus,
        account_state,
        sync_state: sync::SyncAppState::new(sync_service, sync_access),
        sync_receiver,
    })
}

/// Release channel injected at build time by `build.rs`. The build script
/// rejects every value except "test" and "stable".
fn build_channel() -> unfour_app::ReleaseChannel {
    match env!("UNFOUR_RELEASE_CHANNEL") {
        "test" => unfour_app::ReleaseChannel::Test,
        "stable" => unfour_app::ReleaseChannel::Stable,
        value => panic!("invalid compiled UNFOUR_RELEASE_CHANNEL: {value}"),
    }
}

/// HEAD SHA of the workspace this binary was built from, injected by `build.rs`.
/// An empty or "unknown" value (git unavailable) becomes `None`.
fn build_commit() -> Option<String> {
    match env!("UNFOUR_BUILD_COMMIT") {
        "" | "unknown" => None,
        value => Some(value.to_string()),
    }
}

#[cfg(test)]
mod unified_runtime_tests {
    use super::*;
    use unfour_cloud_sync::{SyncDependencies, SyncError};
    use unfour_core::models::WorkspaceVariableInput;

    fn test_root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "unfour-unified-desktop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn local_only_startup_initializes_account_sync_runtime_and_outbox_hook() {
        let root = test_root();
        let db = LocalDb::connect_path(root.join("unfour.sqlite"))
            .await
            .expect("create isolated storage");
        let runtime = initialize_unified_runtime_with_db(
            db.clone(),
            "https://offline-api.example.test",
            "https://offline.example.test",
            false,
        )
        .await
        .expect("network availability must not gate local startup");

        assert_eq!(runtime.account_state.service().generation(), 0);
        assert!(!runtime.sync_state.access.is_allowed());
        let workspace_state = runtime
            .command_bus
            .list_workspaces()
            .await
            .expect("local command bus should be ready");
        let workspace_id = workspace_state.active_workspace_id;
        assert!(!workspace_id.is_empty());

        let sync_error = runtime
            .sync_state
            .service
            .status(&workspace_id)
            .await
            .expect_err("signed-out sync should stay behind the access gate");
        assert_eq!(sync_error, SyncError::Unauthorized);

        // Simulate the point after an entitled account has enabled this
        // workspace. Before a binding exists, local-only mutations correctly
        // remain local; once enabled, the installed hook must enqueue them.
        let dependencies = SyncDependencies::default();
        runtime
            .sync_state
            .service
            .repository()
            .activate_account("account-test", 1, dependencies.clock.now())
            .await
            .expect("activate test account context");
        runtime
            .sync_state
            .service
            .repository()
            .create_binding_with_initial_outbox(
                "account-test",
                1,
                &workspace_id,
                "cloud-workspace-test",
                0,
                dependencies.ids.as_ref(),
                dependencies.clock.as_ref(),
            )
            .await
            .expect("create enabled Cloud Sync binding");

        runtime
            .command_bus
            .workspace_variable_create(
                workspace_id,
                WorkspaceVariableInput {
                    id: None,
                    key: "UNIFIED_DESKTOP_HOOK".to_string(),
                    value: "local".to_string(),
                    is_secret: false,
                    is_enabled: true,
                    description: None,
                    sort_order: 0,
                },
            )
            .await
            .expect("local mutation should commit with SyncOutboxHook");
        let outbox_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_type = 'workspaceVariable'",
        )
        .fetch_one(db.pool())
        .await
        .expect("query Cloud Sync outbox");
        assert_eq!(outbox_count, 1);

        drop(runtime);
        db.pool().close().await;
        let _ = std::fs::remove_dir_all(root);
    }
}
