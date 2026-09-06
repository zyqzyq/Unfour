//! Registry-driven parent mapping and validation for canonical operations.

use sqlx::SqliteConnection;
use unfour_core::domain::DomainMutation;

use crate::{sync_entity_descriptor, ParentDependency, SyncEntityType, SyncError};

pub(super) async fn protocol_parent(
    connection: &mut SqliteConnection,
    mutation: &DomainMutation,
) -> Result<Option<String>, SyncError> {
    let entity_type = SyncEntityType::from(mutation.entity.entity_type);
    let descriptor = sync_entity_descriptor(entity_type);
    if !matches!(descriptor.parent_dependency, ParentDependency::Entity(_))
        || mutation.entity.parent_entity_id.is_some()
    {
        return canonical_parent(
            entity_type,
            &mutation.entity.workspace_id,
            mutation.entity.parent_entity_id.as_deref(),
        );
    }
    let storage = descriptor.local_storage;
    let workspace_column = storage.workspace_column.ok_or(SyncError::InvalidData)?;
    let sql = format!(
        "SELECT {} FROM {} WHERE id = ?1 AND {} = ?2",
        storage.parent_expression, storage.table, workspace_column
    );
    let parent = sqlx::query_scalar::<_, String>(&sql)
        .bind(&mutation.entity.entity_id)
        .bind(&mutation.entity.workspace_id)
        .fetch_one(&mut *connection)
        .await?;
    canonical_parent(entity_type, &mutation.entity.workspace_id, Some(&parent))
}

pub(super) fn canonical_parent(
    entity_type: SyncEntityType,
    workspace_id: &str,
    parent: Option<&str>,
) -> Result<Option<String>, SyncError> {
    match sync_entity_descriptor(entity_type).parent_dependency {
        ParentDependency::None => Ok(None),
        ParentDependency::Workspace => Ok(Some(workspace_id.to_string())),
        ParentDependency::Entity(_) => parent
            .filter(|value| !value.trim().is_empty())
            .map(|value| Some(value.to_string()))
            .ok_or(SyncError::InvalidData),
    }
}

pub(super) fn validate_parent(
    workspace_id: &str,
    entity_type: SyncEntityType,
    entity_id: &str,
    parent: Option<&str>,
) -> Result<(), SyncError> {
    let dependency = sync_entity_descriptor(entity_type).parent_dependency;
    let valid = match dependency {
        ParentDependency::None => parent.is_none(),
        ParentDependency::Workspace => parent == Some(workspace_id),
        ParentDependency::Entity(_) => parent.is_some_and(|value| !value.trim().is_empty()),
    };
    (valid && (entity_type != SyncEntityType::Workspace || entity_id == workspace_id))
        .then_some(())
        .ok_or(SyncError::InvalidData)
}
