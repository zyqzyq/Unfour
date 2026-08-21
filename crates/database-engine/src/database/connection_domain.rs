use sqlx::{FromRow, SqliteConnection};
use unfour_core::domain::{
    connection_mutation, validate_connection_domain_key, validate_external_connection_delete,
    validate_external_connection_upsert, CommandContext, ConnectionSnapshot,
    ConnectionSnapshotConfig, DomainCommandResult, DomainEntityKey, DomainMutation, DomainSnapshot,
    ExternalConnectionApply, ExternalConnectionUpsert, ExternalDelete, MutationOperation,
    MutationOrigin, TombstoneSnapshot,
};

use super::*;

mod support;
use support::*;

pub struct DatabaseConnectionCleanup {
    workspace_id: String,
    credential_ref: Option<String>,
}

impl DatabaseConnectionCleanup {
    pub(super) fn new(workspace_id: String, credential_ref: Option<String>) -> Self {
        Self {
            workspace_id,
            credential_ref,
        }
    }
}

#[derive(FromRow)]
struct DatabaseSnapshotRow {
    id: String,
    workspace_id: String,
    connection_type: String,
    name: String,
    host: Option<String>,
    port: Option<i64>,
    driver: String,
    database_name: Option<String>,
    username: Option<String>,
    ssl_mode: Option<String>,
    read_only: bool,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    revision: i64,
}

#[derive(FromRow)]
struct CurrentExternalDatabaseConnection {
    workspace_id: String,
    name: String,
    host: Option<String>,
    port: Option<i64>,
    credential_ref: Option<String>,
    driver: String,
    database_name: Option<String>,
    username: Option<String>,
    ssl_mode: Option<String>,
    read_only: bool,
    config_json: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    sync_status: String,
}

impl DatabaseService {
    pub async fn read_connection_domain_snapshot(
        &self,
        key: &DomainEntityKey,
    ) -> AppResult<DomainSnapshot> {
        let mut connection = self.db.pool().acquire().await?;
        self.read_connection_domain_snapshot_on(&mut connection, key)
            .await
    }

    pub async fn read_connection_domain_snapshot_on(
        &self,
        connection: &mut SqliteConnection,
        key: &DomainEntityKey,
    ) -> AppResult<DomainSnapshot> {
        validate_connection_domain_key(key)?;
        let row = sqlx::query_as::<_, DatabaseSnapshotRow>(
            r#"
            SELECT c.id, c.workspace_id, c.connection_type, c.name, c.host, c.port,
                   sub.driver, sub.database_name, sub.username, sub.ssl_mode,
                   sub.read_only, c.created_at, c.updated_at, c.deleted_at, c.revision
            FROM connections c
            INNER JOIN database_connections sub ON sub.connection_id = c.id
            WHERE c.workspace_id = ?1 AND c.id = ?2 AND c.connection_type = 'database'
            "#,
        )
        .bind(&key.workspace_id)
        .bind(&key.entity_id)
        .fetch_optional(&mut *connection)
        .await?
        .ok_or_else(|| AppError::NotFound("database connection".to_string()))?;
        if let Some(deleted_at) = row.deleted_at {
            return Ok(DomainSnapshot::Tombstone(TombstoneSnapshot {
                entity: key.clone(),
                deleted_at,
                revision: row.revision,
            }));
        }
        Ok(DomainSnapshot::Connection(ConnectionSnapshot {
            id: row.id,
            workspace_id: row.workspace_id,
            connection_type: row.connection_type,
            name: row.name,
            host: row.host,
            port: decode_port(row.port, "database connection snapshot port")?,
            config: ConnectionSnapshotConfig::Database {
                driver: row.driver,
                database_name: row.database_name,
                username: row.username,
                ssl_mode: row.ssl_mode,
                read_only: row.read_only,
            },
            created_at: row.created_at,
            updated_at: row.updated_at,
            revision: row.revision,
        }))
    }

    pub async fn apply_external_connection_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        change: ExternalConnectionApply,
    ) -> AppResult<DomainCommandResult<Option<DatabaseConnectionCleanup>>> {
        if context.origin != MutationOrigin::External {
            return Err(AppError::Config(
                "external connection apply requires an External command context".to_string(),
            ));
        }
        match change {
            ExternalConnectionApply::Upsert(record) => {
                self.apply_external_connection_upsert_on(connection, context, record)
                    .await
            }
            ExternalConnectionApply::Delete(delete) => {
                self.apply_external_connection_delete_on(connection, context, delete)
                    .await
            }
        }
    }

    async fn apply_external_connection_upsert_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        record: ExternalConnectionUpsert,
    ) -> AppResult<DomainCommandResult<Option<DatabaseConnectionCleanup>>> {
        validate_external_connection_upsert(&record)?;
        if record.connection_type != "database" {
            return Err(AppError::Validation(
                "external database connection must use connection_type=database".to_string(),
            ));
        }
        let ConnectionSnapshotConfig::Database {
            driver,
            database_name,
            username,
            ssl_mode,
            read_only,
        } = record.config
        else {
            return Err(AppError::Validation(
                "external database connection requires database config".to_string(),
            ));
        };
        validate_live_workspace_on(connection, &record.workspace_id).await?;
        let name = normalize_name(&record.name)?;
        let driver = driver.trim().to_ascii_lowercase();
        if !matches!(driver.as_str(), "sqlite" | "postgres" | "mysql") {
            return Err(AppError::Validation(format!(
                "unsupported external database driver: {driver}"
            )));
        }
        let (host, port, database_name, username, ssl_mode) = if driver == "sqlite" {
            if record.host.is_some()
                || record.port.is_some()
                || database_name.is_some()
                || username.is_some()
                || ssl_mode.is_some()
            {
                return Err(AppError::Validation(
                    "external SQLite connection contains non-SQLite endpoint fields".to_string(),
                ));
            }
            (None, None, None, None, None)
        } else {
            if record.port == Some(0) {
                return Err(AppError::Validation(
                    "external database port cannot be 0".to_string(),
                ));
            }
            (
                empty_to_none(record.host.clone()),
                record.port,
                empty_to_none(database_name),
                empty_to_none(username),
                normalize_ssl_mode(ssl_mode)?,
            )
        };

        let existing_owner: Option<(String, String)> =
            sqlx::query_as("SELECT workspace_id, connection_type FROM connections WHERE id = ?1")
                .bind(&record.id)
                .fetch_optional(&mut *connection)
                .await?;
        if let Some((workspace_id, connection_type)) = existing_owner {
            if workspace_id != record.workspace_id {
                return Err(AppError::Validation(
                    "external connection workspace ownership mismatch".to_string(),
                ));
            }
            if connection_type != "database" {
                return Err(AppError::Validation(
                    "external connection cannot change an existing connection type".to_string(),
                ));
            }
            let current = sqlx::query_as::<_, CurrentExternalDatabaseConnection>(
                r#"
                SELECT c.workspace_id, c.name, c.host, c.port, c.credential_ref,
                       sub.driver, sub.database_name, sub.username, sub.ssl_mode,
                       sub.read_only, sub.config_json, c.created_at, c.updated_at,
                       c.deleted_at, c.sync_status
                FROM connections c
                INNER JOIN database_connections sub ON sub.connection_id = c.id
                WHERE c.id = ?1
                "#,
            )
            .bind(&record.id)
            .fetch_one(&mut *connection)
            .await?;
            let current_config = parse_database_config(&record.id, &current.config_json)?;
            let compatible_driver = current.driver == driver;
            let next_credential_ref = if compatible_driver && driver != "sqlite" {
                current.credential_ref.clone()
            } else {
                None
            };
            let next_config = if compatible_driver {
                current_config
            } else {
                empty_database_config()
            };
            let cleanup = current
                .credential_ref
                .clone()
                .filter(|current_ref| Some(current_ref) != next_credential_ref.as_ref())
                .map(|credential_ref| {
                    DatabaseConnectionCleanup::new(
                        record.workspace_id.clone(),
                        Some(credential_ref),
                    )
                });
            let next_config_json = database_config_to_json(&next_config)?;
            if current.deleted_at.is_none()
                && current.workspace_id == record.workspace_id
                && current.name == name
                && current.host == host
                && current.port == port.map(i64::from)
                && current.credential_ref == next_credential_ref
                && current.driver == driver
                && current.database_name == database_name
                && current.username == username
                && current.ssl_mode == ssl_mode
                && current.read_only == read_only
                && current.config_json == next_config_json
                && current.created_at == record.created_at
                && current.updated_at == record.updated_at
                && current.sync_status == "local"
            {
                return Ok(DomainCommandResult::unchanged(None));
            }
            let revision: i64 = sqlx::query_scalar(
                r#"
                UPDATE connections
                SET name = ?1, host = ?2, port = ?3, credential_ref = ?4,
                    created_at = ?5, updated_at = ?6, deleted_at = NULL,
                    revision = revision + 1, sync_status = 'local'
                WHERE id = ?7 AND workspace_id = ?8 AND connection_type = 'database'
                RETURNING revision
                "#,
            )
            .bind(name)
            .bind(host)
            .bind(port.map(i64::from))
            .bind(next_credential_ref)
            .bind(&record.created_at)
            .bind(&record.updated_at)
            .bind(&record.id)
            .bind(&record.workspace_id)
            .fetch_one(&mut *connection)
            .await?;
            sqlx::query(
                r#"
                UPDATE database_connections
                SET driver = ?1, database_name = ?2, username = ?3,
                    ssl_mode = ?4, read_only = ?5, config_json = ?6
                WHERE connection_id = ?7
                "#,
            )
            .bind(driver)
            .bind(database_name)
            .bind(username)
            .bind(ssl_mode)
            .bind(read_only)
            .bind(next_config_json)
            .bind(&record.id)
            .execute(&mut *connection)
            .await?;
            return Ok(DomainCommandResult::new(
                cleanup,
                vec![connection_mutation(
                    context,
                    MutationOperation::Upsert,
                    &record.workspace_id,
                    &record.id,
                    revision,
                )],
            ));
        }

        let empty_config = database_config_to_json(&empty_database_config())?;
        sqlx::query(
            r#"
            INSERT INTO connections (
              id, workspace_id, connection_type, name, host, port, credential_ref,
              created_at, updated_at, revision, sync_status
            ) VALUES (?1, ?2, 'database', ?3, ?4, ?5, NULL, ?6, ?7, 1, 'local')
            "#,
        )
        .bind(&record.id)
        .bind(&record.workspace_id)
        .bind(name)
        .bind(host)
        .bind(port.map(i64::from))
        .bind(&record.created_at)
        .bind(&record.updated_at)
        .execute(&mut *connection)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO database_connections (
              connection_id, driver, database_name, username, ssl_mode, read_only, config_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&record.id)
        .bind(driver)
        .bind(database_name)
        .bind(username)
        .bind(ssl_mode)
        .bind(read_only)
        .bind(empty_config)
        .execute(&mut *connection)
        .await?;
        Ok(DomainCommandResult::new(
            None,
            vec![connection_mutation(
                context,
                MutationOperation::Upsert,
                &record.workspace_id,
                &record.id,
                1,
            )],
        ))
    }

    async fn apply_external_connection_delete_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        delete: ExternalDelete,
    ) -> AppResult<DomainCommandResult<Option<DatabaseConnectionCleanup>>> {
        validate_external_connection_delete(&delete)?;
        let current: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT workspace_id, connection_type, credential_ref, deleted_at FROM connections WHERE id = ?1",
        )
        .bind(&delete.entity.entity_id)
        .fetch_optional(&mut *connection)
        .await?;
        let Some((workspace_id, connection_type, credential_ref, deleted_at)) = current else {
            return Ok(DomainCommandResult::unchanged(None));
        };
        if workspace_id != delete.entity.workspace_id {
            return Err(AppError::Validation(
                "external connection workspace ownership mismatch".to_string(),
            ));
        }
        if connection_type != "database" {
            return Err(AppError::Validation(
                "external database delete cannot target an SSH connection".to_string(),
            ));
        }
        if deleted_at.is_some() {
            return Ok(DomainCommandResult::unchanged(None));
        }
        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE connections
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1,
                sync_status = 'deleted'
            WHERE id = ?2 AND workspace_id = ?3 AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(&delete.deleted_at)
        .bind(&delete.entity.entity_id)
        .bind(&delete.entity.workspace_id)
        .fetch_one(&mut *connection)
        .await?;
        clear_saved_sql_connection_on(
            connection,
            &delete.entity.workspace_id,
            &delete.entity.entity_id,
            &delete.deleted_at,
        )
        .await?;
        let cleanup =
            DatabaseConnectionCleanup::new(delete.entity.workspace_id.clone(), credential_ref);
        Ok(DomainCommandResult::new(
            Some(cleanup),
            vec![connection_mutation(
                context,
                MutationOperation::Delete,
                &delete.entity.workspace_id,
                &delete.entity.entity_id,
                revision,
            )],
        ))
    }

    pub async fn delete_workspace_connections_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: &str,
        deleted_at: &str,
    ) -> AppResult<(Vec<DomainMutation>, Vec<DatabaseConnectionCleanup>)> {
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT id, credential_ref FROM connections
            WHERE workspace_id = ?1 AND connection_type = 'database' AND deleted_at IS NULL
            ORDER BY id
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        let mut mutations = Vec::with_capacity(rows.len());
        let mut cleanups = Vec::with_capacity(rows.len());
        for (connection_id, credential_ref) in rows {
            let revision: i64 = sqlx::query_scalar(
                r#"
                UPDATE connections
                SET deleted_at = ?1, updated_at = ?1, revision = revision + 1,
                    sync_status = 'deleted'
                WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
                RETURNING revision
                "#,
            )
            .bind(deleted_at)
            .bind(workspace_id)
            .bind(&connection_id)
            .fetch_one(&mut *connection)
            .await?;
            clear_saved_sql_connection_on(connection, workspace_id, &connection_id, deleted_at)
                .await?;
            mutations.push(connection_mutation(
                context,
                MutationOperation::Delete,
                workspace_id,
                &connection_id,
                revision,
            ));
            cleanups.push(DatabaseConnectionCleanup::new(
                workspace_id.to_string(),
                credential_ref,
            ));
        }
        Ok((mutations, cleanups))
    }

    pub async fn cleanup_connection_changes(&self, cleanups: Vec<DatabaseConnectionCleanup>) {
        let Some(secret_store) = &self.secret_store else {
            return;
        };
        for cleanup in cleanups {
            if let Some(credential_ref) = cleanup
                .credential_ref
                .filter(|credential_ref| !credential_ref.is_empty())
            {
                let _ = secret_store
                    .delete_credential(cleanup.workspace_id, credential_ref)
                    .await;
            }
        }
    }
}
