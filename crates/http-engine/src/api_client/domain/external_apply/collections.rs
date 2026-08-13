use sqlx::SqliteConnection;
use unfour_core::domain::ExternalApiCollectionUpsert;
use unfour_core::{AppError, AppResult};

use super::helpers::{validate_external_record, validate_owner};
use crate::api_client::domain::{validate_workspace_on, ApiCollectionDomainRow};

pub(super) async fn upsert_collection(
    connection: &mut SqliteConnection,
    record: ExternalApiCollectionUpsert,
) -> AppResult<Option<i64>> {
    validate_external_record(
        &record.id,
        &record.workspace_id,
        &record.created_at,
        &record.updated_at,
    )?;
    validate_workspace_on(connection, &record.workspace_id).await?;
    validate_owner(
        connection,
        "api_collections",
        &record.id,
        &record.workspace_id,
    )
    .await?;
    // Strict-producer / lenient-consumer contract: local commands validate
    // names strictly and the server enforces the 120-rune cap at push time,
    // so the external apply path only rejects blank names. Length or
    // character violations are cosmetic here and must never wedge the puller.
    let name = record.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation(
            "external API collection name cannot be empty".to_string(),
        ));
    }
    let current = sqlx::query_as::<_, ApiCollectionDomainRow>(
        r#"
        SELECT id, workspace_id, name, description, created_at, updated_at,
               deleted_at, revision
        FROM api_collections WHERE id = ?1
        "#,
    )
    .bind(&record.id)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(current) = current {
        if current.deleted_at.is_none()
            && current.name == name
            && current.description == record.description
            && current.created_at == record.created_at
            && current.updated_at == record.updated_at
        {
            return Ok(None);
        }
        return Ok(Some(
            sqlx::query_scalar(
                r#"
            UPDATE api_collections
            SET name = ?1, description = ?2, created_at = ?3, updated_at = ?4,
                deleted_at = NULL, revision = revision + 1, sync_status = 'local'
            WHERE id = ?5 AND workspace_id = ?6 RETURNING revision
            "#,
            )
            .bind(name)
            .bind(record.description)
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
        INSERT INTO api_collections (
          id, workspace_id, name, description, created_at, updated_at,
          revision, sync_status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 'local')
        "#,
    )
    .bind(record.id)
    .bind(record.workspace_id)
    .bind(name)
    .bind(record.description)
    .bind(record.created_at)
    .bind(record.updated_at)
    .execute(&mut *connection)
    .await?;
    Ok(Some(1))
}
