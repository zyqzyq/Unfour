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
        self.reveal_connection_secret_with(
            &workspace_id,
            &connection_id,
            &connection_type,
            &credential_ref,
            self.secret_store
                .read_secret(workspace_id.clone(), credential_ref.clone()),
        )
        .await
    }

    async fn reveal_connection_secret_with(
        &self,
        workspace_id: &str,
        connection_id: &str,
        connection_type: &str,
        credential_ref: &str,
        read: impl std::future::Future<Output = AppResult<String>>,
    ) -> AppResult<String> {
        if !matches!(connection_type, "ssh" | "database") {
            return Err(unfour_core::AppError::Validation(
                "CONNECTION_CREDENTIAL_UNAVAILABLE".into(),
            ));
        }
        // Reuse the cross-process credential lifecycle lock for staging and
        // same-reference rotation/deletion. Hold no SQLite connection or lock
        // while an OS keychain prompt/read can block.
        let _credential_guard = self.credential_stage_guard().await?;
        let revision: Option<i64> = sqlx::query_scalar(LIVE_CONNECTION_CREDENTIAL)
            .bind(workspace_id)
            .bind(connection_id)
            .bind(connection_type)
            .bind(credential_ref)
            .fetch_optional(self.db.pool())
            .await?;
        let revision = revision.ok_or_else(credential_unavailable)?;
        let secret = zeroize::Zeroizing::new(read.await.map_err(|_| credential_unavailable())?);

        // Deletion, workspace removal and GC need not wait for the keychain.
        // Revalidate after I/O against a fresh snapshot under a short writer
        // guard. Revision also rejects a replace/restore (ABA) race. This is
        // the read's linearization point; no secret escapes on a failed check.
        let mut guard = self.db.pool().begin_with("BEGIN IMMEDIATE").await?;
        let current: Result<Option<i64>, _> = sqlx::query_scalar(LIVE_CONNECTION_CREDENTIAL)
            .bind(workspace_id)
            .bind(connection_id)
            .bind(connection_type)
            .bind(credential_ref)
            .fetch_optional(&mut *guard)
            .await;
        guard.rollback().await?;
        if current? != Some(revision) {
            return Err(credential_unavailable());
        }
        Ok(secret.to_string())
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
        let _credential_guard = self.credential_stage_guard().await?;
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
        let _credential_guard = self.credential_stage_guard().await?;
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

const LIVE_CONNECTION_CREDENTIAL: &str = "
    SELECT c.revision FROM connections c
    JOIN workspaces w ON w.id=c.workspace_id AND w.deleted_at IS NULL
    WHERE c.workspace_id=? AND c.id=? AND c.connection_type=?
      AND c.credential_ref=? AND c.deleted_at IS NULL
      AND ((c.connection_type='ssh' AND EXISTS (SELECT 1 FROM ssh_connections s WHERE s.connection_id=c.id))
        OR (c.connection_type='database' AND EXISTS (SELECT 1 FROM database_connections d WHERE d.connection_id=c.id)))";

fn credential_unavailable() -> unfour_core::AppError {
    unfour_core::AppError::Validation("CONNECTION_CREDENTIAL_UNAVAILABLE".into())
}

#[cfg(test)]
mod tests;
