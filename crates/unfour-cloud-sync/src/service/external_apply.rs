//! Apply external data through the owning Core engines on a shared connection.
//! Callers commit before invoking cleanup of credentials and runtime resources.

use sqlx::SqliteConnection;
use unfour_core::domain::{
    validate_external_connection_delete, CommandContext, ExternalApplyPage,
    ExternalConnectionApply, ExternalWorkspaceApply, MutationOrigin, DATABASE_CONNECTION_TYPE,
    SSH_CONNECTION_TYPE,
};
use unfour_database_engine::DatabaseConnectionCleanup;
use unfour_ssh_engine::SshConnectionCleanup;

use super::SyncService;
use crate::SyncError;

#[derive(Default)]
pub(super) struct ExternalApplyCleanup {
    ssh_connections: Vec<SshConnectionCleanup>,
    database_connections: Vec<DatabaseConnectionCleanup>,
}

impl SyncService {
    pub(super) async fn apply_external_page_on(
        &self,
        connection: &mut SqliteConnection,
        action: &'static str,
        page: ExternalApplyPage,
    ) -> Result<ExternalApplyCleanup, SyncError> {
        if external_page_is_empty(&page) {
            return Ok(ExternalApplyCleanup::default());
        }
        let context = CommandContext::new(action, MutationOrigin::External);
        let workspace_deletes: Vec<(String, String)> = page
            .workspaces
            .iter()
            .filter_map(|change| match change {
                ExternalWorkspaceApply::Delete(delete) => Some((
                    delete.entity.workspace_id.clone(),
                    delete.deleted_at.clone(),
                )),
                _ => None,
            })
            .collect();
        let mut cleanup = ExternalApplyCleanup::default();
        for (workspace_id, deleted_at) in &workspace_deletes {
            self.api_client
                .delete_workspace_api_entities_on(
                    connection,
                    &context,
                    workspace_id,
                    Some(deleted_at),
                )
                .await
                .map_err(|_| SyncError::Core)?;
            self.ssh
                .delete_workspace_ssh_task_entities_on(
                    connection,
                    &context,
                    workspace_id,
                    Some(deleted_at),
                )
                .await
                .map_err(|_| SyncError::Core)?;
            let (_, cleanups) = self
                .ssh
                .delete_workspace_connections_on(connection, &context, workspace_id, deleted_at)
                .await
                .map_err(|_| SyncError::Core)?;
            cleanup.ssh_connections.extend(cleanups);
            let (_, cleanups) = self
                .database
                .delete_workspace_connections_on(connection, &context, workspace_id, deleted_at)
                .await
                .map_err(|_| SyncError::Core)?;
            cleanup.database_connections.extend(cleanups);
        }
        self.workspace
            .apply_external_page_on(connection, &context, page.clone())
            .await
            .map_err(|_| SyncError::Core)?;
        for change in page.connections.clone() {
            let connection_type = match &change {
                ExternalConnectionApply::Upsert(record) => record.connection_type.as_str(),
                ExternalConnectionApply::Delete(delete) => {
                    validate_external_connection_delete(delete).map_err(|_| SyncError::Core)?;
                    let row: Option<(String, String)> = sqlx::query_as(
                        "SELECT workspace_id, connection_type FROM connections WHERE id = ?1",
                    )
                    .bind(&delete.entity.entity_id)
                    .fetch_optional(&mut *connection)
                    .await?;
                    let Some((workspace_id, connection_type)) = row else {
                        continue;
                    };
                    if workspace_id != delete.entity.workspace_id {
                        return Err(SyncError::InvalidData);
                    }
                    if connection_type == SSH_CONNECTION_TYPE {
                        SSH_CONNECTION_TYPE
                    } else if connection_type == DATABASE_CONNECTION_TYPE {
                        DATABASE_CONNECTION_TYPE
                    } else {
                        return Err(SyncError::InvalidData);
                    }
                }
            };
            match connection_type {
                SSH_CONNECTION_TYPE => {
                    let outcome = self
                        .ssh
                        .apply_external_connection_on(connection, &context, change)
                        .await
                        .map_err(|_| SyncError::Core)?;
                    if let Some(value) = outcome.value {
                        cleanup.ssh_connections.push(value);
                    }
                }
                DATABASE_CONNECTION_TYPE => {
                    let outcome = self
                        .database
                        .apply_external_connection_on(connection, &context, change)
                        .await
                        .map_err(|_| SyncError::Core)?;
                    if let Some(value) = outcome.value {
                        cleanup.database_connections.push(value);
                    }
                }
                _ => return Err(SyncError::InvalidData),
            }
        }
        self.api_client
            .apply_external_page_on(connection, &context, page.clone())
            .await
            .map_err(|_| SyncError::Core)?;
        self.ssh
            .apply_external_task_page_on(connection, &context, page)
            .await
            .map_err(|_| SyncError::Core)?;
        Ok(cleanup)
    }

    pub(super) async fn finish_external_cleanup(&self, cleanup: ExternalApplyCleanup) {
        self.ssh
            .cleanup_connection_changes(cleanup.ssh_connections)
            .await;
        self.database
            .cleanup_connection_changes(cleanup.database_connections)
            .await;
    }
}

pub(super) fn merge_external_pages(pages: Vec<ExternalApplyPage>) -> ExternalApplyPage {
    let mut merged = ExternalApplyPage::default();
    for mut page in pages {
        merged.workspaces.append(&mut page.workspaces);
        merged.connections.append(&mut page.connections);
        merged
            .workspace_variables
            .append(&mut page.workspace_variables);
        merged
            .workspace_environments
            .append(&mut page.workspace_environments);
        merged
            .workspace_environment_variables
            .append(&mut page.workspace_environment_variables);
        merged.api_collections.append(&mut page.api_collections);
        merged.api_folders.append(&mut page.api_folders);
        merged.api_requests.append(&mut page.api_requests);
        merged.ssh_tasks.append(&mut page.ssh_tasks);
        merged.ssh_task_steps.append(&mut page.ssh_task_steps);
    }
    merged
}

fn external_page_is_empty(page: &ExternalApplyPage) -> bool {
    page.workspaces.is_empty()
        && page.connections.is_empty()
        && page.workspace_variables.is_empty()
        && page.workspace_environments.is_empty()
        && page.workspace_environment_variables.is_empty()
        && page.api_collections.is_empty()
        && page.api_folders.is_empty()
        && page.api_requests.is_empty()
        && page.ssh_tasks.is_empty()
        && page.ssh_task_steps.is_empty()
}
