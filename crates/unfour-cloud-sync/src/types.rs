use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use unfour_core::domain::{DomainEntityKey, DomainEntityType, MutationOperation};

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
        }
    }
}

impl From<sqlx::Error> for SyncError {
    fn from(_: sqlx::Error) -> Self {
        Self::Storage
    }
}

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub trait IdGenerator: Send + Sync {
    fn next_id(&self) -> String;
}

#[derive(Default)]
pub struct UuidGenerator;

impl IdGenerator for UuidGenerator {
    fn next_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncAccountContext {
    pub account_id: String,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncWorkspaceOwner {
    pub account_id: String,
    pub cloud_workspace_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncEntityType {
    Workspace,
    Connection,
    WorkspaceVariable,
    WorkspaceEnvironment,
    WorkspaceEnvironmentVariable,
    ApiCollection,
    ApiFolder,
    ApiRequest,
    SshTask,
    SshTaskStep,
}

impl SyncEntityType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Connection => "connection",
            Self::WorkspaceVariable => "workspaceVariable",
            Self::WorkspaceEnvironment => "workspaceEnvironment",
            Self::WorkspaceEnvironmentVariable => "workspaceEnvironmentVariable",
            Self::ApiCollection => "apiCollection",
            Self::ApiFolder => "apiFolder",
            Self::ApiRequest => "apiRequest",
            Self::SshTask => "sshTask",
            Self::SshTaskStep => "sshTaskStep",
        }
    }

    pub const fn topology_rank(self) -> i64 {
        match self {
            Self::Workspace => 0,
            Self::WorkspaceVariable
            | Self::Connection
            | Self::WorkspaceEnvironment
            | Self::ApiCollection
            | Self::SshTask => 1,
            Self::WorkspaceEnvironmentVariable | Self::ApiFolder | Self::SshTaskStep => 2,
            Self::ApiRequest => 3,
        }
    }

    pub fn parse(value: &str) -> Result<Self, SyncError> {
        match value {
            "workspace" => Ok(Self::Workspace),
            "connection" => Ok(Self::Connection),
            "workspaceVariable" => Ok(Self::WorkspaceVariable),
            "workspaceEnvironment" => Ok(Self::WorkspaceEnvironment),
            "workspaceEnvironmentVariable" => Ok(Self::WorkspaceEnvironmentVariable),
            "apiCollection" => Ok(Self::ApiCollection),
            "apiFolder" => Ok(Self::ApiFolder),
            "apiRequest" => Ok(Self::ApiRequest),
            "sshTask" => Ok(Self::SshTask),
            "sshTaskStep" => Ok(Self::SshTaskStep),
            _ => Err(SyncError::InvalidData),
        }
    }
}

impl From<DomainEntityType> for SyncEntityType {
    fn from(value: DomainEntityType) -> Self {
        match value {
            DomainEntityType::Workspace => Self::Workspace,
            DomainEntityType::Connection => Self::Connection,
            DomainEntityType::WorkspaceVariable => Self::WorkspaceVariable,
            DomainEntityType::WorkspaceEnvironment => Self::WorkspaceEnvironment,
            DomainEntityType::WorkspaceEnvironmentVariable => Self::WorkspaceEnvironmentVariable,
            DomainEntityType::ApiCollection => Self::ApiCollection,
            DomainEntityType::ApiFolder => Self::ApiFolder,
            DomainEntityType::ApiRequest => Self::ApiRequest,
            DomainEntityType::SshTask => Self::SshTask,
            DomainEntityType::SshTaskStep => Self::SshTaskStep,
        }
    }
}

impl From<SyncEntityType> for DomainEntityType {
    fn from(value: SyncEntityType) -> Self {
        match value {
            SyncEntityType::Workspace => Self::Workspace,
            SyncEntityType::Connection => Self::Connection,
            SyncEntityType::WorkspaceVariable => Self::WorkspaceVariable,
            SyncEntityType::WorkspaceEnvironment => Self::WorkspaceEnvironment,
            SyncEntityType::WorkspaceEnvironmentVariable => Self::WorkspaceEnvironmentVariable,
            SyncEntityType::ApiCollection => Self::ApiCollection,
            SyncEntityType::ApiFolder => Self::ApiFolder,
            SyncEntityType::ApiRequest => Self::ApiRequest,
            SyncEntityType::SshTask => Self::SshTask,
            SyncEntityType::SshTaskStep => Self::SshTaskStep,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncOperation {
    Upsert,
    Delete,
}

impl SyncOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Delete => "delete",
        }
    }

    pub fn parse(value: &str) -> Result<Self, SyncError> {
        match value {
            "upsert" => Ok(Self::Upsert),
            "delete" => Ok(Self::Delete),
            _ => Err(SyncError::InvalidData),
        }
    }
}

impl From<MutationOperation> for SyncOperation {
    fn from(value: MutationOperation) -> Self {
        match value {
            MutationOperation::Upsert => Self::Upsert,
            MutationOperation::Delete => Self::Delete,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudWorkspace {
    pub cloud_workspace_id: String,
    pub root_entity_id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub current_cursor: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCloudWorkspaceRequest {
    pub protocol_version: u32,
    pub root_entity_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PushOperation {
    pub operation_id: String,
    pub entity_type: SyncEntityType,
    pub entity_id: String,
    pub parent_entity_id: Option<String>,
    pub operation: SyncOperation,
    pub base_version: i64,
    pub payload_schema_version: i64,
    pub payload: Option<Value>,
}

impl fmt::Debug for PushOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PushOperation")
            .field("operation_id", &self.operation_id)
            .field("entity_type", &self.entity_type)
            .field("entity_id", &self.entity_id)
            .field("operation", &self.operation)
            .field("base_version", &self.base_version)
            .field("payload", &self.payload.as_ref().map(|_| "[CANONICAL]"))
            .finish()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PushRequest {
    pub protocol_version: u32,
    pub operations: Vec<PushOperation>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum PushResultStatus {
    #[serde(rename = "applied")]
    Applied,
    #[serde(rename = "noOp")]
    NoOp,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PushResult {
    pub operation_id: String,
    pub server_version: i64,
    pub cursor: i64,
    pub status: PushResultStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PushResponse {
    pub protocol_version: u32,
    pub current_cursor: i64,
    pub results: Vec<PushResult>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteChange {
    pub cursor: i64,
    pub operation_id: String,
    pub entity_type: SyncEntityType,
    pub entity_id: String,
    pub parent_entity_id: Option<String>,
    pub operation: SyncOperation,
    pub server_version: i64,
    pub payload_schema_version: i64,
    pub payload: Option<Value>,
    pub deleted_at: Option<String>,
}

impl fmt::Debug for RemoteChange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RemoteChange")
            .field("cursor", &self.cursor)
            .field("operation_id", &self.operation_id)
            .field("entity_type", &self.entity_type)
            .field("entity_id", &self.entity_id)
            .field("operation", &self.operation)
            .field("server_version", &self.server_version)
            .field("payload", &self.payload.as_ref().map(|_| "[CANONICAL]"))
            .finish()
    }
}

impl RemoteChange {
    pub fn key(&self, workspace_id: &str) -> DomainEntityKey {
        let mut key = DomainEntityKey::new(
            DomainEntityType::from(self.entity_type),
            workspace_id,
            &self.entity_id,
        );
        key.parent_entity_id.clone_from(&self.parent_entity_id);
        key
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangesPage {
    pub protocol_version: u32,
    pub cloud_workspace_id: String,
    pub current_cursor: i64,
    pub next_cursor: i64,
    pub changes: Vec<RemoteChange>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotItem {
    pub entity_type: SyncEntityType,
    pub entity_id: String,
    pub parent_entity_id: Option<String>,
    pub server_version: i64,
    pub payload_schema_version: i64,
    pub payload: Value,
}

impl fmt::Debug for SnapshotItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SnapshotItem")
            .field("entity_type", &self.entity_type)
            .field("entity_id", &self.entity_id)
            .field("server_version", &self.server_version)
            .field("payload", &"[CANONICAL]")
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotPage {
    pub protocol_version: u32,
    pub cloud_workspace_id: String,
    pub at_cursor: i64,
    pub current_cursor: i64,
    pub items: Vec<SnapshotItem>,
    pub next_page_token: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncConflictDetails {
    pub entity_type: SyncEntityType,
    pub entity_id: String,
    pub parent_entity_id: Option<String>,
    pub server_version: i64,
    pub operation: SyncOperation,
    pub payload_schema_version: Option<i64>,
    pub payload: Option<Value>,
}

impl fmt::Debug for SyncConflictDetails {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SyncConflictDetails")
            .field("entity_type", &self.entity_type)
            .field("entity_id", &self.entity_id)
            .field("server_version", &self.server_version)
            .field("operation", &self.operation)
            .field("payload", &self.payload.as_ref().map(|_| "[CANONICAL]"))
            .finish()
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
}

impl ApiErrorDetail {
    pub(crate) fn conflict_details(&self) -> Option<SyncConflictDetails> {
        self.details
            .as_ref()
            .and_then(|details| serde_json::from_value(details.clone()).ok())
    }

    /// Operation failures have been returned by more than one API revision:
    /// current responses put `operationId` directly in `details`, while an
    /// intermediate response nested it under `failedOperation`. Accept those
    /// explicit operation references, but never infer an operation from array
    /// order or from an entity that is merely mentioned in the message.
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
        });
    }

    // Keep this list deliberately narrow. These keys describe an operation
    // reference in the API error contract; arbitrary recursive searching
    // would risk treating an unrelated payload id as the failed operation.
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

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OutboxEntry {
    pub account_id: String,
    pub operation_id: String,
    pub local_workspace_id: String,
    pub cloud_workspace_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub parent_entity_id: Option<String>,
    pub operation: String,
    pub base_version: i64,
    pub payload_schema_version: i64,
    pub canonical_payload_json: Option<String>,
    pub deleted_at: Option<String>,
    pub content_revision: i64,
    pub status: String,
    pub attempt_count: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadLetterView {
    pub operation_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub entity_name: Option<String>,
    pub error_code: String,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncBinding {
    pub account_id: String,
    pub local_workspace_id: String,
    pub cloud_workspace_id: String,
    pub last_pulled_cursor: i64,
    pub sync_enabled: bool,
    pub state: String,
    pub initial_cursor: Option<i64>,
    pub initial_total: i64,
    pub initial_confirmed: i64,
    pub initialization_checkpoint: Option<String>,
    pub ssh_task_v3_bootstrap_state: String,
    pub connection_v4_bootstrap_state: String,
    pub generation: i64,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub consecutive_failure_count: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflict {
    pub account_id: String,
    pub cloud_workspace_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub server_version: i64,
    pub conflict_remote_payload_json: Option<String>,
    pub conflict_remote_operation: Option<String>,
    pub conflict_parent_entity_id: Option<String>,
    pub conflict_deleted_at: Option<String>,
    pub conflict_operation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictView {
    pub cloud_workspace_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub server_version: i64,
    pub operation: String,
    pub local_payload: Option<Value>,
    pub remote_payload: Option<Value>,
    pub local_secret_present: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub binding: Option<SyncBinding>,
    pub pending_count: i64,
    pub uncertain_count: i64,
    pub in_flight_count: i64,
    pub dead_count: i64,
    pub dead_letters: Vec<DeadLetterView>,
    pub conflict_count: i64,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDiagnostics {
    pub local_workspace_id: String,
    pub remote_workspace_id: String,
    pub last_push_at: Option<String>,
    pub last_pull_at: Option<String>,
    pub pending_outbox_count: i64,
    pub dead_outbox_count: i64,
    pub dead_letters: Vec<DeadLetterView>,
    pub pull_cursor: i64,
    pub last_error_code: Option<String>,
    pub consecutive_failure_count: i64,
    pub next_retry_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadDecision {
    Cancel,
    DownloadToNewWorkspace,
}

#[derive(Clone)]
pub struct SyncDependencies {
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn IdGenerator>,
}

impl Default for SyncDependencies {
    fn default() -> Self {
        Self {
            clock: Arc::new(SystemClock),
            ids: Arc::new(UuidGenerator),
        }
    }
}

impl fmt::Debug for SyncDependencies {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SyncDependencies { clock: .., ids: .. }")
    }
}
