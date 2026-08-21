use sqlx::{FromRow, SqliteConnection};
use unfour_core::domain::{
    connection_mutation, validate_connection_domain_key, validate_external_connection_delete,
    validate_external_connection_upsert, CommandContext, ConnectionSnapshot,
    ConnectionSnapshotConfig, DomainCommandResult, DomainEntityKey, DomainMutation, DomainSnapshot,
    ExternalConnectionApply, ExternalConnectionUpsert, ExternalDelete, MutationOperation,
    MutationOrigin, TombstoneSnapshot,
};

use super::*;

pub struct SshConnectionCleanup {
    workspace_id: String,
    connection_id: String,
    credential_ref: Option<String>,
    cleanup_runtime: bool,
}

impl SshConnectionCleanup {
    pub(super) fn deleted(
        workspace_id: String,
        connection_id: String,
        credential_ref: Option<String>,
    ) -> Self {
        Self {
            workspace_id,
            connection_id,
            credential_ref,
            cleanup_runtime: true,
        }
    }

    fn credential_only(
        workspace_id: String,
        connection_id: String,
        credential_ref: String,
    ) -> Self {
        Self {
            workspace_id,
            connection_id,
            credential_ref: Some(credential_ref),
            cleanup_runtime: false,
        }
    }
}

#[derive(FromRow)]
struct SshSnapshotRow {
    id: String,
    workspace_id: String,
    connection_type: String,
    name: String,
    host: Option<String>,
    port: Option<i64>,
    username: String,
    auth_method: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    revision: i64,
}

#[derive(FromRow)]
struct CurrentExternalSshConnection {
    workspace_id: String,
    name: String,
    host: Option<String>,
    port: Option<i64>,
    credential_ref: Option<String>,
    username: String,
    auth_method: String,
    config_json: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    sync_status: String,
}

impl SshService {
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
        let row = sqlx::query_as::<_, SshSnapshotRow>(
            r#"
            SELECT c.id, c.workspace_id, c.connection_type, c.name, c.host, c.port,
                   sub.username, sub.auth_method, c.created_at, c.updated_at,
                   c.deleted_at, c.revision
            FROM connections c
            INNER JOIN ssh_connections sub ON sub.connection_id = c.id
            WHERE c.workspace_id = ?1 AND c.id = ?2 AND c.connection_type = 'ssh'
            "#,
        )
        .bind(&key.workspace_id)
        .bind(&key.entity_id)
        .fetch_optional(&mut *connection)
        .await?
        .ok_or_else(|| AppError::NotFound("ssh connection".to_string()))?;
        if let Some(deleted_at) = row.deleted_at {
            return Ok(DomainSnapshot::Tombstone(TombstoneSnapshot {
                entity: key.clone(),
                deleted_at,
                revision: row.revision,
            }));
        }
        let port = row
            .port
            .map(decode_ssh_port)
            .transpose()?
            .ok_or_else(|| AppError::Config("ssh connection port is missing".to_string()))?;
        Ok(DomainSnapshot::Connection(ConnectionSnapshot {
            id: row.id,
            workspace_id: row.workspace_id,
            connection_type: row.connection_type,
            name: row.name,
            host: row.host,
            port: Some(port),
            config: ConnectionSnapshotConfig::Ssh {
                username: row.username,
                auth_method: row.auth_method,
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
    ) -> AppResult<DomainCommandResult<Option<SshConnectionCleanup>>> {
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
    ) -> AppResult<DomainCommandResult<Option<SshConnectionCleanup>>> {
        validate_external_connection_upsert(&record)?;
        if record.connection_type != "ssh" {
            return Err(AppError::Validation(
                "external SSH connection must use connection_type=ssh".to_string(),
            ));
        }
        let ConnectionSnapshotConfig::Ssh {
            username,
            auth_method,
        } = record.config
        else {
            return Err(AppError::Validation(
                "external SSH connection requires SSH config".to_string(),
            ));
        };
        validate_live_workspace_on(connection, &record.workspace_id).await?;
        let name = normalize_name(&record.name)?;
        let host = record
            .host
            .as_deref()
            .ok_or_else(|| AppError::Validation("external SSH host is required".to_string()))?;
        let host = normalize_required(host, "ssh host")?;
        let port = record
            .port
            .filter(|port| *port != 0)
            .ok_or_else(|| AppError::Validation("external SSH port is required".to_string()))?;
        let username = normalize_required(&username, "ssh username")?;
        let auth_method = auth_method.trim().to_ascii_lowercase();
        if !matches!(auth_method.as_str(), "password" | "private-key" | "none") {
            return Err(AppError::Validation(format!(
                "unsupported external SSH auth method: {auth_method}"
            )));
        }

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
            if connection_type != "ssh" {
                return Err(AppError::Validation(
                    "external connection cannot change an existing connection type".to_string(),
                ));
            }
            let current = sqlx::query_as::<_, CurrentExternalSshConnection>(
                r#"
                SELECT c.workspace_id, c.name, c.host, c.port, c.credential_ref,
                       sub.username, sub.auth_method, sub.config_json,
                       c.created_at, c.updated_at, c.deleted_at, c.sync_status
                FROM connections c
                INNER JOIN ssh_connections sub ON sub.connection_id = c.id
                WHERE c.id = ?1
                "#,
            )
            .bind(&record.id)
            .fetch_one(&mut *connection)
            .await?;
            let current_config = parse_ssh_config(&record.id, &current.config_json)?;
            let compatible_auth = current.auth_method == auth_method;
            let next_credential_ref = if compatible_auth && auth_method != "none" {
                current.credential_ref.clone()
            } else {
                None
            };
            let next_config = if compatible_auth && auth_method == "private-key" {
                current_config
            } else {
                SshConnectionConfig { key_path: None }
            };
            let cleanup = current
                .credential_ref
                .clone()
                .filter(|current_ref| Some(current_ref) != next_credential_ref.as_ref())
                .map(|credential_ref| {
                    SshConnectionCleanup::credential_only(
                        record.workspace_id.clone(),
                        record.id.clone(),
                        credential_ref,
                    )
                });
            let next_config_json = ssh_config_to_json(&next_config)?;
            if current.deleted_at.is_none()
                && current.workspace_id == record.workspace_id
                && current.name == name
                && current.host.as_deref() == Some(host.as_str())
                && current.port == Some(i64::from(port))
                && current.credential_ref == next_credential_ref
                && current.username == username
                && current.auth_method == auth_method
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
                WHERE id = ?7 AND workspace_id = ?8 AND connection_type = 'ssh'
                RETURNING revision
                "#,
            )
            .bind(name)
            .bind(host)
            .bind(i64::from(port))
            .bind(next_credential_ref)
            .bind(&record.created_at)
            .bind(&record.updated_at)
            .bind(&record.id)
            .bind(&record.workspace_id)
            .fetch_one(&mut *connection)
            .await?;
            sqlx::query(
                r#"
                UPDATE ssh_connections
                SET username = ?1, auth_method = ?2, config_json = ?3
                WHERE connection_id = ?4
                "#,
            )
            .bind(username)
            .bind(auth_method)
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

        let empty_config = ssh_config_to_json(&SshConnectionConfig { key_path: None })?;
        sqlx::query(
            r#"
            INSERT INTO connections (
              id, workspace_id, connection_type, name, host, port, credential_ref,
              created_at, updated_at, revision, sync_status
            ) VALUES (?1, ?2, 'ssh', ?3, ?4, ?5, NULL, ?6, ?7, 1, 'local')
            "#,
        )
        .bind(&record.id)
        .bind(&record.workspace_id)
        .bind(name)
        .bind(host)
        .bind(i64::from(port))
        .bind(&record.created_at)
        .bind(&record.updated_at)
        .execute(&mut *connection)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO ssh_connections (connection_id, username, auth_method, config_json)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(&record.id)
        .bind(username)
        .bind(auth_method)
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
    ) -> AppResult<DomainCommandResult<Option<SshConnectionCleanup>>> {
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
        if connection_type != "ssh" {
            return Err(AppError::Validation(
                "external SSH delete cannot target a database connection".to_string(),
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
        let cleanup = SshConnectionCleanup::deleted(
            delete.entity.workspace_id.clone(),
            delete.entity.entity_id.clone(),
            credential_ref,
        );
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
    ) -> AppResult<(Vec<DomainMutation>, Vec<SshConnectionCleanup>)> {
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT id, credential_ref FROM connections
            WHERE workspace_id = ?1 AND connection_type = 'ssh' AND deleted_at IS NULL
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
            mutations.push(connection_mutation(
                context,
                MutationOperation::Delete,
                workspace_id,
                &connection_id,
                revision,
            ));
            cleanups.push(SshConnectionCleanup::deleted(
                workspace_id.to_string(),
                connection_id,
                credential_ref,
            ));
        }
        Ok((mutations, cleanups))
    }

    pub async fn cleanup_connection_changes(&self, cleanups: Vec<SshConnectionCleanup>) {
        for cleanup in cleanups {
            if let Some(credential_ref) = cleanup
                .credential_ref
                .filter(|credential_ref| !credential_ref.is_empty())
            {
                let _ = self
                    .secret_store
                    .delete_credential(cleanup.workspace_id.clone(), credential_ref)
                    .await;
            }
            if cleanup.cleanup_runtime {
                let _ = self
                    .close_sessions_for_connection(&cleanup.workspace_id, &cleanup.connection_id)
                    .await;
                let _ = self
                    .terminal_history
                    .delete_connection_history(&cleanup.workspace_id, &cleanup.connection_id)
                    .await;
            }
        }
    }
}

async fn validate_live_workspace_on(
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
