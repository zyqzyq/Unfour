//! Version 1 portable business DTOs, independent of storage and sync envelopes.
use serde::{Deserialize, Serialize};
use unfour_core::{
    domain::SnapshotVariableValue,
    models::{FlowInputDefinition, FlowStep},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
    pub environment_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceVariableRecord {
    pub id: String,
    pub key: String,
    pub value: SnapshotVariableValue,
    pub is_secret: bool,
    pub is_enabled: bool,
    pub description: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceEnvironmentRecord {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceEnvironmentVariableRecord {
    pub id: String,
    pub environment_id: String,
    pub key: String,
    pub value: SnapshotVariableValue,
    pub is_secret: bool,
    pub is_enabled: bool,
    pub description: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiCollectionRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiFolderRecord {
    pub id: String,
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiRequestRecord {
    pub id: String,
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub auth_json: String,
    pub method: String,
    pub url: String,
    pub headers: Vec<unfour_core::models::KeyValue>,
    pub query: Vec<unfour_core::models::KeyValue>,
    pub body: Option<String>,
    pub body_kind: String,
    pub settings_json: String,
    pub pre_request_script: Option<String>,
    pub post_response_script: Option<String>,
    pub script_schema_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SshTaskRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SshTaskStepRecord {
    pub id: String,
    pub task_id: String,
    pub name: String,
    pub step_type: String,
    pub position: i64,
    pub enabled: bool,
    pub config_version: i64,
    pub config_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionRecord {
    pub id: String,
    pub connection_type: String,
    pub name: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub config: BundleConnectionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedSqlRecord {
    pub id: String,
    pub connection_id: Option<String>,
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub name: String,
    pub sql: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowRecord {
    pub id: String,
    pub name: String,
    pub inputs: Vec<FlowInputDefinition>,
    pub steps: Vec<FlowStep>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceBundle {
    pub format: String,
    pub version: u32,
    pub workspace: WorkspaceRecord,
    pub variables: Vec<WorkspaceVariableRecord>,
    pub environments: Vec<WorkspaceEnvironmentRecord>,
    pub environment_variables: Vec<WorkspaceEnvironmentVariableRecord>,
    pub collections: Vec<ApiCollectionRecord>,
    pub folders: Vec<ApiFolderRecord>,
    pub requests: Vec<ApiRequestRecord>,
    pub connections: Vec<ConnectionRecord>,
    pub ssh_tasks: Vec<SshTaskRecord>,
    pub ssh_steps: Vec<SshTaskStepRecord>,
    pub saved_sql: Vec<SavedSqlRecord>,
    pub flows: Vec<FlowRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_paths: Vec<LocalPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credential_requirements: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_templates: Vec<super::templates::ApiTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalPath {
    pub entity_id: String,
    pub field: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BundleConnectionConfig {
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
