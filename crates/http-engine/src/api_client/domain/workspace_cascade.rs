use chrono::Utc;
use sqlx::SqliteConnection;
use unfour_core::domain::{CommandContext, DomainEntityType, DomainMutation, MutationOperation};
use unfour_core::AppResult;

use super::collections::{
    soft_delete_collection_on, soft_delete_folder_on, soft_delete_request_on,
};
use super::{effective_parent, mutation, ApiClientService};

impl ApiClientService {
    /// Soft-delete every live API entity in a workspace, children first.
    /// Used when the workspace itself is tombstoned so leftover live
    /// collections/folders/requests cannot remain as orphans.
    pub async fn delete_workspace_api_entities_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: &str,
        deleted_at: Option<&str>,
    ) -> AppResult<Vec<DomainMutation>> {
        let deleted_at = deleted_at
            .map(str::to_string)
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        let mut mutations = Vec::new();

        let requests: Vec<(String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT id, collection_id, parent_folder_id
            FROM api_requests
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY id
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        for (id, collection_id, parent_folder_id) in requests {
            let revision =
                soft_delete_request_on(connection, workspace_id, &id, &deleted_at).await?;
            mutations.push(mutation(
                context,
                DomainEntityType::ApiRequest,
                MutationOperation::Delete,
                workspace_id,
                &id,
                Some(effective_parent(
                    &collection_id,
                    parent_folder_id.as_deref(),
                )),
                revision,
            ));
        }

        let folders: Vec<(String, String, Option<String>)> = sqlx::query_as(
            r#"
            WITH RECURSIVE folder_tree(id, collection_id, parent_folder_id, depth) AS (
              SELECT id, collection_id, parent_folder_id, 0
              FROM api_collection_folders
              WHERE workspace_id = ?1 AND deleted_at IS NULL
                AND (
                  parent_folder_id IS NULL
                  OR NOT EXISTS (
                    SELECT 1 FROM api_collection_folders AS parent
                    WHERE parent.id = api_collection_folders.parent_folder_id
                      AND parent.workspace_id = ?1
                      AND parent.deleted_at IS NULL
                  )
                )
              UNION ALL
              SELECT child.id, child.collection_id, child.parent_folder_id, parent.depth + 1
              FROM api_collection_folders AS child
              JOIN folder_tree AS parent ON child.parent_folder_id = parent.id
              WHERE child.workspace_id = ?1 AND child.deleted_at IS NULL
            )
            SELECT id, collection_id, parent_folder_id
            FROM folder_tree
            ORDER BY depth DESC, id
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        for (id, collection_id, parent_folder_id) in folders {
            let revision =
                soft_delete_folder_on(connection, workspace_id, &id, &deleted_at).await?;
            mutations.push(mutation(
                context,
                DomainEntityType::ApiFolder,
                MutationOperation::Delete,
                workspace_id,
                &id,
                Some(effective_parent(
                    &collection_id,
                    parent_folder_id.as_deref(),
                )),
                revision,
            ));
        }

        let collections: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT id FROM api_collections
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY id
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        for id in collections {
            if let Some(revision) =
                soft_delete_collection_on(connection, workspace_id, &id, &deleted_at).await?
            {
                mutations.push(mutation(
                    context,
                    DomainEntityType::ApiCollection,
                    MutationOperation::Delete,
                    workspace_id,
                    &id,
                    None,
                    revision,
                ));
            }
        }

        Ok(mutations)
    }
}
