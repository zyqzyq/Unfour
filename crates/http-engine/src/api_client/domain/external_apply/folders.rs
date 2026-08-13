use std::collections::HashSet;

use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainEntityType, DomainMutation, ExternalApiFolderUpsert, MutationOperation,
};
use unfour_core::models::ApiCollectionFolder;
use unfour_core::{AppError, AppResult};

use super::helpers::{doomed_orphan_to_skip, validate_external_record, validate_owner};
use crate::api_client::domain::{collection_on, effective_parent, folder_on, mutation};
use crate::api_client::helpers::normalize_entity_id;

/// Apply folder upserts as a fixed point so in-batch parents may arrive in
/// any order. Records are only deferred while their parent is still pending;
/// once a pending parent has been processed and skipped as a doomed orphan
/// (see `doomed_orphan_to_skip`), its children stop being deferred and skip
/// through the absent-parent path themselves instead of being reported as a
/// cyclic hierarchy. The hard error below therefore only fires for true
/// in-batch cycles.
pub(super) async fn apply_folder_upserts(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    mut pending: Vec<ExternalApiFolderUpsert>,
    mutations: &mut Vec<DomainMutation>,
) -> AppResult<()> {
    while !pending.is_empty() {
        let pending_ids = pending
            .iter()
            .map(|record| record.id.clone())
            .collect::<HashSet<_>>();
        let mut deferred = Vec::new();
        let mut applied = 0_usize;
        for record in pending {
            if let Some(parent_id) = record.parent_folder_id.as_deref() {
                let parent_exists: bool = sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                      SELECT 1 FROM api_collection_folders
                      WHERE id = ?1 AND workspace_id = ?2 AND deleted_at IS NULL
                    )
                    "#,
                )
                .bind(parent_id)
                .bind(&record.workspace_id)
                .fetch_one(&mut *connection)
                .await?;
                if !parent_exists && pending_ids.contains(parent_id) {
                    deferred.push(record);
                    continue;
                }
            }
            let workspace_id = record.workspace_id.clone();
            let id = record.id.clone();
            let parent =
                effective_parent(&record.collection_id, record.parent_folder_id.as_deref())
                    .to_string();
            if let Some(revision) = upsert_folder(connection, record).await? {
                mutations.push(mutation(
                    context,
                    DomainEntityType::ApiFolder,
                    MutationOperation::Upsert,
                    &workspace_id,
                    &id,
                    Some(&parent),
                    revision,
                ));
            }
            applied += 1;
        }
        if applied == 0 {
            return Err(AppError::Validation(
                "external API folder hierarchy is cyclic or has an unavailable parent".to_string(),
            ));
        }
        pending = deferred;
    }
    Ok(())
}

async fn upsert_folder(
    connection: &mut SqliteConnection,
    mut record: ExternalApiFolderUpsert,
) -> AppResult<Option<i64>> {
    validate_external_record(
        &record.id,
        &record.workspace_id,
        &record.created_at,
        &record.updated_at,
    )?;
    validate_owner(
        connection,
        "api_collection_folders",
        &record.id,
        &record.workspace_id,
    )
    .await?;
    if doomed_orphan_to_skip(
        collection_on(
            connection,
            &record.workspace_id,
            &record.collection_id,
            false,
        )
        .await,
    )?
    .is_none()
    {
        return Ok(None);
    }
    record.parent_folder_id = normalize_entity_id(record.parent_folder_id);
    // Strict-producer / lenient-consumer contract: local commands validate
    // names strictly and the server enforces the 120-rune cap at push time,
    // so the external apply path only rejects blank names. Length or
    // character violations are cosmetic here and must never wedge the puller.
    let name = record.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation(
            "external API folder name cannot be empty".to_string(),
        ));
    }
    if record.parent_folder_id.as_deref() == Some(record.id.as_str()) {
        return Err(AppError::Validation(
            "external API folder cannot be its own parent".to_string(),
        ));
    }
    if let Some(parent_id) = record.parent_folder_id.as_deref() {
        let Some(parent) = doomed_orphan_to_skip(
            folder_on(connection, &record.workspace_id, parent_id, false).await,
        )?
        else {
            return Ok(None);
        };
        if parent.collection_id != record.collection_id {
            return Err(AppError::Validation(
                "external API folder parent must belong to the same collection".to_string(),
            ));
        }
        let cycle: bool = sqlx::query_scalar(
            r#"
            WITH RECURSIVE descendants(id) AS (
              SELECT id FROM api_collection_folders
              WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
              UNION ALL
              SELECT child.id FROM api_collection_folders child
              JOIN descendants parent ON child.parent_folder_id = parent.id
              WHERE child.workspace_id = ?1 AND child.deleted_at IS NULL
            )
            SELECT EXISTS(SELECT 1 FROM descendants WHERE id = ?3)
            "#,
        )
        .bind(&record.workspace_id)
        .bind(&record.id)
        .bind(parent_id)
        .fetch_one(&mut *connection)
        .await?;
        if cycle {
            return Err(AppError::Validation(
                "external API folder move would create a cycle".to_string(),
            ));
        }
    }
    let current = sqlx::query_as::<_, ApiCollectionFolder>(
        r#"
        SELECT id, workspace_id, collection_id, parent_folder_id, name,
               sort_order, created_at, updated_at, deleted_at, revision,
               sync_status, remote_id
        FROM api_collection_folders WHERE id = ?1
        "#,
    )
    .bind(&record.id)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(current) = current {
        // Local moves never cross collections and cascade deletes rely on
        // that invariant, so a live row must keep its collection; an honest
        // server validates folder-chain consistency and never sends this.
        // Resurrecting a soft-deleted row under a new collection stays
        // allowed: that is an external re-create, not a move.
        if current.deleted_at.is_none() && current.collection_id != record.collection_id {
            return Err(AppError::Validation(
                "external API folder cannot change collection".to_string(),
            ));
        }
        if current.deleted_at.is_none()
            && current.collection_id == record.collection_id
            && current.parent_folder_id == record.parent_folder_id
            && current.name == name
            && current.sort_order == record.sort_order
            && current.created_at == record.created_at
            && current.updated_at == record.updated_at
        {
            return Ok(None);
        }
        return Ok(Some(
            sqlx::query_scalar(
                r#"
            UPDATE api_collection_folders
            SET collection_id = ?1, parent_folder_id = ?2, name = ?3,
                sort_order = ?4, created_at = ?5, updated_at = ?6,
                deleted_at = NULL, revision = revision + 1, sync_status = 'local'
            WHERE id = ?7 AND workspace_id = ?8 RETURNING revision
            "#,
            )
            .bind(record.collection_id)
            .bind(record.parent_folder_id)
            .bind(name)
            .bind(record.sort_order)
            .bind(record.created_at)
            .bind(record.updated_at)
            .bind(record.id)
            .bind(record.workspace_id)
            .fetch_one(&mut *connection)
            .await?,
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO api_collection_folders (
          id, workspace_id, collection_id, parent_folder_id, name, sort_order,
          created_at, updated_at, revision, sync_status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, 'local')
        "#,
    )
    .bind(record.id)
    .bind(record.workspace_id)
    .bind(record.collection_id)
    .bind(record.parent_folder_id)
    .bind(name)
    .bind(record.sort_order)
    .bind(record.created_at)
    .bind(record.updated_at)
    .execute(&mut *connection)
    .await?;
    Ok(Some(1))
}
