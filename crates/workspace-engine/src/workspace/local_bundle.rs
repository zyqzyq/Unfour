use super::*;
const PREFIX: &str = "@unfour-secret:";
impl WorkspaceService {
    /// Called by primary application startup before exposing import commands, never by satellite adapters.
    pub async fn recover_bundle_credentials(&self) -> AppResult<()> {
        let workspaces: Vec<String> = sqlx::query_scalar("SELECT DISTINCT workspace_id FROM workspace_bundle_credential_journal WHERE NOT EXISTS (SELECT 1 FROM workspaces WHERE workspaces.id=workspace_bundle_credential_journal.workspace_id)")
            .fetch_all(self.db.pool()).await?;
        for workspace in workspaces {
            self.cleanup_bundle_credentials(&workspace).await?;
        }
        Ok(())
    }
    /// Resolve JSON string leaves, then serialize, so quotes in credentials cannot alter auth structure.
    pub async fn resolve_json_variables(
        &self,
        workspace: &str,
        environment: Option<&str>,
        input: &str,
        overrides: &[unfour_core::models::KeyValue],
    ) -> AppResult<String> {
        let mut json: serde_json::Value = serde_json::from_str(input)?;
        let mut leaves = Vec::new();
        fn collect(value: &serde_json::Value, pointer: String, leaves: &mut Vec<(String, String)>) {
            match value {
                serde_json::Value::String(text) => leaves.push((pointer, text.clone())),
                serde_json::Value::Object(map) => {
                    for (key, child) in map {
                        collect(
                            child,
                            format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1")),
                            leaves,
                        );
                    }
                }
                serde_json::Value::Array(items) => {
                    for (i, child) in items.iter().enumerate() {
                        collect(child, format!("{pointer}/{i}"), leaves);
                    }
                }
                _ => {}
            }
        }
        collect(&json, String::new(), &mut leaves);
        for (pointer, text) in leaves {
            let resolved = self
                .resolve_variables_with_overrides(workspace, environment, &text, overrides)
                .await?;
            *json.pointer_mut(&pointer).expect("collected JSON leaf") = resolved.into();
        }
        Ok(serde_json::to_string(&json)?)
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
    pub async fn clear_bundle_journal_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<()> {
        sqlx::query("DELETE FROM workspace_bundle_credential_journal WHERE workspace_id=?")
            .bind(workspace)
            .execute(&mut *db)
            .await?;
        Ok(())
    }
    pub async fn cleanup_bundle_credentials(&self, workspace: &str) -> AppResult<()> {
        let references: Vec<String> = sqlx::query_scalar(
            "SELECT credential_ref FROM workspace_bundle_credential_journal WHERE workspace_id=?",
        )
        .bind(workspace)
        .fetch_all(self.db.pool())
        .await?;
        let store = self
            .secret_store
            .as_ref()
            .ok_or_else(|| AppError::Config("credential store unavailable".into()))?;
        for reference in references {
            match store
                .delete_credential(workspace.into(), reference.clone())
                .await
            {
                Ok(()) | Err(AppError::NotFound(_)) => {
                    sqlx::query("DELETE FROM workspace_bundle_credential_journal WHERE workspace_id=? AND credential_ref=?")
                        .bind(workspace).bind(reference).execute(self.db.pool()).await?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
    pub async fn resolve_secret_value(
        &self,
        workspace: &str,
        value: &str,
        secret: bool,
    ) -> AppResult<String> {
        if let Some(reference) = value.strip_prefix(PREFIX).filter(|_| secret) {
            self.secret_store
                .as_ref()
                .ok_or_else(|| AppError::Config("credential store unavailable".into()))?
                .read_secret(workspace.into(), reference.into())
                .await
        } else {
            Ok(value.into())
        }
    }
    /// Keep edits to imported credentials in the credential store as well.
    pub(super) async fn preserve_imported_variable_secret(
        &self,
        workspace: &str,
        previous: &str,
        input: &mut unfour_core::models::WorkspaceVariableInput,
    ) -> AppResult<()> {
        if let Some(reference) = previous.strip_prefix(PREFIX) {
            if input.value.is_empty() {
                return Ok(());
            }
            if input.value != previous {
                let store = self
                    .secret_store
                    .as_ref()
                    .ok_or_else(|| AppError::Config("credential store unavailable".into()))?;
                let metadata = store
                    .inspect_credential(workspace.into(), reference.into())
                    .await?;
                // Allocate a replacement so a later SQLite rollback cannot change the old value.
                let created = store
                    .create_credential(
                        workspace.into(),
                        metadata.kind,
                        "Workspace variable".into(),
                        input.value.clone(),
                    )
                    .await?;
                input.value = format!("{PREFIX}{}", created.credential_ref);
            } else {
                input.value = previous.into();
            }
            input.is_secret = true;
        }
        Ok(())
    }
    pub(super) async fn preserve_imported_variable_edits(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
        environment: Option<&str>,
        inputs: &mut [unfour_core::models::WorkspaceVariableInput],
    ) -> AppResult<()> {
        let values: Vec<(String, String)> = if let Some(environment) = environment {
            sqlx::query_as("SELECT id, value FROM workspace_environment_variables WHERE workspace_id=? AND environment_id=?")
                .bind(workspace).bind(environment).fetch_all(&mut *db).await?
        } else {
            sqlx::query_as("SELECT id, value FROM workspace_variables WHERE workspace_id=?")
                .bind(workspace)
                .fetch_all(&mut *db)
                .await?
        };
        for input in inputs {
            if let Some((_, previous)) = values.iter().find(|(id, _)| input.id.as_ref() == Some(id))
            {
                self.preserve_imported_variable_secret(workspace, previous, input)
                    .await?;
            }
        }
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
                if (secret || unfour_core::redaction::is_sensitive_flow_name(&key))
                    && !value.is_empty()
                {
                    values.push((
                        id,
                        kind.into(),
                        self.resolve_secret_value(workspace, &value, secret).await?,
                    ));
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
        reference: &str,
    ) -> AppResult<()> {
        let table = match kind {
            "variable" => "workspace_variables",
            "environmentVariable" => "workspace_environment_variables",
            _ => return Err(AppError::Validation("WORKSPACE_BUNDLE_INVALID".into())),
        };
        let result = sqlx::query(&format!("UPDATE {table} SET value=?, is_secret=1 WHERE workspace_id=? AND id=? AND deleted_at IS NULL"))
            .bind(format!("{PREFIX}{reference}")).bind(workspace).bind(id).execute(&mut *db).await?;
        if result.rows_affected() != 1 {
            return Err(AppError::Validation(
                "WORKSPACE_BUNDLE_MISSING_REFERENCE".into(),
            ));
        }
        Ok(())
    }
}
