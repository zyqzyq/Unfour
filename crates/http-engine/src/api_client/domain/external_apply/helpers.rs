use sqlx::SqliteConnection;
use unfour_core::domain::{DomainEntityType, ExternalDelete};
use unfour_core::{AppError, AppResult};

pub(super) async fn validate_owner(
    connection: &mut SqliteConnection,
    table: &str,
    entity_id: &str,
    workspace_id: &str,
) -> AppResult<()> {
    let sql = match table {
        "api_collections" => "SELECT workspace_id FROM api_collections WHERE id = ?1",
        "api_collection_folders" => "SELECT workspace_id FROM api_collection_folders WHERE id = ?1",
        "api_requests" => "SELECT workspace_id FROM api_requests WHERE id = ?1",
        _ => return Err(AppError::Config("unsupported API entity table".to_string())),
    };
    let owner: Option<String> = sqlx::query_scalar(sql)
        .bind(entity_id)
        .fetch_optional(&mut *connection)
        .await?;
    if owner.as_deref().is_some_and(|owner| owner != workspace_id) {
        return Err(AppError::Validation(
            "external API entity workspace ownership mismatch".to_string(),
        ));
    }
    Ok(())
}

/// Convert a parent lookup into `None` when the parent is absent or
/// soft-deleted, keeping every other failure as a hard error.
///
/// A missing parent on the external apply path is the doomed-orphan race, not
/// corruption: another device created this entity while a concurrent delete
/// of its parent (collection or folder) was already applied locally. The
/// server cascades deletes, so a skipped entity is guaranteed to arrive as a
/// tombstone at a later cursor; writing nothing keeps the puller from wedging
/// on the page by retrying it forever. Genuine local-edit conflicts are
/// intercepted upstream by the Pro client's intent checks before core apply
/// runs. Workspace ownership mismatches, malformed records, and cycle
/// violations still fail the page.
pub(super) fn doomed_orphan_to_skip<T>(lookup: AppResult<T>) -> AppResult<Option<T>> {
    match lookup {
        Ok(value) => Ok(Some(value)),
        Err(AppError::NotFound(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn validate_external_record(
    id: &str,
    workspace_id: &str,
    created_at: &str,
    updated_at: &str,
) -> AppResult<()> {
    if [id, workspace_id, created_at, updated_at]
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err(AppError::Validation(
            "external API upsert requires ids and timestamps".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_delete(
    delete: &ExternalDelete,
    expected: DomainEntityType,
) -> AppResult<()> {
    if delete.entity.entity_type != expected {
        return Err(AppError::Validation(
            "external delete entity type does not match its apply collection".to_string(),
        ));
    }
    if delete.entity.workspace_id.trim().is_empty()
        || delete.entity.entity_id.trim().is_empty()
        || delete.deleted_at.trim().is_empty()
    {
        return Err(AppError::Validation(
            "external delete requires ids and deleted_at".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_parent(delete: &ExternalDelete, actual: &str) -> AppResult<()> {
    if delete
        .entity
        .parent_entity_id
        .as_deref()
        .is_some_and(|provided| provided != actual)
    {
        return Err(AppError::Validation(
            "external API delete parent entity does not match local metadata".to_string(),
        ));
    }
    Ok(())
}
