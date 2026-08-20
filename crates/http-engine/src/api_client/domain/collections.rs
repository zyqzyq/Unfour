use std::collections::HashSet;

use chrono::Utc;
use sqlx::{FromRow, SqliteConnection};
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityType, DomainMutation, MutationOperation,
};
use unfour_core::models::{ApiCollection, ApiCollectionFolder};
use unfour_core::{AppError, AppResult};

use super::super::helpers::{normalize_entity_id, normalize_folder_name};
use super::{
    collection_on, effective_parent, folder_on, list_collections_on, list_folders_on, mutation,
    normalize_collection_name, validate_workspace_on, ApiClientService,
};

#[derive(Debug, FromRow)]
struct FolderDeleteRow {
    id: String,
    collection_id: String,
    parent_folder_id: Option<String>,
    #[sqlx(rename = "depth")]
    _depth: i64,
}

#[derive(Debug, FromRow)]
struct RequestDeleteRow {
    id: String,
    collection_id: String,
    parent_folder_id: Option<String>,
}

impl ApiClientService {
    pub async fn create_collection_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        name: String,
    ) -> AppResult<DomainCommandResult<ApiCollection>> {
        validate_workspace_on(connection, &workspace_id).await?;
        let name = normalize_collection_name(name)?;
        let id = unfour_core::id::new_id();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO api_collections (
              id, workspace_id, name, created_at, updated_at, revision, sync_status
            ) VALUES (?1, ?2, ?3, ?4, ?4, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(name)
        .bind(now)
        .execute(&mut *connection)
        .await?;
        let value =
            ApiCollection::from(collection_on(connection, &workspace_id, &id, false).await?);
        Ok(DomainCommandResult::new(
            value,
            vec![mutation(
                context,
                DomainEntityType::ApiCollection,
                MutationOperation::Upsert,
                &workspace_id,
                &id,
                None,
                1,
            )],
        ))
    }

    pub async fn rename_collection_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        collection_id: String,
        name: String,
    ) -> AppResult<DomainCommandResult<ApiCollection>> {
        let name = normalize_collection_name(name)?;
        let current = collection_on(connection, &workspace_id, &collection_id, false).await?;
        if current.name == name {
            return Ok(DomainCommandResult::unchanged(ApiCollection::from(current)));
        }
        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE api_collections
            SET name = ?1, updated_at = ?2, revision = revision + 1,
                sync_status = 'pending'
            WHERE workspace_id = ?3 AND id = ?4 AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(name)
        .bind(Utc::now().to_rfc3339())
        .bind(&workspace_id)
        .bind(&collection_id)
        .fetch_one(&mut *connection)
        .await?;
        let value = ApiCollection::from(
            collection_on(connection, &workspace_id, &collection_id, false).await?,
        );
        Ok(DomainCommandResult::new(
            value,
            vec![mutation(
                context,
                DomainEntityType::ApiCollection,
                MutationOperation::Upsert,
                &workspace_id,
                &collection_id,
                None,
                revision,
            )],
        ))
    }

    pub async fn delete_collection_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        collection_id: String,
    ) -> AppResult<DomainCommandResult<Vec<ApiCollection>>> {
        collection_on(connection, &workspace_id, &collection_id, false).await?;
        let mutations = delete_collection_tree_on(
            connection,
            context,
            &workspace_id,
            &collection_id,
            &Utc::now().to_rfc3339(),
        )
        .await?;
        Ok(DomainCommandResult::new(
            list_collections_on(connection, &workspace_id).await?,
            mutations,
        ))
    }

    pub async fn create_collection_folder_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        name: String,
    ) -> AppResult<DomainCommandResult<ApiCollectionFolder>> {
        let name = normalize_folder_name(name)?;
        let parent_folder_id = normalize_entity_id(parent_folder_id);
        collection_on(connection, &workspace_id, &collection_id, false).await?;
        if let Some(parent_id) = parent_folder_id.as_deref() {
            let parent = folder_on(connection, &workspace_id, parent_id, false).await?;
            if parent.collection_id != collection_id {
                return Err(AppError::Validation(
                    "parent folder must belong to the target collection".to_string(),
                ));
            }
        }
        let sort_order = next_folder_sort_order_on(
            connection,
            &workspace_id,
            &collection_id,
            parent_folder_id.as_deref(),
        )
        .await?;
        let id = unfour_core::id::new_id();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO api_collection_folders (
              id, workspace_id, collection_id, parent_folder_id, name, sort_order,
              created_at, updated_at, revision, sync_status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(&collection_id)
        .bind(&parent_folder_id)
        .bind(name)
        .bind(sort_order)
        .bind(now)
        .execute(&mut *connection)
        .await?;
        let parent = effective_parent(&collection_id, parent_folder_id.as_deref());
        Ok(DomainCommandResult::new(
            folder_on(connection, &workspace_id, &id, false).await?,
            vec![mutation(
                context,
                DomainEntityType::ApiFolder,
                MutationOperation::Upsert,
                &workspace_id,
                &id,
                Some(parent),
                1,
            )],
        ))
    }

    pub async fn rename_collection_folder_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        folder_id: String,
        name: String,
    ) -> AppResult<DomainCommandResult<ApiCollectionFolder>> {
        let name = normalize_folder_name(name)?;
        let current = folder_on(connection, &workspace_id, &folder_id, false).await?;
        if current.name == name {
            return Ok(DomainCommandResult::unchanged(current));
        }
        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE api_collection_folders
            SET name = ?1, updated_at = ?2, revision = revision + 1,
                sync_status = 'pending'
            WHERE workspace_id = ?3 AND id = ?4 AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(name)
        .bind(Utc::now().to_rfc3339())
        .bind(&workspace_id)
        .bind(&folder_id)
        .fetch_one(&mut *connection)
        .await?;
        let parent = effective_parent(&current.collection_id, current.parent_folder_id.as_deref());
        Ok(DomainCommandResult::new(
            folder_on(connection, &workspace_id, &folder_id, false).await?,
            vec![mutation(
                context,
                DomainEntityType::ApiFolder,
                MutationOperation::Upsert,
                &workspace_id,
                &folder_id,
                Some(parent),
                revision,
            )],
        ))
    }

    pub async fn delete_collection_folder_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        folder_id: String,
    ) -> AppResult<DomainCommandResult<Vec<ApiCollectionFolder>>> {
        let folder = folder_on(connection, &workspace_id, &folder_id, false).await?;
        let mutations = delete_folder_tree_on(
            connection,
            context,
            &workspace_id,
            &folder_id,
            &Utc::now().to_rfc3339(),
        )
        .await?;
        Ok(DomainCommandResult::new(
            list_folders_on(connection, &workspace_id, Some(&folder.collection_id)).await?,
            mutations,
        ))
    }

    pub async fn move_collection_folder_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        folder_id: String,
        target_parent_folder_id: Option<String>,
    ) -> AppResult<DomainCommandResult<ApiCollectionFolder>> {
        let target_parent_folder_id = normalize_entity_id(target_parent_folder_id);
        let current = folder_on(connection, &workspace_id, &folder_id, false).await?;
        if target_parent_folder_id == current.parent_folder_id {
            return Ok(DomainCommandResult::unchanged(current));
        }
        if target_parent_folder_id.as_deref() == Some(current.id.as_str()) {
            return Err(AppError::Validation(
                "moving folder would create a cycle".to_string(),
            ));
        }
        if let Some(parent_id) = target_parent_folder_id.as_deref() {
            let parent = folder_on(connection, &workspace_id, parent_id, false).await?;
            if parent.collection_id != current.collection_id {
                return Err(AppError::Validation(
                    "target parent folder must belong to the same collection".to_string(),
                ));
            }
            if folder_contains_descendant_on(connection, &workspace_id, &folder_id, parent_id)
                .await?
            {
                return Err(AppError::Validation(
                    "moving folder would create a cycle".to_string(),
                ));
            }
        }
        let sort_order = next_folder_sort_order_on(
            connection,
            &workspace_id,
            &current.collection_id,
            target_parent_folder_id.as_deref(),
        )
        .await?;
        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE api_collection_folders
            SET parent_folder_id = ?1, sort_order = ?2, updated_at = ?3,
                revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?4 AND id = ?5 AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(&target_parent_folder_id)
        .bind(sort_order)
        .bind(Utc::now().to_rfc3339())
        .bind(&workspace_id)
        .bind(&folder_id)
        .fetch_one(&mut *connection)
        .await?;
        let parent = effective_parent(&current.collection_id, target_parent_folder_id.as_deref());
        Ok(DomainCommandResult::new(
            folder_on(connection, &workspace_id, &folder_id, false).await?,
            vec![mutation(
                context,
                DomainEntityType::ApiFolder,
                MutationOperation::Upsert,
                &workspace_id,
                &folder_id,
                Some(parent),
                revision,
            )],
        ))
    }

    pub async fn reorder_collection_folders_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        folder_ids: Vec<String>,
    ) -> AppResult<DomainCommandResult<Vec<ApiCollectionFolder>>> {
        let parent_folder_id = normalize_entity_id(parent_folder_id);
        collection_on(connection, &workspace_id, &collection_id, false).await?;
        let current = list_sibling_folders_on(
            connection,
            &workspace_id,
            &collection_id,
            parent_folder_id.as_deref(),
        )
        .await?;
        validate_reorder_ids(
            &folder_ids,
            current.iter().map(|folder| folder.id.as_str()),
            "folder",
        )?;
        let now = Utc::now().to_rfc3339();
        let mut mutations = Vec::new();
        for (index, folder_id) in folder_ids.iter().enumerate() {
            let sort_order = i64::try_from(index).unwrap_or(i64::MAX);
            let folder = current
                .iter()
                .find(|folder| folder.id == *folder_id)
                .expect("validated folder reorder id");
            if folder.sort_order == sort_order {
                continue;
            }
            let revision: i64 = sqlx::query_scalar(
                r#"
                UPDATE api_collection_folders
                SET sort_order = ?1, updated_at = ?2, revision = revision + 1,
                    sync_status = 'pending'
                WHERE workspace_id = ?3 AND id = ?4 AND deleted_at IS NULL
                RETURNING revision
                "#,
            )
            .bind(sort_order)
            .bind(&now)
            .bind(&workspace_id)
            .bind(folder_id)
            .fetch_one(&mut *connection)
            .await?;
            mutations.push(mutation(
                context,
                DomainEntityType::ApiFolder,
                MutationOperation::Upsert,
                &workspace_id,
                folder_id,
                Some(effective_parent(
                    &collection_id,
                    parent_folder_id.as_deref(),
                )),
                revision,
            ));
        }
        Ok(DomainCommandResult::new(
            list_folders_on(connection, &workspace_id, Some(&collection_id)).await?,
            mutations,
        ))
    }
}

pub(super) async fn delete_collection_tree_on(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    workspace_id: &str,
    collection_id: &str,
    deleted_at: &str,
) -> AppResult<Vec<DomainMutation>> {
    let requests = sqlx::query_as::<_, RequestDeleteRow>(
        r#"
        SELECT id, collection_id, parent_folder_id
        FROM api_requests
        WHERE workspace_id = ?1 AND collection_id = ?2 AND deleted_at IS NULL
        ORDER BY id
        "#,
    )
    .bind(workspace_id)
    .bind(collection_id)
    .fetch_all(&mut *connection)
    .await?;
    let folders = collection_folders_for_delete_on(connection, workspace_id, collection_id).await?;
    let mut mutations = Vec::new();
    for request in requests {
        let revision =
            soft_delete_request_on(connection, workspace_id, &request.id, deleted_at).await?;
        mutations.push(mutation(
            context,
            DomainEntityType::ApiRequest,
            MutationOperation::Delete,
            workspace_id,
            &request.id,
            Some(effective_parent(
                &request.collection_id,
                request.parent_folder_id.as_deref(),
            )),
            revision,
        ));
    }
    for folder in folders {
        let revision =
            soft_delete_folder_on(connection, workspace_id, &folder.id, deleted_at).await?;
        mutations.push(mutation(
            context,
            DomainEntityType::ApiFolder,
            MutationOperation::Delete,
            workspace_id,
            &folder.id,
            Some(effective_parent(
                &folder.collection_id,
                folder.parent_folder_id.as_deref(),
            )),
            revision,
        ));
    }
    if let Some(revision) =
        soft_delete_collection_on(connection, workspace_id, collection_id, deleted_at).await?
    {
        mutations.push(mutation(
            context,
            DomainEntityType::ApiCollection,
            MutationOperation::Delete,
            workspace_id,
            collection_id,
            None,
            revision,
        ));
    }
    Ok(mutations)
}

pub(super) async fn delete_folder_tree_on(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    workspace_id: &str,
    folder_id: &str,
    deleted_at: &str,
) -> AppResult<Vec<DomainMutation>> {
    let folders = folder_tree_for_delete_on(connection, workspace_id, folder_id).await?;
    if folders.is_empty() {
        return Ok(Vec::new());
    }
    let requests = sqlx::query_as::<_, RequestDeleteRow>(
        r#"
        WITH RECURSIVE folder_tree(id) AS (
          SELECT id FROM api_collection_folders
          WHERE workspace_id = ?1 AND id = ?2
          UNION ALL
          SELECT child.id FROM api_collection_folders child
          JOIN folder_tree parent ON child.parent_folder_id = parent.id
          WHERE child.workspace_id = ?1
        )
        SELECT id, collection_id, parent_folder_id
        FROM api_requests
        WHERE workspace_id = ?1 AND parent_folder_id IN (SELECT id FROM folder_tree)
          AND deleted_at IS NULL
        ORDER BY id
        "#,
    )
    .bind(workspace_id)
    .bind(folder_id)
    .fetch_all(&mut *connection)
    .await?;
    let mut mutations = Vec::new();
    for request in requests {
        let revision =
            soft_delete_request_on(connection, workspace_id, &request.id, deleted_at).await?;
        mutations.push(mutation(
            context,
            DomainEntityType::ApiRequest,
            MutationOperation::Delete,
            workspace_id,
            &request.id,
            Some(effective_parent(
                &request.collection_id,
                request.parent_folder_id.as_deref(),
            )),
            revision,
        ));
    }
    for folder in folders {
        let revision =
            soft_delete_folder_on(connection, workspace_id, &folder.id, deleted_at).await?;
        mutations.push(mutation(
            context,
            DomainEntityType::ApiFolder,
            MutationOperation::Delete,
            workspace_id,
            &folder.id,
            Some(effective_parent(
                &folder.collection_id,
                folder.parent_folder_id.as_deref(),
            )),
            revision,
        ));
    }
    Ok(mutations)
}

pub(super) async fn soft_delete_request_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    request_id: &str,
    deleted_at: &str,
) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        r#"
        UPDATE api_requests
        SET deleted_at = ?1, updated_at = ?1, revision = revision + 1,
            sync_status = 'deleted'
        WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
        RETURNING revision
        "#,
    )
    .bind(deleted_at)
    .bind(workspace_id)
    .bind(request_id)
    .fetch_one(&mut *connection)
    .await?)
}

pub(super) async fn soft_delete_folder_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    folder_id: &str,
    deleted_at: &str,
) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        r#"
        UPDATE api_collection_folders
        SET deleted_at = ?1, updated_at = ?1, revision = revision + 1,
            sync_status = 'deleted'
        WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
        RETURNING revision
        "#,
    )
    .bind(deleted_at)
    .bind(workspace_id)
    .bind(folder_id)
    .fetch_one(&mut *connection)
    .await?)
}

pub(super) async fn soft_delete_collection_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    collection_id: &str,
    deleted_at: &str,
) -> AppResult<Option<i64>> {
    Ok(sqlx::query_scalar(
        r#"
        UPDATE api_collections
        SET deleted_at = ?1, updated_at = ?1, revision = revision + 1,
            sync_status = 'deleted'
        WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
        RETURNING revision
        "#,
    )
    .bind(deleted_at)
    .bind(workspace_id)
    .bind(collection_id)
    .fetch_optional(&mut *connection)
    .await?)
}

async fn collection_folders_for_delete_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    collection_id: &str,
) -> AppResult<Vec<FolderDeleteRow>> {
    Ok(sqlx::query_as::<_, FolderDeleteRow>(
        r#"
        WITH RECURSIVE folder_tree(id, collection_id, parent_folder_id, depth) AS (
          SELECT id, collection_id, parent_folder_id, 0
          FROM api_collection_folders
          WHERE workspace_id = ?1 AND collection_id = ?2
            AND parent_folder_id IS NULL AND deleted_at IS NULL
          UNION ALL
          SELECT child.id, child.collection_id, child.parent_folder_id, parent.depth + 1
          FROM api_collection_folders child
          JOIN folder_tree parent ON child.parent_folder_id = parent.id
          WHERE child.workspace_id = ?1 AND child.collection_id = ?2
            AND child.deleted_at IS NULL
        )
        SELECT id, collection_id, parent_folder_id, depth
        FROM folder_tree ORDER BY depth DESC, id
        "#,
    )
    .bind(workspace_id)
    .bind(collection_id)
    .fetch_all(&mut *connection)
    .await?)
}

async fn folder_tree_for_delete_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    folder_id: &str,
) -> AppResult<Vec<FolderDeleteRow>> {
    Ok(sqlx::query_as::<_, FolderDeleteRow>(
        r#"
        WITH RECURSIVE folder_tree(id, collection_id, parent_folder_id, depth) AS (
          SELECT id, collection_id, parent_folder_id, 0
          FROM api_collection_folders
          WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
          UNION ALL
          SELECT child.id, child.collection_id, child.parent_folder_id, parent.depth + 1
          FROM api_collection_folders child
          JOIN folder_tree parent ON child.parent_folder_id = parent.id
          WHERE child.workspace_id = ?1 AND child.deleted_at IS NULL
        )
        SELECT id, collection_id, parent_folder_id, depth
        FROM folder_tree ORDER BY depth DESC, id
        "#,
    )
    .bind(workspace_id)
    .bind(folder_id)
    .fetch_all(&mut *connection)
    .await?)
}

async fn next_folder_sort_order_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    collection_id: &str,
    parent_folder_id: Option<&str>,
) -> AppResult<i64> {
    let value: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT MAX(sort_order) FROM api_collection_folders
        WHERE workspace_id = ?1 AND collection_id = ?2
          AND parent_folder_id IS ?3 AND deleted_at IS NULL
        "#,
    )
    .bind(workspace_id)
    .bind(collection_id)
    .bind(parent_folder_id)
    .fetch_one(&mut *connection)
    .await?;
    Ok(value.unwrap_or(-1) + 1)
}

async fn list_sibling_folders_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    collection_id: &str,
    parent_folder_id: Option<&str>,
) -> AppResult<Vec<ApiCollectionFolder>> {
    Ok(sqlx::query_as::<_, ApiCollectionFolder>(
        r#"
        SELECT id, workspace_id, collection_id, parent_folder_id, name,
               sort_order, created_at, updated_at, deleted_at, revision,
               sync_status, remote_id
        FROM api_collection_folders
        WHERE workspace_id = ?1 AND collection_id = ?2
          AND parent_folder_id IS ?3 AND deleted_at IS NULL
        ORDER BY sort_order, id
        "#,
    )
    .bind(workspace_id)
    .bind(collection_id)
    .bind(parent_folder_id)
    .fetch_all(&mut *connection)
    .await?)
}

async fn folder_contains_descendant_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    folder_id: &str,
    candidate_descendant_id: &str,
) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        r#"
        WITH RECURSIVE folder_tree(id) AS (
          SELECT id FROM api_collection_folders
          WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
          UNION ALL
          SELECT child.id FROM api_collection_folders child
          JOIN folder_tree parent ON child.parent_folder_id = parent.id
          WHERE child.workspace_id = ?1 AND child.deleted_at IS NULL
        )
        SELECT EXISTS(SELECT 1 FROM folder_tree WHERE id = ?3)
        "#,
    )
    .bind(workspace_id)
    .bind(folder_id)
    .bind(candidate_descendant_id)
    .fetch_one(&mut *connection)
    .await?)
}

fn validate_reorder_ids<'a>(
    desired: &[String],
    current: impl Iterator<Item = &'a str>,
    kind: &str,
) -> AppResult<()> {
    let desired_set = desired.iter().map(String::as_str).collect::<HashSet<_>>();
    let current_set = current.collect::<HashSet<_>>();
    if desired.len() != desired_set.len() || desired_set != current_set {
        return Err(AppError::Validation(format!(
            "{kind} reorder must contain every sibling exactly once"
        )));
    }
    Ok(())
}
