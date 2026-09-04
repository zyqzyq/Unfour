//! Materialize and send bounded batches, then classify their transport outcomes.
//! The repository owns leases and durable attempts; uncertain retries retain payloads.

use unfour_core::domain::{DomainEntityKey, DomainEntityType};

use super::SyncService;
use crate::{
    OutboxEntry, PushOperation, PushRequest, SyncAccountContext, SyncBinding, SyncEntityType,
    SyncError, SyncOperation, TransportError, PROTOCOL_VERSION,
};

const PUSH_BATCH_LIMIT: i64 = 50;
const PUSH_BATCH_MAX_BYTES: usize = 512 * 1024;

impl SyncService {
    pub(super) async fn push_one_batch(
        &self,
        account: &SyncAccountContext,
        binding: &SyncBinding,
    ) -> Result<bool, SyncError> {
        let candidates = self
            .repository
            .due_outbox(
                &account.account_id,
                &binding.cloud_workspace_id,
                self.dependencies.clock.now(),
                PUSH_BATCH_LIMIT,
            )
            .await?;
        if candidates.is_empty() {
            return Ok(false);
        }
        let mut entries = Vec::new();
        let mut operations = Vec::new();
        let mut bytes = 0;
        let mut parked_oversized = false;
        for mut entry in candidates {
            let operation = SyncOperation::parse(&entry.operation)?;
            let needs_snapshot = (operation == SyncOperation::Upsert
                && entry.canonical_payload_json.is_none())
                || (operation == SyncOperation::Delete && entry.deleted_at.is_none());
            if needs_snapshot {
                let entity_type = SyncEntityType::parse(&entry.entity_type)?;
                let mut key = DomainEntityKey::new(
                    DomainEntityType::from(entity_type),
                    &entry.local_workspace_id,
                    &entry.entity_id,
                );
                key.parent_entity_id.clone_from(&entry.parent_entity_id);
                let snapshot = self
                    .core()
                    .await?
                    .read_domain_snapshot(&key)
                    .await
                    .map_err(|_| SyncError::Core)?;
                let Some(materialized) = self
                    .repository
                    .materialize_outbox_entry(&entry, snapshot, self.dependencies.clock.now())
                    .await?
                else {
                    continue;
                };
                entry = materialized;
            }
            let operation = build_push_operation(&entry)?;
            let operation_bytes = serde_json::to_vec(&operation)
                .map_err(|_| SyncError::InvalidData)?
                .len();
            if operation_bytes > PUSH_BATCH_MAX_BYTES {
                // A single operation that can never fit in one push request
                // previously failed the whole round with an opaque error and
                // retried forever. Park it as a standard dead letter instead:
                // it becomes visible in the dead-letter UI and is repairable
                // (retry after shrinking the entity, use-remote, or delete).
                self.repository
                    .mark_not_sent(
                        std::slice::from_ref(&entry),
                        "payload_too_large",
                        false,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                parked_oversized = true;
                continue;
            }
            if !operations.is_empty() && bytes + operation_bytes > PUSH_BATCH_MAX_BYTES {
                break;
            }
            bytes += operation_bytes;
            entries.push(entry);
            operations.push(operation);
        }
        if entries.is_empty() {
            return Ok(parked_oversized);
        }
        self.repository
            .mark_in_flight(&entries, &self.worker_id, self.dependencies.clock.now())
            .await?;
        let request = PushRequest {
            protocol_version: PROTOCOL_VERSION,
            operations,
        };
        let response = self
            .transport
            .push(&binding.cloud_workspace_id, &request)
            .await;
        self.account_is_current(account)?;
        match response {
            Ok(response) => {
                if response.protocol_version != PROTOCOL_VERSION || response.current_cursor < 0 {
                    self.repository
                        .mark_uncertain(&entries, self.dependencies.clock.now())
                        .await?;
                    return Err(SyncError::InvalidData);
                }
                self.repository
                    .apply_push_results(
                        binding,
                        &entries,
                        &response.results,
                        self.dependencies.clock.as_ref(),
                    )
                    .await?;
                Ok(true)
            }
            Err(TransportError::Conflict(details)) => {
                self.repository
                    .mark_not_sent(
                        &entries,
                        "base_version_conflict",
                        true,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                self.repository
                    .record_push_conflict(binding, &details, self.dependencies.clock.now())
                    .await?;
                Err(SyncError::Conflict)
            }
            Err(TransportError::Unauthorized) => {
                self.repository
                    .mark_not_sent(
                        &entries,
                        SyncError::Unauthorized.code(),
                        true,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                Err(SyncError::Unauthorized)
            }
            Err(TransportError::EntitlementRequired) => {
                self.repository
                    .mark_not_sent(
                        &entries,
                        SyncError::EntitlementRequired.code(),
                        true,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                Err(SyncError::EntitlementRequired)
            }
            Err(TransportError::ProtocolIncompatible) => {
                self.repository
                    .mark_not_sent(
                        &entries,
                        "protocol_version_unsupported",
                        false,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                Err(SyncError::ProtocolIncompatible)
            }
            Err(TransportError::PermanentOperation { code, operation_id }) => {
                let now = self.dependencies.clock.now();
                if !self
                    .repository
                    .mark_batch_permanent_failure(&entries, &operation_id, &code, now)
                    .await?
                {
                    // A stale, malformed, or otherwise untrusted operation
                    // reference cannot safely identify a row in this batch.
                    // Preserve the old all-entries fallback instead of
                    // guessing that any operation committed.
                    self.repository
                        .mark_not_sent(&entries, &code, false, now)
                        .await?;
                }
                Err(SyncError::Permanent)
            }
            Err(TransportError::Permanent(code)) => {
                self.repository
                    .mark_not_sent(&entries, &code, false, self.dependencies.clock.now())
                    .await?;
                Err(SyncError::Permanent)
            }
            Err(TransportError::NotFound) => {
                self.repository
                    .mark_not_sent(&entries, "not_found", false, self.dependencies.clock.now())
                    .await?;
                Err(SyncError::NotFound)
            }
            Err(TransportError::InvalidResponse | TransportError::ResultUnknown) => {
                self.repository
                    .mark_uncertain(&entries, self.dependencies.clock.now())
                    .await?;
                Err(SyncError::Transport)
            }
            Err(TransportError::Retryable) => {
                self.repository
                    .mark_not_sent(
                        &entries,
                        "retryable_transport",
                        true,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                Err(SyncError::Transport)
            }
        }
    }
}

fn build_push_operation(entry: &OutboxEntry) -> Result<PushOperation, SyncError> {
    let operation = SyncOperation::parse(&entry.operation)?;
    let payload = entry
        .canonical_payload_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| SyncError::InvalidData)?;
    if (operation == SyncOperation::Upsert) != payload.is_some()
        || (operation == SyncOperation::Delete) != entry.deleted_at.is_some()
    {
        return Err(SyncError::InvalidData);
    }
    Ok(PushOperation {
        operation_id: entry.operation_id.clone(),
        entity_type: SyncEntityType::parse(&entry.entity_type)?,
        entity_id: entry.entity_id.clone(),
        parent_entity_id: entry.parent_entity_id.clone(),
        operation,
        base_version: entry.base_version,
        payload_schema_version: entry.payload_schema_version,
        payload,
    })
}
