use chrono::Utc;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::path::Path;
use std::time::{Duration, Instant};
use unfour_core::models::{
    DatabaseBrowseInput, DatabaseBrowseResult, DatabaseCellValue, DatabaseConnection,
    DatabaseConnectionConfig, DatabaseConnectionInput, DatabaseForeignKey, DatabaseIndex,
    DatabaseQueryInput, DatabaseQueryResult, DatabaseQuerySafety, DatabaseResultColumn,
    DatabaseRowMutationInput, DatabaseRowMutationResult, DatabaseSchema, DatabaseTable,
    DatabaseTableColumn, DatabaseTableStructure, DatabaseTableStructureInput, DatabaseTestResult,
    DbQueryHistoryEntry, DbQueryHistoryRecordInput, SavedSql, SavedSqlInput,
};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;
use uuid::Uuid;

#[derive(Clone)]
pub struct DatabaseService {
    db: LocalDb,
    secret_store: Option<SecretStore>,
}

#[derive(Debug, sqlx::FromRow)]
struct StoredDatabaseConnection {
    id: String,
    workspace_id: String,
    name: String,
    host: Option<String>,
    port: Option<i64>,
    driver: String,
    database_name: Option<String>,
    username: Option<String>,
    ssl_mode: Option<String>,
    read_only: bool,
    config_json: String,
    credential_ref: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    revision: i64,
    sync_status: String,
    remote_id: Option<String>,
}

#[derive(Debug)]
struct DatabaseConnectionStorageInput {
    driver: String,
    host: Option<String>,
    port: Option<u16>,
    database_name: Option<String>,
    username: Option<String>,
    ssl_mode: Option<String>,
    read_only: bool,
    config: DatabaseConnectionConfig,
}

impl DatabaseService {
    pub fn new(db: LocalDb) -> Self {
        Self {
            db,
            secret_store: None,
        }
    }

    pub fn with_secret_store(mut self, secret_store: SecretStore) -> Self {
        self.secret_store = Some(secret_store);
        self
    }

    pub async fn list_connections(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<DatabaseConnection>> {
        validate_workspace_id(&workspace_id)?;

        let rows = sqlx::query_as::<_, StoredDatabaseConnection>(
            r#"
            SELECT
              c.id, c.workspace_id, c.name, c.host, c.port,
              sub.driver, sub.database_name, sub.username, sub.ssl_mode,
              sub.read_only, sub.config_json, c.credential_ref,
              c.created_at, c.updated_at, c.deleted_at, c.revision, c.sync_status, c.remote_id
            FROM connections c
            INNER JOIN database_connections sub ON sub.connection_id = c.id
            WHERE c.workspace_id = ?1 AND c.connection_type = 'database' AND c.deleted_at IS NULL
            ORDER BY c.updated_at DESC
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
        let storage = input_to_storage(&input)?;
        let name = normalize_name(&input.name)?;
        let now = Utc::now().to_rfc3339();
        let config_json = database_config_to_json(&storage.config)?;
        let host = storage.host.clone();
        let port = storage.port.map(i64::from);
        let database_name = storage.database_name.clone();
        let username = storage.username.clone();
        let ssl_mode = storage.ssl_mode.clone();
        let credential_ref = empty_to_none(input.credential_ref);

        if let Some(id) = input
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            let result = sqlx::query(
                r#"
                UPDATE connections
                SET name = ?1, host = ?2, port = ?3, credential_ref = ?4,
                    updated_at = ?5, revision = revision + 1, sync_status = 'pending'
                WHERE id = ?6 AND workspace_id = ?7 AND connection_type = 'database' AND deleted_at IS NULL
                "#,
            )
            .bind(name)
            .bind(host)
            .bind(port)
            .bind(credential_ref)
            .bind(&now)
            .bind(id)
            .bind(&input.workspace_id)
            .execute(self.db.pool())
            .await?;

            if result.rows_affected() == 0 {
                return Err(AppError::NotFound("database connection".to_string()));
            }

            sqlx::query(
                r#"
                UPDATE database_connections
                SET driver = ?1, database_name = ?2, username = ?3,
                    ssl_mode = ?4, read_only = ?5, config_json = ?6
                WHERE connection_id = ?7
                "#,
            )
            .bind(&storage.driver)
            .bind(database_name)
            .bind(username)
            .bind(ssl_mode)
            .bind(storage.read_only)
            .bind(&config_json)
            .bind(id)
            .execute(self.db.pool())
            .await?;

            return self.get_connection(&input.workspace_id, id).await;
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO connections (
              id, workspace_id, connection_type, name, host, port, credential_ref,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, 'database', ?3, ?4, ?5, ?6, ?7, ?7, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(name)
        .bind(host)
        .bind(port)
        .bind(credential_ref)
        .bind(now)
        .execute(self.db.pool())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO database_connections (
              connection_id, driver, database_name, username, ssl_mode, read_only, config_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&id)
        .bind(&storage.driver)
        .bind(storage.database_name)
        .bind(storage.username)
        .bind(storage.ssl_mode)
        .bind(storage.read_only)
        .bind(&config_json)
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

        // Read the credential reference before soft-deleting so the stored
        // secret can be purged from the OS keychain.
        let existing = sqlx::query(
            "SELECT credential_ref FROM connections \
             WHERE id = ?1 AND workspace_id = ?2 \
               AND connection_type = 'database' AND deleted_at IS NULL",
        )
        .bind(&connection_id)
        .bind(&workspace_id)
        .fetch_optional(self.db.pool())
        .await?;
        let credential_ref: Option<String> = existing
            .and_then(|row| row.try_get::<Option<String>, _>("credential_ref").ok())
            .flatten();

        let mut tx = self.db.pool().begin().await?;

        let result = sqlx::query(
            r#"
            UPDATE connections
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1, sync_status = 'deleted'
            WHERE id = ?2 AND workspace_id = ?3
              AND connection_type = 'database' AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&connection_id)
        .bind(&workspace_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("database connection".to_string()));
        }

        sqlx::query(
            r#"
            UPDATE saved_sql
            SET connection_id = NULL, updated_at = ?1,
                revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?2 AND connection_id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&connection_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        // Best-effort purge of the stored secret from the OS keychain, only
        // after the soft-delete transaction has committed. The secret store is
        // optional (absent in some runtimes); only purge when it is configured.
        // A failure here (e.g. the credential was already removed) must not
        // surface as a delete error.
        if let Some(credential_ref) = credential_ref.filter(|value| !value.is_empty()) {
            if let Some(secret_store) = &self.secret_store {
                let _ = secret_store
                    .delete_credential(workspace_id.clone(), credential_ref)
                    .await;
            }
        }

        self.list_connections(workspace_id).await
    }

    pub async fn test_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<DatabaseTestResult> {
        let connection = self.get_connection(&workspace_id, &connection_id).await?;
        self.test_connection_inner(connection, None).await
    }

    /// Test connectivity for a connection that may not yet be saved. The
    /// transient `secret` (the password typed in the dialog) is used as an
    /// override; when empty, the stored keychain credential referenced by
    /// `credential_ref` is used instead. This lets the "test connection" action
    /// validate a brand-new connection before it is persisted.
    pub async fn test_connection_input(
        &self,
        input: DatabaseConnectionInput,
        secret: Option<String>,
    ) -> AppResult<DatabaseTestResult> {
        validate_workspace_id(&input.workspace_id)?;
        let storage = input_to_storage(&input)?;
        let connection = DatabaseConnection {
            id: input.id.clone().unwrap_or_default(),
            workspace_id: input.workspace_id.clone(),
            name: input.name.trim().to_string(),
            driver: storage.driver.clone(),
            host: storage.host.clone(),
            port: storage.port,
            database: storage.database_name.clone(),
            username: storage.username.clone(),
            ssl_mode: storage.ssl_mode.clone(),
            sqlite_path: storage.config.sqlite_path.clone(),
            credential_ref: empty_to_none(input.credential_ref.clone()),
            read_only: storage.read_only,
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
            revision: 0,
            sync_status: "new".to_string(),
            remote_id: None,
        };
        let password_override = secret.as_deref().filter(|value| !value.is_empty());
        self.test_connection_inner(connection, password_override).await
    }

    async fn test_connection_inner(
        &self,
        connection: DatabaseConnection,
        password_override: Option<&str>,
    ) -> AppResult<DatabaseTestResult> {
        let started = Instant::now();
        let fields = serde_json::json!({ "driver": &connection.driver });
        unfour_diag::log_operation_event(
            "database_connect_started",
            "database",
            "test_connection",
            "started",
            None,
            None,
            fields.clone(),
        );

        let result = match connection.driver.as_str() {
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
            "postgres" => {
                let pool = self.postgres_pool_with_secret(&connection, password_override).await?;
                let row: (String,) = sqlx::query_as("SELECT version()")
                    .fetch_one(&pool)
                    .await
                    .map_err(sanitize_pg_error)?;

                Ok(DatabaseTestResult {
                    ok: true,
                    message: "PostgreSQL connection OK".to_string(),
                    server_version: Some(row.0),
                })
            }
            "mysql" => {
                let pool = self.mysql_pool_with_secret(&connection, password_override).await?;
                let row: (String,) = sqlx::query_as("SELECT VERSION()")
                    .fetch_one(&pool)
                    .await
                    .map_err(sanitize_mysql_error)?;

                Ok(DatabaseTestResult {
                    ok: true,
                    message: "MySQL connection OK".to_string(),
                    server_version: Some(row.0),
                })
            }
            driver => Err(AppError::Unsupported(format!(
                "database driver is not supported: {}",
                driver
            ))),
        };

        match &result {
            Ok(_) => unfour_diag::log_operation_event(
                "database_connect_completed",
                "database",
                "test_connection",
                "ok",
                Some(started.elapsed().as_millis()),
                None,
                fields,
            ),
            Err(error) => unfour_diag::log_operation_event(
                "database_connect_failed",
                "database",
                "test_connection",
                "error",
                Some(started.elapsed().as_millis()),
                Some(unfour_diag::app_error_kind(error)),
                fields,
            ),
        }
        result
    }

    pub async fn schema(
        &self,
        workspace_id: String,
        connection_id: String,
        catalog: Option<String>,
    ) -> AppResult<DatabaseSchema> {
        let connection = self.get_connection(&workspace_id, &connection_id).await?;
        let catalog = clean_identifier(catalog.as_deref())?.map(str::to_string);

        match connection.driver.as_str() {
            "sqlite" => {
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
                        catalog: None,
                        schema: None,
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
            "postgres" => {
                // PostgreSQL cannot cross-database query, so to browse a catalog
                // other than the connection default we open a pool bound to that
                // database. Every object in the listing belongs to this catalog.
                let effective = Self::effective_connection(&connection, catalog.as_deref());
                let pool = self.postgres_pool(&effective).await?;
                let catalog = effective.database.clone();
                let table_rows = sqlx::query(
                    r#"
                    SELECT table_schema, table_name, table_type
                    FROM information_schema.tables
                    WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
                    ORDER BY table_schema, table_name
                    "#,
                )
                .fetch_all(&pool)
                .await
                .map_err(sanitize_pg_error)?;

                let mut tables = Vec::with_capacity(table_rows.len());
                for row in table_rows {
                    let schema: String = row.try_get("table_schema").map_err(sanitize_pg_error)?;
                    let name: String = row.try_get("table_name").map_err(sanitize_pg_error)?;
                    let table_type: String =
                        row.try_get("table_type").map_err(sanitize_pg_error)?;
                    let kind = if table_type == "VIEW" {
                        "view".to_string()
                    } else {
                        "table".to_string()
                    };
                    let columns = postgres_columns(&pool, &schema, &name)
                        .await
                        .map_err(sanitize_pg_app_error)?;
                    tables.push(postgres_table_from_metadata(
                        catalog.clone(),
                        schema,
                        name,
                        kind,
                        columns,
                    ));
                }

                Ok(DatabaseSchema {
                    connection_id,
                    tables,
                })
            }
            "mysql" => {
                let pool = self.mysql_pool(&connection).await?;
                // Scope to one database (catalog) when given; bound as a
                // parameter so an identifier with quotes cannot break the query.
                // When a specific catalog is requested we list its tables even if
                // it is a system schema (the user opened it explicitly). The
                // unscoped "load everything" path still skips the system schemas
                // so selecting a connection does not eagerly pull them all in.
                let mut sql = String::from(
                    "SELECT table_schema, table_name, table_type FROM information_schema.tables",
                );
                if catalog.is_some() {
                    sql.push_str(" WHERE table_schema = ?");
                } else {
                    sql.push_str(
                        " WHERE table_schema NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys')",
                    );
                }
                sql.push_str(" ORDER BY table_schema, table_name");
                let mut query = sqlx::query(&sql);
                if let Some(cat) = catalog.as_deref() {
                    query = query.bind(cat);
                }
                let table_rows = query.fetch_all(&pool).await.map_err(sanitize_mysql_error)?;

                let mut tables = Vec::with_capacity(table_rows.len());
                for row in table_rows {
                    // Read positionally (table_schema, table_name, table_type)
                    // and tolerate the binary charset MySQL reports for
                    // information_schema columns.
                    let schema: String = mysql_text(&row, 0).map_err(sanitize_mysql_app_error)?;
                    let name: String = mysql_text(&row, 1).map_err(sanitize_mysql_app_error)?;
                    let table_type: String =
                        mysql_text(&row, 2).map_err(sanitize_mysql_app_error)?;
                    let columns = mysql_columns(&pool, &schema, &name)
                        .await
                        .map_err(sanitize_mysql_app_error)?;
                    // In MySQL `table_schema` is the database itself, so it maps
                    // to the catalog level; MySQL has no nested schema.
                    tables.push(mysql_table_from_metadata(schema, name, table_type, columns));
                }

                Ok(DatabaseSchema {
                    connection_id,
                    tables,
                })
            }
            driver => Err(AppError::Unsupported(format!(
                "{} schema browsing is not yet supported",
                display_driver(driver)
            ))),
        }
    }

    /// List the catalogs (databases) the connection can see. SQLite returns an
    /// empty list because a connection is a single file. PostgreSQL and MySQL
    /// enumerate the server's databases so the tree can browse beyond the
    /// connection's default database, one catalog at a time.
    pub async fn list_catalogs(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<Vec<String>> {
        let connection = self.get_connection(&workspace_id, &connection_id).await?;

        match connection.driver.as_str() {
            "sqlite" => Ok(Vec::new()),
            "postgres" => {
                let pool = self.postgres_pool(&connection).await?;
                let rows = sqlx::query(
                    r#"
                    SELECT datname
                    FROM pg_database
                    WHERE datistemplate = false AND datallowconn = true
                    ORDER BY datname
                    "#,
                )
                .fetch_all(&pool)
                .await
                .map_err(sanitize_pg_error)?;
                rows.into_iter()
                    .map(|row| row.try_get::<String, _>("datname").map_err(AppError::from))
                    .collect()
            }
            "mysql" => {
                let pool = self.mysql_pool(&connection).await?;
                // List every schema, including the system databases
                // (information_schema, mysql, performance_schema, sys), so they
                // are browsable from the tree like any other database.
                let rows = sqlx::query(
                    r#"
                    SELECT schema_name
                    FROM information_schema.schemata
                    ORDER BY schema_name
                    "#,
                )
                .fetch_all(&pool)
                .await
                .map_err(sanitize_mysql_error)?;
                rows.iter()
                    // Read positionally and tolerate the binary charset MySQL
                    // reports for information_schema columns (a by-name
                    // "schema_name" lookup can also miss the uppercase column).
                    .map(|row| mysql_text(row, 0))
                    .collect()
            }
            driver => Err(AppError::Unsupported(format!(
                "{} catalog listing is not yet supported",
                display_driver(driver)
            ))),
        }
    }

    pub async fn execute_query(&self, input: DatabaseQueryInput) -> AppResult<DatabaseQueryResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let sql = input.sql.trim();
        if sql.is_empty() {
            return Err(AppError::Validation("SQL cannot be empty".to_string()));
        }
        validate_single_statement(sql)?;
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
                    let effective =
                        Self::effective_connection(&connection, input.catalog.as_deref());
                    let pool = self.postgres_pool(&effective).await?;
                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let started = Instant::now();

                    // Apply the query context on a dedicated connection so the
                    // search_path change and the statement share the same session.
                    let mut conn = pool.acquire().await.map_err(sanitize_pg_error)?;
                    if let Some(schema) = clean_identifier(input.schema.as_deref())? {
                        let stmt = format!("SET search_path TO {}", quote_identifier(schema));
                        sqlx::query(&stmt)
                            .execute(conn.as_mut())
                            .await
                            .map_err(sanitize_pg_error)?;
                    }

                    if returns_rows(sql) {
                        let query_sql = sql_with_limit(sql, limit);
                        let rows = sqlx::query(&query_sql)
                            .fetch_all(conn.as_mut())
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

                    let result = sqlx::query(sql)
                        .execute(conn.as_mut())
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
                    let effective =
                        Self::effective_connection(&connection, input.catalog.as_deref());
                    let pool = self.mysql_pool(&effective).await?;
                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let started = Instant::now();

                    // Apply the query context (active database) on a dedicated
                    // connection so the USE statement and the query share a session.
                    let mut conn = pool.acquire().await.map_err(sanitize_mysql_error)?;
                    if let Some(catalog) = clean_identifier(input.catalog.as_deref())? {
                        let stmt = format!("USE {}", quote_mysql_identifier(catalog));
                        sqlx::query(&stmt)
                            .execute(conn.as_mut())
                            .await
                            .map_err(sanitize_mysql_error)?;
                    }

                    if returns_rows(sql) {
                        let query_sql = sql_with_limit(sql, limit);
                        let rows = sqlx::query(&query_sql)
                            .fetch_all(conn.as_mut())
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

                    let result = sqlx::query(sql)
                        .execute(conn.as_mut())
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

        let id = Uuid::new_v4().to_string();
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

        let filter = normalize_filter(input.filter.as_deref());
        let order_by = input
            .order_by
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let descending = input.order_descending;
        let needs_columns = filter.is_some() || order_by.is_some();
        let timeout = resolve_timeout(input.timeout_ms);

        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;

        let run = async {
            match connection.driver.as_str() {
                "sqlite" => {
                    let pool = sqlite_pool(&connection).await?;
                    ensure_sqlite_table_exists(&pool, table_name).await?;

                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let offset = input.offset.unwrap_or(0);

                    let column_names = if needs_columns {
                        sqlite_columns(&pool, table_name)
                            .await?
                            .into_iter()
                            .map(|column| column.name)
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    let order_sql =
                        order_by_clause(order_by, descending, &column_names, quote_identifier)?;
                    let active_filter = filter.as_ref().filter(|_| !column_names.is_empty());
                    let where_sql = active_filter
                        .map(|_| format!(" WHERE {}", sqlite_filter_where(&column_names)))
                        .unwrap_or_default();
                    let quoted = quote_identifier(table_name);

                    let total_rows = if let Some(needle) = active_filter {
                        let count_sql =
                            format!("SELECT COUNT(*) AS total_rows FROM {}{}", quoted, where_sql);
                        let mut count = sqlx::query(&count_sql);
                        for _ in &column_names {
                            count = count.bind(format!("%{}%", needle));
                        }
                        let row = count.fetch_one(&pool).await?;
                        row.try_get::<i64, _>("total_rows")?.max(0) as u64
                    } else {
                        sqlite_table_row_count(&pool, table_name).await?
                    };

                    let sql = format!(
                        "SELECT * FROM {}{}{} LIMIT {} OFFSET {}",
                        quoted, where_sql, order_sql, limit, offset
                    );
                    let started = Instant::now();
                    let mut query = sqlx::query(&sql);
                    if let Some(needle) = active_filter {
                        for _ in &column_names {
                            query = query.bind(format!("%{}%", needle));
                        }
                    }
                    let rows = query.fetch_all(&pool).await?;
                    let columns = if let Some(row) = rows.first() {
                        sqlite_result_columns(row)
                    } else {
                        sqlite_table_result_columns(&pool, table_name).await?
                    };
                    let values = rows
                        .iter()
                        .map(sqlite_row_values)
                        .collect::<AppResult<Vec<_>>>()?;

                    Ok(browse_result(
                        table_name, sql, limit, offset, total_rows, columns, values, started,
                    ))
                }
                "postgres" => {
                    let effective =
                        Self::effective_connection(&connection, input.catalog.as_deref());
                    let pool = self.postgres_pool(&effective).await?;
                    let schema = input
                        .schema
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or("public");
                    ensure_postgres_table_exists(&pool, schema, table_name)
                        .await
                        .map_err(sanitize_pg_app_error)?;

                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let offset = input.offset.unwrap_or(0);

                    let column_names = if needs_columns {
                        postgres_columns(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_pg_app_error)?
                            .into_iter()
                            .map(|column| column.name)
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    let order_sql =
                        order_by_clause(order_by, descending, &column_names, quote_identifier)?;
                    let active_filter = filter.as_ref().filter(|_| !column_names.is_empty());
                    let where_sql = active_filter
                        .map(|_| format!(" WHERE {}", postgres_filter_where(&column_names)))
                        .unwrap_or_default();

                    let total_rows = if let Some(needle) = active_filter {
                        // The same $1 bind is reused by every column predicate.
                        let count_sql = format!(
                            "SELECT COUNT(*) AS total_rows FROM {}{}",
                            quote_qualified_identifier(schema, table_name),
                            where_sql
                        );
                        let row = sqlx::query(&count_sql)
                            .bind(format!("%{}%", needle))
                            .fetch_one(&pool)
                            .await
                            .map_err(sanitize_pg_error)?;
                        row.try_get::<i64, _>("total_rows")
                            .map_err(sanitize_pg_error)?
                            .max(0) as u64
                    } else {
                        postgres_table_row_count(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_pg_app_error)?
                    };

                    let sql = postgres_browse_sql(
                        schema, table_name, &where_sql, &order_sql, limit, offset,
                    );
                    let started = Instant::now();
                    let mut query = sqlx::query(&sql);
                    if let Some(needle) = active_filter {
                        query = query.bind(format!("%{}%", needle));
                    }
                    let rows = query.fetch_all(&pool).await.map_err(sanitize_pg_error)?;
                    let columns = if let Some(row) = rows.first() {
                        postgres_result_columns(row)
                    } else {
                        postgres_table_result_columns(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_pg_app_error)?
                    };
                    let values = rows
                        .iter()
                        .map(postgres_row_values)
                        .collect::<AppResult<Vec<_>>>()?;

                    Ok(browse_result(
                        table_name, sql, limit, offset, total_rows, columns, values, started,
                    ))
                }
                "mysql" => {
                    let effective =
                        Self::effective_connection(&connection, input.catalog.as_deref());
                    let pool = self.mysql_pool(&effective).await?;
                    // MySQL addresses tables as `database`.`table`; the catalog is
                    // that database. Prefer the explicit catalog, then the legacy
                    // schema field, then the connection's default database.
                    let schema = input
                        .catalog
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .or_else(|| {
                            input
                                .schema
                                .as_deref()
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                        })
                        .or(connection.database.as_deref())
                        .ok_or_else(|| {
                            AppError::Validation("MySQL database name is required".to_string())
                        })?;
                    ensure_mysql_table_exists(&pool, schema, table_name)
                        .await
                        .map_err(sanitize_mysql_app_error)?;

                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let offset = input.offset.unwrap_or(0);

                    let column_names = if needs_columns {
                        mysql_columns(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_mysql_app_error)?
                            .into_iter()
                            .map(|column| column.name)
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    let order_sql = order_by_clause(
                        order_by,
                        descending,
                        &column_names,
                        quote_mysql_identifier,
                    )?;
                    let active_filter = filter.as_ref().filter(|_| !column_names.is_empty());
                    let where_sql = active_filter
                        .map(|_| format!(" WHERE {}", mysql_filter_where(&column_names)))
                        .unwrap_or_default();

                    let total_rows = if let Some(needle) = active_filter {
                        let count_sql = format!(
                            "SELECT COUNT(*) AS total_rows FROM {}{}",
                            quote_mysql_qualified_identifier(schema, table_name),
                            where_sql
                        );
                        let mut count = sqlx::query(&count_sql);
                        for _ in &column_names {
                            count = count.bind(format!("%{}%", needle));
                        }
                        let row = count.fetch_one(&pool).await.map_err(sanitize_mysql_error)?;
                        row.try_get::<i64, _>("total_rows")
                            .map_err(sanitize_mysql_error)?
                            .max(0) as u64
                    } else {
                        mysql_table_row_count(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_mysql_app_error)?
                    };

                    let sql =
                        mysql_browse_sql(schema, table_name, &where_sql, &order_sql, limit, offset);
                    let started = Instant::now();
                    let mut query = sqlx::query(&sql);
                    if let Some(needle) = active_filter {
                        for _ in &column_names {
                            query = query.bind(format!("%{}%", needle));
                        }
                    }
                    let rows = query.fetch_all(&pool).await.map_err(sanitize_mysql_error)?;
                    let columns = if let Some(row) = rows.first() {
                        mysql_result_columns(row)
                    } else {
                        mysql_table_result_columns(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_mysql_app_error)?
                    };
                    let values = rows
                        .iter()
                        .map(mysql_row_values)
                        .collect::<AppResult<Vec<_>>>()?;

                    Ok(browse_result(
                        table_name, sql, limit, offset, total_rows, columns, values, started,
                    ))
                }
                driver => Err(AppError::Unsupported(format!(
                    "{} table browsing is not yet supported",
                    display_driver(driver)
                ))),
            }
        };
        match tokio::time::timeout(timeout, run).await {
            Ok(result) => result,
            Err(_) => Err(AppError::Timeout(format!(
                "table browse exceeded the {} ms timeout",
                timeout.as_millis()
            ))),
        }
    }

    /// Load the full structure (columns, indexes, foreign keys, DDL) for a
    /// single table on demand. Kept separate from `schema` so browsing the
    /// connection tree stays lightweight.
    pub async fn table_structure(
        &self,
        input: DatabaseTableStructureInput,
    ) -> AppResult<DatabaseTableStructure> {
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

        match connection.driver.as_str() {
            "sqlite" => {
                let pool = sqlite_pool(&connection).await?;
                ensure_sqlite_table_exists(&pool, table_name).await?;
                let columns = sqlite_columns(&pool, table_name).await?;
                let indexes = sqlite_indexes(&pool, table_name).await?;
                let foreign_keys = sqlite_foreign_keys(&pool, table_name).await?;
                let kind = sqlite_table_kind(&pool, table_name).await?;
                let ddl = sqlite_ddl(&pool, table_name).await?;
                Ok(DatabaseTableStructure {
                    catalog: None,
                    schema: None,
                    name: table_name.to_string(),
                    kind,
                    columns,
                    indexes,
                    foreign_keys,
                    ddl,
                })
            }
            "postgres" => {
                let effective = Self::effective_connection(&connection, input.catalog.as_deref());
                let pool = self.postgres_pool(&effective).await?;
                let schema = input
                    .schema
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("public");
                ensure_postgres_table_exists(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                let columns = postgres_columns(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                let indexes = postgres_indexes(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                let foreign_keys = postgres_foreign_keys(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                // Resolve the actual object kind (table vs view) from
                // information_schema so views report as "view" instead of the
                // hard-coded "table" the previous implementation returned.
                let kind = postgres_table_kind(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                Ok(DatabaseTableStructure {
                    catalog: connection.database.clone(),
                    schema: Some(schema.to_string()),
                    name: table_name.to_string(),
                    kind,
                    columns,
                    indexes,
                    foreign_keys,
                    ddl: None,
                })
            }
            "mysql" => {
                let effective = Self::effective_connection(&connection, input.catalog.as_deref());
                let pool = self.mysql_pool(&effective).await?;
                // MySQL addresses tables as `database`.`table`; the catalog is
                // that database. Prefer the explicit catalog, then the legacy
                // schema field, then the connection's default database.
                let schema = input
                    .catalog
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .or_else(|| {
                        input
                            .schema
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                    })
                    .or(connection.database.as_deref())
                    .ok_or_else(|| {
                        AppError::Validation("MySQL database name is required".to_string())
                    })?;
                ensure_mysql_table_exists(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                let columns = mysql_columns(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                let indexes = mysql_indexes(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                let foreign_keys = mysql_foreign_keys(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                let ddl = mysql_ddl(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                // Resolve the actual object kind (table vs view) from
                // information_schema so views report as "view" instead of the
                // hard-coded "table" the previous implementation returned.
                let kind = mysql_table_kind(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                Ok(DatabaseTableStructure {
                    catalog: Some(schema.to_string()),
                    schema: None,
                    name: table_name.to_string(),
                    kind,
                    columns,
                    indexes,
                    foreign_keys,
                    ddl,
                })
            }
            driver => Err(AppError::Unsupported(format!(
                "{} table structure is not yet supported",
                display_driver(driver)
            ))),
        }
    }

    /// Insert, update, or delete a single table row. Update and delete require
    /// a non-empty primary key so a malformed request can never rewrite a whole
    /// table. Values are emitted as escaped, coercible string literals.
    pub async fn mutate_table_row(
        &self,
        input: DatabaseRowMutationInput,
    ) -> AppResult<DatabaseRowMutationResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let table_name = input.table_name.trim();
        if table_name.is_empty() {
            return Err(AppError::Validation(
                "table name cannot be empty".to_string(),
            ));
        }
        let operation = input.operation.trim().to_ascii_lowercase();

        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;

        if connection.read_only {
            return Err(AppError::ReadOnly(
                "this connection is read-only; row edits are not allowed".to_string(),
            ));
        }

        match connection.driver.as_str() {
            "sqlite" => {
                let pool = sqlite_pool(&connection).await?;
                ensure_sqlite_table_exists(&pool, table_name).await?;
                let sql = build_row_mutation_sql(
                    SqlDialect::Standard,
                    None,
                    table_name,
                    &operation,
                    &input.values,
                    &input.primary_key,
                )?;
                let result = sqlx::query(&sql).execute(&pool).await?;
                Ok(DatabaseRowMutationResult {
                    affected_rows: result.rows_affected(),
                    sql,
                })
            }
            "postgres" => {
                let effective = Self::effective_connection(&connection, input.catalog.as_deref());
                let pool = self.postgres_pool(&effective).await?;
                let schema = input
                    .schema
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("public");
                ensure_postgres_table_exists(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                let sql = build_row_mutation_sql(
                    SqlDialect::Standard,
                    Some(schema),
                    table_name,
                    &operation,
                    &input.values,
                    &input.primary_key,
                )?;
                let result = sqlx::query(&sql)
                    .execute(&pool)
                    .await
                    .map_err(sanitize_pg_error)?;
                Ok(DatabaseRowMutationResult {
                    affected_rows: result.rows_affected(),
                    sql,
                })
            }
            "mysql" => {
                let effective = Self::effective_connection(&connection, input.catalog.as_deref());
                let pool = self.mysql_pool(&effective).await?;
                // MySQL addresses tables as `database`.`table`; the catalog is
                // that database. Prefer the explicit catalog, then the legacy
                // schema field, then the connection's default database.
                let schema = input
                    .catalog
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .or_else(|| {
                        input
                            .schema
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                    })
                    .or(connection.database.as_deref())
                    .ok_or_else(|| {
                        AppError::Validation("MySQL database name is required".to_string())
                    })?;
                ensure_mysql_table_exists(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                let sql = build_row_mutation_sql(
                    SqlDialect::MySql,
                    Some(schema),
                    table_name,
                    &operation,
                    &input.values,
                    &input.primary_key,
                )?;
                let result = sqlx::query(&sql)
                    .execute(&pool)
                    .await
                    .map_err(sanitize_mysql_error)?;
                Ok(DatabaseRowMutationResult {
                    affected_rows: result.rows_affected(),
                    sql,
                })
            }
            driver => Err(AppError::Unsupported(format!(
                "{} row editing is not yet supported",
                display_driver(driver)
            ))),
        }
    }

    pub fn capability_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "status": "mvp",
            "backend": "sqlx",
            "activeDrivers": ["sqlite", "postgres", "mysql"],
            "reservedDrivers": [],
            "features": [
                "connection-metadata-crud",
                "sqlite-connection-test",
                "sqlite-schema-browser",
                "sqlite-sql-editor",
                "sqlite-read-only-table-data",
                "postgres-connection-test",
                "postgres-schema-browser",
                "postgres-sql-editor",
                "postgres-read-only-table-data",
                "mysql-connection-test",
                "mysql-schema-browser",
                "mysql-sql-editor",
                "mysql-read-only-table-data",
                "paged-query-results",
                "credential-backed-auth",
                "on-demand-table-structure"
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

        let row = sqlx::query_as::<_, StoredDatabaseConnection>(
            r#"
            SELECT
              c.id, c.workspace_id, c.name, c.host, c.port,
              sub.driver, sub.database_name, sub.username, sub.ssl_mode,
              sub.read_only, sub.config_json, c.credential_ref,
              c.created_at, c.updated_at, c.deleted_at, c.revision, c.sync_status, c.remote_id
            FROM connections c
            INNER JOIN database_connections sub ON sub.connection_id = c.id
            WHERE c.id = ?1 AND c.workspace_id = ?2
              AND c.connection_type = 'database' AND c.deleted_at IS NULL
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

    /// Create a PostgreSQL connection pool, loading the password from SecretStore
    /// if a credential reference is present on the connection.
    async fn postgres_pool(&self, connection: &DatabaseConnection) -> AppResult<sqlx::PgPool> {
        self.postgres_pool_with_secret(connection, None).await
    }

    /// PostgreSQL pool that prefers an inline `password_override` (the
    /// not-yet-saved secret from the "test connection" dialog) over the stored
    /// keychain credential. Falls back to the saved credential when no override
    /// is supplied, preserving existing behavior for saved connections.
    async fn postgres_pool_with_secret(
        &self,
        connection: &DatabaseConnection,
        password_override: Option<&str>,
    ) -> AppResult<sqlx::PgPool> {
        let options = pg_connect_options(connection, self.secret_store.as_ref(), password_override).await?;
        PgPoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(|e| sanitize_pg_error(e))
    }

    /// Create a MySQL connection pool, loading the password from SecretStore
    /// if a credential reference is present on the connection.
    async fn mysql_pool(&self, connection: &DatabaseConnection) -> AppResult<sqlx::MySqlPool> {
        self.mysql_pool_with_secret(connection, None).await
    }

    /// MySQL pool that prefers an inline `password_override` (the not-yet-saved
    /// secret from the "test connection" dialog) over the stored keychain
    /// credential. Falls back to the saved credential when no override is
    /// supplied, preserving existing behavior for saved connections.
    async fn mysql_pool_with_secret(
        &self,
        connection: &DatabaseConnection,
        password_override: Option<&str>,
    ) -> AppResult<sqlx::MySqlPool> {
        let options = mysql_connect_options(connection, self.secret_store.as_ref(), password_override).await?;
        MySqlPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await
            .map_err(sanitize_mysql_error)
    }

    /// Return a connection clone with `database` overridden to the given
    /// catalog when the catalog differs from the connection's current database.
    /// This is required for PostgreSQL (and MySQL) because they cannot
    /// cross-database query; the pool must target the database that owns the
    /// table being browsed, inspected, or mutated.
    fn effective_connection(
        connection: &DatabaseConnection,
        catalog: Option<&str>,
    ) -> DatabaseConnection {
        match catalog {
            Some(name) if connection.database.as_deref() != Some(name) => {
                let mut overridden = connection.clone();
                overridden.database = Some(name.to_string());
                overridden
            }
            _ => connection.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// PostgreSQL helpers
// ---------------------------------------------------------------------------

async fn pg_connect_options(
    connection: &DatabaseConnection,
    secret_store: Option<&SecretStore>,
    password_override: Option<&str>,
) -> AppResult<PgConnectOptions> {
    let host = connection
        .host
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("127.0.0.1");
    let port = connection.port.unwrap_or(5432);
    let database = connection
        .database
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Validation("PostgreSQL database name is required".to_string()))?;
    let username = connection
        .username
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Validation("PostgreSQL username is required".to_string()))?;

    let password = match password_override {
        Some(secret) => Some(secret.to_string()),
        None => resolve_database_password(connection, secret_store).await?,
    };

    let mut options = PgConnectOptions::new()
        .host(host)
        .port(port as u16)
        .database(database)
        .username(username);

    if let Some(pw) = password {
        options = options.password(&pw);
    }

    Ok(options)
}

/// Load a database password from SecretStore if a credential reference is
/// present. Returns `None` when no credential_ref is configured.
async fn resolve_database_password(
    connection: &DatabaseConnection,
    secret_store: Option<&SecretStore>,
) -> AppResult<Option<String>> {
    if let Some(credential_ref) = connection
        .credential_ref
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let store = secret_store.ok_or_else(|| {
            AppError::Config(
                "SecretStore is not available; cannot load database password".to_string(),
            )
        })?;
        let secret = store
            .read_secret(connection.workspace_id.clone(), credential_ref.to_string())
            .await
            .map_err(|_| {
                AppError::Config("Failed to load database password from SecretStore".to_string())
            })?;
        Ok(Some(secret))
    } else {
        Ok(None)
    }
}

async fn postgres_columns(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseTableColumn>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT column_name, data_type, is_nullable, column_default
        FROM information_schema.columns
        WHERE table_schema = $1 AND table_name = $2
        ORDER BY ordinal_position
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let name: String = row.try_get("column_name")?;
            let data_type: String = row.try_get("data_type")?;
            let is_nullable: String = row.try_get("is_nullable")?;
            let column_default: Option<String> = row.try_get("column_default")?;

            // Detect primary key from column_default (serial types get nextval)
            // For a more accurate check we'd need pg_constraint, but this is
            // sufficient for the initial phase.
            let primary_key = column_default
                .as_deref()
                .map(|d| d.starts_with("nextval("))
                .unwrap_or(false);

            Ok(DatabaseTableColumn {
                name,
                data_type,
                nullable: is_nullable == "YES",
                primary_key,
                default_value: column_default,
            })
        })
        .collect()
}

async fn postgres_indexes(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseIndex>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT i.relname AS index_name,
               ix.indisunique AS is_unique,
               ix.indisprimary AS is_primary,
               a.attname AS column_name,
               array_position(ix.indkey::int2[], a.attnum) AS ord
        FROM pg_class t
        JOIN pg_namespace n ON n.oid = t.relnamespace
        JOIN pg_index ix ON ix.indrelid = t.oid
        JOIN pg_class i ON i.oid = ix.indexrelid
        JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey::int2[])
        WHERE n.nspname = $1 AND t.relname = $2
        ORDER BY index_name, ord
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut indexes: Vec<DatabaseIndex> = Vec::new();
    for row in rows {
        let name: String = row.try_get("index_name")?;
        let unique: bool = row.try_get("is_unique")?;
        let primary: bool = row.try_get("is_primary")?;
        let column_name: String = row.try_get("column_name")?;

        if let Some(existing) = indexes.iter_mut().find(|idx| idx.name == name) {
            existing.columns.push(column_name);
        } else {
            indexes.push(DatabaseIndex {
                name,
                columns: vec![column_name],
                unique,
                primary,
            });
        }
    }

    Ok(indexes)
}

async fn postgres_foreign_keys(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseForeignKey>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT con.conname AS name,
               att.attname AS column_name,
               cl.relname AS referenced_table,
               fatt.attname AS referenced_column,
               k.ord AS ord
        FROM pg_constraint con
        JOIN pg_class c ON c.oid = con.conrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN LATERAL unnest(con.conkey) WITH ORDINALITY AS k(attnum, ord) ON true
        JOIN pg_attribute att ON att.attrelid = con.conrelid AND att.attnum = k.attnum
        JOIN pg_class cl ON cl.oid = con.confrelid
        JOIN LATERAL unnest(con.confkey) WITH ORDINALITY AS fk(attnum, ford) ON fk.ford = k.ord
        JOIN pg_attribute fatt ON fatt.attrelid = con.confrelid AND fatt.attnum = fk.attnum
        WHERE con.contype = 'f' AND n.nspname = $1 AND c.relname = $2
        ORDER BY name, ord
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut keys: Vec<DatabaseForeignKey> = Vec::new();
    for row in rows {
        let name: String = row.try_get("name")?;
        let column_name: String = row.try_get("column_name")?;
        let referenced_table: String = row.try_get("referenced_table")?;
        let referenced_column: String = row.try_get("referenced_column")?;

        if let Some(existing) = keys.iter_mut().find(|fk| fk.name == name) {
            existing.columns.push(column_name);
            existing.referenced_columns.push(referenced_column);
        } else {
            keys.push(DatabaseForeignKey {
                name,
                columns: vec![column_name],
                referenced_table,
                referenced_columns: vec![referenced_column],
            });
        }
    }

    Ok(keys)
}

async fn ensure_postgres_table_exists(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<(), AppError> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = $1
          AND table_name = $2
          AND table_schema NOT IN ('pg_catalog', 'information_schema')
        LIMIT 1
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_optional(pool)
    .await?;

    row.map(|_| ())
        .ok_or_else(|| AppError::NotFound(format!("{schema}.{table_name}")))
}

/// Resolve the object kind ("table" or "view") from information_schema so the
/// structure panel reports views correctly instead of always returning
/// "table". Falls back to "table" when the row is missing (the caller has
/// already verified existence via `ensure_postgres_table_exists`).
async fn postgres_table_kind(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<String, AppError> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT table_type
        FROM information_schema.tables
        WHERE table_schema = $1 AND table_name = $2
        LIMIT 1
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|(table_type,)| {
            if table_type == "VIEW" {
                "view".to_string()
            } else {
                "table".to_string()
            }
        })
        .unwrap_or_else(|| "table".to_string()))
}

async fn postgres_table_row_count(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<u64, AppError> {
    let sql = format!(
        "SELECT COUNT(*) AS total_rows FROM {}",
        quote_qualified_identifier(schema, table_name)
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let total_rows: i64 = row.try_get("total_rows")?;
    Ok(total_rows.max(0) as u64)
}

async fn postgres_table_result_columns(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseResultColumn>, AppError> {
    Ok(postgres_columns(pool, schema, table_name)
        .await?
        .into_iter()
        .map(|column| DatabaseResultColumn {
            name: column.name,
            data_type: column.data_type,
        })
        .collect())
}

fn postgres_result_columns(row: &sqlx::postgres::PgRow) -> Vec<DatabaseResultColumn> {
    row.columns()
        .iter()
        .map(|column| DatabaseResultColumn {
            name: column.name().to_string(),
            data_type: column.type_info().name().to_string(),
        })
        .collect()
}

fn postgres_table_from_metadata(
    catalog: Option<String>,
    schema: String,
    name: String,
    kind: String,
    columns: Vec<DatabaseTableColumn>,
) -> DatabaseTable {
    DatabaseTable {
        catalog,
        schema: Some(schema),
        name,
        kind,
        columns,
    }
}

fn postgres_row_values(row: &sqlx::postgres::PgRow) -> AppResult<Vec<Option<String>>> {
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
            if let Ok(value) = row.try_get::<i32, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<i16, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<f64, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<f32, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<bool, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
                return Ok(Some(format!("<binary {} bytes>", value.len())));
            }
            if let Ok(value) = row.try_get::<serde_json::Value, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<uuid::Uuid, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(index) {
                return Ok(Some(value.to_rfc3339()));
            }
            if let Ok(value) = row.try_get::<chrono::DateTime<chrono::Local>, _>(index) {
                return Ok(Some(value.to_rfc3339()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveDateTime, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveDate, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveTime, _>(index) {
                return Ok(Some(value.to_string()));
            }

            Ok(Some("<unsupported>".to_string()))
        })
        .collect()
}

/// Sanitize a sqlx::Error into an AppError, stripping password/connection leaks.
fn sanitize_pg_error(error: sqlx::Error) -> AppError {
    let msg = error.to_string();
    if msg.contains("password") || msg.contains("userinfo") {
        AppError::Database(sqlx::Error::Protocol(
            "PostgreSQL connection error (details redacted)".to_string(),
        ))
    } else {
        AppError::Database(error)
    }
}

/// Sanitize an AppError from a helper that already wraps sqlx errors.
fn sanitize_pg_app_error(error: AppError) -> AppError {
    match &error {
        AppError::Database(sqlx_err) => {
            let msg = sqlx_err.to_string();
            if msg.contains("password") || msg.contains("userinfo") {
                AppError::Database(sqlx::Error::Protocol(
                    "PostgreSQL connection error (details redacted)".to_string(),
                ))
            } else {
                error
            }
        }
        _ => error,
    }
}

// ---------------------------------------------------------------------------
// MySQL helpers
// ---------------------------------------------------------------------------

async fn mysql_connect_options(
    connection: &DatabaseConnection,
    secret_store: Option<&SecretStore>,
    password_override: Option<&str>,
) -> AppResult<MySqlConnectOptions> {
    let host = connection
        .host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("127.0.0.1");
    let port = connection.port.unwrap_or(3306);
    // The default database is optional: a server-level connection can browse
    // every database it can see and pick one as the active query context later.
    let database = connection
        .database
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let username = connection
        .username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("MySQL username is required".to_string()))?;
    let password = match password_override {
        Some(secret) => Some(secret.to_string()),
        None => resolve_database_password(connection, secret_store).await?,
    };

    let mut options = MySqlConnectOptions::new()
        .host(host)
        .port(port)
        .username(username);
    if let Some(database) = database {
        options = options.database(database);
    }
    if let Some(password) = password {
        options = options.password(&password);
    }
    Ok(options)
}

/// Read a MySQL text column positionally, tolerating the binary character set
/// that MySQL/MariaDB often report for `information_schema` columns: sqlx
/// refuses to decode those as `String`, so fall back to raw bytes.
fn mysql_text(row: &sqlx::mysql::MySqlRow, index: usize) -> Result<String, AppError> {
    match row.try_get::<String, _>(index) {
        Ok(value) => Ok(value),
        Err(_) => {
            let bytes: Vec<u8> = row.try_get(index)?;
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        }
    }
}

/// Nullable counterpart to [`mysql_text`] for columns such as `column_default`.
fn mysql_text_opt(row: &sqlx::mysql::MySqlRow, index: usize) -> Result<Option<String>, AppError> {
    match row.try_get::<Option<String>, _>(index) {
        Ok(value) => Ok(value),
        Err(_) => {
            let bytes: Option<Vec<u8>> = row.try_get(index)?;
            Ok(bytes.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
        }
    }
}

async fn mysql_columns(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseTableColumn>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT column_name, column_type, is_nullable, column_key, column_default
        FROM information_schema.columns
        WHERE table_schema = ? AND table_name = ?
        ORDER BY ordinal_position
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    rows.iter()
        .map(|row| {
            // Read positionally (column_name, column_type, is_nullable,
            // column_key, column_default) and tolerate the binary charset MySQL
            // reports for information_schema columns.
            let name: String = mysql_text(row, 0)?;
            let data_type: String = mysql_text(row, 1)?;
            let is_nullable: String = mysql_text(row, 2)?;
            let column_key: String = mysql_text(row, 3)?;
            let default_value: Option<String> = mysql_text_opt(row, 4)?;
            Ok(DatabaseTableColumn {
                name,
                data_type,
                nullable: is_nullable == "YES",
                primary_key: column_key == "PRI",
                default_value,
            })
        })
        .collect()
}

async fn mysql_indexes(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseIndex>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT index_name, non_unique, seq_in_index, column_name
        FROM information_schema.statistics
        WHERE table_schema = ? AND table_name = ?
        ORDER BY index_name, seq_in_index
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut indexes: Vec<DatabaseIndex> = Vec::new();
    for row in &rows {
        // Read positionally (index_name, non_unique, seq_in_index, column_name)
        // and tolerate the binary charset MySQL reports for information_schema
        // text columns. NON_UNIQUE widened to BIGINT in MySQL 8, so accept i32.
        let name: String = mysql_text(row, 0)?;
        let non_unique: i64 = row
            .try_get::<i64, _>(1)
            .or_else(|_| row.try_get::<i32, _>(1).map(i64::from))?;
        let column_name: String = mysql_text(row, 3)?;

        if let Some(existing) = indexes.iter_mut().find(|idx| idx.name == name) {
            existing.columns.push(column_name);
        } else {
            indexes.push(DatabaseIndex {
                primary: name == "PRIMARY",
                unique: non_unique == 0,
                name,
                columns: vec![column_name],
            });
        }
    }

    Ok(indexes)
}

async fn mysql_foreign_keys(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseForeignKey>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT constraint_name, column_name, referenced_table_name, referenced_column_name
        FROM information_schema.key_column_usage
        WHERE table_schema = ? AND table_name = ? AND referenced_table_name IS NOT NULL
        ORDER BY constraint_name, ordinal_position
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut keys: Vec<DatabaseForeignKey> = Vec::new();
    for row in &rows {
        // Read positionally (constraint_name, column_name,
        // referenced_table_name, referenced_column_name) and tolerate the binary
        // charset MySQL reports for information_schema columns.
        let name: String = mysql_text(row, 0)?;
        let column_name: String = mysql_text(row, 1)?;
        let referenced_table: String = mysql_text(row, 2)?;
        let referenced_column: String = mysql_text(row, 3)?;

        if let Some(existing) = keys.iter_mut().find(|fk| fk.name == name) {
            existing.columns.push(column_name);
            existing.referenced_columns.push(referenced_column);
        } else {
            keys.push(DatabaseForeignKey {
                name,
                columns: vec![column_name],
                referenced_table,
                referenced_columns: vec![referenced_column],
            });
        }
    }

    Ok(keys)
}

async fn mysql_ddl(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<Option<String>, AppError> {
    let sql = format!(
        "SHOW CREATE TABLE {}",
        quote_mysql_qualified_identifier(schema, table_name)
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    // SHOW CREATE TABLE returns (Table, Create Table); read the DDL positionally
    // so the result is not tied to the server's column-name casing.
    Ok(row.try_get::<String, _>(1).ok())
}

async fn ensure_mysql_table_exists(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<(), AppError> {
    // Select a literal rather than table_name: MySQL returns information_schema
    // text columns as VARBINARY, which cannot decode into a Rust String. We only
    // need to know whether the row exists.
    let row: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT 1
        FROM information_schema.tables
        WHERE table_schema = ? AND table_name = ?
        LIMIT 1
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_optional(pool)
    .await?;

    row.map(|_| ())
        .ok_or_else(|| AppError::NotFound(format!("{schema}.{table_name}")))
}

/// Resolve the object kind ("table" or "view") from information_schema so the
/// structure panel reports views correctly instead of always returning
/// "table". Reads the `table_type` column positionally and tolerates the
/// binary charset MySQL reports for information_schema text columns. Falls
/// back to "table" when the row is missing (the caller has already verified
/// existence via `ensure_mysql_table_exists`).
async fn mysql_table_kind(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<String, AppError> {
    let row = sqlx::query(
        r#"
        SELECT table_type
        FROM information_schema.tables
        WHERE table_schema = ? AND table_name = ?
        LIMIT 1
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|row| {
            let table_type = mysql_text(&row, 0).unwrap_or_default();
            if table_type.eq_ignore_ascii_case("VIEW") {
                "view".to_string()
            } else {
                "table".to_string()
            }
        })
        .unwrap_or_else(|| "table".to_string()))
}

async fn mysql_table_row_count(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<u64, AppError> {
    let sql = format!(
        "SELECT COUNT(*) AS total_rows FROM {}",
        quote_mysql_qualified_identifier(schema, table_name)
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let total_rows: i64 = row.try_get("total_rows")?;
    Ok(total_rows.max(0) as u64)
}

async fn mysql_table_result_columns(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseResultColumn>, AppError> {
    Ok(mysql_columns(pool, schema, table_name)
        .await?
        .into_iter()
        .map(|column| DatabaseResultColumn {
            name: column.name,
            data_type: column.data_type,
        })
        .collect())
}

fn mysql_result_columns(row: &sqlx::mysql::MySqlRow) -> Vec<DatabaseResultColumn> {
    row.columns()
        .iter()
        .map(|column| DatabaseResultColumn {
            name: column.name().to_string(),
            data_type: column.type_info().name().to_string(),
        })
        .collect()
}

fn mysql_table_from_metadata(
    catalog: String,
    name: String,
    table_type: String,
    columns: Vec<DatabaseTableColumn>,
) -> DatabaseTable {
    DatabaseTable {
        catalog: Some(catalog),
        schema: None,
        name,
        kind: if table_type == "VIEW" {
            "view".to_string()
        } else {
            "table".to_string()
        },
        columns,
    }
}

fn mysql_row_values(row: &sqlx::mysql::MySqlRow) -> AppResult<Vec<Option<String>>> {
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
            if let Ok(value) = row.try_get::<u64, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<i32, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<u32, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<f64, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<f32, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<bool, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveDateTime, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveDate, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveTime, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
                return Ok(Some(format!("<binary {} bytes>", value.len())));
            }
            if let Ok(value) = row.try_get::<serde_json::Value, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<uuid::Uuid, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(index) {
                return Ok(Some(value.to_rfc3339()));
            }
            if let Ok(value) = row.try_get::<chrono::DateTime<chrono::Local>, _>(index) {
                return Ok(Some(value.to_rfc3339()));
            }

            Ok(Some("<unsupported>".to_string()))
        })
        .collect()
}

fn sanitize_mysql_error(error: sqlx::Error) -> AppError {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("password")
        || lower.contains("access denied")
        || lower.contains("userinfo")
        || lower.contains("mysql://")
    {
        AppError::Database(sqlx::Error::Protocol(
            "MySQL connection error (details redacted)".to_string(),
        ))
    } else {
        AppError::Database(error)
    }
}

fn sanitize_mysql_app_error(error: AppError) -> AppError {
    match error {
        AppError::Database(sqlx_error) => sanitize_mysql_error(sqlx_error),
        other => other,
    }
}

// ---------------------------------------------------------------------------
// SQLite helpers (unchanged)
// ---------------------------------------------------------------------------

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
            let default_value: Option<String> = row.try_get("dflt_value")?;

            Ok(DatabaseTableColumn {
                name,
                data_type,
                nullable: notnull == 0,
                primary_key: primary_key > 0,
                default_value,
            })
        })
        .collect()
}

async fn sqlite_table_kind(pool: &sqlx::SqlitePool, table_name: &str) -> AppResult<String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT type FROM sqlite_master WHERE name = ?1 LIMIT 1")
            .bind(table_name)
            .fetch_optional(pool)
            .await?;
    Ok(row
        .map(|value| value.0)
        .unwrap_or_else(|| "table".to_string()))
}

async fn sqlite_ddl(pool: &sqlx::SqlitePool, table_name: &str) -> AppResult<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT sql FROM sqlite_master WHERE name = ?1 LIMIT 1")
            .bind(table_name)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|value| value.0))
}

async fn sqlite_indexes(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<Vec<DatabaseIndex>> {
    let list_sql = format!("PRAGMA index_list({})", quote_identifier(table_name));
    let list_rows = sqlx::query(&list_sql).fetch_all(pool).await?;

    let mut indexes = Vec::with_capacity(list_rows.len());
    for row in list_rows {
        let name: String = row.try_get("name")?;
        let unique: i64 = row.try_get("unique")?;
        let origin: String = row.try_get("origin").unwrap_or_default();

        let info_sql = format!("PRAGMA index_info({})", quote_identifier(&name));
        let info_rows = sqlx::query(&info_sql).fetch_all(pool).await?;
        let columns = info_rows
            .iter()
            .map(|info| info.try_get::<String, _>("name"))
            .collect::<Result<Vec<_>, _>>()?;

        indexes.push(DatabaseIndex {
            name,
            columns,
            unique: unique != 0,
            primary: origin == "pk",
        });
    }

    Ok(indexes)
}

async fn sqlite_foreign_keys(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<Vec<DatabaseForeignKey>> {
    let sql = format!("PRAGMA foreign_key_list({})", quote_identifier(table_name));
    let rows = sqlx::query(&sql).fetch_all(pool).await?;

    // Rows for the same foreign key share an `id`; group them in order.
    let mut grouped: Vec<(i64, DatabaseForeignKey)> = Vec::new();
    for row in rows {
        let id: i64 = row.try_get("id")?;
        let referenced_table: String = row.try_get("table")?;
        let from: String = row.try_get("from")?;
        let to: String = row.try_get("to")?;

        if let Some((_, fk)) = grouped.iter_mut().find(|(existing, _)| *existing == id) {
            fk.columns.push(from);
            fk.referenced_columns.push(to);
        } else {
            grouped.push((
                id,
                DatabaseForeignKey {
                    name: format!("fk_{}_{}", table_name, id),
                    columns: vec![from],
                    referenced_table,
                    referenced_columns: vec![to],
                },
            ));
        }
    }

    Ok(grouped.into_iter().map(|(_, fk)| fk).collect())
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
        .ok_or_else(|| AppError::NotFound(table_name.to_string()))
}

async fn sqlite_table_row_count(pool: &sqlx::SqlitePool, table_name: &str) -> AppResult<u64> {
    let sql = format!(
        "SELECT COUNT(*) AS total_rows FROM {}",
        quote_identifier(table_name)
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let total_rows: i64 = row.try_get("total_rows")?;
    Ok(total_rows.max(0) as u64)
}

async fn sqlite_table_result_columns(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<Vec<DatabaseResultColumn>> {
    Ok(sqlite_columns(pool, table_name)
        .await?
        .into_iter()
        .map(|column| DatabaseResultColumn {
            name: column.name,
            data_type: column.data_type,
        })
        .collect())
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
            if let Ok(value) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(index) {
                return Ok(Some(value.to_rfc3339()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveDateTime, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveDate, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveTime, _>(index) {
                return Ok(Some(value.to_string()));
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

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn stored_to_database_connection(row: StoredDatabaseConnection) -> AppResult<DatabaseConnection> {
    let config = parse_database_config(&row.id, &row.config_json)?;
    let port = decode_port(row.port, "database connection port")?;
    Ok(DatabaseConnection {
        id: row.id,
        workspace_id: row.workspace_id,
        name: row.name,
        driver: row.driver,
        host: row.host,
        port,
        database: row.database_name,
        username: row.username,
        ssl_mode: row.ssl_mode,
        sqlite_path: config.sqlite_path,
        credential_ref: row.credential_ref,
        read_only: row.read_only,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
        revision: row.revision,
        sync_status: row.sync_status,
        remote_id: row.remote_id,
    })
}

fn input_to_storage(input: &DatabaseConnectionInput) -> AppResult<DatabaseConnectionStorageInput> {
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

        return Ok(DatabaseConnectionStorageInput {
            driver,
            host: None,
            port: None,
            database_name: None,
            username: None,
            read_only: input.read_only,
            ssl_mode: None,
            config: DatabaseConnectionConfig {
                sqlite_path: Some(sqlite_path.to_string()),
                connect_timeout_ms: None,
                statement_timeout_ms: None,
                default_schema: None,
            },
        });
    }

    Ok(DatabaseConnectionStorageInput {
        driver,
        host: empty_to_none(input.host.clone()),
        port: input.port,
        database_name: empty_to_none(input.database.clone()),
        username: empty_to_none(input.username.clone()),
        read_only: input.read_only,
        ssl_mode: normalize_ssl_mode(input.ssl_mode.clone())?,
        config: DatabaseConnectionConfig {
            sqlite_path: None,
            connect_timeout_ms: None,
            statement_timeout_ms: None,
            default_schema: None,
        },
    })
}

fn database_config_to_json(config: &DatabaseConnectionConfig) -> AppResult<String> {
    serde_json::to_string(config).map_err(AppError::from)
}

fn parse_database_config(
    connection_id: &str,
    config_json: &str,
) -> AppResult<DatabaseConnectionConfig> {
    serde_json::from_str::<DatabaseConnectionConfig>(config_json).map_err(|error| {
        AppError::Config(format!(
            "invalid database_connections.config_json for connection {connection_id}: {error}"
        ))
    })
}

fn normalize_ssl_mode(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = empty_to_none(value) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "disable" | "prefer" | "require" | "verify-ca" | "verify-full"
    ) {
        Ok(Some(normalized))
    } else {
        Err(AppError::Validation(format!(
            "unsupported database ssl mode: {value}"
        )))
    }
}

fn decode_port(value: Option<i64>, label: &str) -> AppResult<Option<u16>> {
    match value {
        None => Ok(None),
        Some(port) if (1..=u16::MAX as i64).contains(&port) => Ok(Some(port as u16)),
        Some(port) => Err(AppError::Config(format!("{label} out of range: {port}"))),
    }
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

/// Trim and validate an optional SQL identifier used as a session-context
/// target (PostgreSQL `search_path` schema or MySQL `USE` database). Returns
/// `None` for empty input. Callers still quote/escape the result before use;
/// this rejects control characters as defense in depth.
fn clean_identifier(value: Option<&str>) -> AppResult<Option<&str>> {
    let trimmed = match value.map(str::trim).filter(|item| !item.is_empty()) {
        None => return Ok(None),
        Some(item) => item,
    };
    if trimmed.chars().any(|ch| ch == '\0' || ch.is_control()) {
        return Err(AppError::Validation(
            "database or schema name contains invalid characters".to_string(),
        ));
    }
    Ok(Some(trimmed))
}

fn quote_mysql_identifier(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}

fn quote_qualified_identifier(schema: &str, table_name: &str) -> String {
    format!(
        "{}.{}",
        quote_identifier(schema),
        quote_identifier(table_name)
    )
}

fn postgres_browse_sql(
    schema: &str,
    table_name: &str,
    where_sql: &str,
    order_sql: &str,
    limit: u32,
    offset: u32,
) -> String {
    format!(
        "SELECT * FROM {}{}{} LIMIT {} OFFSET {}",
        quote_qualified_identifier(schema, table_name),
        where_sql,
        order_sql,
        limit,
        offset
    )
}

fn quote_mysql_qualified_identifier(schema: &str, table_name: &str) -> String {
    format!(
        "{}.{}",
        quote_mysql_identifier(schema),
        quote_mysql_identifier(table_name)
    )
}

fn mysql_browse_sql(
    schema: &str,
    table_name: &str,
    where_sql: &str,
    order_sql: &str,
    limit: u32,
    offset: u32,
) -> String {
    format!(
        "SELECT * FROM {}{}{} LIMIT {} OFFSET {}",
        quote_mysql_qualified_identifier(schema, table_name),
        where_sql,
        order_sql,
        limit,
        offset
    )
}

const DEFAULT_QUERY_TIMEOUT_MS: u64 = 30_000;
const MIN_QUERY_TIMEOUT_MS: u64 = 1_000;
const MAX_QUERY_TIMEOUT_MS: u64 = 300_000;

/// Resolve a per-statement timeout, clamping caller input into a sane band and
/// applying a default so a runaway query cannot hang a session indefinitely.
fn resolve_timeout(timeout_ms: Option<u64>) -> Duration {
    let ms = timeout_ms
        .unwrap_or(DEFAULT_QUERY_TIMEOUT_MS)
        .clamp(MIN_QUERY_TIMEOUT_MS, MAX_QUERY_TIMEOUT_MS);
    Duration::from_millis(ms)
}

/// Trim a browse filter to a non-empty needle, or `None` when blank.
fn normalize_filter(filter: Option<&str>) -> Option<String> {
    filter
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Build a validated, quoted `ORDER BY` fragment. The column must be one of the
/// table's real columns (defense in depth on top of identifier quoting); an
/// empty/absent column yields no clause.
fn order_by_clause(
    order_by: Option<&str>,
    descending: bool,
    columns: &[String],
    quote: fn(&str) -> String,
) -> AppResult<String> {
    let Some(column) = order_by.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(String::new());
    };
    if !columns.iter().any(|name| name == column) {
        return Err(AppError::Validation(format!(
            "unknown sort column: {}",
            column
        )));
    }
    Ok(format!(
        " ORDER BY {} {}",
        quote(column),
        if descending { "DESC" } else { "ASC" }
    ))
}

/// `(CAST(col AS TEXT) LIKE ? OR ...)` for SQLite. One placeholder per column.
fn sqlite_filter_where(columns: &[String]) -> String {
    let parts = columns
        .iter()
        .map(|name| format!("CAST({} AS TEXT) LIKE ?", quote_identifier(name)))
        .collect::<Vec<_>>();
    format!("({})", parts.join(" OR "))
}

/// `(CAST(col AS TEXT) ILIKE $1 OR ...)` for PostgreSQL. Reuses a single bind.
fn postgres_filter_where(columns: &[String]) -> String {
    let parts = columns
        .iter()
        .map(|name| format!("CAST({} AS TEXT) ILIKE $1", quote_identifier(name)))
        .collect::<Vec<_>>();
    format!("({})", parts.join(" OR "))
}

/// `(CAST(col AS CHAR) LIKE ? OR ...)` for MySQL. One placeholder per column.
fn mysql_filter_where(columns: &[String]) -> String {
    let parts = columns
        .iter()
        .map(|name| format!("CAST({} AS CHAR) LIKE ?", quote_mysql_identifier(name)))
        .collect::<Vec<_>>();
    format!("({})", parts.join(" OR "))
}

#[allow(clippy::too_many_arguments)]
fn browse_result(
    table_name: &str,
    sql: String,
    limit: u32,
    offset: u32,
    total_rows: u64,
    columns: Vec<DatabaseResultColumn>,
    rows: Vec<Vec<Option<String>>>,
    started: Instant,
) -> DatabaseBrowseResult {
    DatabaseBrowseResult {
        table_name: table_name.to_string(),
        sql,
        limit,
        offset,
        total_rows,
        read_only: true,
        result: DatabaseQueryResult {
            columns,
            rows,
            affected_rows: 0,
            duration_ms: started.elapsed().as_millis(),
            safety: DatabaseQuerySafety {
                classification: "read".to_string(),
                requires_confirmation: false,
                confirmed: true,
                message: None,
            },
        },
    }
}

#[derive(Clone, Copy)]
enum SqlDialect {
    /// SQLite and PostgreSQL: double-quoted identifiers, `''` escapes only.
    Standard,
    /// MySQL/MariaDB: backtick identifiers and backslash escaping.
    MySql,
}

impl SqlDialect {
    fn quote_ident(&self, value: &str) -> String {
        match self {
            SqlDialect::Standard => quote_identifier(value),
            SqlDialect::MySql => quote_mysql_identifier(value),
        }
    }

    fn quote_qualified(&self, schema: Option<&str>, table: &str) -> String {
        match schema {
            Some(schema) => format!("{}.{}", self.quote_ident(schema), self.quote_ident(table)),
            None => self.quote_ident(table),
        }
    }

    fn literal(&self, value: Option<&str>) -> String {
        match value {
            None => "NULL".to_string(),
            Some(value) => match self {
                SqlDialect::Standard => format!("'{}'", value.replace('\'', "''")),
                SqlDialect::MySql => {
                    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "''"))
                }
            },
        }
    }
}

fn build_row_mutation_sql(
    dialect: SqlDialect,
    schema: Option<&str>,
    table_name: &str,
    operation: &str,
    values: &[DatabaseCellValue],
    primary_key: &[DatabaseCellValue],
) -> AppResult<String> {
    let qualified = dialect.quote_qualified(schema, table_name);

    let predicate = |cell: &DatabaseCellValue| match cell.value.as_deref() {
        None => format!("{} IS NULL", dialect.quote_ident(&cell.column)),
        Some(value) => format!(
            "{} = {}",
            dialect.quote_ident(&cell.column),
            dialect.literal(Some(value))
        ),
    };

    match operation {
        "insert" => {
            if values.is_empty() {
                return Err(AppError::Validation(
                    "insert requires at least one column value".to_string(),
                ));
            }
            let columns = values
                .iter()
                .map(|cell| dialect.quote_ident(&cell.column))
                .collect::<Vec<_>>()
                .join(", ");
            let literals = values
                .iter()
                .map(|cell| dialect.literal(cell.value.as_deref()))
                .collect::<Vec<_>>()
                .join(", ");
            Ok(format!(
                "INSERT INTO {} ({}) VALUES ({})",
                qualified, columns, literals
            ))
        }
        "update" => {
            if values.is_empty() {
                return Err(AppError::Validation(
                    "update requires at least one column value".to_string(),
                ));
            }
            if primary_key.is_empty() {
                return Err(AppError::Validation(
                    "update requires a primary key to identify the row".to_string(),
                ));
            }
            let assignments = values
                .iter()
                .map(|cell| {
                    format!(
                        "{} = {}",
                        dialect.quote_ident(&cell.column),
                        dialect.literal(cell.value.as_deref())
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let where_clause = primary_key
                .iter()
                .map(predicate)
                .collect::<Vec<_>>()
                .join(" AND ");
            Ok(format!(
                "UPDATE {} SET {} WHERE {}",
                qualified, assignments, where_clause
            ))
        }
        "delete" => {
            if primary_key.is_empty() {
                return Err(AppError::Validation(
                    "delete requires a primary key to identify the row".to_string(),
                ));
            }
            let where_clause = primary_key
                .iter()
                .map(predicate)
                .collect::<Vec<_>>()
                .join(" AND ");
            Ok(format!("DELETE FROM {} WHERE {}", qualified, where_clause))
        }
        other => Err(AppError::Validation(format!(
            "unsupported row operation: {}",
            other
        ))),
    }
}

fn returns_rows(sql: &str) -> bool {
    let keyword = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        keyword.as_str(),
        "select" | "with" | "pragma" | "explain" | "show"
    )
}

fn validate_single_statement(sql: &str) -> AppResult<()> {
    let trimmed = sql.trim();
    let without_trailing = trimmed.trim_end_matches(';').trim_end();
    if without_trailing.contains(';') {
        return Err(AppError::Validation(
            "only one SQL statement can be executed at a time".to_string(),
        ));
    }
    Ok(())
}

/// Data-modifying keywords used to detect a write hidden behind an
/// `EXPLAIN`/`WITH` wrapper. PostgreSQL executes `EXPLAIN ANALYZE <write>` and
/// data-modifying CTEs (`WITH t AS (DELETE ... RETURNING *) ...`), both of which
/// the leading keyword alone would misread as a safe, no-confirmation read.
const WRITE_KEYWORDS: &[&str] = &["insert", "update", "delete", "replace", "merge", "upsert"];
const SCHEMA_KEYWORDS: &[&str] = &[
    "create", "alter", "drop", "truncate", "vacuum", "reindex", "grant", "revoke",
];

/// Scan a statement's tokens for a data-modifying or schema-changing keyword.
/// Returns the matching safety classification when one is found. This errs
/// toward over-detection: a keyword appearing inside a string literal only
/// triggers an extra confirmation prompt, it never lets a real write through.
fn detect_wrapped_write(sql: &str) -> Option<DatabaseQuerySafety> {
    let mut has_write = false;
    let mut has_schema = false;
    for token in sql.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        if token.is_empty() {
            continue;
        }
        let lowered = token.to_ascii_lowercase();
        if SCHEMA_KEYWORDS.contains(&lowered.as_str()) {
            has_schema = true;
        } else if WRITE_KEYWORDS.contains(&lowered.as_str()) {
            has_write = true;
        }
    }

    let classification = if has_schema {
        "schema-change"
    } else if has_write {
        "mutation"
    } else {
        return None;
    };

    Some(DatabaseQuerySafety {
        classification: classification.to_string(),
        requires_confirmation: true,
        confirmed: false,
        message: Some(
            "This statement can modify data or schema despite its EXPLAIN/WITH prefix. Confirm to execute it."
                .to_string(),
        ),
    })
}

fn read_safety() -> DatabaseQuerySafety {
    DatabaseQuerySafety {
        classification: "read".to_string(),
        requires_confirmation: false,
        confirmed: true,
        message: None,
    }
}

fn classify_query(sql: &str) -> DatabaseQuerySafety {
    let keyword = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();

    match keyword.as_str() {
        "select" | "pragma" | "show" => read_safety(),
        // EXPLAIN and WITH can wrap a statement that actually writes (EXPLAIN
        // ANALYZE <write> and data-modifying CTEs in PostgreSQL), so look past
        // the wrapper before trusting them as no-confirmation reads.
        "explain" | "with" => detect_wrapped_write(sql).unwrap_or_else(read_safety),
        "insert" | "update" | "delete" | "replace" => DatabaseQuerySafety {
            classification: "mutation".to_string(),
            requires_confirmation: true,
            confirmed: false,
            message: Some("This SQL statement may change data. Confirm to execute it.".to_string()),
        },
        "create" | "alter" | "drop" | "truncate" | "vacuum" | "reindex" => DatabaseQuerySafety {
            classification: "schema-change".to_string(),
            requires_confirmation: true,
            confirmed: false,
            message: Some(
                "This SQL statement may change schema or database storage. Confirm to execute it."
                    .to_string(),
            ),
        },
        "begin" | "commit" | "rollback" => DatabaseQuerySafety {
            classification: "transaction-control".to_string(),
            requires_confirmation: true,
            confirmed: false,
            message: Some(
                "Transaction control statements require confirmation in this editor.".to_string(),
            ),
        },
        _ => DatabaseQuerySafety {
            classification: "unknown".to_string(),
            requires_confirmation: true,
            confirmed: false,
            message: Some(
                "Unrecognized SQL statement type requires confirmation before execution."
                    .to_string(),
            ),
        },
    }
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "database_tests/mod.rs"]
mod tests;
