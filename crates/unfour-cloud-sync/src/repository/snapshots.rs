//! Snapshot staging and metadata committed alongside Core snapshot application.
//! Staging is disposable; downloaded bindings and entity state commit with live data.

use sqlx::SqliteConnection;

use super::SyncRepository;
use crate::{SnapshotItem, SyncError};

impl SyncRepository {
    pub async fn local_workspace_exists_on(
        connection: &mut SqliteConnection,
        workspace_id: &str,
    ) -> Result<bool, SyncError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ?1)")
            .bind(workspace_id)
            .fetch_one(&mut *connection)
            .await
            .map_err(Into::into)
    }

    pub async fn active_workspace_name_exists_on(
        connection: &mut SqliteConnection,
        name: &str,
    ) -> Result<bool, SyncError> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE name = ?1 COLLATE NOCASE AND deleted_at IS NULL)",
        )
        .bind(name)
        .fetch_one(&mut *connection)
        .await
        .map_err(Into::into)
    }

    pub async fn stage_snapshot_page(
        &self,
        stage_id: &str,
        account_id: &str,
        cloud_workspace_id: &str,
        page_cursor: i64,
        items: &[SnapshotItem],
        now: &str,
    ) -> Result<(), SyncError> {
        let mut tx = self.pool.begin().await?;
        for item in items {
            let payload =
                serde_json::to_string(&item.payload).map_err(|_| SyncError::InvalidData)?;
            sqlx::query(
                r#"INSERT INTO cloud_sync_snapshot_staging (
                     stage_id, account_id, cloud_workspace_id, at_cursor, entity_type,
                     entity_id, parent_entity_id, server_version, payload_schema_version,
                     payload_json, topology_rank, created_at
                   ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
            )
            .bind(stage_id)
            .bind(account_id)
            .bind(cloud_workspace_id)
            .bind(page_cursor)
            .bind(item.entity_type.as_str())
            .bind(&item.entity_id)
            .bind(&item.parent_entity_id)
            .bind(item.server_version)
            .bind(item.payload_schema_version)
            .bind(payload)
            .bind(item.entity_type.topology_rank())
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn staged_snapshot_chunk(
        &self,
        stage_id: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<(String, String, Option<String>, i64, i64, String)>, SyncError> {
        sqlx::query_as(
            r#"SELECT entity_type, entity_id, parent_entity_id, server_version,
                      payload_schema_version, payload_json
               FROM cloud_sync_snapshot_staging WHERE stage_id = ?1
               ORDER BY topology_rank, entity_type, entity_id LIMIT ?2 OFFSET ?3"#,
        )
        .bind(stage_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn clear_snapshot_stage(&self, stage_id: &str) -> Result<(), SyncError> {
        sqlx::query("DELETE FROM cloud_sync_snapshot_staging WHERE stage_id = ?1")
            .bind(stage_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_download_binding_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        account_generation: u64,
        workspace_id: &str,
        cloud_workspace_id: &str,
        cursor: i64,
        now: &str,
    ) -> Result<(), SyncError> {
        sqlx::query(
            r#"INSERT INTO cloud_sync_workspace_bindings (
                 account_id, local_workspace_id, cloud_workspace_id, last_pulled_cursor,
                 sync_enabled, state, initial_cursor, initial_total, initial_confirmed,
                 ssh_task_v3_bootstrap_state, connection_v4_bootstrap_state,
                 generation, last_success_at, created_at, updated_at
               ) VALUES (?1, ?2, ?3, ?4, 1, 'reconciling', ?4, 0, 0,
                         'completed', 'completed', ?5, ?6, ?6, ?6)"#,
        )
        .bind(account_id)
        .bind(workspace_id)
        .bind(cloud_workspace_id)
        .bind(cursor)
        .bind(account_generation as i64)
        .bind(now)
        .execute(&mut *connection)
        .await?;
        Ok(())
    }

    pub async fn record_snapshot_state_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        cloud_workspace_id: &str,
        item: &SnapshotItem,
        now: &str,
    ) -> Result<(), SyncError> {
        sqlx::query(
            r#"INSERT INTO cloud_sync_entity_state (
                 account_id, cloud_workspace_id, entity_type, entity_id, server_version,
                 sync_status, updated_at
               ) VALUES (?1, ?2, ?3, ?4, ?5, 'synced', ?6)
               ON CONFLICT(account_id, cloud_workspace_id, entity_type, entity_id) DO UPDATE SET
                 server_version = excluded.server_version, sync_status = 'synced', updated_at = excluded.updated_at"#,
        ).bind(account_id).bind(cloud_workspace_id).bind(item.entity_type.as_str())
         .bind(&item.entity_id).bind(item.server_version).bind(now).execute(&mut *connection).await?;
        Ok(())
    }
}
