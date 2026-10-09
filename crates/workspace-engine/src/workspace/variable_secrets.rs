use super::*;
use unfour_core::models::WorkspaceVariableInput;

const PREFIX: &str = "@unfour-secret:";

impl WorkspaceService {
    pub async fn bundle_workspace_live_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<bool> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=? AND deleted_at IS NULL)",
        )
        .bind(workspace)
        .fetch_one(db)
        .await?)
    }
    pub async fn is_bundle_credential(&self, workspace: &str, reference: &str) -> AppResult<bool> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspace_bundle_credential_journal WHERE workspace_id=? AND credential_ref=?)")
            .bind(workspace).bind(reference).fetch_one(self.db.pool()).await?)
    }
    pub async fn adopt_bundle_credential(&self, workspace: &str, reference: &str) -> AppResult<()> {
        let mut db = self.db.pool().acquire().await?;
        self.track_bundle_reference_on(&mut db, workspace, reference, "attached")
            .await
    }
    /// Stage replacement values outside the business transaction. Every keychain
    /// write has a durable reference first, including writes that later fail.
    pub async fn stage_variable_secret_edits(
        &self,
        workspace: &str,
        environment: Option<&str>,
        inputs: &mut [WorkspaceVariableInput],
    ) -> AppResult<Vec<String>> {
        let mut db = self.db.pool().acquire().await?;
        get_workspace_on(&mut db, workspace, false).await?;
        super::variables::validate_variables(inputs)?;
        let previous: Vec<(String, String, bool)> = match environment {
            Some(environment) => sqlx::query_as("SELECT id, value, is_secret FROM workspace_environment_variables WHERE workspace_id=? AND environment_id=? AND deleted_at IS NULL")
                .bind(workspace).bind(environment).fetch_all(&mut *db).await?,
            None => sqlx::query_as("SELECT id, value, is_secret FROM workspace_variables WHERE workspace_id=? AND deleted_at IS NULL")
                .bind(workspace).fetch_all(&mut *db).await?,
        };
        drop(db);
        let mut staged = Vec::new();
        let result: AppResult<()> = async {
            for input in inputs {
                let old = previous
                    .iter()
                    .find(|(id, _, _)| input.id.as_ref() == Some(id));
                if input.value.is_empty()
                    || input.value.starts_with(PREFIX)
                    || !old.is_some_and(|(_, value, secret)| {
                        value.starts_with(PREFIX) || (*secret && value.is_empty())
                    })
                {
                    continue;
                }
                let store = self
                    .secret_store
                    .as_ref()
                    .ok_or_else(|| AppError::Config("credential store unavailable".into()))?;
                let kind = if environment.is_some() {
                    "environment-variable"
                } else {
                    "workspace-variable"
                };
                let reference = store.make_ref(workspace, kind, &unfour_core::id::new_id());
                self.journal_bundle_credential(workspace, &reference)
                    .await?;
                staged.push(reference.clone());
                store
                    .rotate_credential(workspace.into(), reference.clone(), input.value.clone())
                    .await?;
                input.value = format!("{PREFIX}{reference}");
                input.is_secret = true;
            }
            Ok(())
        }
        .await;
        if let Err(error) = result {
            // Only our fresh, unpublished references can be deleted here.
            self.discard_staged_credentials(&staged).await?;
            for reference in &staged {
                if let Some(store) = &self.secret_store {
                    if matches!(
                        store
                            .delete_credential(workspace.into(), reference.clone())
                            .await,
                        Ok(()) | Err(AppError::NotFound(_))
                    ) {
                        sqlx::query("DELETE FROM workspace_bundle_credential_journal WHERE credential_ref=?")
                            .bind(reference).execute(self.db.pool()).await?;
                    }
                }
            }
            return Err(error);
        }
        Ok(staged)
    }

    pub async fn discard_bundle_stages(&self, workspace: &str) -> AppResult<()> {
        sqlx::query("UPDATE workspace_bundle_credential_journal SET state='garbage' WHERE workspace_id=? AND state='staged'")
            .bind(workspace).execute(self.db.pool()).await?;
        Ok(())
    }

    pub async fn discard_staged_credentials(&self, references: &[String]) -> AppResult<()> {
        for reference in references {
            sqlx::query("UPDATE workspace_bundle_credential_journal SET state='garbage' WHERE credential_ref=? AND state='staged'")
                .bind(reference).execute(self.db.pool()).await?;
        }
        Ok(())
    }

    pub async fn bundle_variable_references_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<Vec<String>> {
        let values: Vec<String> = sqlx::query_scalar("SELECT value FROM workspace_variables WHERE workspace_id=? AND deleted_at IS NULL UNION ALL SELECT value FROM workspace_environment_variables WHERE workspace_id=? AND deleted_at IS NULL")
            .bind(workspace).bind(workspace).fetch_all(&mut *db).await?;
        Ok(values
            .into_iter()
            .filter_map(|value| value.strip_prefix(PREFIX).map(str::to_owned))
            .collect())
    }

    pub async fn bundle_credential_workspaces_on(
        &self,
        db: &mut SqliteConnection,
        include_legacy: bool,
    ) -> AppResult<Vec<String>> {
        let query = if include_legacy {
            "SELECT id FROM workspaces UNION SELECT workspace_id FROM workspace_bundle_credential_journal"
        } else {
            "SELECT DISTINCT workspace_id FROM workspace_bundle_credential_journal"
        };
        Ok(sqlx::query_scalar(query).fetch_all(db).await?)
    }

    pub async fn bundle_credentials_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<Vec<(String, String)>> {
        Ok(sqlx::query_as("SELECT credential_ref, state FROM workspace_bundle_credential_journal WHERE workspace_id=?")
            .bind(workspace).fetch_all(db).await?)
    }

    pub async fn track_bundle_reference_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
        reference: &str,
        state: &str,
    ) -> AppResult<()> {
        sqlx::query("INSERT INTO workspace_bundle_credential_journal (workspace_id, credential_ref, state) VALUES (?,?,?) ON CONFLICT(credential_ref) DO UPDATE SET state=excluded.state")
            .bind(workspace).bind(reference).bind(state).execute(db).await?;
        Ok(())
    }

    pub async fn forget_bundle_reference_on(
        &self,
        db: &mut SqliteConnection,
        reference: &str,
    ) -> AppResult<()> {
        sqlx::query("DELETE FROM workspace_bundle_credential_journal WHERE credential_ref=?")
            .bind(reference)
            .execute(db)
            .await?;
        Ok(())
    }
}
