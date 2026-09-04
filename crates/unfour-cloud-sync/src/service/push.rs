//! Materialize and send bounded batches, then classify their transport outcomes.
//! The repository owns leases and durable attempts; uncertain retries retain payloads.

use std::collections::HashSet;

use unfour_core::domain::{DomainEntityKey, DomainEntityType};

use super::SyncService;
use crate::{
    OutboxEntry, PushOperation, PushRequest, RemoteSyncProblemCategory, SyncAccountContext,
    SyncBinding, SyncEntityType, SyncError, SyncOperation, SyncPhase, TransportError,
    PROTOCOL_VERSION,
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
        // Recheck at the outbound side-effect boundary. A periodic flight can
        // outlive an account switch while it materializes local snapshots;
        // the post-response fence alone would prevent commits but not the old
        // flight from sending a request with the new session. `mark_in_flight`
        // awaits, so the live generation must be rechecked with no await
        // between that fence and `transport.push`.
        self.account_is_current(account)?;
        self.repository
            .mark_in_flight(&entries, &self.worker_id, self.dependencies.clock.now())
            .await?;
        self.account_is_current(account)?;
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
                if let Err(reason) = validate_push_response(&entries, &response) {
                    self.repository
                        .mark_uncertain(&entries, self.dependencies.clock.now())
                        .await?;
                    let _ = self
                        .repository
                        .record_local_diagnostic(
                            &account.account_id,
                            Some(&binding.cloud_workspace_id),
                            "permanent",
                            reason,
                            SyncPhase::Push,
                            self.dependencies.clock.now(),
                        )
                        .await;
                    self.wake_retry_scheduler();
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
            Err(TransportError::RemoteConflict { problem, details }) => {
                self.record_remote_problem(
                    &account.account_id,
                    Some(&binding.cloud_workspace_id),
                    &problem,
                )
                .await;
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
                self.wake_retry_scheduler();
                Err(SyncError::Conflict)
            }
            Err(TransportError::Remote(problem)) => {
                self.record_remote_problem(
                    &account.account_id,
                    Some(&binding.cloud_workspace_id),
                    &problem,
                )
                .await;
                let now = self.dependencies.clock.now();
                let error = problem.sync_error();
                match problem.category {
                    RemoteSyncProblemCategory::Auth | RemoteSyncProblemCategory::Entitlement => {
                        self.repository
                            .mark_not_sent(&entries, error.code(), true, now)
                            .await?;
                        self.wake_retry_scheduler();
                    }
                    RemoteSyncProblemCategory::Protocol => {
                        self.repository
                            .mark_not_sent(&entries, "protocol_version_unsupported", false, now)
                            .await?;
                    }
                    RemoteSyncProblemCategory::OperationPermanent => {
                        let marked = match problem.operation_id.as_deref() {
                            Some(operation_id) => {
                                self.repository
                                    .mark_batch_permanent_failure(
                                        &entries,
                                        operation_id,
                                        &problem.server_error_code,
                                        now,
                                    )
                                    .await?
                            }
                            None => false,
                        };
                        if !marked {
                            self.repository
                                .release_batch_for_attention(
                                    &entries,
                                    &problem.server_error_code,
                                    now,
                                )
                                .await?;
                        }
                    }
                    RemoteSyncProblemCategory::Workspace
                    | RemoteSyncProblemCategory::SnapshotRequired
                    | RemoteSyncProblemCategory::RequestPermanent => {
                        self.repository
                            .release_batch_for_attention(&entries, &problem.server_error_code, now)
                            .await?;
                    }
                    RemoteSyncProblemCategory::InvalidResponse
                    | RemoteSyncProblemCategory::ResultUnknown => {
                        self.repository.mark_uncertain(&entries, now).await?;
                        self.wake_retry_scheduler();
                    }
                    RemoteSyncProblemCategory::Retryable => {
                        self.repository
                            .mark_not_sent(&entries, &problem.server_error_code, true, now)
                            .await?;
                        self.wake_retry_scheduler();
                    }
                    RemoteSyncProblemCategory::Conflict => {
                        self.repository.mark_uncertain(&entries, now).await?;
                        self.wake_retry_scheduler();
                    }
                }
                Err(error)
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
                self.wake_retry_scheduler();
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
                self.wake_retry_scheduler();
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
                    self.repository
                        .release_batch_for_attention(&entries, &code, now)
                        .await?;
                }
                Err(SyncError::Permanent)
            }
            Err(TransportError::Permanent(code)) => {
                self.repository
                    .release_batch_for_attention(&entries, &code, self.dependencies.clock.now())
                    .await?;
                Err(SyncError::Permanent)
            }
            Err(TransportError::NotFound) => {
                self.repository
                    .release_batch_for_attention(
                        &entries,
                        "not_found",
                        self.dependencies.clock.now(),
                    )
                    .await?;
                Err(SyncError::NotFound)
            }
            Err(TransportError::InvalidResponse | TransportError::ResultUnknown) => {
                self.repository
                    .mark_uncertain(&entries, self.dependencies.clock.now())
                    .await?;
                self.wake_retry_scheduler();
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
                self.wake_retry_scheduler();
                Err(SyncError::Transport)
            }
        }
    }
}

fn validate_push_response(
    entries: &[OutboxEntry],
    response: &crate::PushResponse,
) -> Result<(), &'static str> {
    if response.protocol_version != PROTOCOL_VERSION || response.current_cursor < 0 {
        return Err("push_invalid_response");
    }
    if entries.len() != response.results.len() {
        return Err("push_result_count_mismatch");
    }
    let expected = entries
        .iter()
        .map(|entry| entry.operation_id.as_str())
        .collect::<HashSet<_>>();
    let actual = response
        .results
        .iter()
        .map(|result| result.operation_id.as_str())
        .collect::<HashSet<_>>();
    if actual.len() != response.results.len() || actual != expected {
        return Err("push_missing_operation_result");
    }
    Ok(())
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
