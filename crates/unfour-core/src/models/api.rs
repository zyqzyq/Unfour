use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Largest timeout that can cross the JavaScript boundary without losing
/// integer precision. This is an input-safety bound, not a practical timeout
/// policy or duration cap.
pub const MAX_API_TIMEOUT_MS: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEnvironment {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub variables: Vec<KeyValue>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCollection {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ApiCollectionFolder {
    pub id: String,
    pub workspace_id: String,
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequestInput {
    pub workspace_id: String,
    pub name: Option<String>,
    pub parent_folder_id: Option<String>,
    pub collection_id: Option<String>,
    #[serde(default)]
    pub auth_json: Option<String>,
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValue>,
    pub query: Vec<KeyValue>,
    pub body: Option<String>,
    pub body_kind: String,
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub pre_request_script: Option<String>,
    #[serde(default)]
    pub post_response_script: Option<String>,
    #[serde(default = "default_script_schema_version")]
    pub script_schema_version: i64,
    #[serde(default)]
    pub temporary_variables: Vec<KeyValue>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequestSettings {
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiClientPreferences {
    #[serde(default)]
    pub request_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse {
    pub history_id: String,
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<KeyValue>,
    pub body: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ApiHistoryItem {
    pub id: String,
    pub workspace_id: String,
    pub name: Option<String>,
    pub method: String,
    pub url: String,
    pub status: Option<i64>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ApiHistoryDetail {
    pub id: String,
    pub workspace_id: String,
    pub name: Option<String>,
    pub method: String,
    pub url: String,
    pub request_headers_json: String,
    pub request_query_json: String,
    pub request_body: Option<String>,
    pub status: Option<i64>,
    pub duration_ms: Option<i64>,
    pub response_headers_json: String,
    pub response_body_preview: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ApiSavedRequest {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub sort_order: i64,
    pub auth_json: String,
    pub method: String,
    pub url: String,
    pub headers_json: String,
    pub query_json: String,
    pub body: Option<String>,
    pub body_kind: String,
    #[serde(default = "default_request_settings_json")]
    pub settings_json: String,
    #[serde(default)]
    pub pre_request_script: Option<String>,
    #[serde(default)]
    pub post_response_script: Option<String>,
    #[serde(default = "default_script_schema_version")]
    pub script_schema_version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

fn default_script_schema_version() -> i64 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptExecutionStatus {
    Skipped,
    Success,
    Failed,
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptConsoleLevel {
    Log,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptConsoleEntry {
    pub level: ScriptConsoleLevel,
    pub message: String,
    pub sequence: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTestResult {
    pub name: String,
    pub passed: bool,
    pub error_message: Option<String>,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptErrorKind {
    Runtime,
    Timeout,
    Validation,
    Limit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptError {
    pub kind: ScriptErrorKind,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptExecutionResult {
    pub status: ScriptExecutionStatus,
    pub duration_ms: u128,
    pub console: Vec<ScriptConsoleEntry>,
    pub tests: Vec<ScriptTestResult>,
    pub error: Option<ScriptError>,
}

impl ScriptExecutionResult {
    pub fn skipped() -> Self {
        Self {
            status: ScriptExecutionStatus::Skipped,
            duration_ms: 0,
            console: Vec::new(),
            tests: Vec::new(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestExecutionResult {
    pub response: Option<ApiResponse>,
    pub http_error: Option<String>,
    #[serde(default)]
    pub http_error_code: Option<String>,
    pub pre_request: ScriptExecutionResult,
    pub post_response: ScriptExecutionResult,
}

fn default_request_settings_json() -> String {
    r#"{"timeoutMs":null}"#.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiCollectionExportFormat {
    Json,
    Yaml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCollectionExportArtifact {
    pub content: String,
    pub media_type: String,
    pub suggested_file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCollectionExportResult {
    pub saved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCollectionImportResult {
    pub imported: bool,
    pub collection: Option<ApiCollection>,
    pub folder_count: u32,
    pub request_count: u32,
}
