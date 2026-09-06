//! Conflict views and explicit keep-local/use-remote orchestration.
//! Scope selection and durable resolution remain owned by the repository.

use unfour_core::domain::DomainEntityKey;

use super::SyncService;
use crate::conflict_scope;
use crate::entity_registry::local_delete_is_already_equivalent_on;
use crate::remote_compatibility::{remote_change_disposition, RemoteEntityDisposition};
use crate::{
    canonical_payload, parse_remote_change, RemoteChange, SyncConflictView, SyncEntityType,
    SyncError, SyncOperation, SyncRepository,
};

impl SyncService {
    pub async fn conflicts(&self, workspace_id: &str) -> Result<Vec<SyncConflictView>, SyncError> {
        let account = self.account().await?;
        let conflicts = self
            .repository
            .conflicts(&account.account_id, workspace_id)
            .await?;
        let mut views = Vec::with_capacity(conflicts.len());
        for conflict in conflicts {
            let entity_type = SyncEntityType::parse(&conflict.entity_type)?;
            let mut key =
                DomainEntityKey::new(entity_type.into(), workspace_id, &conflict.entity_id);
            key.parent_entity_id
                .clone_from(&conflict.conflict_parent_entity_id);
            let local_payload = match self.core().await?.read_domain_snapshot(&key).await {
                Ok(snapshot) => canonical_payload(snapshot)?,
                Err(_) => None,
            };
            let remote_payload = conflict
                .conflict_remote_payload_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|_| SyncError::InvalidData)?;
            let local_secret_present = self
                .repository
                .local_secret_present(workspace_id, entity_type, &conflict.entity_id)
                .await?;
            views.push(SyncConflictView {
                cloud_workspace_id: conflict.cloud_workspace_id,
                entity_type: conflict.entity_type,
                entity_id: conflict.entity_id,
                server_version: conflict.server_version,
                operation: conflict
                    .conflict_remote_operation
                    .ok_or(SyncError::InvalidData)?,
                local_payload,
                remote_payload,
                local_secret_present,
            });
        }
        Ok(views)
    }

    pub async fn keep_local(
        &self,
        workspace_id: &str,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<(), SyncError> {
        let account = self.account().await?;
        let binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        let conflict = self
            .repository
            .conflict(
                &account.account_id,
                &binding.cloud_workspace_id,
                entity_type,
                entity_id,
            )
            .await?;
        ensure_conflict_is_supported(&conflict)?;
        if matches!(
            entity_type,
            SyncEntityType::Connection
                | SyncEntityType::ApiCollection
                | SyncEntityType::ApiFolder
                | SyncEntityType::ApiRequest
        ) {
            let scoped = self
                .repository
                .scoped_conflicts(&binding, &conflict)
                .await?;
            let mut snapshots = Vec::with_capacity(scoped.len());
            for item in &scoped {
                ensure_conflict_is_supported(item)?;
                let scoped_type = SyncEntityType::parse(&item.entity_type)?;
                let mut key =
                    DomainEntityKey::new(scoped_type.into(), workspace_id, &item.entity_id);
                key.parent_entity_id
                    .clone_from(&item.conflict_parent_entity_id);
                snapshots.push(
                    self.core()
                        .await?
                        .read_domain_snapshot(&key)
                        .await
                        .map_err(|_| SyncError::Core)?,
                );
            }
            self.repository
                .keep_local_snapshots(
                    &binding,
                    &conflict,
                    snapshots,
                    self.dependencies.ids.as_ref(),
                    self.dependencies.clock.as_ref(),
                )
                .await?;
        } else {
            self.repository
                .keep_local(
                    &binding,
                    &conflict,
                    self.dependencies.ids.as_ref(),
                    self.dependencies.clock.as_ref(),
                )
                .await?;
        }
        self.sync_workspace_for(account, workspace_id).await
    }

    pub async fn use_remote(
        &self,
        workspace_id: &str,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<(), SyncError> {
        let account = self.account().await?;
        let binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        let conflict = self
            .repository
            .conflict(
                &account.account_id,
                &binding.cloud_workspace_id,
                entity_type,
                entity_id,
            )
            .await?;
        let change = conflict_change(&conflict, binding.last_pulled_cursor)?;
        ensure_remote_change_is_supported(&change)?;
        let external = parse_remote_change(workspace_id, &change)?;
        let now = self.dependencies.clock.now().to_rfc3339();
        let mut tx = self.repository.pool().begin().await?;
        SyncRepository::assert_binding_generation_on(&mut tx, &binding).await?;
        let scoped_conflicts = conflict_scope::conflicts_on(&mut tx, &binding, &conflict).await?;
        for scoped_conflict in &scoped_conflicts {
            ensure_conflict_is_supported(scoped_conflict)?;
        }
        conflict_scope::abandon_intents_on(&mut tx, &binding, &conflict).await?;
        let cleanup = self
            .apply_external_page_on(&mut tx, "pro.sync.conflict.use_remote", external)
            .await?;
        for scoped_conflict in &scoped_conflicts {
            let entity_type = crate::SyncEntityType::parse(&scoped_conflict.entity_type)?;
            if !cleanup.materialized(entity_type.into(), &scoped_conflict.entity_id) {
                let scoped_change = conflict_change(scoped_conflict, binding.last_pulled_cursor)?;
                if scoped_change.operation != SyncOperation::Delete
                    || !local_delete_is_already_equivalent_on(
                        &mut tx,
                        workspace_id,
                        entity_type,
                        &scoped_conflict.entity_id,
                    )
                    .await?
                {
                    return Err(SyncError::Core);
                }
            }
            SyncRepository::clear_conflict_on(&mut tx, &binding, scoped_conflict, false, &now)
                .await?;
        }
        tx.commit().await?;
        self.finish_external_cleanup(cleanup).await;
        Ok(())
    }
}

fn ensure_conflict_is_supported(conflict: &crate::SyncConflict) -> Result<(), SyncError> {
    ensure_remote_change_is_supported(&conflict_change(conflict, 0)?)
}

fn ensure_remote_change_is_supported(change: &RemoteChange) -> Result<(), SyncError> {
    match remote_change_disposition(change)? {
        RemoteEntityDisposition::Apply(_) => Ok(()),
        RemoteEntityDisposition::SkipUnknownEntity
        | RemoteEntityDisposition::SkipUnsupportedPayload => Err(SyncError::CompatibilityWaiting),
    }
}

fn conflict_change(conflict: &crate::SyncConflict, cursor: i64) -> Result<RemoteChange, SyncError> {
    Ok(RemoteChange {
        cursor,
        operation_id: conflict.conflict_operation_id.clone().unwrap_or_default(),
        entity_type: conflict.entity_type.clone(),
        entity_id: conflict.entity_id.clone(),
        parent_entity_id: conflict.conflict_parent_entity_id.clone(),
        operation: SyncOperation::parse(
            conflict
                .conflict_remote_operation
                .as_deref()
                .ok_or(SyncError::InvalidData)?,
        )?,
        server_version: conflict.server_version,
        payload_schema_version: conflict
            .conflict_payload_schema_version
            .ok_or(SyncError::InvalidData)?,
        payload: conflict
            .conflict_remote_payload_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|_| SyncError::InvalidData)?,
        deleted_at: conflict.conflict_deleted_at.clone(),
    })
}
