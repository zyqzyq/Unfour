use super::*;

impl DatabaseService {
    pub async fn record_query_history(&self, input: DbQueryHistoryRecordInput) -> AppResult<()> {
        validate_workspace_id(&input.workspace_id)?;
        let catalog = self
            .resolve_saved_catalog(
                &input.workspace_id,
                input.connection_id.as_deref(),
                input.catalog,
            )
            .await?;
        let schema = empty_to_none(input.schema);
        let workspace_id = input.workspace_id;
        let id = input.id.trim().to_string();
        if id.is_empty() {
            return Err(AppError::Validation(
                "database query history id cannot be empty".to_string(),
            ));
        }

        let sql = input.sql.trim().to_string();
        if sql.is_empty() {
            return Err(AppError::Validation(
                "database query history SQL cannot be empty".to_string(),
            ));
        }

        let connection_name = input.connection_name.trim().to_string();
        if connection_name.is_empty() {
            return Err(AppError::Validation(
                "database query history connection name cannot be empty".to_string(),
            ));
        }

        let status = input.status.trim().to_string();
        if !matches!(status.as_str(), "success" | "failed") {
            return Err(AppError::Validation(
                "database query history status must be success or failed".to_string(),
            ));
        }

        let executed_at = input.executed_at.trim().to_string();
        if executed_at.is_empty() {
            return Err(AppError::Validation(
                "database query history timestamp cannot be empty".to_string(),
            ));
        }

        sqlx::query(
            r#"
            INSERT INTO db_query_history (
              id, workspace_id, connection_id, connection_name, sql, status,
              classification, row_count, affected_rows, duration_ms, error, created_at, catalog, schema
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
        )
        .bind(id)
        .bind(workspace_id)
        .bind(empty_to_none(input.connection_id))
        .bind(connection_name)
        .bind(sql)
        .bind(status)
        .bind(empty_to_none(input.classification))
        .bind(input.row_count)
        .bind(input.affected_rows)
        .bind(input.duration_ms)
        .bind(empty_to_none(input.error))
        .bind(executed_at)
        .bind(catalog)
        .bind(schema)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    pub async fn list_query_history(
        &self,
        workspace_id: String,
        limit: Option<i64>,
    ) -> AppResult<Vec<DbQueryHistoryEntry>> {
        validate_workspace_id(&workspace_id)?;
        let limit = limit.unwrap_or(200).clamp(1, 200);

        let entries = sqlx::query_as::<_, DbQueryHistoryEntry>(
            r#"
            SELECT
              id, workspace_id, connection_id, connection_name, sql, status,
              classification, row_count, affected_rows, duration_ms, error,
              created_at AS executed_at, catalog, schema
            FROM db_query_history
            WHERE workspace_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .bind(workspace_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(entries)
    }

    pub async fn clear_query_history(&self, workspace_id: String) -> AppResult<()> {
        validate_workspace_id(&workspace_id)?;

        sqlx::query(
            r#"
            DELETE FROM db_query_history
            WHERE workspace_id = ?1
            "#,
        )
        .bind(workspace_id)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }
}
