use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainEntityType, DomainMutation, ExternalDelete, MutationOperation,
};
use unfour_core::models::{ApiCollectionFolder, ApiSavedRequest};
use unfour_core::{AppError, AppResult};

use super::helpers::{validate_delete, validate_parent};
use crate::api_client::domain::collections::{
    delete_collection_tree_on, delete_folder_tree_on, soft_delete_request_on,
};
use crate::api_client::domain::{effective_parent, mutation};

pub(super) async fn apply_request_delete(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    delete: ExternalDelete,
    mutations: &mut Vec<DomainMutation>,
) -> AppResult<()> {
    validate_delete(&delete, DomainEntityType::ApiRequest)?;
    let Some(current) = owned_request_for_delete(connection, &delete).await? else {
        return Ok(());
    };
    if current.deleted_at.is_some() {
        return Ok(());
    }
    validate_parent(
        &delete,
        effective_parent(&current.collection_id, current.parent_folder_id.as_deref()),
    )?;
    let revision = soft_delete_request_on(
        connection,
        &delete.entity.workspace_id,
        &delete.entity.entity_id,
        &delete.deleted_at,
    )
    .await?;
    mutations.push(mutation(
        context,
        DomainEntityType::ApiRequest,
        MutationOperation::Delete,
        &delete.entity.workspace_id,
        &delete.entity.entity_id,
        Some(effective_parent(
            &current.collection_id,
            current.parent_folder_id.as_deref(),
        )),
        revision,
    ));
    Ok(())
}

pub(super) async fn apply_folder_delete(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    delete: ExternalDelete,
    mutations: &mut Vec<DomainMutation>,
) -> AppResult<()> {
    validate_delete(&delete, DomainEntityType::ApiFolder)?;
    let Some(current) = owned_folder_for_delete(connection, &delete).await? else {
        return Ok(());
    };
    if current.deleted_at.is_some() {
        return Ok(());
    }
    validate_parent(
        &delete,
        effective_parent(&current.collection_id, current.parent_folder_id.as_deref()),
    )?;
    mutations.extend(
        delete_folder_tree_on(
            connection,
            context,
            &delete.entity.workspace_id,
            &delete.entity.entity_id,
            &delete.deleted_at,
        )
        .await?,
    );
    Ok(())
}

pub(super) async fn apply_collection_delete(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    delete: ExternalDelete,
    mutations: &mut Vec<DomainMutation>,
) -> AppResult<()> {
    validate_delete(&delete, DomainEntityType::ApiCollection)?;
    let owner: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT workspace_id, deleted_at FROM api_collections WHERE id = ?1")
            .bind(&delete.entity.entity_id)
            .fetch_optional(&mut *connection)
            .await?;
    let Some((workspace_id, deleted)) = owner else {
        return Ok(());
    };
    if workspace_id != delete.entity.workspace_id {
        return Err(AppError::Validation(
            "external API collection workspace ownership mismatch".to_string(),
        ));
    }
    if deleted.is_some() {
        return Ok(());
    }
    mutations.extend(
        delete_collection_tree_on(
            connection,
            context,
            &delete.entity.workspace_id,
            &delete.entity.entity_id,
            &delete.deleted_at,
        )
        .await?,
    );
    Ok(())
}

async fn owned_request_for_delete(
    connection: &mut SqliteConnection,
    delete: &ExternalDelete,
) -> AppResult<Option<ApiSavedRequest>> {
    let row = sqlx::query_as::<_, ApiSavedRequest>(
        r#"
        SELECT id, workspace_id, name, collection_id, parent_folder_id,
               sort_order, auth_json, method, url, headers_json, query_json,
               body, body_kind, settings_json, pre_request_script, post_response_script,
               script_schema_version, created_at, updated_at, deleted_at,
               revision, sync_status, remote_id
        FROM api_requests WHERE id = ?1
        "#,
    )
    .bind(&delete.entity.entity_id)
    .fetch_optional(&mut *connection)
    .await?;
    if row
        .as_ref()
        .is_some_and(|row| row.workspace_id != delete.entity.workspace_id)
    {
        return Err(AppError::Validation(
            "external API request workspace ownership mismatch".to_string(),
        ));
    }
    Ok(row)
}

async fn owned_folder_for_delete(
    connection: &mut SqliteConnection,
    delete: &ExternalDelete,
) -> AppResult<Option<ApiCollectionFolder>> {
    let row = sqlx::query_as::<_, ApiCollectionFolder>(
        r#"
        SELECT id, workspace_id, collection_id, parent_folder_id, name,
               sort_order, created_at, updated_at, deleted_at, revision,
               sync_status, remote_id
        FROM api_collection_folders WHERE id = ?1
        "#,
    )
    .bind(&delete.entity.entity_id)
    .fetch_optional(&mut *connection)
    .await?;
    if row
        .as_ref()
        .is_some_and(|row| row.workspace_id != delete.entity.workspace_id)
    {
        return Err(AppError::Validation(
            "external API folder workspace ownership mismatch".to_string(),
        ));
    }
    Ok(row)
}
