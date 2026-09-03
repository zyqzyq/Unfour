mod collections;
mod external_apply;
mod requests;
mod secrets;
mod snapshot;
mod workspace_cascade;

use sqlx::{FromRow, SqliteConnection};
use unfour_core::domain::{
    CommandContext, DomainEntityKey, DomainEntityType, DomainMutation, MutationOperation,
};
use unfour_core::models::{ApiCollection, ApiCollectionFolder, ApiSavedRequest};
use unfour_core::{AppError, AppResult};

use super::ApiClientService;

#[derive(Debug, Clone, FromRow)]
struct ApiCollectionDomainRow {
    id: String,
    workspace_id: String,
    name: String,
    description: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    revision: i64,
}

impl From<ApiCollectionDomainRow> for ApiCollection {
    fn from(row: ApiCollectionDomainRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            name: row.name,
            description: row.description,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(super) fn mutation(
    context: &CommandContext,
    entity_type: DomainEntityType,
    operation: MutationOperation,
    workspace_id: &str,
    entity_id: &str,
    parent_entity_id: Option<&str>,
    revision: i64,
) -> DomainMutation {
    let key = DomainEntityKey::new(entity_type, workspace_id, entity_id);
    DomainMutation::new(context.origin, operation, key, revision)
        .with_optional_parent(parent_entity_id)
}

trait DomainMutationParentExt {
    fn with_optional_parent(self, parent_entity_id: Option<&str>) -> Self;
}

impl DomainMutationParentExt for DomainMutation {
    fn with_optional_parent(mut self, parent_entity_id: Option<&str>) -> Self {
        self.entity.parent_entity_id = parent_entity_id.map(str::to_string);
        self
    }
}

pub(super) fn effective_parent<'a>(
    collection_id: &'a str,
    parent_folder_id: Option<&'a str>,
) -> &'a str {
    parent_folder_id.unwrap_or(collection_id)
}

fn normalize_collection_name(name: String) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation(
            "collection name cannot be empty".to_string(),
        ));
    }
    if name.chars().count() > 120 {
        return Err(AppError::Validation(
            "collection name must be 120 characters or fewer".to_string(),
        ));
    }
    Ok(name.to_string())
}

async fn validate_workspace_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ?1 AND deleted_at IS NULL)",
    )
    .bind(workspace_id)
    .fetch_one(&mut *connection)
    .await?;
    if !exists {
        return Err(AppError::NotFound("workspace".to_string()));
    }
    Ok(())
}

async fn collection_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    collection_id: &str,
    include_deleted: bool,
) -> AppResult<ApiCollectionDomainRow> {
    sqlx::query_as::<_, ApiCollectionDomainRow>(
        r#"
        SELECT id, workspace_id, name, description, created_at, updated_at,
               deleted_at, revision
        FROM api_collections
        WHERE workspace_id = ?1 AND id = ?2 AND (?3 OR deleted_at IS NULL)
        "#,
    )
    .bind(workspace_id)
    .bind(collection_id)
    .bind(include_deleted)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::NotFound("api collection".to_string()))
}

async fn folder_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    folder_id: &str,
    include_deleted: bool,
) -> AppResult<ApiCollectionFolder> {
    sqlx::query_as::<_, ApiCollectionFolder>(
        r#"
        SELECT id, workspace_id, collection_id, parent_folder_id, name,
               sort_order, created_at, updated_at, deleted_at, revision,
               sync_status, remote_id
        FROM api_collection_folders
        WHERE workspace_id = ?1 AND id = ?2 AND (?3 OR deleted_at IS NULL)
        "#,
    )
    .bind(workspace_id)
    .bind(folder_id)
    .bind(include_deleted)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::NotFound("api collection folder".to_string()))
}

async fn request_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    request_id: &str,
    include_deleted: bool,
) -> AppResult<ApiSavedRequest> {
    sqlx::query_as::<_, ApiSavedRequest>(
        r#"
        SELECT id, workspace_id, name, collection_id, parent_folder_id,
               sort_order, auth_json, method, url, headers_json, query_json,
               body, body_kind, settings_json, pre_request_script, post_response_script,
               script_schema_version, created_at, updated_at, deleted_at,
               revision, sync_status, remote_id
        FROM api_requests
        WHERE workspace_id = ?1 AND id = ?2 AND (?3 OR deleted_at IS NULL)
        "#,
    )
    .bind(workspace_id)
    .bind(request_id)
    .bind(include_deleted)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::NotFound("api request".to_string()))
}

async fn list_collections_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
) -> AppResult<Vec<ApiCollection>> {
    let rows = sqlx::query_as::<_, ApiCollectionDomainRow>(
        r#"
        SELECT id, workspace_id, name, description, created_at, updated_at,
               deleted_at, revision
        FROM api_collections
        WHERE workspace_id = ?1 AND deleted_at IS NULL
        ORDER BY name COLLATE NOCASE
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&mut *connection)
    .await?;
    Ok(rows.into_iter().map(ApiCollection::from).collect())
}

async fn list_folders_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    collection_id: Option<&str>,
) -> AppResult<Vec<ApiCollectionFolder>> {
    let rows = if let Some(collection_id) = collection_id {
        sqlx::query_as::<_, ApiCollectionFolder>(
            r#"
            SELECT id, workspace_id, collection_id, parent_folder_id, name,
                   sort_order, created_at, updated_at, deleted_at, revision,
                   sync_status, remote_id
            FROM api_collection_folders
            WHERE workspace_id = ?1 AND collection_id = ?2 AND deleted_at IS NULL
            ORDER BY COALESCE(parent_folder_id, ''), sort_order, name COLLATE NOCASE
            "#,
        )
        .bind(workspace_id)
        .bind(collection_id)
        .fetch_all(&mut *connection)
        .await?
    } else {
        sqlx::query_as::<_, ApiCollectionFolder>(
            r#"
            SELECT id, workspace_id, collection_id, parent_folder_id, name,
                   sort_order, created_at, updated_at, deleted_at, revision,
                   sync_status, remote_id
            FROM api_collection_folders
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY collection_id, COALESCE(parent_folder_id, ''), sort_order,
                     name COLLATE NOCASE
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?
    };
    Ok(rows)
}

async fn list_requests_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
) -> AppResult<Vec<ApiSavedRequest>> {
    Ok(sqlx::query_as::<_, ApiSavedRequest>(
        r#"
        SELECT id, workspace_id, name, collection_id, parent_folder_id,
               sort_order, auth_json, method, url, headers_json, query_json,
               body, body_kind, settings_json, pre_request_script, post_response_script,
               script_schema_version, created_at, updated_at, deleted_at,
               revision, sync_status, remote_id
        FROM api_requests
        WHERE workspace_id = ?1 AND deleted_at IS NULL
        ORDER BY collection_id, COALESCE(parent_folder_id, ''), sort_order,
                 updated_at DESC
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&mut *connection)
    .await?)
}
