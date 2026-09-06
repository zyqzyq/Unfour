use std::future::Future;
use std::pin::Pin;

use unfour_command_bus::SshTaskExecutionGuard;
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;

/// Rejects execution when the durable latest-remote cache proves that this
/// saved SSH Task aggregate is incomplete for the current reader.
#[derive(Clone)]
pub struct CloudSyncSshTaskExecutionGuard {
    db: LocalDb,
}

impl CloudSyncSshTaskExecutionGuard {
    pub fn new(db: LocalDb) -> Self {
        Self { db }
    }
}

impl SshTaskExecutionGuard for CloudSyncSshTaskExecutionGuard {
    fn validate<'a>(
        &'a self,
        workspace_id: &'a str,
        task_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let incomplete: bool = sqlx::query_scalar(
                r#"SELECT EXISTS (
                     SELECT 1
                     FROM cloud_sync_workspace_ownership AS owner
                     JOIN cloud_sync_remote_entity AS remote
                       ON remote.account_id = owner.account_id
                      AND remote.cloud_workspace_id = owner.cloud_workspace_id
                     LEFT JOIN cloud_sync_entity_state AS state
                       ON state.account_id = remote.account_id
                      AND state.cloud_workspace_id = remote.cloud_workspace_id
                      AND state.entity_type = remote.entity_type
                      AND state.entity_id = remote.entity_id
                     WHERE owner.local_workspace_id = ?1
                       AND remote.compatibility_state = 'deferred_compatibility'
                       AND remote.server_version >= COALESCE(state.server_version, 0)
                       AND (
                         (remote.entity_type = 'sshTask' AND remote.entity_id = ?2)
                         OR (remote.entity_type = 'sshTaskStep' AND remote.parent_entity_id = ?2)
                         OR (
                           remote.parent_entity_id = ?2
                           AND remote.entity_type NOT IN (
                             'workspace', 'connection', 'workspaceVariable',
                             'workspaceEnvironment', 'workspaceEnvironmentVariable',
                             'apiCollection', 'apiFolder', 'apiRequest',
                             'sshTask', 'sshTaskStep'
                           )
                         )
                       )
                   )"#,
            )
            .bind(workspace_id)
            .bind(task_id)
            .fetch_one(self.db.pool())
            .await?;

            if incomplete {
                return Err(AppError::SshTaskIncompleteRemoteState);
            }
            Ok(())
        })
    }
}
