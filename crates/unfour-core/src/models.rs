use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub last_opened_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceState {
    pub active_workspace_id: String,
    pub workspaces: Vec<Workspace>,
}

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
    pub folders: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLayout {
    pub workspace_id: String,
    pub sidebar_collapsed: bool,
    pub active_tab_id: String,
    pub tabs: Vec<WorkspaceLayoutTab>,
    pub selected_api_request_id: Option<String>,
    pub selected_database_connection_id: Option<String>,
    pub selected_ssh_connection_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLayoutTab {
    pub id: String,
    pub title: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub folder_path: Option<String>,
    pub collection_id: Option<String>,
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValue>,
    pub query: Vec<KeyValue>,
    pub body: Option<String>,
    pub body_kind: String,
    pub timeout_ms: Option<u64>,
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
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
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
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ApiSavedRequest {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub folder_path: Option<String>,
    pub collection_id: Option<String>,
    pub method: String,
    pub url: String,
    pub headers_json: String,
    pub query_json: String,
    pub body: Option<String>,
    pub body_kind: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnectionInput {
    pub id: Option<String>,
    pub workspace_id: String,
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub username: Option<String>,
    pub sqlite_path: Option<String>,
    pub credential_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectionInput {
    pub id: Option<String>,
    pub workspace_id: String,
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub auth_kind: String,
    pub key_path: Option<String>,
    pub credential_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialCreateInput {
    pub workspace_id: String,
    pub kind: String,
    pub label: String,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialDeleteInput {
    pub workspace_id: String,
    pub credential_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialInspectInput {
    pub workspace_id: String,
    pub credential_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRotateInput {
    pub workspace_id: String,
    pub credential_ref: String,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialMetadata {
    pub workspace_id: String,
    pub kind: String,
    pub label: String,
    pub credential_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnection {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_kind: String,
    pub key_path: Option<String>,
    pub credential_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectInput {
    pub workspace_id: String,
    pub connection_id: String,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

/// Input for a one-shot, read-only SSH diagnostic command. The `command` is
/// validated against a fixed allowlist of read-only utilities before it is run;
/// it is never an interactive shell and never a write/control operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshDiagnosticInput {
    pub workspace_id: String,
    pub connection_id: String,
    pub command: String,
    pub timeout_ms: Option<u64>,
}

/// Result of a one-shot SSH diagnostic command. Captured stdout/stderr are
/// best-effort secret-scrubbed by the surfacing adapter before reaching a
/// client; this struct carries the raw engine output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshDiagnosticResult {
    pub connection_id: String,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_status: Option<i32>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshSessionInput {
    pub workspace_id: String,
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshResizeInput {
    pub workspace_id: String,
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshCloseInput {
    pub workspace_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshReconnectCancelInput {
    pub workspace_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshLogExportInput {
    pub workspace_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshSessionSummary {
    pub session_id: String,
    pub workspace_id: String,
    pub connection_id: String,
    pub status: String,
    pub reconnect_attempt: u8,
    pub auth_kind: String,
    pub host: String,
    pub username: String,
    pub cols: u16,
    pub rows: u16,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshSessionEvent {
    pub session_id: String,
    pub kind: String,
    pub data: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshLogExport {
    pub session_id: String,
    pub filename: String,
    pub content: String,
    pub line_count: usize,
    pub redacted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectionConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_kind: String,
    pub key_path: Option<String>,
}

/// Input for host-key fingerprint operations (get / reset).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshHostKeyInput {
    pub host: String,
    pub port: u16,
}

/// Information about a stored SSH host-key fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshHostFingerprintInfo {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub created_at: String,
}

/// Input for importing known_hosts content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKnownHostsImportInput {
    pub workspace_id: String,
    pub content: String,
}

/// Result of a known_hosts import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKnownHostsImportResult {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
}

/// Result of a known_hosts export operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKnownHostsExportResult {
    pub content: String,
    pub entry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnection {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub username: Option<String>,
    pub sqlite_path: Option<String>,
    pub credential_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StoredConnection {
    pub id: String,
    pub workspace_id: String,
    pub kind: String,
    pub name: String,
    pub config_json: String,
    pub credential_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnectionConfig {
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub username: Option<String>,
    pub sqlite_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTestResult {
    pub ok: bool,
    pub message: String,
    pub server_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSchema {
    pub connection_id: String,
    pub tables: Vec<DatabaseTable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTable {
    pub schema: Option<String>,
    pub name: String,
    pub kind: String,
    pub columns: Vec<DatabaseTableColumn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTableColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    #[serde(default)]
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseIndex {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseForeignKey {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTableStructureInput {
    pub workspace_id: String,
    pub connection_id: String,
    pub schema: Option<String>,
    pub table_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseCellValue {
    pub column: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseRowMutationInput {
    pub workspace_id: String,
    pub connection_id: String,
    pub schema: Option<String>,
    pub table_name: String,
    /// One of: "insert", "update", "delete".
    pub operation: String,
    /// Column values to write for insert/update operations.
    #[serde(default)]
    pub values: Vec<DatabaseCellValue>,
    /// Primary-key columns identifying the row for update/delete operations.
    #[serde(default)]
    pub primary_key: Vec<DatabaseCellValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseRowMutationResult {
    pub affected_rows: u64,
    pub sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTableStructure {
    pub schema: Option<String>,
    pub name: String,
    pub kind: String,
    pub columns: Vec<DatabaseTableColumn>,
    pub indexes: Vec<DatabaseIndex>,
    pub foreign_keys: Vec<DatabaseForeignKey>,
    pub ddl: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseQueryInput {
    pub workspace_id: String,
    pub connection_id: String,
    pub sql: String,
    pub limit: Option<u32>,
    pub confirm_mutation: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DbQueryHistoryEntry {
    pub id: String,
    pub workspace_id: String,
    pub connection_id: Option<String>,
    pub connection_name: String,
    pub sql: String,
    pub status: String,
    pub classification: Option<String>,
    pub row_count: Option<i64>,
    pub affected_rows: Option<i64>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
    pub executed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbQueryHistoryRecordInput {
    pub id: String,
    pub workspace_id: String,
    pub connection_id: Option<String>,
    pub connection_name: String,
    pub sql: String,
    pub status: String,
    pub classification: Option<String>,
    pub row_count: Option<i64>,
    pub affected_rows: Option<i64>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
    pub executed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseBrowseInput {
    pub workspace_id: String,
    pub connection_id: String,
    pub schema: Option<String>,
    pub table_name: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseBrowseResult {
    pub table_name: String,
    pub sql: String,
    pub limit: u32,
    pub offset: u32,
    pub total_rows: u64,
    pub read_only: bool,
    pub result: DatabaseQueryResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseQueryResult {
    pub columns: Vec<DatabaseResultColumn>,
    pub rows: Vec<Vec<Option<String>>>,
    pub affected_rows: u64,
    pub duration_ms: u128,
    pub safety: DatabaseQuerySafety,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseQuerySafety {
    pub classification: String,
    pub requires_confirmation: bool,
    pub confirmed: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseResultColumn {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealth {
    pub app_name: String,
    pub storage_ready: bool,
    pub command_bus_ready: bool,
    pub ai_reserved_capabilities: Vec<String>,
    pub sync_strategy: String,
}
