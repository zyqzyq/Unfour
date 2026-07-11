use serde::{Deserialize, Serialize};

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
    /// Transient plaintext password (password auth) or key passphrase
    /// (private-key auth). It is written to the OS keychain on save and never
    /// persisted to SQLite; only the resulting credential reference is stored.
    #[serde(default)]
    pub secret: Option<String>,
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
    /// Transient, in-memory credential override used to validate a not-yet-saved
    /// secret (e.g. the "test connection" action in the edit dialog). When set,
    /// it is used for authentication instead of the stored keychain credential
    /// and is never persisted. `None` falls back to the saved credential.
    #[serde(default)]
    pub secret: Option<String>,
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
pub struct SshTestResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectionConfig {
    /// Advanced/private-key-only SSH metadata. Common endpoint and auth method
    /// fields live in `connections` / `ssh_connections` columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
}

/// Input for host-key fingerprint operations (get / reset).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshHostKeyInput {
    pub workspace_id: String,
    pub host: String,
    pub port: u16,
}

/// Information about a stored SSH host-key fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshHostFingerprintInfo {
    pub workspace_id: String,
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

/// Input for exporting known_hosts content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKnownHostsExportInput {
    pub workspace_id: String,
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
