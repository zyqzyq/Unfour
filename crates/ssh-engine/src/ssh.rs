use chrono::Utc;
use sqlx::Row;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use unfour_core::models::{
    SftpCancelTransferInput, SftpDeleteInput, SftpDirectoryListing, SftpFileEntry, SftpOpenResult,
    SftpPathInput, SftpRenameInput, SftpSessionInput, SftpTransferInput, SftpTransferState,
    SshCloseInput, SshConnectInput, SshConnection, SshConnectionConfig, SshConnectionInput,
    SshDiagnosticInput, SshDiagnosticResult, SshHostFingerprintInfo, SshHostKeyInput,
    SshKnownHostsExportInput, SshKnownHostsExportResult, SshKnownHostsImportInput,
    SshKnownHostsImportResult, SshLogExport, SshLogExportInput, SshReconnectCancelInput,
    SshResizeInput, SshSessionEvent, SshSessionInput, SshSessionSummary, SshTestResult,
};
use unfour_core::redaction::redact_sensitive_lines;
use unfour_core::{AppError, AppResult};
use unfour_local_storage::{LocalDb, TerminalHistoryService};
use unfour_secret_store::SecretStore;
use uuid::Uuid;

use crate::host_key::HostKeyStore;

#[path = "diagnostics.rs"]
mod diagnostics;
#[path = "validation.rs"]
mod validation;
use diagnostics::{validate_diagnostic_command, validate_one_shot_command};
use validation::{
    empty_to_none, validate_connection_id, validate_pty_size, validate_session_id,
    validate_workspace_id,
};

mod connection;
mod diagnostic_execution;
mod native_transport;
mod session;
mod session_helpers;
mod sftp;
mod storage;
#[path = "task/mod.rs"]
mod task;

#[cfg(all(feature = "ssh-native", test))]
use native_transport::terminal_output_from_channel_message;
use session_helpers::*;
use storage::*;

/// Callback invoked when terminal output data arrives from a native SSH channel.
/// The payload is a JSON string with `sessionId` and `data` fields.
#[cfg(feature = "ssh-native")]
pub type TerminalOutputCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

/// Callback invoked when an SFTP transfer changes state or reports progress.
#[cfg(feature = "ssh-native")]
pub type SftpTransferCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

/// Callback invoked for SSH Task run, step, output, and transfer events.
#[cfg(feature = "ssh-native")]
pub type TaskRunCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct SshService {
    db: LocalDb,
    task_log_dir: Arc<std::path::PathBuf>,
    #[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
    secret_store: SecretStore,
    terminal_history: TerminalHistoryService,
    sessions: Arc<Mutex<HashMap<String, SshSessionState>>>,
    #[cfg(feature = "ssh-native")]
    on_terminal_output: Arc<Mutex<Option<TerminalOutputCallback>>>,
    #[cfg(feature = "ssh-native")]
    on_sftp_transfer: Arc<Mutex<Option<SftpTransferCallback>>>,
    #[cfg(feature = "ssh-native")]
    transfers: Arc<Mutex<HashMap<String, SftpTransferRuntime>>>,
    #[cfg(feature = "ssh-native")]
    on_task_run: Arc<Mutex<Option<TaskRunCallback>>>,
    #[cfg(feature = "ssh-native")]
    task_runs: Arc<Mutex<HashMap<String, task::TaskRunRuntime>>>,
}

#[derive(Debug, sqlx::FromRow)]
struct StoredSshConnection {
    id: String,
    workspace_id: String,
    name: String,
    host: String,
    port: i64,
    username: String,
    auth_method: String,
    config_json: String,
    credential_ref: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    revision: i64,
    sync_status: String,
    remote_id: Option<String>,
}

#[derive(Debug)]
struct SshConnectionStorageInput {
    host: String,
    port: u16,
    username: String,
    auth_method: String,
    config: SshConnectionConfig,
}

#[derive(Debug)]
struct SshSessionState {
    summary: SshSessionSummary,
    events: Vec<SshSessionEvent>,
    pending_output: String,
    intentional_close: bool,
    #[cfg(feature = "ssh-native")]
    native_handle: Option<NativeSshHandle>,
    #[cfg(feature = "ssh-native")]
    cancel_tx: Option<tokio::sync::watch::Sender<bool>>,
    #[cfg(feature = "ssh-native")]
    sftp: Option<SftpChannelState>,
    #[cfg(feature = "ssh-native")]
    sftp_generation: u64,
}

#[cfg(feature = "ssh-native")]
struct SftpChannelState {
    session: Arc<russh_sftp::client::SftpSession>,
    home_path: String,
    generation: u64,
}

#[cfg(feature = "ssh-native")]
impl std::fmt::Debug for SftpChannelState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SftpChannelState")
            .field("home_path", &self.home_path)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "ssh-native")]
struct SftpTransferRuntime {
    state: SftpTransferState,
    cancel_tx: tokio::sync::watch::Sender<bool>,
}

#[cfg(feature = "ssh-native")]
const KEEPALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);
#[cfg(feature = "ssh-native")]
const KEEPALIVE_MAX_MISSES: usize = 2;
#[cfg(any(feature = "ssh-native", test))]
const RECONNECT_BACKOFF_SECS: [u64; 3] = [1, 2, 4];
#[cfg(feature = "ssh-native")]
const PERSIST_FLUSH_BYTES: usize = 16 * 1024;
#[cfg(feature = "ssh-native")]
const PERSIST_FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
// Terminal output is coalesced and emitted on this cadence (or when the buffer
// reaches EMIT_FLUSH_BYTES) instead of once per SSH packet. A full-screen redraw
// (vim/less/top) arrives as a burst of many small packets; emitting a Tauri
// event for each one floods the WebView2 event IPC on Windows and stalls event
// delivery to the frontend, even though the command IPC keeps working. Batching
// collapses the burst into a few events and keeps the channel responsive.
#[cfg(feature = "ssh-native")]
const EMIT_FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);
#[cfg(feature = "ssh-native")]
const EMIT_FLUSH_BYTES: usize = 16 * 1024;

/// Maximum number of in-memory session events retained per session. The event
/// log is only consumed by `export_log`; older entries are dropped once this
/// cap is reached so a long-lived session cannot grow it without bound.
const MAX_SESSION_EVENTS: usize = 2000;

#[cfg(feature = "ssh-native")]
struct NativeSshHandle {
    handle: std::sync::Arc<tokio::sync::Mutex<russh::client::Handle<SshClientHandler>>>,
    // The channel is split into independent halves so that the supervisor's
    // blocking `wait()` on the read half never holds a lock that keyboard input
    // needs to write. The write half's methods take `&self`, so it can be shared
    // without a mutex; only the supervisor touches the read half.
    writer: std::sync::Arc<russh::ChannelWriteHalf<russh::client::Msg>>,
    reader: std::sync::Arc<tokio::sync::Mutex<russh::ChannelReadHalf>>,
}

#[cfg(feature = "ssh-native")]
impl Clone for NativeSshHandle {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            writer: self.writer.clone(),
            reader: self.reader.clone(),
        }
    }
}

#[cfg(feature = "ssh-native")]
impl std::fmt::Debug for NativeSshHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeSshHandle").finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Native SSH handler (russh client::Handler implementation)
// ---------------------------------------------------------------------------

#[cfg(feature = "ssh-native")]
struct SshClientHandler {
    host_key_store: HostKeyStore,
    workspace_id: String,
    host: String,
    port: u16,
}

#[cfg(feature = "ssh-native")]
impl russh::client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key
            .fingerprint(ssh_key::HashAlg::Sha256)
            .to_string();

        self.host_key_store
            .verify_or_record(&self.workspace_id, &self.host, self.port, &fingerprint)
            .await
            .map(|()| true)
            .map_err(|e| {
                // Convert AppError to a russh::Error that preserves the message.
                russh::Error::IO(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    e.to_string(),
                ))
            })
    }
}

// ---------------------------------------------------------------------------
// SshService implementation
// ---------------------------------------------------------------------------

impl SshService {
    pub fn new(db: LocalDb, secret_store: SecretStore) -> Self {
        Self {
            db: db.clone(),
            task_log_dir: Arc::new(std::env::temp_dir().join("unfour-task-logs")),
            secret_store,
            terminal_history: TerminalHistoryService::new(db.clone()),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(feature = "ssh-native")]
            on_terminal_output: Arc::new(Mutex::new(None)),
            #[cfg(feature = "ssh-native")]
            on_sftp_transfer: Arc::new(Mutex::new(None)),
            #[cfg(feature = "ssh-native")]
            transfers: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(feature = "ssh-native")]
            on_task_run: Arc::new(Mutex::new(None)),
            #[cfg(feature = "ssh-native")]
            task_runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_task_log_dir(mut self, task_log_dir: std::path::PathBuf) -> Self {
        self.task_log_dir = Arc::new(task_log_dir);
        self
    }

    /// Register a callback invoked for every chunk of terminal output data
    /// received from a native SSH channel.  The payload is a JSON string
    /// suitable for forwarding as a Tauri event body.
    #[cfg(feature = "ssh-native")]
    pub fn set_terminal_output_callback(&self, callback: TerminalOutputCallback) {
        if let Ok(mut slot) = self.on_terminal_output.lock() {
            *slot = Some(callback);
        }
    }

    #[cfg(feature = "ssh-native")]
    pub fn set_sftp_transfer_callback(&self, callback: SftpTransferCallback) {
        if let Ok(mut slot) = self.on_sftp_transfer.lock() {
            *slot = Some(callback);
        }
    }

    #[cfg(feature = "ssh-native")]
    pub fn set_task_run_callback(&self, callback: TaskRunCallback) {
        if let Ok(mut slot) = self.on_task_run.lock() {
            *slot = Some(callback);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "ssh_tests/mod.rs"]
mod tests;
