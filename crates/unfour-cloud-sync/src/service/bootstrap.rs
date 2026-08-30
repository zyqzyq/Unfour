//! Connection v4 upgrade: collect snapshots and reconcile within one Core transaction.
//! The snapshot does not replace the existing incremental pull cursor.

use unfour_core::domain::{DomainEntityType, DomainSnapshot};

use super::external_apply::merge_external_pages;
use super::{SyncService, MAX_REMOTE_PAGES};
use crate::{
    parse_snapshot_item, SnapshotItem, SyncAccountContext, SyncBinding, SyncEntityType, SyncError,
    SyncRepository, PROTOCOL_VERSION,
};

impl SyncService {
    pub(super) async fn connection_snapshots(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<DomainSnapshot>, SyncError> {
        let core = self.core().await?;
        let keys = core
            .list_connection_domain_entities(workspace_id.to_string())
            .await
            .map_err(|_| SyncError::Core)?;
        let mut snapshots = Vec::with_capacity(keys.len());
        for key in keys {
            if key.entity_type != DomainEntityType::Connection
                || key.workspace_id != workspace_id
                || key.parent_entity_id.is_some()
            {
                return Err(SyncError::InvalidData);
            }
            snapshots.push(
                core.read_domain_snapshot(&key)
                    .await
                    .map_err(|_| SyncError::Core)?,
            );
        }
        Ok(snapshots)
    }

    pub(super) async fn connection_snapshot_items(
        &self,
        account: &SyncAccountContext,
        binding: &SyncBinding,
    ) -> Result<Vec<SnapshotItem>, SyncError> {
        let mut fixed_cursor = None;
        let mut page_token = None;
        let mut items = Vec::new();
        for _ in 0..MAX_REMOTE_PAGES {
            let page = self
                .transport
                .snapshot(
                    &binding.cloud_workspace_id,
                    fixed_cursor,
                    page_token.as_deref(),
                )
                .await
                .map_err(SyncError::from)?;
            self.account_is_current(account)?;
            if page.protocol_version != PROTOCOL_VERSION
                || page.cloud_workspace_id != binding.cloud_workspace_id
                || page.at_cursor < 0
                || page.current_cursor < page.at_cursor
            {
                return Err(SyncError::InvalidData);
            }
            if fixed_cursor.is_some_and(|cursor| cursor != page.at_cursor) {
                return Err(SyncError::InvalidData);
            }
            fixed_cursor = Some(page.at_cursor);
            items.extend(
                page.items
                    .into_iter()
                    .filter(|item| item.entity_type == SyncEntityType::Connection),
            );
            page_token = page.next_page_token;
            if page_token.is_none() {
                return Ok(items);
            }
        }
        Err(SyncError::InvalidData)
    }

    pub(super) async fn bootstrap_connection_v4(
        &self,
        account: &SyncAccountContext,
        binding: &SyncBinding,
        local_snapshots: Vec<DomainSnapshot>,
        remote_items: &[SnapshotItem],
    ) -> Result<(), SyncError> {
        let mut tx = self.repository.pool().begin().await?;
        let Some(plan) = SyncRepository::prepare_connection_v4_on(
            &mut tx,
            binding,
            local_snapshots,
            remote_items,
            self.dependencies.ids.as_ref(),
            self.dependencies.clock.as_ref(),
        )
        .await?
        else {
            tx.commit().await?;
            return Ok(());
        };

        let pages = plan
            .apply_items
            .iter()
            .map(|item| parse_snapshot_item(&binding.local_workspace_id, item))
            .collect::<Result<Vec<_>, _>>()?;
        let cleanup = self
            .apply_external_page_on(
                &mut tx,
                "pro.sync.connection_v4_bootstrap",
                merge_external_pages(pages),
            )
            .await?;
        SyncRepository::complete_connection_v4_on(
            &mut tx,
            binding,
            &plan,
            self.dependencies.clock.as_ref(),
        )
        .await?;
        self.account_is_current(account)?;
        tx.commit().await?;
        self.finish_external_cleanup(cleanup).await;
        Ok(())
    }
}
