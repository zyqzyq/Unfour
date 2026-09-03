use unfour_core::domain::DomainEntityKey;

use super::{SyncService, MAX_REMOTE_PAGES};
use crate::{
    parse_remote_change, parse_snapshot_item, OutboxEntry, RemoteChange, SnapshotItem, SyncBinding,
    SyncError, SyncOperation, PAYLOAD_SCHEMA_VERSION, PROTOCOL_VERSION,
};

impl SyncService {
    pub async fn retry_dead_letter_current_local(
        &self,
        workspace_id: &str,
        operation_id: &str,
    ) -> Result<String, SyncError> {
        let account = self.account().await?;
        let binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        self.repository.assert_workspace_owner(&binding).await?;
        let entry = self
            .repository
            .dead_letter(&account.account_id, workspace_id, operation_id)
            .await?;
        let entity_type = crate::SyncEntityType::parse(&entry.entity_type)?;
        let mut key = DomainEntityKey::new(entity_type.into(), workspace_id, &entry.entity_id);
        key.parent_entity_id.clone_from(&entry.parent_entity_id);
        let snapshot = self
            .core()
            .await?
            .read_domain_snapshot(&key)
            .await
            .map_err(|_| SyncError::Core)?;
        let new_operation_id = self
            .repository
            .retry_dead_letter_current_snapshot(
                &binding,
                operation_id,
                snapshot,
                self.dependencies.ids.as_ref(),
                self.dependencies.clock.as_ref(),
            )
            .await?;
        self.sync_workspace_for(account, workspace_id).await?;
        Ok(new_operation_id)
    }

    pub async fn use_remote_dead_letter(
        &self,
        workspace_id: &str,
        operation_id: &str,
    ) -> Result<(), SyncError> {
        let account = self.account().await?;
        let binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        self.repository.assert_workspace_owner(&binding).await?;
        let entry = self
            .repository
            .dead_letter(&account.account_id, workspace_id, operation_id)
            .await?;
        let remote = self
            .snapshot_dead_letter_entity(&account, &binding, &entry)
            .await?;
        let now = self.dependencies.clock.now();
        let external = match &remote {
            Some(item) => parse_snapshot_item(workspace_id, item)?,
            None => parse_remote_change(
                workspace_id,
                &RemoteChange {
                    cursor: binding.last_pulled_cursor,
                    operation_id: "dead-letter-use-remote".into(),
                    entity_type: crate::SyncEntityType::parse(&entry.entity_type)?,
                    entity_id: entry.entity_id.clone(),
                    parent_entity_id: entry.parent_entity_id.clone(),
                    operation: SyncOperation::Delete,
                    server_version: entry.base_version.max(1),
                    payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                    payload: None,
                    deleted_at: Some(now.to_rfc3339()),
                },
            )?,
        };

        let mut tx = self.repository.pool().begin().await?;
        crate::SyncRepository::assert_recovery_binding_on(&mut tx, &binding).await?;
        let current_entry = crate::SyncRepository::dead_letter_on(
            &mut tx,
            &binding.account_id,
            workspace_id,
            operation_id,
        )
        .await?;
        if current_entry.entity_type != entry.entity_type
            || current_entry.entity_id != entry.entity_id
            || current_entry.parent_entity_id != entry.parent_entity_id
        {
            return Err(SyncError::AccountChanged);
        }
        if remote.is_none() {
            crate::SyncRepository::ensure_remote_absence_is_safe_on(
                &mut tx,
                &binding,
                &current_entry,
            )
            .await?;
        }
        let cleanup = self
            .apply_external_page_on(&mut tx, "pro.sync.dead_letter.use_remote", external)
            .await?;
        let current_entry = crate::SyncRepository::dead_letter_on(
            &mut tx,
            &binding.account_id,
            workspace_id,
            operation_id,
        )
        .await?;
        if current_entry.entity_type != entry.entity_type
            || current_entry.entity_id != entry.entity_id
            || current_entry.parent_entity_id != entry.parent_entity_id
        {
            return Err(SyncError::AccountChanged);
        }
        crate::SyncRepository::finish_remote_dead_letter_on(
            &mut tx,
            &binding,
            &current_entry,
            remote.as_ref(),
            now,
        )
        .await?;
        tx.commit().await?;
        self.finish_external_cleanup(cleanup).await;
        let _ = self.sync_workspace_for(account, workspace_id).await;
        Ok(())
    }

    async fn snapshot_dead_letter_entity(
        &self,
        account: &crate::SyncAccountContext,
        binding: &SyncBinding,
        entry: &OutboxEntry,
    ) -> Result<Option<SnapshotItem>, SyncError> {
        let at_cursor = binding.last_pulled_cursor;
        let mut page_token = None;
        let mut found = None;
        for _ in 0..MAX_REMOTE_PAGES {
            let page = self
                .transport
                .snapshot(
                    &binding.cloud_workspace_id,
                    Some(at_cursor),
                    page_token.as_deref(),
                )
                .await
                .map_err(SyncError::from)?;
            self.account_is_current(account)?;
            if page.protocol_version != PROTOCOL_VERSION
                || page.cloud_workspace_id != binding.cloud_workspace_id
                || page.at_cursor != at_cursor
                || page.current_cursor < page.at_cursor
            {
                return Err(SyncError::InvalidData);
            }
            for item in page.items {
                if item.entity_type.as_str() == entry.entity_type
                    && item.entity_id == entry.entity_id
                    && found.replace(item).is_some()
                {
                    return Err(SyncError::InvalidData);
                }
            }
            page_token = page.next_page_token;
            if page_token.is_none() {
                return Ok(found);
            }
        }
        Err(SyncError::InvalidData)
    }
}
