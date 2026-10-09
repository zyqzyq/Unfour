use std::collections::HashSet;

use sqlx::SqliteConnection;
use unfour_core::{AppError, AppResult};

use crate::CommandBus;

impl CommandBus {
    pub(crate) async fn track_connection_credential_edit(
        &self,
        workspace: &str,
        id: Option<&str>,
        kind: &str,
    ) -> AppResult<()> {
        let Some(id) = id else {
            return Ok(());
        };
        let reference: Option<String> = sqlx::query_scalar("SELECT credential_ref FROM connections WHERE workspace_id=? AND id=? AND connection_type=? AND deleted_at IS NULL")
            .bind(workspace).bind(id).bind(kind).fetch_optional(self.db.pool()).await?.flatten();
        if let Some(reference) = reference {
            self.workspace
                .adopt_bundle_credential(workspace, &reference)
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn stage_ssh_secret_edit(
        &self,
        input: &mut unfour_core::models::SshConnectionInput,
    ) -> AppResult<Vec<String>> {
        let Some(secret) = input.secret.as_ref().filter(|value| !value.is_empty()) else {
            return Ok(vec![]);
        };
        if !matches!(input.auth_kind.as_str(), "password" | "private-key") {
            return Ok(vec![]);
        }
        let kind = if input.auth_kind == "private-key" {
            "ssh-key-passphrase"
        } else {
            "ssh-password"
        };
        let reference = self
            .stage_connection_credential(
                &input.workspace_id,
                input.credential_ref.as_deref(),
                kind,
                secret,
            )
            .await?;
        input.secret = None;
        input.credential_ref = Some(reference.clone());
        Ok(vec![reference])
    }
    pub(crate) async fn stage_connection_credential(
        &self,
        workspace: &str,
        previous: Option<&str>,
        kind: &str,
        secret: &str,
    ) -> AppResult<String> {
        if let Some(previous) = previous {
            self.secret_store
                .inspect_credential(workspace.into(), previous.into())
                .await?;
            if !self
                .workspace
                .is_bundle_credential(workspace, previous)
                .await?
            {
                let mut db = self.db.pool().acquire().await?;
                let used = self
                    .live_bundle_references_on(&mut db, workspace)
                    .await?
                    .contains(previous);
                drop(db);
                if used {
                    self.workspace
                        .adopt_bundle_credential(workspace, previous)
                        .await?;
                }
            }
        }
        // Copy on write keeps siblings and encrypted snapshots on the old value
        // until the business transaction publishes this fresh reference.
        let reference = self
            .secret_store
            .make_ref(workspace, kind, &unfour_core::id::new_id());
        self.workspace
            .journal_bundle_credential(workspace, &reference)
            .await?;
        if let Err(error) = self
            .secret_store
            .rotate_credential(workspace.into(), reference.clone(), secret.into())
            .await
        {
            return self
                .finish_bundle_credential_edits(&[reference], Err(error))
                .await;
        }
        Ok(reference)
    }

    async fn live_bundle_references_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<HashSet<String>> {
        if !self
            .workspace
            .bundle_workspace_live_on(db, workspace)
            .await?
        {
            return Ok(HashSet::new());
        }
        let mut live = HashSet::new();
        for rows in [
            self.ssh.bundle_connection_fields_on(db, workspace).await?,
            self.database
                .bundle_connection_fields_on(db, workspace)
                .await?,
        ] {
            live.extend(rows.into_iter().filter_map(|(_, _, reference)| reference));
        }
        Ok(live)
    }

    /// Runs in the business transaction, so rollback retains the previous
    /// ownership and commit durably records any reclamation work.
    pub(crate) async fn reconcile_bundle_credentials_on(
        &self,
        db: &mut SqliteConnection,
    ) -> AppResult<()> {
        for workspace in self.workspace.bundle_credential_workspaces_on(db).await? {
            let entries = self.workspace.bundle_credentials_on(db, &workspace).await?;
            if entries.is_empty() {
                continue;
            }
            let live = self.live_bundle_references_on(db, &workspace).await?;
            for (reference, state) in entries {
                if live.contains(&reference) && state != "attached" {
                    self.workspace
                        .track_bundle_reference_on(db, &workspace, &reference, "attached")
                        .await?;
                } else if !live.contains(&reference) && state == "attached" {
                    self.workspace
                        .track_bundle_reference_on(db, &workspace, &reference, "garbage")
                        .await?;
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn cleanup_bundle_garbage(&self) -> AppResult<()> {
        self.collect_bundle_credentials(false).await
    }

    /// Recovery shares the OS lock held from journaling through commit by every
    /// staging writer, including satellites. Process exit releases the lock.
    pub(crate) async fn recover_bundle_credentials(&self) -> AppResult<()> {
        let _credential_guard = self.credential_stage_guard().await?;
        self.collect_bundle_credentials(true).await
    }

    async fn collect_bundle_credentials(&self, recovery: bool) -> AppResult<()> {
        // Serialize reference checks and deletion with all SQLite writers,
        // including other processes and encrypted backup capture.
        let mut db = self.db.pool().begin_with("BEGIN IMMEDIATE").await?;
        for workspace in self
            .workspace
            .bundle_credential_workspaces_on(&mut db)
            .await?
        {
            let entries = self
                .workspace
                .bundle_credentials_on(&mut db, &workspace)
                .await?;
            let live = self.live_bundle_references_on(&mut db, &workspace).await?;
            for (reference, state) in entries {
                if live.contains(&reference) {
                    if state != "attached" {
                        self.workspace
                            .track_bundle_reference_on(&mut db, &workspace, &reference, "attached")
                            .await?;
                    }
                } else if recovery || state == "garbage" || state == "attached" {
                    match self
                        .secret_store
                        .delete_credential(workspace.clone(), reference.clone())
                        .await
                    {
                        Ok(()) | Err(AppError::NotFound(_)) => {
                            self.workspace
                                .forget_bundle_reference_on(&mut db, &reference)
                                .await?
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
        }
        db.commit().await?;
        Ok(())
    }

    pub(crate) async fn finish_bundle_credential_edits<T>(
        &self,
        references: &[String],
        result: AppResult<T>,
    ) -> AppResult<T> {
        if self
            .workspace
            .discard_staged_credentials(references)
            .await
            .is_err()
            || self.cleanup_bundle_garbage().await.is_err()
        {
            tracing::warn!("Workspace credential cleanup remains pending");
        }
        result
    }
}
