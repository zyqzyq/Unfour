use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MutationOrigin {
    Local,
    External,
    Migration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MutationOperation {
    Upsert,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DomainEntityType {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainEntityKey {
    pub entity_type: DomainEntityType,
    pub workspace_id: String,
    pub entity_id: String,
    #[serde(default)]
    pub parent_entity_id: Option<String>,
}

impl DomainEntityKey {
    pub fn new(
        entity_type: DomainEntityType,
        workspace_id: impl Into<String>,
        entity_id: impl Into<String>,
    ) -> Self {
        Self {
            entity_type,
            workspace_id: workspace_id.into(),
            entity_id: entity_id.into(),
            parent_entity_id: None,
        }
    }

    pub fn with_parent_entity_id(mut self, parent_entity_id: impl Into<String>) -> Self {
        self.parent_entity_id = Some(parent_entity_id.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainMutation {
    pub origin: MutationOrigin,
    pub operation: MutationOperation,
    pub entity: DomainEntityKey,
    pub revision: i64,
}

impl DomainMutation {
    pub fn new(
        origin: MutationOrigin,
        operation: MutationOperation,
        entity: DomainEntityKey,
        revision: i64,
    ) -> Self {
        Self {
            origin,
            operation,
            entity,
            revision,
        }
    }

    pub fn with_parent_entity_id(mut self, parent_entity_id: impl Into<String>) -> Self {
        self.entity.parent_entity_id = Some(parent_entity_id.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandContext {
    pub command_id: String,
    pub command_name: String,
    pub origin: MutationOrigin,
}

impl CommandContext {
    pub fn new(command_name: impl Into<String>, origin: MutationOrigin) -> Self {
        Self {
            command_id: crate::id::new_id(),
            command_name: command_name.into(),
            origin,
        }
    }

    pub fn local(command_name: impl Into<String>) -> Self {
        Self::new(command_name, MutationOrigin::Local)
    }

    pub fn external(command_name: impl Into<String>) -> Self {
        Self::new(command_name, MutationOrigin::External)
    }

    pub fn migration(command_name: impl Into<String>) -> Self {
        Self::new(command_name, MutationOrigin::Migration)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainCommandResult<T> {
    pub value: T,
    pub mutations: Vec<DomainMutation>,
}

impl<T> DomainCommandResult<T> {
    pub fn new(value: T, mutations: Vec<DomainMutation>) -> Self {
        Self { value, mutations }
    }

    pub fn unchanged(value: T) -> Self {
        Self {
            value,
            mutations: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum SnapshotVariableValue {
    Plain(String),
    SecretRedacted,
}

impl fmt::Debug for SnapshotVariableValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plain(value) => formatter.debug_tuple("Plain").field(value).finish(),
            Self::SecretRedacted => formatter.write_str("SecretRedacted"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub id: String,
    pub name: String,
    pub environment_type: String,
    pub mcp_policy: String,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConnectionSnapshotConfig {
    Ssh {
        username: String,
        auth_method: String,
    },
    Database {
        driver: String,
        database_name: Option<String>,
        username: Option<String>,
        ssl_mode: Option<String>,
        read_only: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub connection_type: String,
    pub name: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub config: ConnectionSnapshotConfig,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceVariableSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub key: String,
    pub value: SnapshotVariableValue,
    pub is_secret: bool,
    pub is_enabled: bool,
    pub description: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEnvironmentSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEnvironmentVariableSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub environment_id: String,
    pub key: String,
    pub value: SnapshotVariableValue,
    pub is_secret: bool,
    pub is_enabled: bool,
    pub description: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCollectionSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiFolderSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequestSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub auth_json: String,
    pub method: String,
    pub url: String,
    pub headers: Vec<crate::models::KeyValue>,
    pub query: Vec<crate::models::KeyValue>,
    pub body: Option<String>,
    pub body_kind: String,
    pub pre_request_script: Option<String>,
    pub post_response_script: Option<String>,
    pub script_schema_version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskStepSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub name: String,
    pub step_type: String,
    pub position: i64,
    pub enabled: bool,
    pub config_version: i64,
    pub config_json: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TombstoneSnapshot {
    pub entity: DomainEntityKey,
    pub deleted_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "entityType", content = "snapshot", rename_all = "camelCase")]
pub enum DomainSnapshot {
    Workspace(WorkspaceSnapshot),
    Connection(ConnectionSnapshot),
    WorkspaceVariable(WorkspaceVariableSnapshot),
    WorkspaceEnvironment(WorkspaceEnvironmentSnapshot),
    WorkspaceEnvironmentVariable(WorkspaceEnvironmentVariableSnapshot),
    ApiCollection(ApiCollectionSnapshot),
    ApiFolder(ApiFolderSnapshot),
    ApiRequest(ApiRequestSnapshot),
    SshTask(SshTaskSnapshot),
    SshTaskStep(SshTaskStepSnapshot),
    Tombstone(TombstoneSnapshot),
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "value", rename_all = "camelCase")]
pub enum ExternalVariableValue {
    Set(String),
    PreserveLocal,
    Clear,
}

impl fmt::Debug for ExternalVariableValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Set(_) => formatter.write_str("Set([REDACTED])"),
            Self::PreserveLocal => formatter.write_str("PreserveLocal"),
            Self::Clear => formatter.write_str("Clear"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalDelete {
    pub entity: DomainEntityKey,
    pub deleted_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalWorkspaceUpsert {
    pub id: String,
    pub name: String,
    pub environment_type: String,
    pub mcp_policy: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalConnectionUpsert {
    pub id: String,
    pub workspace_id: String,
    pub connection_type: String,
    pub name: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub config: ConnectionSnapshotConfig,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalWorkspaceVariableUpsert {
    pub id: String,
    pub workspace_id: String,
    pub key: String,
    pub value: ExternalVariableValue,
    pub is_secret: bool,
    pub is_enabled: bool,
    pub description: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalWorkspaceEnvironmentUpsert {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalWorkspaceEnvironmentVariableUpsert {
    pub id: String,
    pub workspace_id: String,
    pub environment_id: String,
    pub key: String,
    pub value: ExternalVariableValue,
    pub is_secret: bool,
    pub is_enabled: bool,
    pub description: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalApiCollectionUpsert {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalApiFolderUpsert {
    pub id: String,
    pub workspace_id: String,
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalApiRequestUpsert {
    pub id: String,
    pub workspace_id: String,
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub auth_json: String,
    pub method: String,
    pub url: String,
    pub headers: Vec<crate::models::KeyValue>,
    pub query: Vec<crate::models::KeyValue>,
    pub body: Option<String>,
    pub body_kind: String,
    pub pre_request_script: Option<String>,
    pub post_response_script: Option<String>,
    pub script_schema_version: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSshTaskUpsert {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSshTaskStepUpsert {
    pub id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub name: String,
    pub step_type: String,
    pub position: i64,
    pub enabled: bool,
    pub config_version: i64,
    pub config_json: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

macro_rules! external_change {
    ($name:ident, $upsert:ty) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "operation", content = "record", rename_all = "camelCase")]
        pub enum $name {
            Upsert($upsert),
            Delete(ExternalDelete),
        }
    };
}

external_change!(ExternalWorkspaceApply, ExternalWorkspaceUpsert);
external_change!(ExternalConnectionApply, ExternalConnectionUpsert);
external_change!(
    ExternalWorkspaceVariableApply,
    ExternalWorkspaceVariableUpsert
);
external_change!(
    ExternalWorkspaceEnvironmentApply,
    ExternalWorkspaceEnvironmentUpsert
);
external_change!(
    ExternalWorkspaceEnvironmentVariableApply,
    ExternalWorkspaceEnvironmentVariableUpsert
);
external_change!(ExternalApiCollectionApply, ExternalApiCollectionUpsert);
external_change!(ExternalApiFolderApply, ExternalApiFolderUpsert);
external_change!(ExternalSshTaskApply, ExternalSshTaskUpsert);
external_change!(ExternalSshTaskStepApply, ExternalSshTaskStepUpsert);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "record", rename_all = "camelCase")]
pub enum ExternalApiRequestApply {
    Upsert(Box<ExternalApiRequestUpsert>),
    Delete(ExternalDelete),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalApplyPage {
    pub workspaces: Vec<ExternalWorkspaceApply>,
    #[serde(default)]
    pub connections: Vec<ExternalConnectionApply>,
    pub workspace_variables: Vec<ExternalWorkspaceVariableApply>,
    pub workspace_environments: Vec<ExternalWorkspaceEnvironmentApply>,
    pub workspace_environment_variables: Vec<ExternalWorkspaceEnvironmentVariableApply>,
    #[serde(default)]
    pub api_collections: Vec<ExternalApiCollectionApply>,
    #[serde(default)]
    pub api_folders: Vec<ExternalApiFolderApply>,
    #[serde(default)]
    pub api_requests: Vec<ExternalApiRequestApply>,
    #[serde(default)]
    pub ssh_tasks: Vec<ExternalSshTaskApply>,
    #[serde(default)]
    pub ssh_task_steps: Vec<ExternalSshTaskStepApply>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SecretMaterialStatus {
    Present,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMaterialOutcome {
    pub entity: DomainEntityKey,
    pub status: SecretMaterialStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalApplyReport {
    pub applied_count: usize,
    pub mutations: Vec<DomainMutation>,
    pub secret_material_outcomes: Vec<SecretMaterialOutcome>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_variable_value_debug_never_exposes_the_value() {
        let value = ExternalVariableValue::Set("top-secret".to_string());
        let debug = format!("{value:?}");
        assert!(!debug.contains("top-secret"));
        assert!(debug.contains("REDACTED"));
    }

    #[test]
    fn legacy_external_apply_pages_default_new_api_collections() {
        let page = serde_json::from_value::<ExternalApplyPage>(serde_json::json!({
            "workspaces": [],
            "workspaceVariables": [],
            "workspaceEnvironments": [],
            "workspaceEnvironmentVariables": [],
        }))
        .unwrap();
        assert!(page.api_collections.is_empty());
        assert!(page.connections.is_empty());
        assert!(page.api_folders.is_empty());
        assert!(page.api_requests.is_empty());
        assert!(page.ssh_tasks.is_empty());
        assert!(page.ssh_task_steps.is_empty());
    }
}
