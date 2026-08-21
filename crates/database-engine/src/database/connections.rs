use super::*;
use sqlx::SqliteConnection;
use unfour_core::domain::{CommandContext, DomainCommandResult, MutationOperation};

use super::connection_domain::DatabaseConnectionCleanup;
use unfour_core::domain::connection_mutation;

#[derive(Debug, sqlx::FromRow)]
struct StoredDatabaseConnection {
    id: String,
    workspace_id: String,
    name: String,
    pub(super) host: Option<String>,
    pub(super) port: Option<i64>,
    pub(super) driver: String,
    pub(super) database_name: Option<String>,
    pub(super) username: Option<String>,
    pub(super) ssl_mode: Option<String>,
    pub(super) read_only: bool,
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
pub(super) struct DatabaseConnectionStorageInput {
    pub(super) driver: String,
    pub(super) host: Option<String>,
    pub(super) port: Option<u16>,
    pub(super) database_name: Option<String>,
    pub(super) username: Option<String>,
    pub(super) ssl_mode: Option<String>,
    pub(super) read_only: bool,
    pub(super) config: DatabaseConnectionConfig,
}

impl DatabaseService {
    pub async fn list_connections(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<DatabaseConnection>> {
        let mut connection = self.db.pool().acquire().await?;
        self.list_connections_on(&mut connection, &workspace_id)
            .await
    }

    pub async fn list_connections_on(
        &self,
        connection: &mut SqliteConnection,
        workspace_id: &str,
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
        .fetch_all(&mut *connection)
        .await?;

        rows.into_iter()
            .map(stored_to_database_connection)
            .collect()
    }

    pub async fn save_connection(
        &self,
        input: DatabaseConnectionInput,
    ) -> AppResult<DatabaseConnection> {
        let context = CommandContext::local("database.connection.save");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .save_connection_on(&mut transaction, &context, input)
            .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    pub async fn save_connection_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        input: DatabaseConnectionInput,
    ) -> AppResult<DomainCommandResult<DatabaseConnection>> {
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
        validate_credential_ref_for_workspace(credential_ref.as_deref(), &input.workspace_id)?;

        let (id, revision) = if let Some(id) = input
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            let revision: Option<i64> = sqlx::query_scalar(
                r#"
                UPDATE connections
                SET name = ?1, host = ?2, port = ?3, credential_ref = ?4,
                    updated_at = ?5, revision = revision + 1, sync_status = 'pending'
                WHERE id = ?6 AND workspace_id = ?7 AND connection_type = 'database' AND deleted_at IS NULL
                RETURNING revision
                "#,
            )
            .bind(name)
            .bind(host)
            .bind(port)
            .bind(credential_ref)
            .bind(&now)
            .bind(id)
            .bind(&input.workspace_id)
            .fetch_optional(&mut *connection)
            .await?;
            let revision =
                revision.ok_or_else(|| AppError::NotFound("database connection".to_string()))?;

            let subtype = sqlx::query(
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
            .execute(&mut *connection)
            .await?;
            if subtype.rows_affected() != 1 {
                return Err(AppError::Config(
                    "database connection subtype row is missing".to_string(),
                ));
            }
            (id.to_string(), revision)
        } else {
            let id = unfour_core::id::new_id();
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
            .execute(&mut *connection)
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
            .execute(&mut *connection)
            .await?;
            (id, 1)
        };

        let saved = self
            .get_connection_on(connection, &input.workspace_id, &id)
            .await?;
        Ok(DomainCommandResult::new(
            saved,
            vec![connection_mutation(
                context,
                MutationOperation::Upsert,
                &input.workspace_id,
                &id,
                revision,
            )],
        ))
    }

    pub async fn delete_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<Vec<DatabaseConnection>> {
        let context = CommandContext::local("database.connection.delete");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .delete_connection_on(&mut transaction, &context, workspace_id, connection_id)
            .await?;
        transaction.commit().await?;
        let (connections, cleanup) = outcome.value;
        self.cleanup_connection_changes(vec![cleanup]).await;
        Ok(connections)
    }

    pub async fn delete_connection_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<DomainCommandResult<(Vec<DatabaseConnection>, DatabaseConnectionCleanup)>> {
        validate_workspace_id(&workspace_id)?;
        validate_connection_id(&connection_id)?;
        let now = Utc::now().to_rfc3339();

        // Read the credential reference before soft-deleting so the stored
        // secret can be purged from the OS keychain.
        let existing: Option<Option<String>> = sqlx::query_scalar(
            "SELECT credential_ref FROM connections \
             WHERE id = ?1 AND workspace_id = ?2 \
               AND connection_type = 'database' AND deleted_at IS NULL",
        )
        .bind(&connection_id)
        .bind(&workspace_id)
        .fetch_optional(&mut *connection)
        .await?;
        let credential_ref =
            existing.ok_or_else(|| AppError::NotFound("database connection".to_string()))?;

        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE connections
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1, sync_status = 'deleted'
            WHERE id = ?2 AND workspace_id = ?3
              AND connection_type = 'database' AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(&now)
        .bind(&connection_id)
        .bind(&workspace_id)
        .fetch_one(&mut *connection)
        .await?;

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
        .execute(&mut *connection)
        .await?;
        let remaining = self.list_connections_on(connection, &workspace_id).await?;
        let cleanup = DatabaseConnectionCleanup::new(workspace_id.clone(), credential_ref);
        Ok(DomainCommandResult::new(
            (remaining, cleanup),
            vec![connection_mutation(
                context,
                MutationOperation::Delete,
                &workspace_id,
                &connection_id,
                revision,
            )],
        ))
    }

    pub(super) async fn get_connection(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> AppResult<DatabaseConnection> {
        let mut connection = self.db.pool().acquire().await?;
        self.get_connection_on(&mut connection, workspace_id, connection_id)
            .await
    }

    pub(super) async fn get_connection_on(
        &self,
        connection: &mut SqliteConnection,
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
        .fetch_optional(&mut *connection)
        .await?;

        row.map(stored_to_database_connection)
            .transpose()?
            .ok_or_else(|| AppError::NotFound("database connection".to_string()))
    }
}

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

pub(super) fn input_to_storage(
    input: &DatabaseConnectionInput,
) -> AppResult<DatabaseConnectionStorageInput> {
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

pub(super) fn database_config_to_json(config: &DatabaseConnectionConfig) -> AppResult<String> {
    serde_json::to_string(config).map_err(AppError::from)
}

pub(super) fn parse_database_config(
    connection_id: &str,
    config_json: &str,
) -> AppResult<DatabaseConnectionConfig> {
    serde_json::from_str::<DatabaseConnectionConfig>(config_json).map_err(|error| {
        AppError::Config(format!(
            "invalid database_connections.config_json for connection {connection_id}: {error}"
        ))
    })
}

pub(super) fn normalize_ssl_mode(value: Option<String>) -> AppResult<Option<String>> {
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

pub(super) fn decode_port(value: Option<i64>, label: &str) -> AppResult<Option<u16>> {
    match value {
        None => Ok(None),
        Some(port) if (1..=u16::MAX as i64).contains(&port) => Ok(Some(port as u16)),
        Some(port) => Err(AppError::Config(format!("{label} out of range: {port}"))),
    }
}

pub(super) fn normalize_name(name: &str) -> AppResult<String> {
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

pub(super) fn empty_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

/// Format: `<service>:<workspace_id>:<kind>:<record_id>`.
pub(super) fn validate_credential_ref_for_workspace(
    credential_ref: Option<&str>,
    workspace_id: &str,
) -> AppResult<()> {
    let Some(credential_ref) = credential_ref else {
        return Ok(());
    };
    let parsed_workspace = parse_credential_ref_workspace(credential_ref)?;
    if parsed_workspace != workspace_id {
        return Err(AppError::Validation(
            "credential reference does not belong to the workspace".to_string(),
        ));
    }
    Ok(())
}

fn parse_credential_ref_workspace(credential_ref: &str) -> AppResult<&str> {
    let mut parts = credential_ref.splitn(4, ':');
    let service_name = parts.next().unwrap_or_default();
    let workspace_id = parts.next().unwrap_or_default();
    let kind = parts.next().unwrap_or_default();
    let record_id = parts.next().unwrap_or_default();
    if service_name.is_empty() || workspace_id.is_empty() || kind.is_empty() || record_id.is_empty()
    {
        return Err(AppError::Validation(
            "credential reference is invalid".to_string(),
        ));
    }
    Ok(workspace_id)
}

pub(super) fn validate_workspace_id(workspace_id: &str) -> AppResult<()> {
    if workspace_id.trim().is_empty() {
        return Err(AppError::Validation(
            "workspace id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_connection_id(connection_id: &str) -> AppResult<()> {
    if connection_id.trim().is_empty() {
        return Err(AppError::Validation(
            "database connection id cannot be empty".to_string(),
        ));
    }
    Ok(())
}
