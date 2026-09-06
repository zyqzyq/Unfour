use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use unfour_cloud_sync::{CloudSyncSshTaskExecutionGuard, SyncDependencies, SyncOutboxHook};
use unfour_command_bus::{CommandBus, CommandBusExtensions};
use unfour_local_storage::LocalDb;

use crate::StorageMode;

pub(super) async fn unified_command_bus(mode: StorageMode) -> unfour_core::AppResult<CommandBus> {
    let db = match mode {
        StorageMode::Default => LocalDb::connect_existing_default().await?,
        StorageMode::Ephemeral => {
            let options = SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true)
                .foreign_keys(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await?;
            LocalDb::from_pool(pool)
        }
    };

    unified_command_bus_with_db(db, mode).await
}

async fn unified_command_bus_with_db(
    db: LocalDb,
    mode: StorageMode,
) -> unfour_core::AppResult<CommandBus> {
    // The standalone adapter shares the same compatibility migration entry
    // point as desktop. Ephemeral mode applies it only in memory and therefore
    // preserves the no-filesystem registry/CI contract.
    unfour_cloud_sync_storage::migrate(db.pool()).await?;
    let dependencies = SyncDependencies::default();
    let hook = Arc::new(SyncOutboxHook::new(
        dependencies.ids,
        dependencies.clock,
        None,
    ));
    let execution_guard = Arc::new(CloudSyncSshTaskExecutionGuard::new(db.clone()));
    let extensions =
        CommandBusExtensions::new(vec![hook]).with_ssh_task_execution_guards(vec![execution_guard]);

    match mode {
        StorageMode::Default => {
            CommandBus::from_existing_db_without_seeding_with_extensions(db, extensions).await
        }
        StorageMode::Ephemeral => CommandBus::from_db_with_extensions(db, extensions).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unfour_core::models::SshTaskRunInput;

    #[tokio::test]
    async fn mutable_mcp_runtime_installs_cloud_sync_task_execution_guard() {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let db = LocalDb::from_pool(pool);
        let bus = unified_command_bus_with_db(db.clone(), StorageMode::Ephemeral)
            .await
            .unwrap();
        let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
        sqlx::query(
            r#"INSERT INTO cloud_sync_workspace_bindings (
                 account_id, local_workspace_id, cloud_workspace_id,
                 sync_enabled, state, initial_cursor, created_at, updated_at
               ) VALUES ('mcp-account', ?1, 'mcp-cloud', 1, 'error', 0,
                         '2026-09-06T00:00:00Z', '2026-09-06T00:00:00Z')"#,
        )
        .bind(&workspace_id)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO cloud_sync_workspace_ownership (
                 local_workspace_id, account_id, cloud_workspace_id, created_at, updated_at
               ) VALUES (?1, 'mcp-account', 'mcp-cloud',
                         '2026-09-06T00:00:00Z', '2026-09-06T00:00:00Z')"#,
        )
        .bind(&workspace_id)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO cloud_sync_remote_entity (
                 account_id, cloud_workspace_id, entity_type, entity_id,
                 parent_entity_id, server_version, payload_schema_version,
                 operation, canonical_payload_json, deleted_at, operation_id,
                 compatibility_state, created_at, updated_at
               ) VALUES ('mcp-account', 'mcp-cloud', 'sshTask', 'mcp-task',
                         NULL, 2, 2, 'upsert', '{}', NULL, 'mcp-future-task',
                         'deferred_compatibility', '2026-09-06T00:00:00Z',
                         '2026-09-06T00:00:00Z')"#,
        )
        .execute(db.pool())
        .await
        .unwrap();

        let error = bus
            .run_ssh_task(SshTaskRunInput {
                workspace_id,
                task_id: "mcp-task".into(),
                connection_id: None,
                inputs: Default::default(),
                secret_input_names: Vec::new(),
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), "SSH_TASK_INCOMPLETE_REMOTE_STATE");
    }
}
