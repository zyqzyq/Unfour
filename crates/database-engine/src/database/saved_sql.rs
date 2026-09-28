use super::*;
use sqlx::SqliteConnection;

impl DatabaseService {
    // Legacy clients may omit context. Only an explicitly configured server
    // database is safe to infer; SQLite paths are not catalogs.
    pub(super) async fn resolve_saved_catalog(
        &self,
        workspace_id: &str,
        connection_id: Option<&str>,
        catalog: Option<String>,
    ) -> AppResult<Option<String>> {
        if let Some(catalog) = empty_to_none(catalog) {
            return Ok(Some(catalog));
        }
        let Some(id) = connection_id.map(str::trim).filter(|id| !id.is_empty()) else {
            return Ok(None);
        };
        match self.get_connection(workspace_id, id).await {
            Ok(connection) if connection.driver != "sqlite" => {
                Ok(empty_to_none(connection.database))
            }
            Ok(_) | Err(AppError::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub async fn list_saved_sql(&self, workspace_id: String) -> AppResult<Vec<SavedSql>> {
        validate_workspace_id(&workspace_id)?;
        let rows = sqlx::query_as::<_, SavedSql>(
            r#"
            SELECT id, workspace_id, connection_id, catalog, schema, name, sql, created_at, updated_at,
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
        let catalog = self
            .resolve_saved_catalog(&input.workspace_id, connection_id.as_deref(), input.catalog)
            .await?;
        let schema = empty_to_none(input.schema);
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
                    revision = revision + 1, sync_status = 'pending', catalog = ?7, schema = ?8
                WHERE id = ?5 AND workspace_id = ?6 AND deleted_at IS NULL
                "#,
            )
            .bind(&name)
            .bind(&sql)
            .bind(&connection_id)
            .bind(&now)
            .bind(id)
            .bind(&input.workspace_id)
            .bind(&catalog)
            .bind(&schema)
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
              id, workspace_id, connection_id, catalog, schema, name, sql, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?7, ?8, ?4, ?5, ?6, ?6, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(&connection_id)
        .bind(&name)
        .bind(&sql)
        .bind(&now)
        .bind(&catalog)
        .bind(&schema)
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
            SELECT id, workspace_id, connection_id, catalog, schema, name, sql, created_at, updated_at,
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
