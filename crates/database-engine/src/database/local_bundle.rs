use super::*;
use sqlx::SqliteConnection;

impl DatabaseService {
    pub async fn bundle_connection_fields_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<Vec<(String, Option<String>, Option<String>)>> {
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as("SELECT c.id, d.config_json, c.credential_ref FROM connections c JOIN database_connections d ON d.connection_id=c.id WHERE c.workspace_id=? AND c.deleted_at IS NULL")
            .bind(workspace).fetch_all(&mut *db).await?;
        rows.into_iter()
            .map(|(id, config, credential)| {
                let config: DatabaseConnectionConfig = serde_json::from_str(&config)?;
                Ok((id, config.sqlite_path, credential))
            })
            .collect()
    }
    pub async fn restore_bundle_path_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
        id: &str,
        path: &str,
    ) -> AppResult<()> {
        let result = sqlx::query("UPDATE database_connections SET config_json=json_set(config_json,'$.sqlitePath',?) WHERE driver='sqlite' AND connection_id IN (SELECT id FROM connections WHERE id=? AND workspace_id=? AND deleted_at IS NULL)")
            .bind(path).bind(id).bind(workspace).execute(&mut *db).await?;
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
        let store = self
            .secret_store
            .as_ref()
            .ok_or_else(|| AppError::Config("credential store unavailable".into()))?;
        store
            .inspect_credential(workspace.into(), credential.into())
            .await?;
        let result = sqlx::query("UPDATE connections SET credential_ref=? WHERE id=? AND workspace_id=? AND connection_type='database' AND deleted_at IS NULL")
            .bind(credential).bind(id).bind(workspace).execute(&mut *db).await?;
        if result.rows_affected() != 1 {
            return Err(AppError::Validation(
                "WORKSPACE_BUNDLE_MISSING_REFERENCE".into(),
            ));
        }
        Ok(())
    }
}
