//! Download a pinned snapshot into staging, then atomically install a new workspace.
//! Never reuse this path as an implicit replacement of an existing local workspace.

use super::external_apply::merge_external_pages;
use super::{SyncService, MAX_REMOTE_PAGES};
use crate::canonical::snapshot_workspace_name;
use crate::{
    parse_snapshot_item, CloudWorkspace, DownloadDecision, SnapshotItem, SyncAccountContext,
    SyncEntityType, SyncError, SyncPhase, SyncRepository, TransportError, PROTOCOL_VERSION,
};

impl SyncService {
    pub async fn download_workspace(
        &self,
        cloud_workspace_id: &str,
        decision: DownloadDecision,
    ) -> Result<String, SyncError> {
        if decision != DownloadDecision::DownloadToNewWorkspace {
            return Err(SyncError::SafeReplaceUnavailable);
        }
        let account = self.account().await?;
        let listed = self.transport.list_workspaces().await;
        let cloud = self
            .finish_transport(&account.account_id, None, listed)
            .await?
            .into_iter()
            .find(|workspace| workspace.cloud_workspace_id == cloud_workspace_id)
            .ok_or(SyncError::NotFound)?;
        self.account_is_current(&account)?;
        if self
            .repository
            .binding_by_cloud(&account.account_id, cloud_workspace_id)
            .await?
            .is_some()
        {
            return Err(SyncError::InvalidData);
        }

        let stage_id = self.dependencies.ids.next_id();
        let result = self
            .download_and_apply_staged(&account, &cloud, &stage_id)
            .await;
        let _ = self.repository.clear_snapshot_stage(&stage_id).await;
        result
    }

    async fn download_and_apply_staged(
        &self,
        account: &SyncAccountContext,
        cloud: &CloudWorkspace,
        stage_id: &str,
    ) -> Result<String, SyncError> {
        let mut fixed_cursor = None;
        let mut page_token = None;
        let mut root_workspace_name = None;
        let mut completed = false;
        for _ in 0..MAX_REMOTE_PAGES {
            let page = match self
                .transport
                .snapshot(
                    &cloud.cloud_workspace_id,
                    fixed_cursor,
                    page_token.as_deref(),
                )
                .await
            {
                Ok(page) => page,
                Err(TransportError::Remote(problem)) => {
                    self.record_remote_problem(
                        &account.account_id,
                        Some(&cloud.cloud_workspace_id),
                        &problem,
                    )
                    .await;
                    return Err(problem.sync_error());
                }
                Err(TransportError::RemoteConflict { problem, .. }) => {
                    self.record_remote_problem(
                        &account.account_id,
                        Some(&cloud.cloud_workspace_id),
                        &problem,
                    )
                    .await;
                    return Err(SyncError::Conflict);
                }
                Err(error) => return Err(SyncError::from(error)),
            };
            self.account_is_current(account)?;
            if page.protocol_version != PROTOCOL_VERSION
                || page.cloud_workspace_id != cloud.cloud_workspace_id
                || page.at_cursor < 0
                || page.current_cursor < page.at_cursor
            {
                let _ = self
                    .repository
                    .record_local_diagnostic(
                        &account.account_id,
                        Some(&cloud.cloud_workspace_id),
                        "permanent",
                        "snapshot_invalid_response",
                        SyncPhase::Snapshot,
                        self.dependencies.clock.now(),
                    )
                    .await;
                return Err(SyncError::InvalidData);
            }
            if fixed_cursor.is_some_and(|cursor| cursor != page.at_cursor) {
                let _ = self
                    .repository
                    .record_local_diagnostic(
                        &account.account_id,
                        Some(&cloud.cloud_workspace_id),
                        "permanent",
                        "snapshot_invalid_response",
                        SyncPhase::Snapshot,
                        self.dependencies.clock.now(),
                    )
                    .await;
                return Err(SyncError::InvalidData);
            }
            fixed_cursor = Some(page.at_cursor);
            for item in &page.items {
                if let Some(name) = snapshot_workspace_name(&cloud.root_entity_id, item)? {
                    if root_workspace_name.replace(name).is_some() {
                        return Err(SyncError::InvalidData);
                    }
                }
            }
            self.repository
                .stage_snapshot_page(
                    stage_id,
                    &account.account_id,
                    &cloud.cloud_workspace_id,
                    page.at_cursor,
                    &page.items,
                    &self.dependencies.clock.now().to_rfc3339(),
                )
                .await?;
            page_token = page.next_page_token;
            if page_token.is_none() {
                completed = true;
                break;
            }
        }
        if !completed {
            let _ = self
                .repository
                .record_local_diagnostic(
                    &account.account_id,
                    Some(&cloud.cloud_workspace_id),
                    "permanent",
                    "snapshot_invalid_response",
                    SyncPhase::Snapshot,
                    self.dependencies.clock.now(),
                )
                .await;
            return Err(SyncError::InvalidData);
        }
        let at_cursor = fixed_cursor.ok_or(SyncError::InvalidData)?;
        let root_workspace_name = root_workspace_name.ok_or(SyncError::InvalidData)?;
        let now = self.dependencies.clock.now().to_rfc3339();
        let mut tx = self.repository.pool().begin().await?;
        if SyncRepository::local_workspace_exists_on(&mut tx, &cloud.root_entity_id).await? {
            return Err(SyncError::LocalWorkspaceNotEmpty);
        }
        if SyncRepository::active_workspace_name_exists_on(&mut tx, &root_workspace_name).await? {
            return Err(SyncError::WorkspaceNameConflict);
        }
        let rows: Vec<(String, String, Option<String>, i64, i64, String)> = sqlx::query_as(
            r#"SELECT entity_type, entity_id, parent_entity_id, server_version,
                      payload_schema_version, payload_json
               FROM cloud_sync_snapshot_staging WHERE stage_id = ?1
               ORDER BY topology_rank, entity_type, entity_id"#,
        )
        .bind(stage_id)
        .fetch_all(&mut *tx)
        .await?;
        if rows.is_empty() {
            return Err(SyncError::InvalidData);
        }
        let mut pages = Vec::with_capacity(rows.len());
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let item = SnapshotItem {
                entity_type: SyncEntityType::parse(&row.0)?,
                entity_id: row.1,
                parent_entity_id: row.2,
                server_version: row.3,
                payload_schema_version: row.4,
                payload: serde_json::from_str(&row.5).map_err(|_| SyncError::InvalidData)?,
            };
            pages.push(parse_snapshot_item(&cloud.root_entity_id, &item)?);
            items.push(item);
        }
        let cleanup = self
            .apply_external_page_on(&mut tx, "pro.sync.snapshot", merge_external_pages(pages))
            .await?;
        SyncRepository::insert_download_binding_on(
            &mut tx,
            &account.account_id,
            account.generation,
            &cloud.root_entity_id,
            &cloud.cloud_workspace_id,
            at_cursor,
            &now,
        )
        .await?;
        for item in &items {
            SyncRepository::record_snapshot_state_on(
                &mut tx,
                &account.account_id,
                &cloud.cloud_workspace_id,
                item,
                &now,
            )
            .await?;
        }
        tx.commit().await?;
        self.finish_external_cleanup(cleanup).await;
        let binding = self
            .repository
            .binding(&account.account_id, &cloud.root_entity_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        self.repository
            .set_binding_state(&binding, "active", None, self.dependencies.clock.now())
            .await?;
        Ok(cloud.root_entity_id.clone())
    }
}
