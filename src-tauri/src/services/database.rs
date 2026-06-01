use crate::app_error::{AppError, AppResult};
use crate::local_db::LocalDb;
use crate::models::{
    DatabaseBrowseInput, DatabaseBrowseResult, DatabaseConnection, DatabaseConnectionConfig,
    DatabaseConnectionInput, DatabaseQueryInput, DatabaseQueryResult, DatabaseResultColumn,
    DatabaseSchema, DatabaseTable, DatabaseTableColumn, DatabaseTestResult, StoredConnection,
};
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::path::Path;
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone)]
pub struct DatabaseService {
    db: LocalDb,
}

impl DatabaseService {
    pub fn new(db: LocalDb) -> Self {
        Self { db }
    }

    pub async fn list_connections(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<DatabaseConnection>> {
        validate_workspace_id(&workspace_id)?;

        let rows = sqlx::query_as::<_, StoredConnection>(
            r#"
            SELECT
              id, workspace_id, kind, name, config_json, credential_ref, created_at,
              updated_at, deleted_at, revision, sync_status, remote_id
            FROM connections
            WHERE workspace_id = ?1 AND kind = 'database' AND deleted_at IS NULL
            ORDER BY updated_at DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter()
            .map(stored_to_database_connection)
            .collect()
    }

    pub async fn save_connection(
        &self,
        input: DatabaseConnectionInput,
    ) -> AppResult<DatabaseConnection> {
        validate_workspace_id(&input.workspace_id)?;
        let config = input_to_config(&input)?;
        let name = normalize_name(&input.name)?;
        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(&config)?;

        if let Some(id) = input
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            let result = sqlx::query(
                r#"
                UPDATE connections
                SET name = ?1, config_json = ?2, credential_ref = ?3,
                    updated_at = ?4, revision = revision + 1, sync_status = 'pending'
                WHERE id = ?5 AND workspace_id = ?6 AND kind = 'database' AND deleted_at IS NULL
                "#,
            )
            .bind(name)
            .bind(config_json)
            .bind(empty_to_none(input.credential_ref))
            .bind(now)
            .bind(id)
            .bind(&input.workspace_id)
            .execute(self.db.pool())
            .await?;

            if result.rows_affected() == 0 {
                return Err(AppError::NotFound("database connection".to_string()));
            }

            return self.get_connection(&input.workspace_id, id).await;
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO connections (
              id, workspace_id, kind, name, config_json, credential_ref,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, 'database', ?3, ?4, ?5, ?6, ?6, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(name)
        .bind(config_json)
        .bind(empty_to_none(input.credential_ref))
        .bind(now)
        .execute(self.db.pool())
        .await?;

        self.get_connection(&input.workspace_id, &id).await
    }

    pub async fn delete_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<Vec<DatabaseConnection>> {
        validate_workspace_id(&workspace_id)?;
        validate_connection_id(&connection_id)?;
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE connections
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1, sync_status = 'deleted'
            WHERE id = ?2 AND workspace_id = ?3 AND kind = 'database' AND deleted_at IS NULL
            "#,
        )
        .bind(now)
        .bind(connection_id)
        .bind(&workspace_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("database connection".to_string()));
        }

        self.list_connections(workspace_id).await
    }

    pub async fn test_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<DatabaseTestResult> {
        let connection = self.get_connection(&workspace_id, &connection_id).await?;

        match connection.driver.as_str() {
            "sqlite" => {
                let pool = sqlite_pool(&connection).await?;
                let version: (String,) = sqlx::query_as("SELECT sqlite_version()")
                    .fetch_one(&pool)
                    .await?;

                Ok(DatabaseTestResult {
                    ok: true,
                    message: "SQLite connection OK".to_string(),
                    server_version: Some(version.0),
                })
            }
            "postgres" | "mysql" => Ok(DatabaseTestResult {
                ok: false,
                message: format!(
                    "{} metadata is saved; credential-backed live connections are reserved for the next phase.",
                    display_driver(&connection.driver)
                ),
                server_version: None,
            }),
            driver => Err(AppError::Unsupported(format!(
                "database driver is not supported: {}",
                driver
            ))),
        }
    }

    pub async fn schema(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<DatabaseSchema> {
        let connection = self.get_connection(&workspace_id, &connection_id).await?;
        if connection.driver != "sqlite" {
            return Err(AppError::Unsupported(format!(
                "{} schema browsing is reserved for the next phase",
                display_driver(&connection.driver)
            )));
        }

        let pool = sqlite_pool(&connection).await?;
        let table_rows = sqlx::query(
            r#"
            SELECT name, type
            FROM sqlite_master
            WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
            ORDER BY type, name
            "#,
        )
        .fetch_all(&pool)
        .await?;

        let mut tables = Vec::with_capacity(table_rows.len());
        for row in table_rows {
            let name: String = row.try_get("name")?;
            let kind: String = row.try_get("type")?;
            let columns = sqlite_columns(&pool, &name).await?;
            tables.push(DatabaseTable {
                name,
                kind,
                columns,
            });
        }

        Ok(DatabaseSchema {
            connection_id,
            tables,
        })
    }

    pub async fn execute_query(&self, input: DatabaseQueryInput) -> AppResult<DatabaseQueryResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let sql = input.sql.trim();
        if sql.is_empty() {
            return Err(AppError::Validation("SQL cannot be empty".to_string()));
        }

        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        if connection.driver != "sqlite" {
            return Err(AppError::Unsupported(format!(
                "{} query execution is reserved for the next phase",
                display_driver(&connection.driver)
            )));
        }

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
            });
        }

        let result = sqlx::query(sql).execute(&pool).await?;
        Ok(DatabaseQueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows: result.rows_affected(),
            duration_ms: started.elapsed().as_millis(),
        })
    }

    pub async fn browse_table(
        &self,
        input: DatabaseBrowseInput,
    ) -> AppResult<DatabaseBrowseResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let table_name = input.table_name.trim();
        if table_name.is_empty() {
            return Err(AppError::Validation(
                "table name cannot be empty".to_string(),
            ));
        }

        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        if connection.driver != "sqlite" {
            return Err(AppError::Unsupported(format!(
                "{} table browsing is reserved for the next phase",
                display_driver(&connection.driver)
            )));
        }

        let pool = sqlite_pool(&connection).await?;
        ensure_sqlite_table_exists(&pool, table_name).await?;

        let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
        let sql = format!(
            "SELECT * FROM {} LIMIT {}",
            quote_identifier(table_name),
            limit
        );
        let started = Instant::now();
        let rows = sqlx::query(&sql).fetch_all(&pool).await?;
        let columns = rows.first().map(sqlite_result_columns).unwrap_or_default();
        let values = rows
            .iter()
            .map(sqlite_row_values)
            .collect::<AppResult<Vec<_>>>()?;

        Ok(DatabaseBrowseResult {
            sql,
            result: DatabaseQueryResult {
                columns,
                rows: values,
                affected_rows: 0,
                duration_ms: started.elapsed().as_millis(),
            },
        })
    }

    pub fn capability_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "status": "mvp-started",
            "backend": "sqlx",
            "activeDrivers": ["sqlite"],
            "reservedDrivers": ["postgres", "mysql"],
            "features": [
                "connection-metadata-crud",
                "sqlite-connection-test",
                "sqlite-schema-browser",
                "sqlite-sql-editor",
                "paged-query-results"
            ]
        })
    }

    async fn get_connection(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> AppResult<DatabaseConnection> {
        validate_workspace_id(workspace_id)?;
        validate_connection_id(connection_id)?;

        let row = sqlx::query_as::<_, StoredConnection>(
            r#"
            SELECT
              id, workspace_id, kind, name, config_json, credential_ref, created_at,
              updated_at, deleted_at, revision, sync_status, remote_id
            FROM connections
            WHERE id = ?1 AND workspace_id = ?2 AND kind = 'database' AND deleted_at IS NULL
            "#,
        )
        .bind(connection_id)
        .bind(workspace_id)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(stored_to_database_connection)
            .transpose()?
            .ok_or_else(|| AppError::NotFound("database connection".to_string()))
    }
}

async fn sqlite_columns(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<Vec<DatabaseTableColumn>> {
    let sql = format!("PRAGMA table_info({})", quote_identifier(table_name));
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let name: String = row.try_get("name")?;
            let data_type: String = row.try_get("type")?;
            let notnull: i64 = row.try_get("notnull")?;
            let primary_key: i64 = row.try_get("pk")?;

            Ok(DatabaseTableColumn {
                name,
                data_type,
                nullable: notnull == 0,
                primary_key: primary_key > 0,
            })
        })
        .collect()
}

async fn ensure_sqlite_table_exists(pool: &sqlx::SqlitePool, table_name: &str) -> AppResult<()> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT name
        FROM sqlite_master
        WHERE name = ?1 AND type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
        LIMIT 1
        "#,
    )
    .bind(table_name)
    .fetch_optional(pool)
    .await?;

    row.map(|_| ())
        .ok_or_else(|| AppError::NotFound("database table".to_string()))
}

fn sqlite_result_columns(row: &sqlx::sqlite::SqliteRow) -> Vec<DatabaseResultColumn> {
    row.columns()
        .iter()
        .map(|column| DatabaseResultColumn {
            name: column.name().to_string(),
            data_type: column.type_info().name().to_string(),
        })
        .collect()
}

fn sqlite_row_values(row: &sqlx::sqlite::SqliteRow) -> AppResult<Vec<Option<String>>> {
    (0..row.columns().len())
        .map(|index| {
            let raw = row.try_get_raw(index)?;
            if raw.is_null() {
                return Ok(None);
            }

            if let Ok(value) = row.try_get::<String, _>(index) {
                return Ok(Some(value));
            }
            if let Ok(value) = row.try_get::<i64, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<f64, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<bool, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
                return Ok(Some(format!("<binary {} bytes>", value.len())));
            }

            Ok(Some("<unsupported>".to_string()))
        })
        .collect()
}

async fn sqlite_pool(connection: &DatabaseConnection) -> AppResult<sqlx::SqlitePool> {
    let path = connection
        .sqlite_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("SQLite path is required".to_string()))?;

    if path != ":memory:" && !Path::new(path).exists() {
        return Err(AppError::Validation(format!(
            "SQLite file does not exist: {}",
            path
        )));
    }

    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .foreign_keys(true);

    SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(AppError::from)
}

fn stored_to_database_connection(row: StoredConnection) -> AppResult<DatabaseConnection> {
    let config = serde_json::from_str::<DatabaseConnectionConfig>(&row.config_json)?;
    Ok(DatabaseConnection {
        id: row.id,
        workspace_id: row.workspace_id,
        name: row.name,
        driver: config.driver,
        host: config.host,
        port: config.port,
        database: config.database,
        username: config.username,
        sqlite_path: config.sqlite_path,
        credential_ref: row.credential_ref,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
        revision: row.revision,
        sync_status: row.sync_status,
        remote_id: row.remote_id,
    })
}

fn input_to_config(input: &DatabaseConnectionInput) -> AppResult<DatabaseConnectionConfig> {
    let driver = input.driver.trim().to_ascii_lowercase();
    if !matches!(driver.as_str(), "sqlite" | "postgres" | "mysql") {
        return Err(AppError::Validation(format!(
            "unsupported database driver: {}",
            input.driver
        )));
    }

    if driver == "sqlite" {
        let sqlite_path = input
            .sqlite_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::Validation("SQLite path is required".to_string()))?;

        return Ok(DatabaseConnectionConfig {
            driver,
            host: None,
            port: None,
            database: None,
            username: None,
            sqlite_path: Some(sqlite_path.to_string()),
        });
    }

    Ok(DatabaseConnectionConfig {
        driver,
        host: empty_to_none(input.host.clone()),
        port: input.port,
        database: empty_to_none(input.database.clone()),
        username: empty_to_none(input.username.clone()),
        sqlite_path: None,
    })
}

fn normalize_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "database connection name cannot be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 80 {
        return Err(AppError::Validation(
            "database connection name must be 80 characters or fewer".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn validate_workspace_id(workspace_id: &str) -> AppResult<()> {
    if workspace_id.trim().is_empty() {
        return Err(AppError::Validation(
            "workspace id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_connection_id(connection_id: &str) -> AppResult<()> {
    if connection_id.trim().is_empty() {
        return Err(AppError::Validation(
            "database connection id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn returns_rows(sql: &str) -> bool {
    let keyword = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(keyword.as_str(), "select" | "with" | "pragma" | "explain")
}

fn sql_with_limit(sql: &str, limit: u32) -> String {
    let trimmed = sql.trim().trim_end_matches(';');
    let lower = trimmed.to_ascii_lowercase();
    if (lower.starts_with("select") || lower.starts_with("with")) && !lower.contains(" limit ") {
        format!("{} LIMIT {}", trimmed, limit)
    } else {
        trimmed.to_string()
    }
}

fn display_driver(driver: &str) -> &'static str {
    match driver {
        "postgres" => "PostgreSQL",
        "mysql" => "MySQL/MariaDB",
        "sqlite" => "SQLite",
        _ => "Database",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Executor;
    use std::fs;
    use std::path::PathBuf;

    async fn service_with_workspace() -> (DatabaseService, String) {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory app db");
        let db = LocalDb::from_pool(pool);
        db.migrate().await.expect("run migrations");

        let workspace_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, last_opened_at, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, 'Test Workspace', 1, ?2, ?2, ?2, 1, 'local')
            "#,
        )
        .bind(&workspace_id)
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("insert workspace");

        (DatabaseService::new(db), workspace_id)
    }

    async fn sqlite_fixture() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("unfour-database-service-{}.sqlite", Uuid::new_v4()));
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect fixture sqlite");

        pool.execute(
            r#"
            CREATE TABLE deploys (
              id INTEGER PRIMARY KEY,
              service TEXT NOT NULL,
              version TEXT NOT NULL
            )
            "#,
        )
        .await
        .expect("create deploys");
        pool.execute(
            "INSERT INTO deploys (service, version) VALUES ('api', '1.0.0'), ('worker', '1.0.1')",
        )
        .await
        .expect("insert deploys");
        pool.close().await;

        path
    }

    fn sqlite_input(workspace_id: &str, path: &PathBuf) -> DatabaseConnectionInput {
        DatabaseConnectionInput {
            id: None,
            workspace_id: workspace_id.to_string(),
            name: "Local fixture".to_string(),
            driver: "sqlite".to_string(),
            host: None,
            port: None,
            database: None,
            username: None,
            sqlite_path: Some(path.to_string_lossy().to_string()),
            credential_ref: Some("  ".to_string()),
        }
    }

    #[tokio::test]
    async fn connection_crud_is_workspace_scoped_and_soft_deletes() {
        let (service, workspace_id) = service_with_workspace().await;
        let path = sqlite_fixture().await;

        let created = service
            .save_connection(sqlite_input(&workspace_id, &path))
            .await
            .expect("save connection");
        assert_eq!(created.name, "Local fixture");
        assert_eq!(created.driver, "sqlite");
        assert!(created.credential_ref.is_none());

        let listed = service
            .list_connections(workspace_id.clone())
            .await
            .expect("list connections");
        assert_eq!(listed.len(), 1);

        let updated = service
            .save_connection(DatabaseConnectionInput {
                id: Some(created.id.clone()),
                name: "Renamed fixture".to_string(),
                ..sqlite_input(&workspace_id, &path)
            })
            .await
            .expect("update connection");
        assert_eq!(updated.name, "Renamed fixture");
        assert_eq!(updated.revision, created.revision + 1);

        let after_delete = service
            .delete_connection(workspace_id.clone(), created.id)
            .await
            .expect("delete connection");
        assert!(after_delete.is_empty());

        let listed = service
            .list_connections(workspace_id)
            .await
            .expect("list after delete");
        assert!(listed.is_empty());
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn sqlite_schema_query_and_safe_browse_work() {
        let (service, workspace_id) = service_with_workspace().await;
        let path = sqlite_fixture().await;
        let connection = service
            .save_connection(sqlite_input(&workspace_id, &path))
            .await
            .expect("save connection");

        let test = service
            .test_connection(workspace_id.clone(), connection.id.clone())
            .await
            .expect("test connection");
        assert!(test.ok);
        assert!(test.server_version.is_some());

        let schema = service
            .schema(workspace_id.clone(), connection.id.clone())
            .await
            .expect("schema");
        let deploys = schema
            .tables
            .iter()
            .find(|table| table.name == "deploys")
            .expect("deploys table");
        assert!(deploys
            .columns
            .iter()
            .any(|column| column.name == "service" && !column.primary_key));

        let query = service
            .execute_query(DatabaseQueryInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id.clone(),
                sql: "select service, version from deploys order by id".to_string(),
                limit: Some(1),
            })
            .await
            .expect("query");
        assert_eq!(query.rows.len(), 1);
        assert_eq!(query.rows[0][0].as_deref(), Some("api"));

        let browse = service
            .browse_table(DatabaseBrowseInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id.clone(),
                table_name: "deploys".to_string(),
                limit: Some(2),
            })
            .await
            .expect("browse table");
        assert_eq!(browse.sql, "SELECT * FROM \"deploys\" LIMIT 2");
        assert_eq!(browse.result.rows.len(), 2);

        let missing = service
            .browse_table(DatabaseBrowseInput {
                workspace_id,
                connection_id: connection.id,
                table_name: "missing".to_string(),
                limit: Some(10),
            })
            .await;
        assert!(matches!(missing, Err(AppError::NotFound(_))));
        let _ = fs::remove_file(path);
    }
}
