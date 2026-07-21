use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTask {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskStep {
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
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskLocalBinding {
    pub task_id: String,
    pub workspace_id: String,
    pub default_connection_id: Option<String>,
    pub last_used_connection_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskDetail {
    pub task: SshTask,
    pub steps: Vec<SshTaskStep>,
    pub local_binding: Option<SshTaskLocalBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskStepInput {
    pub id: Option<String>,
    pub name: String,
    pub step_type: String,
    pub position: i64,
    pub enabled: bool,
    pub config_version: Option<i64>,
    pub config_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskSaveInput {
    pub id: Option<String>,
    pub workspace_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub default_connection_id: Option<String>,
    #[serde(default)]
    pub steps: Vec<SshTaskStepInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskRunInput {
    pub workspace_id: String,
    pub task_id: String,
    pub connection_id: Option<String>,
    #[serde(default)]
    pub inputs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskCancelInput {
    pub workspace_id: String,
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskRun {
    pub id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub connection_id: Option<String>,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
    pub log_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskRunEvent {
    pub run_id: String,
    pub task_id: String,
    pub kind: String,
    pub step_id: Option<String>,
    pub step_name: Option<String>,
    pub step_type: Option<String>,
    pub position: Option<i64>,
    pub status: Option<String>,
    pub stream: Option<String>,
    pub data: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub direction: Option<String>,
    pub transferred_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: Option<u64>,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskCleanupInput {
    pub workspace_id: String,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskCleanupResult {
    pub deleted_runs: usize,
    pub deleted_logs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskCommandConfig {
    pub command: String,
    #[serde(default)]
    pub working_directory: String,
    #[serde(default = "default_task_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub continue_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskUploadConfig {
    pub local_path: String,
    pub remote_path: String,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshTaskDownloadConfig {
    pub remote_path: String,
    pub local_path: String,
    #[serde(default)]
    pub overwrite: bool,
}

fn default_task_timeout_seconds() -> u64 {
    300
}
