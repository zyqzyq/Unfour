use super::*;

impl CommandBus {
    /// Desktop editor only. Resolve a live saved connection, never an arbitrary
    /// credential reference. No command dispatch or MCP tool exposes this read.
    pub async fn reveal_connection_secret(
        &self,
        workspace_id: String,
        connection_id: String,
        connection_type: String,
        credential_ref: String,
    ) -> AppResult<String> {
        if !matches!(connection_type.as_str(), "ssh" | "database") {
            return Err(unfour_core::AppError::Validation(
                "CONNECTION_CREDENTIAL_UNAVAILABLE".into(),
            ));
        }
        let mut guard = self.db.pool().begin_with("BEGIN IMMEDIATE").await?;
        let current: Option<String> = sqlx::query_scalar(
            "SELECT credential_ref FROM connections WHERE workspace_id=? AND id=? AND connection_type=? AND deleted_at IS NULL",
        ).bind(&workspace_id).bind(&connection_id).bind(&connection_type)
            .fetch_optional(&mut *guard).await?.flatten();
        let reference = current
            .filter(|value| value == &credential_ref)
            .ok_or_else(|| {
                unfour_core::AppError::Validation("CONNECTION_CREDENTIAL_UNAVAILABLE".into())
            })?;
        let result = self
            .secret_store
            .read_secret(workspace_id, reference)
            .await
            .map_err(|_| {
                unfour_core::AppError::Validation("CONNECTION_CREDENTIAL_UNAVAILABLE".into())
            });
        guard.rollback().await?;
        result
    }

    pub async fn create_credential(
        &self,
        input: CredentialCreateInput,
    ) -> AppResult<CredentialMetadata> {
        let credential = self
            .secret_store
            .create_credential(input.workspace_id, input.kind, input.label, input.secret)
            .await?;
        self.activity_log
            .record(
                Some(&credential.workspace_id),
                "credential.create",
                Some(&credential.credential_ref),
                serde_json::json!({
                    "kind": credential.kind,
                    "label": credential.label,
                    "secretStored": true
                }),
            )
            .await?;
        Ok(credential)
    }

    pub async fn delete_credential(&self, input: CredentialDeleteInput) -> AppResult<()> {
        let guard = self.db.pool().begin_with("BEGIN IMMEDIATE").await?;
        self.secret_store
            .delete_credential(input.workspace_id.clone(), input.credential_ref.clone())
            .await?;
        guard.rollback().await?;
        self.activity_log
            .record(
                Some(&input.workspace_id),
                "credential.delete",
                Some(&input.credential_ref),
                serde_json::json!({ "deleted": true }),
            )
            .await?;
        Ok(())
    }

    pub async fn inspect_credential(
        &self,
        input: CredentialInspectInput,
    ) -> AppResult<CredentialMetadata> {
        self.secret_store
            .inspect_credential(input.workspace_id, input.credential_ref)
            .await
    }

    pub async fn rotate_credential(
        &self,
        input: CredentialRotateInput,
    ) -> AppResult<CredentialMetadata> {
        let guard = self.db.pool().begin_with("BEGIN IMMEDIATE").await?;
        let credential = self
            .secret_store
            .rotate_credential(input.workspace_id, input.credential_ref, input.secret)
            .await?;
        guard.rollback().await?;
        self.activity_log
            .record(
                Some(&credential.workspace_id),
                "credential.rotate",
                Some(&credential.credential_ref),
                serde_json::json!({
                    "kind": credential.kind,
                    "secretStored": true
                }),
            )
            .await?;
        Ok(credential)
    }
}
