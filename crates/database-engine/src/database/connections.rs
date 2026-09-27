use super::connection_storage::ConnectionSubtype;
use super::saved_sql::clear_saved_sql_connection_on;
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

#[derive(Debug, sqlx::FromRow)]
struct CurrentDatabaseConnectionSave {
    name: String,
    host: Option<String>,
    port: Option<i64>,
    driver: String,
    database_name: Option<String>,
    username: Option<String>,
    ssl_mode: Option<String>,
    read_only: bool,
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

        let (id, revision, shared_changed) = if let Some(id) = input
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            let current = sqlx::query_as::<_, CurrentDatabaseConnectionSave>(
                r#"
                SELECT c.name, c.host, c.port, sub.driver, sub.database_name,
                       sub.username, sub.ssl_mode, sub.read_only
                FROM connections c
                INNER JOIN database_connections sub ON sub.connection_id = c.id
                WHERE c.id = ?1 AND c.workspace_id = ?2
                  AND c.connection_type = 'database' AND c.deleted_at IS NULL
                "#,
            )
            .bind(id)
            .bind(&input.workspace_id)
            .fetch_optional(&mut *connection)
            .await?
            .ok_or_else(|| AppError::NotFound("database connection".to_string()))?;
            let shared_changed = current.name != name
                || current.host != host
                || current.port != port
                || current.driver != storage.driver
                || current.database_name != database_name
                || current.username != username
                || current.ssl_mode != ssl_mode
                || current.read_only != storage.read_only;
            let revision = if shared_changed {
                sqlx::query_scalar(
                    r#"
                    UPDATE connections
                    SET name = ?1, host = ?2, port = ?3, credential_ref = ?4,
                        updated_at = ?5, revision = revision + 1, sync_status = 'pending'
                    WHERE id = ?6 AND workspace_id = ?7
                      AND connection_type = 'database' AND deleted_at IS NULL
                    RETURNING revision
                    "#,
                )
                .bind(&name)
                .bind(&host)
                .bind(port)
                .bind(&credential_ref)
                .bind(&now)
                .bind(id)
                .bind(&input.workspace_id)
                .fetch_one(&mut *connection)
                .await?
            } else {
                sqlx::query_scalar(
                    r#"
                    UPDATE connections
                    SET credential_ref = ?1
                    WHERE id = ?2 AND workspace_id = ?3
                      AND connection_type = 'database' AND deleted_at IS NULL
                    RETURNING revision
                    "#,
                )
                .bind(&credential_ref)
                .bind(id)
                .bind(&input.workspace_id)
                .fetch_one(&mut *connection)
                .await?
            };

            let subtype = if shared_changed {
                ConnectionSubtype {
                    driver: &storage.driver,
                    database_name: database_name.as_deref(),
                    username: username.as_deref(),
                    ssl_mode: ssl_mode.as_deref(),
                    read_only: storage.read_only,
                    config_json: &config_json,
                }
                .update_on(connection, id)
                .await?
            } else {
                sqlx::query(
                    r#"
                    UPDATE database_connections
                    SET config_json = ?1
                    WHERE connection_id = ?2
                    "#,
                )
                .bind(&config_json)
                .bind(id)
                .execute(&mut *connection)
                .await?
            };
            if subtype.rows_affected() != 1 {
                return Err(AppError::Config(
                    "database connection subtype row is missing".to_string(),
                ));
            }
            (id.to_string(), revision, shared_changed)
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

            ConnectionSubtype {
                driver: &storage.driver,
                database_name: storage.database_name.as_deref(),
                username: storage.username.as_deref(),
                ssl_mode: storage.ssl_mode.as_deref(),
                read_only: storage.read_only,
                config_json: &config_json,
            }
            .insert_on(connection, &id)
            .await?;
            (id, 1, true)
        };

        let saved = self
            .get_connection_on(connection, &input.workspace_id, &id)
            .await?;
        let mutations = if shared_changed {
            vec![connection_mutation(
                context,
                MutationOperation::Upsert,
                &input.workspace_id,
                &id,
                revision,
            )]
        } else {
            Vec::new()
        };
        Ok(DomainCommandResult::new(saved, mutations))
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

        clear_saved_sql_connection_on(connection, &workspace_id, &connection_id, &now).await?;
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
