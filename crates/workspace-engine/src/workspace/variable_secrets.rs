use super::*;

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

    pub async fn bundle_credential_workspaces_on(
        &self,
        db: &mut SqliteConnection,
    ) -> AppResult<Vec<String>> {
        Ok(sqlx::query_scalar(
            "SELECT DISTINCT workspace_id FROM workspace_bundle_credential_journal",
        )
        .fetch_all(db)
        .await?)
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
