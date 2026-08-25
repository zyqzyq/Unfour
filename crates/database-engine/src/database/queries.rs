use super::*;

impl DatabaseService {
    pub async fn execute_query(&self, input: DatabaseQueryInput) -> AppResult<DatabaseQueryResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let sql = input.sql.trim();
        if sql.is_empty() {
            return Err(AppError::Validation("SQL cannot be empty".to_string()));
        }
        validate_single_statement(sql)?;
        let catalog = clean_identifier(input.catalog.as_deref())?;
        let safety = classify_query(sql);

        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;

        // A read-only connection blocks anything other than a read, taking
        // precedence over the confirmation prompt: confirming cannot override it.
        if connection.read_only && safety.classification != "read" {
            return Err(AppError::ReadOnly(format!(
                "this connection is read-only; {} statements are not allowed",
                safety.classification
            )));
        }

        if safety.requires_confirmation && input.confirm_mutation != Some(true) {
            return Err(AppError::ConfirmationRequired {
                message: safety.message.clone().unwrap_or_else(|| {
                    "SQL statement requires confirmation before execution".to_string()
                }),
                details: serde_json::json!({
                    "classification": safety.classification,
                    "requiresConfirmation": safety.requires_confirmation,
                    "confirmed": false
                }),
            });
        }

        let timeout = resolve_timeout(input.timeout_ms);
        let query_started = Instant::now();
        let driver = connection.driver.clone();
        let sql_operation = safety.classification.clone();
        let query_fields = serde_json::json!({
            "driver": &driver,
            "sql_operation": &sql_operation,
        });
        unfour_diag::log_operation_event(
            "query_started",
            "database",
            "execute_query",
            "started",
            None,
            None,
            query_fields.clone(),
        );
        let run = async {
            match connection.driver.as_str() {
                "sqlite" => {
                    let pool = sqlite_pool(&connection).await?;
                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let started = Instant::now();

                    if returns_rows(sql) {
                        let query_sql = sql_with_limit(sql, limit);
                        let rows = sqlx::query(&query_sql).fetch_all(&pool).await?;
                        let columns = rows.first().map(sqlite_result_columns).unwrap_or_default();
                        let values = rows
                            .iter()
                            .take(limit as usize)
                            .map(sqlite_row_values)
                            .collect::<AppResult<Vec<_>>>()?;

                        return Ok(DatabaseQueryResult {
                            columns,
                            rows: values,
                            affected_rows: 0,
                            duration_ms: started.elapsed().as_millis(),
                            safety,
                        });
                    }

                    let result = sqlx::query(sql).execute(&pool).await?;
                    Ok(DatabaseQueryResult {
                        columns: Vec::new(),
                        rows: Vec::new(),
                        affected_rows: result.rows_affected(),
                        duration_ms: started.elapsed().as_millis(),
                        safety: DatabaseQuerySafety {
                            confirmed: input.confirm_mutation == Some(true),
                            ..safety
                        },
                    })
                }
                "postgres" => {
                    let effective = Self::effective_connection(&connection, catalog);
                    let pool = self.postgres_pool(&effective).await?;
                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let started = Instant::now();

                    // Apply the query context on a dedicated connection so the
                    // search_path change and the statement share the same session.
                    let mut conn = pool.acquire().await.map_err(sanitize_pg_error)?;
                    if let Some(schema) = clean_identifier(input.schema.as_deref())? {
                        let stmt = format!("SET search_path TO {}", quote_identifier(schema));
                        // Session commands such as SET are not portable through
                        // SQLx's prepared-query API. Execute this context change
                        // through the database's simple-query protocol instead.
                        conn.as_mut()
                            .execute(stmt.as_str())
                            .await
                            .map_err(sanitize_pg_error)?;
                    }

                    if returns_rows(sql) {
                        let query_sql = sql_with_limit(sql, limit);
                        // The SQL editor accepts arbitrary user statements and
                        // does not expose bind parameters. Use the simple query
                        // protocol so commands such as SET/BEGIN/ROLLBACK and
                        // engine-specific statements are not rejected during
                        // prepared-statement parsing.
                        let rows = conn
                            .as_mut()
                            .fetch_all(query_sql.as_str())
                            .await
                            .map_err(sanitize_pg_error)?;
                        let columns = rows
                            .first()
                            .map(postgres_result_columns)
                            .unwrap_or_default();
                        let values = rows
                            .iter()
                            .take(limit as usize)
                            .map(postgres_row_values)
                            .collect::<AppResult<Vec<_>>>()?;

                        return Ok(DatabaseQueryResult {
                            columns,
                            rows: values,
                            affected_rows: 0,
                            duration_ms: started.elapsed().as_millis(),
                            safety,
                        });
                    }

                    let result = conn
                        .as_mut()
                        .execute(sql)
                        .await
                        .map_err(sanitize_pg_error)?;
                    Ok(DatabaseQueryResult {
                        columns: Vec::new(),
                        rows: Vec::new(),
                        affected_rows: result.rows_affected(),
                        duration_ms: started.elapsed().as_millis(),
                        safety: DatabaseQuerySafety {
                            confirmed: input.confirm_mutation == Some(true),
                            ..safety
                        },
                    })
                }
                "mysql" => {
                    let effective = Self::effective_connection(&connection, catalog);
                    let pool = self.mysql_pool(&effective).await?;
                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let started = Instant::now();

                    // The effective connection is opened against the selected
                    // catalog, so no session-level USE command is needed here.
                    // This is important because MySQL does not support USE in
                    // the prepared-statement protocol.
                    let mut conn = pool.acquire().await.map_err(sanitize_mysql_error)?;

                    if returns_rows(sql) {
                        let query_sql = sql_with_limit(sql, limit);
                        // Keep editor SQL on the simple-query protocol. SQLx's
                        // prepared API rejects valid MySQL/MariaDB commands
                        // such as SET, transaction control, and some DDL/SHOW
                        // variants even though they are valid editor input.
                        let rows = conn
                            .as_mut()
                            .fetch_all(query_sql.as_str())
                            .await
                            .map_err(sanitize_mysql_error)?;
                        let columns = rows.first().map(mysql_result_columns).unwrap_or_default();
                        let values = rows
                            .iter()
                            .take(limit as usize)
                            .map(mysql_row_values)
                            .collect::<AppResult<Vec<_>>>()?;

                        return Ok(DatabaseQueryResult {
                            columns,
                            rows: values,
                            affected_rows: 0,
                            duration_ms: started.elapsed().as_millis(),
                            safety,
                        });
                    }

                    let result = conn
                        .as_mut()
                        .execute(sql)
                        .await
                        .map_err(sanitize_mysql_error)?;
                    Ok(DatabaseQueryResult {
                        columns: Vec::new(),
                        rows: Vec::new(),
                        affected_rows: result.rows_affected(),
                        duration_ms: started.elapsed().as_millis(),
                        safety: DatabaseQuerySafety {
                            confirmed: input.confirm_mutation == Some(true),
                            ..safety
                        },
                    })
                }
                driver => Err(AppError::Unsupported(format!(
                    "{} query execution is not yet supported",
                    display_driver(driver)
                ))),
            }
        };
        let result = match tokio::time::timeout(timeout, run).await {
            Ok(result) => result,
            Err(_) => Err(AppError::Timeout(format!(
                "query exceeded the {} ms timeout",
                timeout.as_millis()
            ))),
        };

        match &result {
            Ok(result) => {
                unfour_diag::log_operation_event(
                    "query_completed",
                    "database",
                    "execute_query",
                    "ok",
                    Some(query_started.elapsed().as_millis()),
                    None,
                    serde_json::json!({
                        "driver": &driver,
                        "sql_operation": &sql_operation,
                        "row_count": result.rows.len(),
                        "affected_rows": result.affected_rows,
                    }),
                );
            }
            Err(error) => {
                unfour_diag::log_operation_event(
                    "query_failed",
                    "database",
                    "execute_query",
                    "error",
                    Some(query_started.elapsed().as_millis()),
                    Some(unfour_diag::app_error_kind(error)),
                    query_fields,
                );
            }
        }
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
