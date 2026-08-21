use sqlx::SqliteConnection;
use unfour_core::domain::{CommandContext, DomainCommandResult, MutationOperation};

use super::connection_domain::{connection_mutation, SshConnectionCleanup};
use super::*;

pub struct PreparedSshConnectionSave {
    input: Option<SshConnectionInput>,
    secret_rollback: Option<SshConnectionSecretRollback>,
}

enum SshConnectionSecretRollback {
    DeleteCreated {
        workspace_id: String,
        credential_ref: String,
    },
    RestoreRotated {
        workspace_id: String,
        credential_ref: String,
        previous_secret: String,
    },
}

impl PreparedSshConnectionSave {
    pub fn take_transaction_input(&mut self) -> SshConnectionInput {
        self.input
            .take()
            .expect("prepared SSH connection input can only be consumed once")
    }
}

impl SshService {
    pub async fn list_connections(&self, workspace_id: String) -> AppResult<Vec<SshConnection>> {
        let mut connection = self.db.pool().acquire().await?;
        self.list_connections_on(&mut connection, &workspace_id)
            .await
    }

    pub async fn list_connections_on(
        &self,
        connection: &mut SqliteConnection,
        workspace_id: &str,
    ) -> AppResult<Vec<SshConnection>> {
        validate_workspace_id(workspace_id)?;
        let rows = sqlx::query_as::<_, StoredSshConnection>(
            r#"
            SELECT
              c.id, c.workspace_id, c.name, c.host, c.port,
              sub.username, sub.auth_method, sub.config_json, c.credential_ref,
              c.created_at, c.updated_at, c.deleted_at, c.revision, c.sync_status, c.remote_id
            FROM connections c
            INNER JOIN ssh_connections sub ON sub.connection_id = c.id
            WHERE c.workspace_id = ?1 AND c.connection_type = 'ssh' AND c.deleted_at IS NULL
            ORDER BY c.updated_at DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        rows.into_iter().map(stored_to_ssh_connection).collect()
    }

    pub async fn save_connection(&self, input: SshConnectionInput) -> AppResult<SshConnection> {
        let mut prepared = self.prepare_connection_save(input).await?;
        let transaction_input = prepared.take_transaction_input();
        let context = CommandContext::local("ssh.connection.save");
        let result: AppResult<SshConnection> = async {
            let mut transaction = self.db.pool().begin().await?;
            let outcome = self
                .save_connection_on(&mut transaction, &context, transaction_input)
                .await?;
            transaction.commit().await?;
            Ok(outcome.value)
        }
        .await;
        match result {
            Ok(connection) => Ok(connection),
            Err(error) => {
                if let Err(rollback_error) = self.rollback_connection_save(prepared).await {
                    return Err(AppError::Config(format!(
                        "ssh connection save failed ({error}); secret rollback failed ({rollback_error})"
                    )));
                }
                Err(error)
            }
        }
    }

    /// Validate and stage any keychain change before a SQLite transaction is
    /// opened. The returned transaction input never contains plaintext secret
    /// material, and the rollback token can compensate a failed transaction.
    pub async fn prepare_connection_save(
        &self,
        mut input: SshConnectionInput,
    ) -> AppResult<PreparedSshConnectionSave> {
        validate_workspace_id(&input.workspace_id)?;
        normalize_name(&input.name)?;
        let storage = input_to_storage(&input)?;
        let existing_ref = empty_to_none(input.credential_ref.take());
        let secret = input.secret.take().filter(|value| !value.is_empty());
        let (credential_ref, secret_rollback) = match storage.auth_method.as_str() {
            "none" => (None, None),
            "password" => match secret {
                Some(secret) => {
                    let (credential_ref, rollback) = self
                        .prepare_secret_change(
                            &input.workspace_id,
                            "ssh-password",
                            existing_ref,
                            secret,
                        )
                        .await?;
                    (Some(credential_ref), Some(rollback))
                }
                None => match existing_ref {
                    Some(existing) => (Some(existing), None),
                    None => {
                        return Err(AppError::Validation(
                            "password ssh auth requires a password".to_string(),
                        ));
                    }
                },
            },
            "private-key" => match secret {
                Some(secret) => {
                    let (credential_ref, rollback) = self
                        .prepare_secret_change(
                            &input.workspace_id,
                            "ssh-key-passphrase",
                            existing_ref,
                            secret,
                        )
                        .await?;
                    (Some(credential_ref), Some(rollback))
                }
                None => (existing_ref, None),
            },
            _ => (existing_ref, None),
        };
        input.credential_ref = credential_ref;

        Ok(PreparedSshConnectionSave {
            input: Some(input),
            secret_rollback,
        })
    }

    pub async fn rollback_connection_save(
        &self,
        prepared: PreparedSshConnectionSave,
    ) -> AppResult<()> {
        let Some(rollback) = prepared.secret_rollback else {
            return Ok(());
        };
        match rollback {
            SshConnectionSecretRollback::DeleteCreated {
                workspace_id,
                credential_ref,
            } => match self
                .secret_store
                .delete_credential(workspace_id, credential_ref)
                .await
            {
                Ok(()) | Err(AppError::NotFound(_)) => Ok(()),
                Err(error) => Err(error),
            },
            SshConnectionSecretRollback::RestoreRotated {
                workspace_id,
                credential_ref,
                previous_secret,
            } => {
                self.secret_store
                    .rotate_credential(workspace_id, credential_ref, previous_secret)
                    .await?;
                Ok(())
            }
        }
    }

    async fn prepare_secret_change(
        &self,
        workspace_id: &str,
        kind: &str,
        existing_ref: Option<String>,
        secret: String,
    ) -> AppResult<(String, SshConnectionSecretRollback)> {
        if let Some(credential_ref) = existing_ref {
            let rollback = match self
                .secret_store
                .read_secret(workspace_id.to_string(), credential_ref.clone())
                .await
            {
                Ok(previous_secret) => SshConnectionSecretRollback::RestoreRotated {
                    workspace_id: workspace_id.to_string(),
                    credential_ref: credential_ref.clone(),
                    previous_secret,
                },
                Err(AppError::NotFound(_)) => SshConnectionSecretRollback::DeleteCreated {
                    workspace_id: workspace_id.to_string(),
                    credential_ref: credential_ref.clone(),
                },
                Err(error) => return Err(error),
            };
            self.secret_store
                .rotate_credential(workspace_id.to_string(), credential_ref.clone(), secret)
                .await?;
            Ok((credential_ref, rollback))
        } else {
            let metadata = self
                .secret_store
                .create_credential(
                    workspace_id.to_string(),
                    kind.to_string(),
                    format!("ssh {kind} credential"),
                    secret,
                )
                .await?;
            let credential_ref = metadata.credential_ref;
            Ok((
                credential_ref.clone(),
                SshConnectionSecretRollback::DeleteCreated {
                    workspace_id: workspace_id.to_string(),
                    credential_ref,
                },
            ))
        }
    }

    pub async fn save_connection_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        input: SshConnectionInput,
    ) -> AppResult<DomainCommandResult<SshConnection>> {
        validate_workspace_id(&input.workspace_id)?;
        let name = normalize_name(&input.name)?;
        let storage = input_to_storage(&input)?;
        if input
            .secret
            .as_ref()
            .is_some_and(|secret| !secret.is_empty())
        {
            return Err(AppError::Validation(
                "plaintext SSH secret must be prepared outside the database transaction"
                    .to_string(),
            ));
        }
        let credential_ref = match storage.auth_method.as_str() {
            "none" => None,
            "password" => Some(empty_to_none(input.credential_ref.clone()).ok_or_else(|| {
                AppError::Validation(
                    "password ssh auth requires a credential reference".to_string(),
                )
            })?),
            _ => empty_to_none(input.credential_ref.clone()),
        };
        let now = Utc::now().to_rfc3339();
        let config_json = ssh_config_to_json(&storage.config)?;

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
                WHERE id = ?6 AND workspace_id = ?7
                  AND connection_type = 'ssh' AND deleted_at IS NULL
                RETURNING revision
                "#,
            )
            .bind(name)
            .bind(&storage.host)
            .bind(i64::from(storage.port))
            .bind(credential_ref)
            .bind(&now)
            .bind(id)
            .bind(&input.workspace_id)
            .fetch_optional(&mut *connection)
            .await?;
            let revision =
                revision.ok_or_else(|| AppError::NotFound("ssh connection".to_string()))?;
            let subtype = sqlx::query(
                r#"
                UPDATE ssh_connections
                SET username = ?1, auth_method = ?2, config_json = ?3
                WHERE connection_id = ?4
                "#,
            )
            .bind(&storage.username)
            .bind(&storage.auth_method)
            .bind(&config_json)
            .bind(id)
            .execute(&mut *connection)
            .await?;
            if subtype.rows_affected() != 1 {
                return Err(AppError::Config(
                    "ssh connection subtype row is missing".to_string(),
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
                ) VALUES (?1, ?2, 'ssh', ?3, ?4, ?5, ?6, ?7, ?7, 1, 'local')
                "#,
            )
            .bind(&id)
            .bind(&input.workspace_id)
            .bind(name)
            .bind(&storage.host)
            .bind(i64::from(storage.port))
            .bind(credential_ref)
            .bind(&now)
            .execute(&mut *connection)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO ssh_connections (connection_id, username, auth_method, config_json)
                VALUES (?1, ?2, ?3, ?4)
                "#,
            )
            .bind(&id)
            .bind(&storage.username)
            .bind(&storage.auth_method)
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
    ) -> AppResult<Vec<SshConnection>> {
        let context = CommandContext::local("ssh.connection.delete");
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
    ) -> AppResult<DomainCommandResult<(Vec<SshConnection>, SshConnectionCleanup)>> {
        validate_workspace_id(&workspace_id)?;
        validate_connection_id(&connection_id)?;
        let existing: Option<Option<String>> = sqlx::query_scalar(
            r#"
            SELECT credential_ref FROM connections
            WHERE id = ?1 AND workspace_id = ?2
              AND connection_type = 'ssh' AND deleted_at IS NULL
            "#,
        )
        .bind(&connection_id)
        .bind(&workspace_id)
        .fetch_optional(&mut *connection)
        .await?;
        let credential_ref =
            existing.ok_or_else(|| AppError::NotFound("ssh connection".to_string()))?;
        let now = Utc::now().to_rfc3339();
        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE connections
            SET deleted_at = ?1, updated_at = ?1,
                revision = revision + 1, sync_status = 'deleted'
            WHERE id = ?2 AND workspace_id = ?3
              AND connection_type = 'ssh' AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(&now)
        .bind(&connection_id)
        .bind(&workspace_id)
        .fetch_one(&mut *connection)
        .await?;
        let remaining = self.list_connections_on(connection, &workspace_id).await?;
        let cleanup = SshConnectionCleanup::deleted(
            workspace_id.clone(),
            connection_id.clone(),
            credential_ref,
        );
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

    pub(super) async fn get_connection_on(
        &self,
        connection: &mut SqliteConnection,
        workspace_id: &str,
        id: &str,
    ) -> AppResult<SshConnection> {
        validate_workspace_id(workspace_id)?;
        validate_connection_id(id)?;
        let row = sqlx::query_as::<_, StoredSshConnection>(
            r#"
            SELECT
              c.id, c.workspace_id, c.name, c.host, c.port,
              sub.username, sub.auth_method, sub.config_json, c.credential_ref,
              c.created_at, c.updated_at, c.deleted_at, c.revision, c.sync_status, c.remote_id
            FROM connections c
            INNER JOIN ssh_connections sub ON sub.connection_id = c.id
            WHERE c.id = ?1 AND c.workspace_id = ?2
              AND c.connection_type = 'ssh' AND c.deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(&mut *connection)
        .await?;
        row.map(stored_to_ssh_connection)
            .transpose()?
            .ok_or_else(|| AppError::NotFound("ssh connection".to_string()))
    }
}
