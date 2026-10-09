use super::*;
impl WorkspaceService {
    /// Resolve JSON string leaves, then serialize, so quotes in credentials cannot alter auth structure.
    pub async fn resolve_json_variables(
        &self,
        workspace: &str,
        environment: Option<&str>,
        input: &str,
        overrides: &[unfour_core::models::KeyValue],
    ) -> AppResult<String> {
        self.variable_resolver(workspace, environment, overrides)
            .await?
            .resolve_json(input)
    }
    pub async fn journal_bundle_credential(
        &self,
        workspace: &str,
        reference: &str,
    ) -> AppResult<()> {
        sqlx::query("INSERT INTO workspace_bundle_credential_journal (workspace_id, credential_ref) VALUES (?,?)")
            .bind(workspace).bind(reference).execute(self.db.pool()).await?;
        Ok(())
    }
    pub async fn attach_bundle_credentials_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE workspace_bundle_credential_journal SET state='attached' WHERE workspace_id=?",
        )
        .bind(workspace)
        .execute(&mut *db)
        .await?;
        Ok(())
    }
    pub async fn bundle_variable_values_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<Vec<(String, String, String)>> {
        let mut values = Vec::new();
        for (table, kind) in [
            ("workspace_variables", "variable"),
            ("workspace_environment_variables", "environmentVariable"),
        ] {
            let rows: Vec<(String, String, String, bool)> = sqlx::query_as(&format!("SELECT id, key, value, is_secret FROM {table} WHERE workspace_id=? AND deleted_at IS NULL"))
                .bind(workspace).fetch_all(&mut *db).await?;
            for (id, key, value, secret) in rows {
                if unfour_core::redaction::is_sensitive_workspace_variable(&key, &value, secret)
                    && !value.is_empty()
                {
                    values.push((id, kind.into(), value));
                }
            }
        }
        Ok(values)
    }
    pub async fn restore_bundle_variable_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
        id: &str,
        kind: &str,
        value: &str,
    ) -> AppResult<()> {
        let table = match kind {
            "variable" => "workspace_variables",
            "environmentVariable" => "workspace_environment_variables",
            _ => return Err(AppError::Validation("WORKSPACE_BUNDLE_INVALID".into())),
        };
        let result = sqlx::query(&format!("UPDATE {table} SET value=?, is_secret=1 WHERE workspace_id=? AND id=? AND deleted_at IS NULL"))
            .bind(value).bind(workspace).bind(id).execute(&mut *db).await?;
        if result.rows_affected() != 1 {
            return Err(AppError::Validation(
                "WORKSPACE_BUNDLE_MISSING_REFERENCE".into(),
            ));
        }
        Ok(())
    }
}
