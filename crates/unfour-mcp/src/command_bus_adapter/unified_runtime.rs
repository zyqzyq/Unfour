use std::sync::Arc;

use unfour_cloud_sync::{CloudSyncSshTaskExecutionGuard, SyncDependencies, SyncOutboxHook};
use unfour_command_bus::{CommandBus, CommandBusExtensions};
use unfour_local_storage::LocalDb;

use crate::StorageMode;

pub(super) async fn unified_command_bus(mode: StorageMode) -> unfour_core::AppResult<CommandBus> {
    let db = match mode {
        StorageMode::Default => LocalDb::connect_existing_default().await?,
        StorageMode::Ephemeral => LocalDb::connect_ephemeral().await?,
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
    use crate::command_bus_adapter::LocalCommandBusAdapter;
    use std::{future::Future, task::Poll};
    use unfour_core::models::SshTaskRunInput;

    #[test]
    fn mutable_mcp_ephemeral_survives_cancelled_workspace_reads() {
        // Exercise the supported mutable MCP constructor, including unified
        // migrations and extensions, rather than the deprecated test bus.
        let adapter = LocalCommandBusAdapter::from_storage_mode(StorageMode::Ephemeral).unwrap();
        adapter.run(async {
            let retained = adapter
                .bus
                .create_workspace("Retained MCP workspace".into())
                .await
                .unwrap();
            let original = adapter.bus.list_workspaces().await.unwrap();
            let mut cancelled = 0;
            for _ in 0..64 {
                tokio::task::yield_now().await;
                {
                    let read = adapter.bus.list_workspaces();
                    tokio::pin!(read);
                    // Drop an actual command at its first Pending boundary,
                    // as a timeout/cancellation can do during pool acquisition.
                    let first_poll =
                        std::future::poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx))).await;
                    match first_poll {
                        Poll::Pending => cancelled += 1,
                        Poll::Ready(result) => {
                            result.unwrap();
                        }
                    }
                }
                let current = adapter
                    .bus
                    .list_workspaces()
                    .await
                    .expect("cancelled mutable MCP reads must retain the schema and records");
                assert_eq!(current.active_workspace_id, original.active_workspace_id);
                assert_eq!(current.workspaces.len(), original.workspaces.len());
                assert!(current.workspaces.iter().any(
                    |workspace| workspace.id == retained.id && workspace.name == retained.name
                ));
            }
            assert!(
                cancelled > 0,
                "the regression must cancel an in-flight read"
            );
            // Writes still traverse the unified extensions and their schema.
            let created = adapter
                .bus
                .create_workspace("After cancellation".into())
                .await
                .unwrap();
            assert!(adapter
                .bus
                .list_workspaces()
                .await
                .unwrap()
                .workspaces
                .iter()
                .any(|workspace| workspace.id == created.id));
        });
        adapter.shutdown();
    }

    #[tokio::test]
    async fn mutable_mcp_runtime_installs_cloud_sync_task_execution_guard() {
        let db = LocalDb::connect_ephemeral().await.unwrap();
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
