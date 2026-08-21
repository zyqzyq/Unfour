use sqlx::SqliteConnection;
use unfour_core::models::DatabaseConnectionConfig;
use unfour_core::{AppError, AppResult};

pub(super) fn empty_database_config() -> DatabaseConnectionConfig {
    DatabaseConnectionConfig {
        sqlite_path: None,
        connect_timeout_ms: None,
        statement_timeout_ms: None,
        default_schema: None,
    }
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
