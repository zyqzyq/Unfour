use sqlx::SqliteConnection;
use unfour_core::domain::{
    DomainEntityKey, DomainEntityType, ExternalConnectionUpsert, ExternalDelete,
};
use unfour_core::models::DatabaseConnectionConfig;
use unfour_core::{AppError, AppResult};

use super::super::connections::{validate_connection_id, validate_workspace_id};

pub(super) fn empty_database_config() -> DatabaseConnectionConfig {
    DatabaseConnectionConfig {
        sqlite_path: None,
        connect_timeout_ms: None,
        statement_timeout_ms: None,
        default_schema: None,
    }
}

pub(super) fn validate_connection_domain_key(key: &DomainEntityKey) -> AppResult<()> {
    validate_workspace_id(&key.workspace_id)?;
    validate_connection_id(&key.entity_id)?;
    if key.entity_type != DomainEntityType::Connection || key.parent_entity_id.is_some() {
        return Err(AppError::Validation(
            "connection domain key must use entity type Connection without a parent".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_external_record(record: &ExternalConnectionUpsert) -> AppResult<()> {
    if [
        record.id.as_str(),
        record.workspace_id.as_str(),
        record.connection_type.as_str(),
        record.created_at.as_str(),
        record.updated_at.as_str(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        return Err(AppError::Validation(
            "external connection upsert requires ids, type, and timestamps".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_external_delete(delete: &ExternalDelete) -> AppResult<()> {
    validate_connection_domain_key(&delete.entity)?;
    if delete.deleted_at.trim().is_empty() {
        return Err(AppError::Validation(
            "external connection delete requires deleted_at".to_string(),
        ));
    }
    Ok(())
}

pub(super) async fn validate_live_workspace_on(
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

pub(super) async fn clear_saved_sql_connection_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    connection_id: &str,
    updated_at: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE saved_sql
        SET connection_id = NULL, updated_at = ?1,
            revision = revision + 1, sync_status = 'pending'
        WHERE workspace_id = ?2 AND connection_id = ?3 AND deleted_at IS NULL
        "#,
    )
    .bind(updated_at)
    .bind(workspace_id)
    .bind(connection_id)
    .execute(&mut *connection)
    .await?;
    Ok(())
}
