//! Validate and apply incremental pages without acknowledging unseen changes.
//! Remote preparation, Core application, entity state and cursor commit atomically.

use std::collections::HashSet;

use super::external_apply::merge_external_pages;
use super::{SyncService, MAX_REMOTE_PAGES};
use crate::canonical::parse_remote_change_as;
use crate::remote_compatibility::{remote_change_disposition, RemoteEntityDisposition};
use crate::{
    ChangesPage, SyncAccountContext, SyncBinding, SyncEntityType, SyncError, SyncOperation,
    SyncPhase, SyncRepository, TransportError, PROTOCOL_VERSION,
};

const PULL_PAGE_LIMIT: usize = 200;

impl SyncService {
    pub(super) async fn pull(
        &self,
        account: &SyncAccountContext,
        binding: &SyncBinding,
    ) -> Result<(), SyncError> {
        let mut cursor = binding.last_pulled_cursor;
        let mut pinned_current = None;
        for _ in 0..MAX_REMOTE_PAGES {
            let page = match self
                .transport
                .changes(&binding.cloud_workspace_id, cursor, PULL_PAGE_LIMIT)
                .await
            {
                Ok(page) => page,
                Err(TransportError::Remote(problem)) => {
                    self.record_remote_problem(
                        &account.account_id,
                        Some(&binding.cloud_workspace_id),
                        &problem,
                    )
                    .await;
                    return Err(problem.sync_error());
                }
                Err(TransportError::RemoteConflict { problem, .. }) => {
                    self.record_remote_problem(
                        &account.account_id,
                        Some(&binding.cloud_workspace_id),
                        &problem,
                    )
                    .await;
                    return Err(SyncError::Conflict);
                }
                Err(error) => return Err(SyncError::from(error)),
            };
            self.account_is_current(account)?;
            if let Err(reason) = validate_changes_page(binding, cursor, pinned_current, &page) {
                let _ = self
                    .repository
                    .record_local_diagnostic(
                        &account.account_id,
                        Some(&binding.cloud_workspace_id),
                        "permanent",
                        reason,
                        SyncPhase::Changes,
                        self.dependencies.clock.now(),
                    )
                    .await;
                return Err(SyncError::InvalidData);
            }
            pinned_current = Some(page.current_cursor);
            let now = self.dependencies.clock.now().to_rfc3339();
            let mut tx = self.repository.pool().begin().await?;
            let mut received = Vec::with_capacity(page.changes.len());
            for change in &page.changes {
                let disposition = remote_change_disposition(change)?;
                let compatibility_state = match disposition {
                    RemoteEntityDisposition::Apply(_) => "supported",
                    RemoteEntityDisposition::SkipUnknownEntity
                    | RemoteEntityDisposition::SkipUnsupportedPayload => {
                        SyncRepository::record_remote_skip_on(
                            &mut tx,
                            &binding.account_id,
                            &binding.cloud_workspace_id,
                            "remote_entity_compatibility_deferred",
                            SyncPhase::Changes,
                            &change.entity_type,
                            &change.entity_id,
                            &now,
                        )
                        .await?;
                        "deferred_compatibility"
                    }
                };
                let is_latest = SyncRepository::record_remote_change_on(
                    &mut tx,
                    binding,
                    change,
                    compatibility_state,
                    &now,
                )
                .await?;
                received.push((change, disposition, is_latest));
            }
            // Re-evaluate all retained envelopes before deciding whether this
            // reader must wait. A row deferred by an older binary may already
            // be supported after restart, even when the server has no new
            // changes to redeliver.
            let remote_records =
                SyncRepository::reclassify_remote_entities_on(&mut tx, binding, &now).await?;
            if remote_records
                .iter()
                .any(|record| record.compatibility_state == "deferred_compatibility")
            {
                SyncRepository::advance_cursor_on(&mut tx, binding, page.next_cursor, &now).await?;
                SyncRepository::mark_binding_compatibility_waiting_on(&mut tx, binding, &now)
                    .await?;
                tx.commit().await?;
                cursor = page.next_cursor;
                if page.next_cursor == page.current_cursor {
                    return Err(SyncError::CompatibilityWaiting);
                }
                continue;
            }
            let mut safe_pages = Vec::new();
            let mut safe_changes = Vec::new();
            let ssh_task_aggregate_deletes = page
                .changes
                .iter()
                .filter(|change| {
                    matches!(
                        remote_change_disposition(change),
                        Ok(RemoteEntityDisposition::Apply(SyncEntityType::SshTask))
                    )
                })
                .filter(|change| change.operation == SyncOperation::Delete)
                .map(|change| (change.operation_id.as_str(), change.entity_id.as_str()))
                .collect::<HashSet<_>>();
            for (change, disposition, is_latest) in received {
                if !is_latest {
                    continue;
                }
                let entity_type = match disposition {
                    RemoteEntityDisposition::Apply(entity_type) => entity_type,
                    RemoteEntityDisposition::SkipUnknownEntity
                    | RemoteEntityDisposition::SkipUnsupportedPayload => continue,
                };
                let aggregate_delete_root = (entity_type == SyncEntityType::SshTaskStep
                    && change.operation == SyncOperation::Delete)
                    .then_some(change.parent_entity_id.as_deref())
                    .flatten()
                    .filter(|task_id| {
                        ssh_task_aggregate_deletes
                            .contains(&(change.operation_id.as_str(), *task_id))
                    })
                    .map(|task_id| (SyncEntityType::SshTask, task_id));
                if SyncRepository::prepare_remote_change_with_aggregate_root_on(
                    &mut tx,
                    binding,
                    change,
                    entity_type,
                    aggregate_delete_root,
                    &now,
                )
                .await?
                {
                    safe_pages.push(parse_remote_change_as(
                        &binding.local_workspace_id,
                        entity_type,
                        change,
                    )?);
                    safe_changes.push((change, entity_type));
                }
            }
            let external = merge_external_pages(safe_pages);
            let cleanup = self
                .apply_external_page_on(&mut tx, "pro.sync.pull", external)
                .await?;
            for (change, entity_type) in safe_changes {
                SyncRepository::record_applied_remote_on(
                    &mut tx,
                    binding,
                    change,
                    entity_type,
                    &now,
                )
                .await?;
            }
            SyncRepository::advance_cursor_on(&mut tx, binding, page.next_cursor, &now).await?;
            tx.commit().await?;
            self.finish_external_cleanup(cleanup).await;
            cursor = page.next_cursor;
            if page.next_cursor == page.current_cursor {
                return self.reapply_retained_remote(binding).await;
            }
        }
        Err(SyncError::InvalidData)
    }

    async fn reapply_retained_remote(&self, binding: &SyncBinding) -> Result<(), SyncError> {
        let now = self.dependencies.clock.now().to_rfc3339();
        let mut tx = self.repository.pool().begin().await?;
        SyncRepository::assert_binding_generation_on(&mut tx, binding).await?;
        let records = SyncRepository::reclassify_remote_entities_on(&mut tx, binding, &now).await?;
        if records
            .iter()
            .any(|record| record.compatibility_state == "deferred_compatibility")
        {
            SyncRepository::mark_binding_compatibility_waiting_on(&mut tx, binding, &now).await?;
            tx.commit().await?;
            return Err(SyncError::CompatibilityWaiting);
        }

        let mut blocked_parents = HashSet::new();
        let mut pages = Vec::new();
        let mut applied = Vec::new();
        for record in records {
            if SyncRepository::entity_state_is_current_synced_on(&mut tx, binding, &record).await? {
                continue;
            }
            let change = record.as_change(binding.last_pulled_cursor)?;
            let entity_type = match remote_change_disposition(&change)? {
                RemoteEntityDisposition::Apply(entity_type) => entity_type,
                _ => return Err(SyncError::CompatibilityWaiting),
            };
            if change.operation == SyncOperation::Upsert
                && change
                    .parent_entity_id
                    .as_ref()
                    .is_some_and(|parent| blocked_parents.contains(parent))
            {
                blocked_parents.insert(change.entity_id.clone());
                continue;
            }
            if SyncRepository::entity_state_version_on(
                &mut tx,
                binding,
                &record.entity_type,
                &record.entity_id,
            )
            .await?
            .is_some_and(|version| version > record.server_version)
            {
                continue;
            }
            if !SyncRepository::prepare_remote_replay_on(
                &mut tx,
                binding,
                &change,
                record.operation_id.as_deref(),
                entity_type,
                &now,
            )
            .await?
            {
                blocked_parents.insert(change.entity_id.clone());
                continue;
            }
            if change.operation == SyncOperation::Delete {
                blocked_parents.insert(change.entity_id.clone());
            }
            pages.push(parse_remote_change_as(
                &binding.local_workspace_id,
                entity_type,
                &change,
            )?);
            applied.push(record);
        }
        let cleanup = self
            .apply_external_page_on(
                &mut tx,
                "pro.sync.remote_reapply",
                merge_external_pages(pages),
            )
            .await?;
        for record in &applied {
            SyncRepository::record_replayed_remote_state_on(&mut tx, binding, record, &now).await?;
        }
        tx.commit().await?;
        self.finish_external_cleanup(cleanup).await;
        Ok(())
    }
}

fn validate_changes_page(
    binding: &SyncBinding,
    requested_cursor: i64,
    pinned_current: Option<i64>,
    page: &ChangesPage,
) -> Result<(), &'static str> {
    if page.protocol_version != PROTOCOL_VERSION {
        return Err("pull_invalid_protocol_version");
    }
    if page.cloud_workspace_id != binding.cloud_workspace_id {
        return Err("pull_workspace_mismatch");
    }
    if pinned_current.is_some_and(|current| current != page.current_cursor) {
        return Err("pull_current_cursor_changed");
    }
    if page.current_cursor < requested_cursor
        || page.next_cursor < requested_cursor
        || page.next_cursor > page.current_cursor
    {
        return Err("pull_cursor_out_of_range");
    }
    let cursor_sequence_is_complete = if page.changes.is_empty() {
        page.next_cursor == requested_cursor && page.current_cursor == requested_cursor
    } else {
        page.changes.first().map(|change| change.cursor) == requested_cursor.checked_add(1)
            && page.changes.last().map(|change| change.cursor) == Some(page.next_cursor)
            && page
                .changes
                .windows(2)
                .all(|pair| pair[0].cursor.checked_add(1) == Some(pair[1].cursor))
    };
    let valid = cursor_sequence_is_complete
        && page
            .changes
            .iter()
            .all(|change| change.cursor > requested_cursor && change.cursor <= page.next_cursor);
    valid.then_some(()).ok_or("pull_cursor_gap")
}
