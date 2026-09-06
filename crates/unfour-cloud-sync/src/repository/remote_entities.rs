//! Durable latest-remote-envelope storage for Protocol 5 tolerant readers.

use sqlx::SqliteConnection;

use super::SyncRepository;
use crate::remote_compatibility::{remote_change_disposition, RemoteEntityDisposition};
use crate::{
    RemoteChange, RemoteEntityRecord, SnapshotItem, SyncBinding, SyncError, SyncOperation,
};

impl SyncRepository {
    pub(crate) async fn record_remote_change_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        change: &RemoteChange,
        compatibility_state: &str,
        now: &str,
    ) -> Result<bool, SyncError> {
        let payload_json = change
            .payload
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| SyncError::InvalidData)?;
        Self::record_remote_envelope_on(
            connection,
            &binding.account_id,
            &binding.cloud_workspace_id,
            &change.entity_type,
            &change.entity_id,
            change.parent_entity_id.as_deref(),
            change.server_version,
            change.payload_schema_version,
            change.operation,
            payload_json.as_deref(),
            change.deleted_at.as_deref(),
            Some(&change.operation_id),
            compatibility_state,
            now,
        )
        .await
    }

    pub(crate) async fn record_snapshot_remote_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        cloud_workspace_id: &str,
        item: &SnapshotItem,
        compatibility_state: &str,
        now: &str,
    ) -> Result<bool, SyncError> {
        let payload_json =
            serde_json::to_string(&item.payload).map_err(|_| SyncError::InvalidData)?;
        Self::record_remote_envelope_on(
            connection,
            account_id,
            cloud_workspace_id,
            &item.entity_type,
            &item.entity_id,
            item.parent_entity_id.as_deref(),
            item.server_version,
            item.payload_schema_version,
            SyncOperation::Upsert,
            Some(&payload_json),
            None,
            None,
            compatibility_state,
            now,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_remote_envelope_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        cloud_workspace_id: &str,
        entity_type: &str,
        entity_id: &str,
        parent_entity_id: Option<&str>,
        server_version: i64,
        payload_schema_version: i64,
        operation: SyncOperation,
        payload_json: Option<&str>,
        deleted_at: Option<&str>,
        operation_id: Option<&str>,
        compatibility_state: &str,
        now: &str,
    ) -> Result<bool, SyncError> {
        if entity_type.trim().is_empty()
            || entity_id.trim().is_empty()
            || server_version < 1
            || payload_schema_version < 1
            || !matches!(compatibility_state, "supported" | "deferred_compatibility")
            || (operation == SyncOperation::Upsert) != payload_json.is_some()
            || (operation == SyncOperation::Delete) != deleted_at.is_some()
        {
            return Err(SyncError::InvalidData);
        }
        let changed = sqlx::query(
            r#"INSERT INTO cloud_sync_remote_entity (
                 account_id, cloud_workspace_id, entity_type, entity_id,
                 parent_entity_id, server_version, payload_schema_version,
                 operation, canonical_payload_json, deleted_at, operation_id,
                 compatibility_state, created_at, updated_at
               ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
               ON CONFLICT(account_id, cloud_workspace_id, entity_type, entity_id) DO UPDATE SET
                 parent_entity_id = excluded.parent_entity_id,
                 server_version = excluded.server_version,
                 payload_schema_version = excluded.payload_schema_version,
                 operation = excluded.operation,
                 canonical_payload_json = excluded.canonical_payload_json,
                 deleted_at = excluded.deleted_at,
                 operation_id = excluded.operation_id,
                 compatibility_state = excluded.compatibility_state,
                 updated_at = excluded.updated_at
               WHERE excluded.server_version > cloud_sync_remote_entity.server_version"#,
        )
        .bind(account_id)
        .bind(cloud_workspace_id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(parent_entity_id)
        .bind(server_version)
        .bind(payload_schema_version)
        .bind(operation.as_str())
        .bind(payload_json)
        .bind(deleted_at)
        .bind(operation_id)
        .bind(compatibility_state)
        .bind(now)
        .execute(&mut *connection)
        .await?
        .rows_affected();
        Ok(changed == 1)
    }

    pub(crate) async fn reclassify_remote_entities_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        now: &str,
    ) -> Result<Vec<RemoteEntityRecord>, SyncError> {
        let records = Self::remote_entities_on(connection, binding).await?;
        for record in &records {
            let change = record.as_change(binding.last_pulled_cursor)?;
            let state = match remote_change_disposition(&change)? {
                RemoteEntityDisposition::Apply(_) => "supported",
                RemoteEntityDisposition::SkipUnknownEntity
                | RemoteEntityDisposition::SkipUnsupportedPayload => "deferred_compatibility",
            };
            sqlx::query(
                r#"UPDATE cloud_sync_remote_entity
                   SET compatibility_state = ?1, updated_at = ?2
                   WHERE account_id = ?3 AND cloud_workspace_id = ?4
                     AND entity_type = ?5 AND entity_id = ?6"#,
            )
            .bind(state)
            .bind(now)
            .bind(&binding.account_id)
            .bind(&binding.cloud_workspace_id)
            .bind(&record.entity_type)
            .bind(&record.entity_id)
            .execute(&mut *connection)
            .await?;
        }
        Self::remote_entities_on(connection, binding).await
    }

    pub(crate) async fn remote_entities_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
    ) -> Result<Vec<RemoteEntityRecord>, SyncError> {
        sqlx::query_as::<_, RemoteEntityRecord>(
            r#"SELECT account_id, cloud_workspace_id, entity_type, entity_id,
                      parent_entity_id, server_version, payload_schema_version,
                      operation, canonical_payload_json, deleted_at, operation_id,
                      compatibility_state
               FROM cloud_sync_remote_entity
               WHERE account_id = ?1 AND cloud_workspace_id = ?2
               ORDER BY CASE entity_type
                 WHEN 'workspace' THEN 0
                 WHEN 'workspaceVariable' THEN 1
                 WHEN 'workspaceEnvironment' THEN 1
                 WHEN 'connection' THEN 1
                 WHEN 'apiCollection' THEN 1
                 WHEN 'sshTask' THEN 1
                 WHEN 'workspaceEnvironmentVariable' THEN 2
                 WHEN 'apiFolder' THEN 2
                 WHEN 'sshTaskStep' THEN 2
                 WHEN 'apiRequest' THEN 3
                 ELSE 100 END, entity_type, entity_id"#,
        )
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .fetch_all(&mut *connection)
        .await
        .map_err(Into::into)
    }

    pub(crate) async fn mark_binding_compatibility_waiting_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        now: &str,
    ) -> Result<(), SyncError> {
        let changed = sqlx::query(
            r#"UPDATE cloud_sync_workspace_bindings
               SET state = 'error', last_error = 'cloud_sync_compatibility_waiting',
                   updated_at = ?1
               WHERE account_id = ?2 AND local_workspace_id = ?3
                 AND cloud_workspace_id = ?4 AND generation = ?5"#,
        )
        .bind(now)
        .bind(&binding.account_id)
        .bind(&binding.local_workspace_id)
        .bind(&binding.cloud_workspace_id)
        .bind(binding.generation)
        .execute(&mut *connection)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(SyncError::AccountChanged);
        }
        Ok(())
    }

    pub(crate) async fn entity_state_version_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Option<i64>, SyncError> {
        sqlx::query_scalar(
            r#"SELECT server_version FROM cloud_sync_entity_state
               WHERE account_id = ?1 AND cloud_workspace_id = ?2
                 AND entity_type = ?3 AND entity_id = ?4"#,
        )
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(entity_type)
        .bind(entity_id)
        .fetch_optional(&mut *connection)
        .await
        .map_err(Into::into)
    }

    pub(crate) async fn entity_state_is_current_synced_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        record: &RemoteEntityRecord,
    ) -> Result<bool, SyncError> {
        let current: Option<(i64, String)> = sqlx::query_as(
            r#"SELECT server_version, sync_status FROM cloud_sync_entity_state
               WHERE account_id = ?1 AND cloud_workspace_id = ?2
                 AND entity_type = ?3 AND entity_id = ?4"#,
        )
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(&record.entity_type)
        .bind(&record.entity_id)
        .fetch_optional(&mut *connection)
        .await?;
        Ok(current.is_some_and(|(version, status)| {
            version == record.server_version && status == "synced"
        }))
    }

    pub(crate) async fn record_replayed_remote_state_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        record: &RemoteEntityRecord,
        now: &str,
    ) -> Result<(), SyncError> {
        sqlx::query(
            r#"INSERT INTO cloud_sync_entity_state (
                 account_id, cloud_workspace_id, entity_type, entity_id,
                 server_version, last_operation_id, sync_status, updated_at
               ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'synced', ?7)
               ON CONFLICT(account_id, cloud_workspace_id, entity_type, entity_id) DO UPDATE SET
                 server_version = excluded.server_version,
                 last_operation_id = excluded.last_operation_id,
                 sync_status = 'synced',
                 conflict_payload_schema_version = NULL,
                 conflict_remote_payload_json = NULL,
                 conflict_remote_operation = NULL,
                 conflict_parent_entity_id = NULL,
                 conflict_deleted_at = NULL,
                 conflict_operation_id = NULL,
                 updated_at = excluded.updated_at
               WHERE cloud_sync_entity_state.sync_status <> 'conflict'
                 AND excluded.server_version >= cloud_sync_entity_state.server_version"#,
        )
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(&record.entity_type)
        .bind(&record.entity_id)
        .bind(record.server_version)
        .bind(&record.operation_id)
        .bind(now)
        .execute(&mut *connection)
        .await?;
        Ok(())
    }
}

impl RemoteEntityRecord {
    pub(crate) fn as_change(&self, cursor: i64) -> Result<RemoteChange, SyncError> {
        Ok(RemoteChange {
            cursor,
            operation_id: self
                .operation_id
                .clone()
                .unwrap_or_else(|| "snapshot-remote-record".to_string()),
            entity_type: self.entity_type.clone(),
            entity_id: self.entity_id.clone(),
            parent_entity_id: self.parent_entity_id.clone(),
            operation: SyncOperation::parse(&self.operation)?,
            server_version: self.server_version,
            payload_schema_version: self.payload_schema_version,
            payload: self
                .canonical_payload_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|_| SyncError::InvalidData)?,
            deleted_at: self.deleted_at.clone(),
        })
    }
}
