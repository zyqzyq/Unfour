use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::SyncConflictDetails;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SyncError {
    #[error("cloud sync is not authorized")]
    Unauthorized,
    #[error("cloud sync entitlement is unavailable")]
    EntitlementRequired,
    #[error("cloud sync protocol is incompatible")]
    ProtocolIncompatible,
    #[error("cloud sync resource was not found")]
    NotFound,
    #[error("cloud sync data is invalid")]
    InvalidData,
    #[error("cloud sync request failed")]
    Transport,
    #[error("cloud sync storage failed")]
    Storage,
    #[error("cloud sync core apply failed")]
    Core,
    #[error("cloud sync account changed while a request was running")]
    AccountChanged,
    #[error("cloud sync has an unresolved entity conflict")]
    Conflict,
    #[error("the target local workspace is not empty")]
    LocalWorkspaceNotEmpty,
    #[error("an active local workspace already uses the cloud workspace name")]
    WorkspaceNameConflict,
    #[error("safe cloud replacement is not available")]
    SafeReplaceUnavailable,
    #[error("the cloud workspace already contains data and needs an explicit initial direction")]
    CloudWorkspaceNotEmpty,
    #[error("cloud sync is blocked by a dead-letter operation")]
    DeadLetterBlocked,
    #[error("cloud sync rejected a permanent request error")]
    Permanent,
    #[error("the local workspace is owned by another cloud sync account")]
    WorkspaceOwnedByAnotherAccount,
    #[error("cloud sync found multiple historical owners for the local workspace")]
    WorkspaceOwnershipAmbiguous,
    #[error("cloud sync workspace ownership metadata is inconsistent")]
    WorkspaceOwnershipInvariant,
    #[error("cloud sync must restart from a new snapshot")]
    SnapshotRequired,
    #[error("the cloud sync workspace was deleted")]
    WorkspaceDeleted,
}

impl SyncError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "cloud_sync_unauthorized",
            Self::EntitlementRequired => "cloud_sync_entitlement_required",
            Self::ProtocolIncompatible => "cloud_sync_protocol_incompatible",
            Self::NotFound => "cloud_sync_not_found",
            Self::InvalidData => "cloud_sync_invalid_data",
            Self::Transport => "cloud_sync_transport_failed",
            Self::Storage => "cloud_sync_storage_failed",
            Self::Core => "cloud_sync_core_apply_failed",
            Self::AccountChanged => "cloud_sync_account_changed",
            Self::Conflict => "cloud_sync_conflict",
            Self::LocalWorkspaceNotEmpty => "cloud_sync_local_workspace_not_empty",
            Self::WorkspaceNameConflict => "cloud_sync_workspace_name_conflict",
            Self::SafeReplaceUnavailable => "cloud_sync_safe_replace_unavailable",
            Self::CloudWorkspaceNotEmpty => "cloud_sync_cloud_workspace_not_empty",
            Self::DeadLetterBlocked => "cloud_sync_dead_letter_blocked",
            Self::Permanent => "cloud_sync_permanent_failure",
            Self::WorkspaceOwnedByAnotherAccount => "cloud_sync_workspace_owned_by_another_account",
            Self::WorkspaceOwnershipAmbiguous => "cloud_sync_workspace_ownership_ambiguous",
            Self::WorkspaceOwnershipInvariant => "cloud_sync_workspace_ownership_invariant",
            Self::SnapshotRequired => "cloud_sync_snapshot_required",
            Self::WorkspaceDeleted => "cloud_sync_workspace_deleted",
        }
    }
}

impl From<sqlx::Error> for SyncError {
    fn from(_: sqlx::Error) -> Self {
        Self::Storage
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiErrorEnvelope {
    pub error: ApiErrorDetail,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
    pub request_id: String,
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    Account,
    ListWorkspaces,
    CreateWorkspace,
    Push,
    Changes,
    Snapshot,
}

impl SyncPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::ListWorkspaces => "list_workspaces",
            Self::CreateWorkspace => "create_workspace",
            Self::Push => "push",
            Self::Changes => "changes",
            Self::Snapshot => "snapshot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteSyncProblemCategory {
    Auth,
    Entitlement,
    Protocol,
    Conflict,
    OperationPermanent,
    RequestPermanent,
    Workspace,
    SnapshotRequired,
    InvalidResponse,
    Retryable,
    ResultUnknown,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RemoteSyncProblem {
    pub server_error_code: String,
    pub request_id: Option<String>,
    pub http_status: Option<u16>,
    pub phase: SyncPhase,
    pub operation_id: Option<String>,
    pub operation_index: Option<i64>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub category: RemoteSyncProblemCategory,
}

impl RemoteSyncProblem {
    pub fn sync_error(&self) -> SyncError {
        match self.category {
            RemoteSyncProblemCategory::Auth => SyncError::Unauthorized,
            RemoteSyncProblemCategory::Entitlement => SyncError::EntitlementRequired,
            RemoteSyncProblemCategory::Protocol => SyncError::ProtocolIncompatible,
            RemoteSyncProblemCategory::Conflict => SyncError::Conflict,
            RemoteSyncProblemCategory::Workspace
                if self.server_error_code == "sync_workspace_deleted" =>
            {
                SyncError::WorkspaceDeleted
            }
            RemoteSyncProblemCategory::Workspace => SyncError::NotFound,
            RemoteSyncProblemCategory::SnapshotRequired => SyncError::SnapshotRequired,
            RemoteSyncProblemCategory::OperationPermanent
            | RemoteSyncProblemCategory::RequestPermanent => SyncError::Permanent,
            RemoteSyncProblemCategory::InvalidResponse => SyncError::InvalidData,
            RemoteSyncProblemCategory::Retryable | RemoteSyncProblemCategory::ResultUnknown => {
                SyncError::Transport
            }
        }
    }

    pub const fn diagnostic_category(&self) -> &'static str {
        match self.category {
            RemoteSyncProblemCategory::Auth
            | RemoteSyncProblemCategory::Entitlement
            | RemoteSyncProblemCategory::Retryable
            | RemoteSyncProblemCategory::ResultUnknown => "retryable",
            RemoteSyncProblemCategory::Conflict => "conflict",
            // The service only dead-letters an operation after it has matched
            // the server context to the current atomic batch. A malformed or
            // stale operation reference remains an attention-level failure.
            RemoteSyncProblemCategory::OperationPermanent => "permanent",
            RemoteSyncProblemCategory::Protocol
            | RemoteSyncProblemCategory::RequestPermanent
            | RemoteSyncProblemCategory::Workspace
            | RemoteSyncProblemCategory::SnapshotRequired
            | RemoteSyncProblemCategory::InvalidResponse => "permanent",
        }
    }
}

impl fmt::Debug for RemoteSyncProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RemoteSyncProblem")
            .field("server_error_code", &self.server_error_code)
            .field("request_id", &self.request_id)
            .field("http_status", &self.http_status)
            .field("phase", &self.phase)
            .field("operation_id", &self.operation_id)
            .field("operation_index", &self.operation_index)
            .field("entity_type", &self.entity_type)
            .field("entity_id", &self.entity_id)
            .field("category", &self.category)
            .finish()
    }
}

impl fmt::Debug for ApiErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiErrorDetail")
            .field("code", &self.code)
            .field("request_id", &self.request_id)
            .field("details", &self.details.as_ref().map(|_| "[PRESENT]"))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PermanentOperationDetails {
    pub operation_id: String,
    pub error_code: Option<String>,
    pub operation_index: Option<i64>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
}

impl ApiErrorDetail {
    pub(crate) fn conflict_details(&self) -> Option<SyncConflictDetails> {
        self.details
            .as_ref()
            .and_then(|details| serde_json::from_value(details.clone()).ok())
    }

    /// Accept the explicit operation references used by known API revisions,
    /// but never infer an operation from array order or an arbitrary entity id.
    pub(crate) fn permanent_operation_details(&self) -> Option<PermanentOperationDetails> {
        self.details
            .as_ref()
            .and_then(find_permanent_operation_details)
    }
}

fn find_permanent_operation_details(value: &Value) -> Option<PermanentOperationDetails> {
    let object = value.as_object()?;
    if let Some(operation_id) = ["operationId", "failedOperationId"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .filter(|operation_id| !operation_id.trim().is_empty())
    {
        let error_code = ["errorCode", "code", "reasonCode"]
            .iter()
            .find_map(|key| object.get(*key).and_then(Value::as_str))
            .filter(|code| !code.trim().is_empty())
            .map(str::to_string);
        return Some(PermanentOperationDetails {
            operation_id: operation_id.to_string(),
            error_code,
            operation_index: object
                .get("operationIndex")
                .and_then(Value::as_i64)
                .filter(|index| *index >= 0),
            entity_type: object
                .get("entityType")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string),
            entity_id: object
                .get("entityId")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string),
        });
    }

    for key in [
        "failedOperation",
        "operationError",
        "operationDetails",
        "operation",
    ] {
        if let Some(details) = object.get(key) {
            if let Some(found) = find_permanent_operation_details(details) {
                return Some(found);
            }
        }
    }
    None
}
