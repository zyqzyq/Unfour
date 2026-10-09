use std::collections::HashSet;

use sqlx::SqliteConnection;
use unfour_core::{AppError, AppResult};

use crate::CommandBus;

impl CommandBus {
    pub(crate) async fn stage_ssh_secret_edit(
        &self,
        input: &mut unfour_core::models::SshConnectionInput,
    ) -> AppResult<Vec<String>> {
        let Some(secret) = input.secret.as_ref().filter(|value| !value.is_empty()) else {
            return Ok(vec![]);
        };
        let Some(previous) = input.credential_ref.as_deref() else {
            return Ok(vec![]);
        };
        self.secret_store
            .inspect_credential(input.workspace_id.clone(), previous.into())
            .await?;
        if !self
            .workspace
            .is_bundle_credential(&input.workspace_id, previous)
            .await?
        {
            let mut db = self.db.pool().acquire().await?;
            let used = self
                .ssh
                .bundle_connection_fields_on(&mut db, &input.workspace_id)
                .await?
                .iter()
                .any(|(_, _, reference)| reference.as_deref() == Some(previous));
            drop(db);
            if used {
                self.workspace
                    .adopt_bundle_credential(&input.workspace_id, previous)
                    .await?;
            }
        }
        // Imported connections can share handles. Copy on write also prevents
        // a backup from observing a staged password with the old SQL fields.
        let kind = if input.auth_kind == "private-key" {
            "ssh-key-passphrase"
        } else {
            "ssh-password"
        };
        let reference =
            self.secret_store
                .make_ref(&input.workspace_id, kind, &unfour_core::id::new_id());
        self.workspace
            .journal_bundle_credential(&input.workspace_id, &reference)
            .await?;
        if let Err(error) = self
            .secret_store
            .rotate_credential(
                input.workspace_id.clone(),
                reference.clone(),
                secret.clone(),
            )
            .await
        {
            return self
                .finish_bundle_credential_edits(&[reference], Err(error))
                .await;
        }
        input.secret = None;
        input.credential_ref = Some(reference.clone());
        Ok(vec![reference])
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
        let mut live: HashSet<_> = self
            .workspace
            .bundle_variable_references_on(db, workspace)
            .await?
            .into_iter()
            .collect();
        live.extend(
            self.api_client
                .bundle_credential_references_on(db, workspace)
                .await?,
        );
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
        for workspace in self
            .workspace
            .bundle_credential_workspaces_on(db, false)
            .await?
        {
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

    /// Primary startup only: interrupted stages are safe to reclaim before
    /// imports/edits are exposed. Satellite construction never calls this.
    pub(crate) async fn recover_bundle_credentials(&self) -> AppResult<()> {
        self.collect_bundle_credentials(true).await
    }

    async fn collect_bundle_credentials(&self, recovery: bool) -> AppResult<()> {
        // Serialize reference checks and deletion with all SQLite writers,
        // including other processes and encrypted backup capture.
        let mut db = self.db.pool().begin_with("BEGIN IMMEDIATE").await?;
        for workspace in self
            .workspace
            .bundle_credential_workspaces_on(&mut db, recovery)
            .await?
        {
            let mut entries = self
                .workspace
                .bundle_credentials_on(&mut db, &workspace)
                .await?;
            let live = self.live_bundle_references_on(&mut db, &workspace).await?;
            if recovery {
                // Adopt handles saved by the original V2 implementation, which
                // discarded ownership metadata on successful import.
                for reference in &live {
                    if !entries.iter().any(|(existing, _)| existing == reference) {
                        if self
                            .secret_store
                            .inspect_credential(workspace.clone(), reference.clone())
                            .await
                            .is_err()
                        {
                            continue; // Malformed legacy strings cannot identify this store's credentials.
                        }
                        self.workspace
                            .track_bundle_reference_on(&mut db, &workspace, &reference, "attached")
                            .await?;
                        entries.push((reference.clone(), "attached".into()));
                    }
                }
            }
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
