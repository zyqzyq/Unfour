use sqlx::SqliteConnection;

use crate::{SyncBinding, SyncConflict, SyncEntityType, SyncError, SyncOperation};

fn is_hierarchical_delete(entity_type: SyncEntityType, operation: SyncOperation) -> bool {
    operation == SyncOperation::Delete
        && matches!(
            entity_type,
            SyncEntityType::Workspace
                | SyncEntityType::WorkspaceEnvironment
                | SyncEntityType::ApiCollection
                | SyncEntityType::ApiFolder
                | SyncEntityType::SshTask
        )
}

struct HierarchyScope {
    workspace_scope: bool,
    environment_scope: bool,
    ssh_task_scope: bool,
    api_descendant_json: String,
}

async fn hierarchy_scope(
    connection: &mut SqliteConnection,
    binding: &SyncBinding,
    entity_type: SyncEntityType,
    entity_id: &str,
    operation: SyncOperation,
) -> Result<HierarchyScope, SyncError> {
    let hierarchical_delete = is_hierarchical_delete(entity_type, operation);
    let descendants = if hierarchical_delete {
        api_descendant_ids(
            connection,
            &binding.local_workspace_id,
            entity_type,
            entity_id,
        )
        .await?
    } else {
        Vec::new()
    };
    Ok(HierarchyScope {
        workspace_scope: hierarchical_delete && entity_type == SyncEntityType::Workspace,
        environment_scope: hierarchical_delete
            && entity_type == SyncEntityType::WorkspaceEnvironment,
        ssh_task_scope: hierarchical_delete && entity_type == SyncEntityType::SshTask,
        api_descendant_json: serde_json::to_string(&descendants)
            .map_err(|_| SyncError::InvalidData)?,
    })
}

async fn api_descendant_ids(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    entity_type: SyncEntityType,
    entity_id: &str,
) -> Result<Vec<String>, SyncError> {
    let rows: Vec<(String,)> = match entity_type {
        SyncEntityType::ApiCollection => {
            sqlx::query_as(
                r#"SELECT id FROM api_collection_folders
                   WHERE workspace_id = ?1 AND collection_id = ?2
                   UNION ALL
                   SELECT id FROM api_requests
                   WHERE workspace_id = ?1 AND collection_id = ?2"#,
            )
            .bind(workspace_id)
            .bind(entity_id)
            .fetch_all(&mut *connection)
            .await?
        }
        SyncEntityType::ApiFolder => {
            sqlx::query_as(
                r#"WITH RECURSIVE folder_tree(id) AS (
                     SELECT id FROM api_collection_folders
                     WHERE workspace_id = ?1 AND id = ?2
                     UNION
                     SELECT f.id FROM api_collection_folders AS f
                     JOIN folder_tree AS tree ON f.parent_folder_id = tree.id
                     WHERE f.workspace_id = ?1
                   )
                   SELECT id FROM folder_tree WHERE id != ?2
                   UNION ALL
                   SELECT requests.id FROM api_requests AS requests
                   WHERE requests.workspace_id = ?1
                     AND requests.parent_folder_id IN (SELECT id FROM folder_tree)"#,
            )
            .bind(workspace_id)
            .bind(entity_id)
            .fetch_all(&mut *connection)
            .await?
        }
        _ => Vec::new(),
    };
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

const API_DESCENDANT_MATCH: &str = r#"
    entity_type IN ('apiFolder', 'apiRequest')
    AND entity_id IN (SELECT value FROM json_each(?7))
"#;

pub(crate) async fn has_local_intent_on(
    connection: &mut SqliteConnection,
    binding: &SyncBinding,
    entity_type: SyncEntityType,
    entity_id: &str,
    operation: SyncOperation,
) -> Result<bool, SyncError> {
    let scope = hierarchy_scope(connection, binding, entity_type, entity_id, operation).await?;
    let sql = format!(
        r#"SELECT
             EXISTS(
               SELECT 1 FROM cloud_sync_outbox
               WHERE account_id = ?1 AND cloud_workspace_id = ?2
                 AND status IN ('pending', 'uncertain', 'in_flight', 'dead')
                 AND (
                   ?3
                   OR (entity_type = ?4 AND entity_id = ?5)
                   OR (?6 AND entity_type = 'workspaceEnvironmentVariable'
                              AND parent_entity_id = ?5)
                   OR ({API_DESCENDANT_MATCH})
                   OR (?8 AND entity_type = 'sshTaskStep' AND parent_entity_id = ?5)
                 )
             )
             OR EXISTS(
               SELECT 1 FROM cloud_sync_entity_state
               WHERE account_id = ?1 AND cloud_workspace_id = ?2
                 AND sync_status = 'conflict'
                 AND (
                   ?3
                   OR (entity_type = ?4 AND entity_id = ?5)
                   OR (?6 AND entity_type = 'workspaceEnvironmentVariable'
                              AND conflict_parent_entity_id = ?5)
                   OR ({API_DESCENDANT_MATCH})
                   OR (?8 AND entity_type = 'sshTaskStep'
                              AND conflict_parent_entity_id = ?5)
                 )
             )"#
    );
    sqlx::query_scalar(&sql)
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(scope.workspace_scope)
        .bind(entity_type.as_str())
        .bind(entity_id)
        .bind(scope.environment_scope)
        .bind(&scope.api_descendant_json)
        .bind(scope.ssh_task_scope)
        .fetch_one(&mut *connection)
        .await
        .map_err(Into::into)
}

pub(crate) async fn conflicts_on(
    connection: &mut SqliteConnection,
    binding: &SyncBinding,
    conflict: &SyncConflict,
) -> Result<Vec<SyncConflict>, SyncError> {
    let entity_type = SyncEntityType::parse(&conflict.entity_type)?;
    let operation = SyncOperation::parse(
        conflict
            .conflict_remote_operation
            .as_deref()
            .ok_or(SyncError::InvalidData)?,
    )?;
    let scope = hierarchy_scope(
        connection,
        binding,
        entity_type,
        &conflict.entity_id,
        operation,
    )
    .await?;
    let sql = format!(
        r#"SELECT account_id, cloud_workspace_id, entity_type, entity_id, server_version,
                  conflict_remote_payload_json, conflict_remote_operation,
                  conflict_parent_entity_id, conflict_deleted_at, conflict_operation_id
           FROM cloud_sync_entity_state
           WHERE account_id = ?1 AND cloud_workspace_id = ?2 AND sync_status = 'conflict'
             AND (
               ?3
               OR (entity_type = ?4 AND entity_id = ?5)
               OR (?6 AND entity_type = 'workspaceEnvironmentVariable'
                          AND conflict_parent_entity_id = ?5)
               OR ({API_DESCENDANT_MATCH})
               OR (?8 AND entity_type = 'sshTaskStep'
                          AND conflict_parent_entity_id = ?5)
             )
           ORDER BY CASE entity_type
             WHEN 'workspace' THEN 0
             WHEN 'apiRequest' THEN 3
             WHEN 'workspaceEnvironmentVariable' THEN 2
             WHEN 'apiFolder' THEN 2
             WHEN 'sshTaskStep' THEN 2
             ELSE 1 END, entity_type, entity_id"#
    );
    sqlx::query_as::<_, SyncConflict>(&sql)
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(scope.workspace_scope)
        .bind(entity_type.as_str())
        .bind(&conflict.entity_id)
        .bind(scope.environment_scope)
        .bind(&scope.api_descendant_json)
        .bind(scope.ssh_task_scope)
        .fetch_all(&mut *connection)
        .await
        .map_err(Into::into)
}

pub(crate) async fn abandon_intents_on(
    connection: &mut SqliteConnection,
    binding: &SyncBinding,
    conflict: &SyncConflict,
) -> Result<(), SyncError> {
    let entity_type = SyncEntityType::parse(&conflict.entity_type)?;
    let operation = SyncOperation::parse(
        conflict
            .conflict_remote_operation
            .as_deref()
            .ok_or(SyncError::InvalidData)?,
    )?;
    let scope = hierarchy_scope(
        connection,
        binding,
        entity_type,
        &conflict.entity_id,
        operation,
    )
    .await?;
    let sql = format!(
        r#"DELETE FROM cloud_sync_outbox
           WHERE account_id = ?1 AND cloud_workspace_id = ?2
             AND (
               ?3
               OR (entity_type = ?4 AND entity_id = ?5)
               OR (?6 AND entity_type = 'workspaceEnvironmentVariable'
                          AND parent_entity_id = ?5)
               OR ({API_DESCENDANT_MATCH})
               OR (?8 AND entity_type = 'sshTaskStep' AND parent_entity_id = ?5)
             )"#
    );
    sqlx::query(&sql)
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(scope.workspace_scope)
        .bind(entity_type.as_str())
        .bind(&conflict.entity_id)
        .bind(scope.environment_scope)
        .bind(&scope.api_descendant_json)
        .bind(scope.ssh_task_scope)
        .execute(&mut *connection)
        .await?;
    Ok(())
}
