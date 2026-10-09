use super::*;
const PREFIX: &str = "@unfour-secret:";
impl WorkspaceService {
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
    /// Keychain writes are staged before entering the caller's business transaction.
    pub(super) async fn preserve_imported_variable_secret(
        &self,
        workspace: &str,
        previous: &str,
        input: &mut unfour_core::models::WorkspaceVariableInput,
    ) -> AppResult<()> {
        if previous.starts_with(PREFIX) {
            input.is_secret = true;
        }
        if let Some(reference) = input.value.strip_prefix(PREFIX) {
            self.secret_store
                .as_ref()
                .ok_or_else(|| AppError::Config("credential store unavailable".into()))?
                .inspect_credential(workspace.into(), reference.into())
                .await?;
            input.is_secret = true;
        } else if previous.starts_with(PREFIX) && !input.value.is_empty() {
            return Err(AppError::Validation(
                "workspace secret edit was not staged".into(),
            ));
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
                if unfour_core::redaction::is_sensitive_workspace_variable(&key, &value, secret)
                    && !value.is_empty()
                {
                    values.push((
                        id,
                        kind.into(),
                        self.resolve_secret_value(workspace, &value, true).await?,
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
