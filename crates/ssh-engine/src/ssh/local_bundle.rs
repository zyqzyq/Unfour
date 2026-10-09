//! Device-local fields for explicit workspace file exchange, never sync snapshots.
use super::*;
use sqlx::SqliteConnection;

impl SshService {
    pub async fn bundle_connection_fields_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<Vec<(String, Option<String>, Option<String>)>> {
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as("SELECT c.id, s.config_json, c.credential_ref FROM connections c JOIN ssh_connections s ON s.connection_id=c.id WHERE c.workspace_id=? AND c.deleted_at IS NULL")
            .bind(workspace).fetch_all(&mut *db).await?;
        rows.into_iter()
            .map(|(id, config, credential)| {
                let config: SshConnectionConfig = serde_json::from_str(&config)?;
                Ok((id, config.key_path, credential))
            })
            .collect()
    }
    pub async fn bundle_transfer_fields_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<Vec<(String, String, bool)>> {
        let rows: Vec<(String, String, bool)> = sqlx::query_as("SELECT id, config_json, enabled FROM ssh_task_step WHERE workspace_id=? AND deleted_at IS NULL AND step_type IN ('upload','download')")
            .bind(workspace).fetch_all(&mut *db).await?;
        rows.into_iter()
            .map(|(id, config, enabled)| {
                let config: serde_json::Value = serde_json::from_str(&config)?;
                Ok((
                    id,
                    config["localPath"].as_str().unwrap_or_default().into(),
                    enabled,
                ))
            })
            .collect()
    }
    pub async fn restore_bundle_path_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
        id: &str,
        field: &str,
        path: &str,
    ) -> AppResult<()> {
        let result = match field {
            "keyPath" => sqlx::query("UPDATE ssh_connections SET config_json=json_set(config_json,'$.keyPath',?) WHERE connection_id IN (SELECT id FROM connections WHERE id=? AND workspace_id=? AND deleted_at IS NULL)")
                .bind(path).bind(id).bind(workspace).execute(&mut *db).await?,
            "localPath" => sqlx::query("UPDATE ssh_task_step SET config_json=json_set(config_json,'$.localPath',?) WHERE id=? AND workspace_id=? AND deleted_at IS NULL AND step_type IN ('upload','download')")
                .bind(path).bind(id).bind(workspace).execute(&mut *db).await?,
            _ => return Err(AppError::Validation("WORKSPACE_BUNDLE_INVALID".into())),
        };
        if result.rows_affected() != 1 {
            return Err(AppError::Validation(
                "WORKSPACE_BUNDLE_MISSING_REFERENCE".into(),
            ));
        }
        Ok(())
    }
    pub async fn restore_bundle_credential_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
        id: &str,
        credential: &str,
    ) -> AppResult<()> {
        self.secret_store
            .inspect_credential(workspace.into(), credential.into())
            .await?;
        let result = sqlx::query("UPDATE connections SET credential_ref=? WHERE id=? AND workspace_id=? AND connection_type='ssh' AND deleted_at IS NULL")
            .bind(credential).bind(id).bind(workspace).execute(&mut *db).await?;
        if result.rows_affected() != 1 {
            return Err(AppError::Validation(
                "WORKSPACE_BUNDLE_MISSING_REFERENCE".into(),
            ));
        }
        Ok(())
    }
}
