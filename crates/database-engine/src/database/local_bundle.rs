use super::*;
use sqlx::SqliteConnection;

impl DatabaseService {
    pub async fn bundle_sql_routes_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<Vec<(String, String, bool)>> {
        let rows = sqlx::query_as("SELECT c.id, d.ssh_connection_id, jump.revision IS NOT d.ssh_revision
            FROM connections c JOIN database_connections d ON d.connection_id=c.id
            LEFT JOIN connections jump ON jump.id=d.ssh_connection_id AND jump.workspace_id=c.workspace_id AND jump.connection_type='ssh' AND jump.deleted_at IS NULL
            WHERE c.workspace_id=? AND c.deleted_at IS NULL AND d.ssh_connection_id IS NOT NULL ORDER BY c.id")
            .bind(workspace).fetch_all(db).await?;
        Ok(rows)
    }

    pub async fn restore_bundle_sql_route_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
        database: &str,
        jump: &str,
        needs_review: bool,
    ) -> AppResult<()> {
        let target = self.get_connection_on(db, workspace, database).await?;
        let revision = self
            .validate_sql_jump(db, workspace, &target.driver, Some(jump), None)
            .await?;
        // Imported IDs start a new revision history. An already stale route
        // must still require explicit review before its first SQL login.
        let pin = if needs_review { Some(-1) } else { revision };
        sqlx::query("UPDATE database_connections SET ssh_connection_id=?, ssh_revision=?, ssh_trust_generation=? WHERE connection_id=?")
            .bind(jump).bind(pin).bind(i64::from(needs_review)).bind(database).execute(db).await?;
        Ok(())
    }

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
