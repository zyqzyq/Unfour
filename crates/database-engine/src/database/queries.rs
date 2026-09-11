use super::*;

impl DatabaseService {
    pub async fn execute_query(&self, input: DatabaseQueryInput) -> AppResult<DatabaseQueryResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        let statements = super::script_parser::split_script(&input.sql, &connection.driver)?;
        if statements.len() != 1 {
            return Err(AppError::Validation(
                "exactly one SQL statement is required".into(),
            ));
        }
        let mut safety = classify_query_for_driver(&input.sql, &connection.driver);
        if connection.read_only && safety.classification != "read" {
            return Err(AppError::ReadOnly(format!(
                "this connection is read-only; {} statements are not allowed",
                safety.classification
            )));
        }
        if safety.requires_confirmation && input.confirm_mutation != Some(true) {
            return Err(AppError::ConfirmationRequired {
                message: safety
                    .message
                    .clone()
                    .unwrap_or_else(|| "SQL statement requires confirmation".into()),
                details: serde_json::json!({ "classification": safety.classification, "requiresConfirmation": true, "confirmed": false }),
            });
        }
        safety.confirmed = !safety.requires_confirmation || input.confirm_mutation == Some(true);
        let timeout = resolve_timeout(input.timeout_ms);
        let run = async {
            let mut conn = self.script_connection(&connection, &input).await?;
            conn.execute(
                &input.sql,
                input.limit.unwrap_or(100).clamp(1, 1_000),
                safety,
            )
            .await
            .map(|(result, _)| result)
        };
        let started = Instant::now();
        let result = tokio::time::timeout(timeout, run)
            .await
            .unwrap_or_else(|_| {
                Err(AppError::Timeout(format!(
                    "query exceeded the {} ms timeout; completion may be uncertain",
                    timeout.as_millis()
                )))
            });
        super::script_connection::log_query_outcome(&connection.driver, started, &result);
        result
    }
    pub async fn record_query_history(&self, input: DbQueryHistoryRecordInput) -> AppResult<()> {
        validate_workspace_id(&input.workspace_id)?;
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
              classification, row_count, affected_rows, duration_ms, error, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
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
              created_at AS executed_at
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

    pub async fn list_saved_sql(&self, workspace_id: String) -> AppResult<Vec<SavedSql>> {
        validate_workspace_id(&workspace_id)?;
        let rows = sqlx::query_as::<_, SavedSql>(
            r#"
            SELECT id, workspace_id, connection_id, name, sql, created_at, updated_at,
                   deleted_at, revision, sync_status, remote_id
            FROM saved_sql
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY updated_at DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    pub async fn save_sql(&self, input: SavedSqlInput) -> AppResult<SavedSql> {
        validate_workspace_id(&input.workspace_id)?;
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::Validation(
                "saved SQL name cannot be empty".to_string(),
            ));
        }
        if name.chars().count() > 120 {
            return Err(AppError::Validation(
                "saved SQL name must be 120 characters or fewer".to_string(),
            ));
        }
        let sql = input.sql.trim().to_string();
        if sql.is_empty() {
            return Err(AppError::Validation(
                "saved SQL cannot be empty".to_string(),
            ));
        }
        let connection_id = empty_to_none(input.connection_id);
        if let Some(connection_id) = &connection_id {
            self.get_connection(&input.workspace_id, connection_id)
                .await?;
        }
        let now = Utc::now().to_rfc3339();

        if let Some(id) = input
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            let result = sqlx::query(
                r#"
                UPDATE saved_sql
                SET name = ?1, sql = ?2, connection_id = ?3, updated_at = ?4,
                    revision = revision + 1, sync_status = 'pending'
                WHERE id = ?5 AND workspace_id = ?6 AND deleted_at IS NULL
                "#,
            )
            .bind(&name)
            .bind(&sql)
            .bind(&connection_id)
            .bind(&now)
            .bind(id)
            .bind(&input.workspace_id)
            .execute(self.db.pool())
            .await?;
            if result.rows_affected() == 0 {
                return Err(AppError::NotFound("saved SQL".to_string()));
            }
            return self.get_saved_sql(&input.workspace_id, id).await;
        }

        let id = unfour_core::id::new_id();
        sqlx::query(
            r#"
            INSERT INTO saved_sql (
              id, workspace_id, connection_id, name, sql, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(&connection_id)
        .bind(&name)
        .bind(&sql)
        .bind(&now)
        .execute(self.db.pool())
        .await?;
        self.get_saved_sql(&input.workspace_id, &id).await
    }

    pub async fn delete_saved_sql(
        &self,
        workspace_id: String,
        id: String,
    ) -> AppResult<Vec<SavedSql>> {
        validate_workspace_id(&workspace_id)?;
        let id = id.trim().to_string();
        if id.is_empty() {
            return Err(AppError::Validation(
                "saved SQL id cannot be empty".to_string(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE saved_sql
            SET deleted_at = ?1, updated_at = ?1,
                revision = revision + 1, sync_status = 'deleted'
            WHERE id = ?2 AND workspace_id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&id)
        .bind(&workspace_id)
        .execute(self.db.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("saved SQL".to_string()));
        }
        self.list_saved_sql(workspace_id).await
    }

    async fn get_saved_sql(&self, workspace_id: &str, id: &str) -> AppResult<SavedSql> {
        let row = sqlx::query_as::<_, SavedSql>(
            r#"
            SELECT id, workspace_id, connection_id, name, sql, created_at, updated_at,
                   deleted_at, revision, sync_status, remote_id
            FROM saved_sql
            WHERE id = ?1 AND workspace_id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(self.db.pool())
        .await?;
        row.ok_or_else(|| AppError::NotFound("saved SQL".to_string()))
    }
}
