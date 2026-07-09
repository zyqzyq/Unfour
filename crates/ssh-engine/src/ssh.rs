use chrono::Utc;
use sqlx::Row;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use unfour_core::models::{
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

/// Callback invoked when terminal output data arrives from a native SSH channel.
/// The payload is a JSON string with `sessionId` and `data` fields.
#[cfg(feature = "ssh-native")]
pub type TerminalOutputCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct SshService {
    db: LocalDb,
    #[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
    secret_store: SecretStore,
    terminal_history: TerminalHistoryService,
    sessions: Arc<Mutex<HashMap<String, SshSessionState>>>,
    #[cfg(feature = "ssh-native")]
    on_terminal_output: Arc<Mutex<Option<TerminalOutputCallback>>>,
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
            secret_store,
            terminal_history: TerminalHistoryService::new(db.clone()),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(feature = "ssh-native")]
            on_terminal_output: Arc::new(Mutex::new(None)),
        }
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

    pub async fn list_connections(&self, workspace_id: String) -> AppResult<Vec<SshConnection>> {
        validate_workspace_id(&workspace_id)?;

        let rows = sqlx::query_as::<_, StoredSshConnection>(
            r#"
            SELECT
              c.id, c.workspace_id, c.name, c.host, c.port,
              sub.username, sub.auth_method, sub.config_json, c.credential_ref,
              c.created_at, c.updated_at, c.deleted_at, c.revision, c.sync_status, c.remote_id
            FROM connections c
            INNER JOIN ssh_connections sub ON sub.connection_id = c.id
            WHERE c.workspace_id = ?1 AND c.connection_type = 'ssh' AND c.deleted_at IS NULL
            ORDER BY c.updated_at DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter().map(stored_to_ssh_connection).collect()
    }

    pub async fn save_connection(&self, input: SshConnectionInput) -> AppResult<SshConnection> {
        validate_workspace_id(&input.workspace_id)?;
        let name = normalize_name(&input.name)?;
        let storage = input_to_storage(&input)?;
        let credential_ref = self
            .resolve_credential_ref(
                &input.workspace_id,
                &storage.auth_method,
                empty_to_none(input.credential_ref.clone()),
                input.secret.clone(),
            )
            .await?;
        let now = Utc::now().to_rfc3339();
        let config_json = ssh_config_to_json(&storage.config)?;

        if let Some(id) = input
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            let result = sqlx::query(
                r#"
                UPDATE connections
                SET name = ?1, host = ?2, port = ?3, credential_ref = ?4,
                    updated_at = ?5, revision = revision + 1, sync_status = 'pending'
                WHERE id = ?6 AND workspace_id = ?7 AND connection_type = 'ssh' AND deleted_at IS NULL
                "#,
            )
            .bind(name)
            .bind(&storage.host)
            .bind(i64::from(storage.port))
            .bind(credential_ref)
            .bind(&now)
            .bind(id)
            .bind(&input.workspace_id)
            .execute(self.db.pool())
            .await?;

            if result.rows_affected() == 0 {
                return Err(AppError::NotFound("ssh connection".to_string()));
            }

            sqlx::query(
                r#"
                UPDATE ssh_connections
                SET username = ?1, auth_method = ?2, config_json = ?3
                WHERE connection_id = ?4
                "#,
            )
            .bind(&storage.username)
            .bind(&storage.auth_method)
            .bind(&config_json)
            .bind(id)
            .execute(self.db.pool())
            .await?;

            return self.get_connection(&input.workspace_id, id).await;
        }

        let id = unfour_core::id::new_id();
        sqlx::query(
            r#"
            INSERT INTO connections (
              id, workspace_id, connection_type, name, host, port, credential_ref,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, 'ssh', ?3, ?4, ?5, ?6, ?7, ?7, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(name)
        .bind(&storage.host)
        .bind(i64::from(storage.port))
        .bind(credential_ref)
        .bind(now)
        .execute(self.db.pool())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO ssh_connections (connection_id, username, auth_method, config_json)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(&id)
        .bind(&storage.username)
        .bind(&storage.auth_method)
        .bind(&config_json)
        .execute(self.db.pool())
        .await?;

        self.get_connection(&input.workspace_id, &id).await
    }

    pub async fn delete_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<Vec<SshConnection>> {
        validate_workspace_id(&workspace_id)?;
        validate_connection_id(&connection_id)?;
        let now = Utc::now().to_rfc3339();

        // Read the credential reference before soft-deleting so the stored
        // secret can be purged from the OS keychain; otherwise it leaks as an
        // orphaned credential.
        let existing = sqlx::query(
            "SELECT credential_ref FROM connections \
             WHERE id = ?1 AND workspace_id = ?2 \
               AND connection_type = 'ssh' AND deleted_at IS NULL",
        )
        .bind(&connection_id)
        .bind(&workspace_id)
        .fetch_optional(self.db.pool())
        .await?;
        let credential_ref: Option<String> = existing
            .and_then(|row| row.try_get::<Option<String>, _>("credential_ref").ok())
            .flatten();

        let result = sqlx::query(
            r#"
            UPDATE connections
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1, sync_status = 'deleted'
            WHERE id = ?2 AND workspace_id = ?3
              AND connection_type = 'ssh' AND deleted_at IS NULL
            "#,
        )
        .bind(now)
        .bind(&connection_id)
        .bind(&workspace_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("ssh connection".to_string()));
        }

        // Best-effort purge of the stored secret. A failure here (e.g. the
        // credential was already removed) must not block the deletion itself;
        // the keychain backend already logs failures.
        if let Some(credential_ref) = credential_ref.filter(|value| !value.is_empty()) {
            let _ = self
                .secret_store
                .delete_credential(workspace_id.clone(), credential_ref)
                .await;
        }

        self.close_sessions_for_connection(&workspace_id, &connection_id)
            .await?;
        self.terminal_history
            .delete_connection_history(&workspace_id, &connection_id)
            .await?;

        self.list_connections(workspace_id).await
    }

    pub async fn test_connection(&self, input: SshConnectionInput) -> AppResult<SshTestResult> {
        validate_workspace_id(&input.workspace_id)?;
        let storage = input_to_storage(&input)?;
        let now = Utc::now().to_rfc3339();
        let temp_id = Uuid::new_v4().to_string();
        let connection = SshConnection {
            id: temp_id.clone(),
            workspace_id: input.workspace_id.clone(),
            name: input.name.trim().to_string(),
            host: storage.host,
            port: storage.port,
            username: storage.username,
            auth_kind: storage.auth_method,
            key_path: storage.config.key_path,
            credential_ref: input.credential_ref.clone(),
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
            revision: 0,
            sync_status: "new".to_string(),
            remote_id: None,
        };
        let connect_input = SshConnectInput {
            workspace_id: input.workspace_id.clone(),
            connection_id: temp_id,
            cols: Some(80),
            rows: Some(24),
            secret: input.secret.clone(),
        };
        let started = Instant::now();
        let fields = serde_json::json!({
            "auth_method": &connection.auth_kind,
            "host": &connection.host,
            "port": connection.port,
        });
        unfour_diag::log_operation_event(
            "ssh_test_started",
            "ssh",
            "test_connection",
            "started",
            None,
            None,
            fields.clone(),
        );

        #[cfg(feature = "ssh-native")]
        let result = self.connect_native(&connection, &connect_input).await;
        #[cfg(not(feature = "ssh-native"))]
        let result = self.connect_simulated(&connection, &connect_input).await;
        match result {
            Ok(summary) => {
                unfour_diag::log_operation_event(
                    "ssh_test_succeeded",
                    "ssh",
                    "test_connection",
                    "ok",
                    Some(started.elapsed().as_millis()),
                    None,
                    fields,
                );
                let _ = self
                    .close_session(SshCloseInput {
                        workspace_id: summary.workspace_id.clone(),
                        session_id: summary.session_id.clone(),
                    })
                    .await;
                Ok(SshTestResult {
                    ok: true,
                    message: format!(
                        "Connected to {}@{} successfully",
                        summary.username, summary.host
                    ),
                })
            }
            Err(error) => {
                unfour_diag::log_operation_event(
                    "ssh_test_failed",
                    "ssh",
                    "test_connection",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some(unfour_diag::app_error_kind(&error)),
                    fields,
                );
                Ok(SshTestResult {
                    ok: false,
                    message: error.to_string(),
                })
            }
        }
    }

    pub async fn connect(&self, input: SshConnectInput) -> AppResult<SshSessionSummary> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        validate_connection_ready_for_session(&connection)?;
        let started = Instant::now();
        let fields = serde_json::json!({
            "auth_method": &connection.auth_kind,
            "host": &connection.host,
            "port": connection.port,
        });
        unfour_diag::log_operation_event(
            "ssh_connect_started",
            "ssh",
            "connect",
            "started",
            None,
            None,
            fields.clone(),
        );

        #[cfg(feature = "ssh-native")]
        let result = self.connect_native(&connection, &input).await;
        #[cfg(not(feature = "ssh-native"))]
        let result = self.connect_simulated(&connection, &input).await;
        let summary = match result {
            Ok(summary) => {
                unfour_diag::log_operation_event(
                    "ssh_connect_succeeded",
                    "ssh",
                    "connect",
                    "ok",
                    Some(started.elapsed().as_millis()),
                    None,
                    fields,
                );
                summary
            }
            Err(error) => {
                unfour_diag::log_operation_event(
                    "ssh_connect_failed",
                    "ssh",
                    "connect",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some(unfour_diag::app_error_kind(&error)),
                    fields,
                );
                return Err(error);
            }
        };

        self.terminal_history.save_session(&summary).await?;
        self.terminal_history
            .append_output(
                &summary.workspace_id,
                &summary.session_id,
                &summary.connection_id,
                &format!(
                    "Connected to {}@{}. PTY {}x{} allocated.\r\n",
                    summary.username, summary.host, summary.cols, summary.rows
                ),
            )
            .await?;
        Ok(summary)
    }

    /// Run a single, read-only diagnostic command over SSH and capture its
    /// output. The command is validated against a fixed allowlist of read-only
    /// utilities (no shell, no chaining, no write/control operations) before it
    /// is executed. Captured output is line-redacted for sensitive material.
    /// Requires the `ssh-native` feature; otherwise returns an unsupported error.
    pub async fn run_diagnostic(
        &self,
        input: SshDiagnosticInput,
    ) -> AppResult<SshDiagnosticResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let command = validate_diagnostic_command(&input.command)?;
        let timeout = std::time::Duration::from_millis(
            input.timeout_ms.unwrap_or(15_000).clamp(1_000, 60_000),
        );
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        validate_connection_ready_for_session(&connection)?;

        #[cfg(feature = "ssh-native")]
        {
            self.run_diagnostic_native(&connection, &command, timeout)
                .await
        }
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (timeout, &connection, &command);
            Err(AppError::Unsupported(
                "ssh diagnostics require a build with the ssh-native feature".to_string(),
            ))
        }
    }

    /// Run a single non-interactive SSH command over a fresh native connection.
    /// This lower-level execution primitive intentionally performs only basic
    /// command-shape validation; policy, environment gating, and high-risk
    /// confirmation live in the command-bus/MCP adapter path before this method
    /// is called.
    pub async fn run_command(&self, input: SshDiagnosticInput) -> AppResult<SshDiagnosticResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let command = validate_one_shot_command(&input.command)?;
        let timeout = std::time::Duration::from_millis(
            input.timeout_ms.unwrap_or(15_000).clamp(1_000, 60_000),
        );
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        validate_connection_ready_for_session(&connection)?;

        #[cfg(feature = "ssh-native")]
        {
            self.run_diagnostic_native(&connection, &command, timeout)
                .await
        }
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (timeout, &connection, &command);
            Err(AppError::Unsupported(
                "ssh command execution requires a build with the ssh-native feature".to_string(),
            ))
        }
    }

    pub async fn list_sessions(&self, workspace_id: String) -> AppResult<Vec<SshSessionSummary>> {
        validate_workspace_id(&workspace_id)?;
        let active_sessions = self
            .sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?
            .values()
            .filter(|state| state.summary.workspace_id == workspace_id)
            .map(|state| state.summary.clone())
            .collect::<Vec<_>>();
        let mut sessions = self.terminal_history.list_sessions(&workspace_id).await?;
        for active in active_sessions {
            if let Some(stored) = sessions
                .iter_mut()
                .find(|stored| stored.session_id == active.session_id)
            {
                *stored = active;
            } else {
                sessions.push(active);
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    pub async fn session_history(&self, input: SshCloseInput) -> AppResult<Vec<SshSessionEvent>> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;
        self.flush_session_history(&input.session_id).await?;
        self.terminal_history
            .hydrate(&input.workspace_id, &input.session_id)
            .await
    }

    #[cfg(feature = "ssh-native")]
    fn buffer_session_output(&self, session_id: &str, output: &str) -> bool {
        if output.is_empty() {
            return false;
        }
        let Ok(mut sessions) = self.sessions.lock() else {
            return false;
        };
        let Some(state) = sessions.get_mut(session_id) else {
            return false;
        };
        state.pending_output.push_str(output);
        state.pending_output.len() >= PERSIST_FLUSH_BYTES
    }

    /// Flush a session's buffered output to the database on a detached task.
    ///
    /// Persistence must never run inline on the SSH read loop: awaiting the
    /// database write there stalls draining of russh's bounded channel buffer,
    /// which back-pressures the session task and blocks outgoing keystroke
    /// writes. Concurrent flushes each take a disjoint slice of the pending
    /// buffer under the lock, so the worst case is out-of-order history rows
    /// (cosmetic) rather than lost or duplicated output.
    #[cfg(feature = "ssh-native")]
    fn spawn_flush_session_history(&self, session_id: &str) {
        let service = self.clone();
        let session_id = session_id.to_string();
        tokio::spawn(async move {
            let _ = service.flush_session_history(&session_id).await;
        });
    }

    async fn flush_session_history(&self, session_id: &str) -> AppResult<()> {
        let pending = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            let Some(state) = sessions.get_mut(session_id) else {
                return Ok(());
            };
            if state.pending_output.is_empty() {
                return Ok(());
            }
            (
                state.summary.workspace_id.clone(),
                state.summary.connection_id.clone(),
                std::mem::take(&mut state.pending_output),
            )
        };

        if let Err(error) = self
            .terminal_history
            .append_output(&pending.0, session_id, &pending.1, &pending.2)
            .await
        {
            // Re-acquire the lock only on the error path so the guard is not
            // held across the await above (a held `std::sync::MutexGuard` across
            // an await makes the future non-`Send`).
            if let Ok(mut sessions) = self.sessions.lock() {
                if let Some(state) = sessions.get_mut(session_id) {
                    state.pending_output.insert_str(0, &pending.2);
                }
            }
            return Err(error);
        }
        Ok(())
    }

    async fn persist_session_summary(&self, session_id: &str) -> AppResult<()> {
        let summary = self
            .sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?
            .get(session_id)
            .map(|state| state.summary.clone());
        if let Some(summary) = summary {
            self.terminal_history.update_session(&summary).await?;
        }
        Ok(())
    }

    pub async fn send_input(&self, input: SshSessionInput) -> AppResult<SshSessionEvent> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;
        if input.data.is_empty() {
            return Err(AppError::Validation(
                "ssh input cannot be empty".to_string(),
            ));
        }

        // Write to native channel if available.
        #[cfg(feature = "ssh-native")]
        {
            let native_handle: Option<NativeSshHandle> = {
                let sessions = self
                    .sessions
                    .lock()
                    .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
                let state =
                    session_for_workspace(&sessions, &input.workspace_id, &input.session_id)?;
                ensure_session_active(state)?;
                state.native_handle.clone()
            };
            if let Some(native) = native_handle {
                native
                    .writer
                    .data_bytes(input.data.clone().into_bytes())
                    .await
                    .map_err(|e| {
                        AppError::Config(format!("failed to write to ssh channel: {}", e))
                    })?;
            }
        }

        let event = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            let state =
                session_for_workspace_mut(&mut sessions, &input.workspace_id, &input.session_id)?;
            ensure_session_active(state)?;

            let now = Utc::now().to_rfc3339();
            let input_event = SshSessionEvent {
                session_id: input.session_id.clone(),
                kind: "input".to_string(),
                data: redact_ssh_log(&input.data).0,
                created_at: now.clone(),
            };
            record_session_event(state, input_event.clone());
            let event = SshSessionEvent {
                session_id: input.session_id.clone(),
                kind: "output".to_string(),
                data: "Input accepted by SSH PTY stream.\r\n".to_string(),
                created_at: now.clone(),
            };
            #[cfg(not(feature = "ssh-native"))]
            {
                // Persist the redacted input together with the terminal output
                // so it survives after the in-memory session entry is dropped on
                // close (issue #4) and still appears in exported logs.
                state.pending_output.push_str(&input_event.data);
                record_session_event(state, event.clone());
                state.pending_output.push_str(&event.data);
            }
            state.summary.updated_at = now;
            event
        };
        #[cfg(not(feature = "ssh-native"))]
        self.flush_session_history(&input.session_id).await?;
        Ok(event)
    }

    pub async fn resize(&self, input: SshResizeInput) -> AppResult<SshSessionEvent> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;
        validate_pty_size(input.cols, input.rows)?;
        let started = Instant::now();
        let fields = serde_json::json!({
            "cols": input.cols,
            "rows": input.rows,
        });

        // Propagate resize to native channel.
        #[cfg(feature = "ssh-native")]
        {
            let native_handle: Option<NativeSshHandle> = {
                let sessions = self
                    .sessions
                    .lock()
                    .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
                let state =
                    session_for_workspace(&sessions, &input.workspace_id, &input.session_id)?;
                ensure_session_active(state)?;
                state.native_handle.clone()
            };
            if let Some(native) = native_handle {
                let _ = native
                    .writer
                    .window_change(input.cols as u32, input.rows as u32, 0, 0)
                    .await;
            }
        }

        let event = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            let state =
                session_for_workspace_mut(&mut sessions, &input.workspace_id, &input.session_id)?;
            ensure_session_active(state)?;

            let now = Utc::now().to_rfc3339();
            state.summary.cols = input.cols;
            state.summary.rows = input.rows;
            state.summary.updated_at = now.clone();
            let event = SshSessionEvent {
                session_id: input.session_id.clone(),
                kind: "resize".to_string(),
                data: format!("PTY resized to {}x{}.\r\n", input.cols, input.rows),
                created_at: now,
            };
            record_session_event(state, event.clone());
            event
        };
        self.persist_session_summary(&input.session_id).await?;
        unfour_diag::log_operation_event(
            "ssh_pty_resize",
            "ssh",
            "resize",
            "ok",
            Some(started.elapsed().as_millis()),
            None,
            fields,
        );
        Ok(event)
    }

    // Return the persisted, disconnected summary for a session that was already
    // closed and dropped from memory, so a repeated close is idempotent.
    async fn persisted_disconnected_summary(
        &self,
        workspace_id: &str,
        session_id: &str,
    ) -> AppResult<SshSessionSummary> {
        match self
            .terminal_history
            .get_session(workspace_id, session_id)
            .await?
        {
            Some(mut summary) => {
                summary.status = "disconnected".to_string();
                summary.reconnect_attempt = 0;
                Ok(summary)
            }
            None => Err(AppError::NotFound("ssh session".to_string())),
        }
    }

    pub async fn close_session(&self, input: SshCloseInput) -> AppResult<SshSessionSummary> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;
        let started = Instant::now();
        unfour_diag::log_operation_event(
            "ssh_disconnect_started",
            "ssh",
            "close_session",
            "started",
            None,
            None,
            serde_json::json!({}),
        );

        // Extract the native handle and mark the session as intentionally closed
        // while the lock is held. The guard is dropped at the end of this block,
        // so it is never held across an await.
        #[cfg(feature = "ssh-native")]
        let native_handle: Option<NativeSshHandle> = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            match sessions.get_mut(&input.session_id) {
                Some(state) if state.summary.workspace_id == input.workspace_id => {
                    state.intentional_close = true;
                    if let Some(cancel_tx) = state.cancel_tx.take() {
                        let _ = cancel_tx.send(true);
                    }
                    state.native_handle.take()
                }
                _ => None,
            }
        };

        // Close the native transport outside the mutex lock.
        #[cfg(feature = "ssh-native")]
        if let Some(native) = native_handle {
            let _ = native.writer.close().await;
            let handle = native.handle.lock().await;
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "session closed", "en")
                .await;
        }

        // Mark the session disconnected and read back its summary. The lock block
        // ends (releasing the guard) before any await, keeping the future `Send`.
        let found: Option<SshSessionSummary> = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            match sessions.get_mut(&input.session_id) {
                Some(state) if state.summary.workspace_id == input.workspace_id => {
                    if state.summary.status != "disconnected" {
                        let now = Utc::now().to_rfc3339();
                        state.intentional_close = true;
                        state.summary.status = "disconnected".to_string();
                        state.summary.reconnect_attempt = 0;
                        state.summary.updated_at = now;
                        state.pending_output.push_str("SSH session closed.\r\n");
                    }
                    Some(state.summary.clone())
                }
                _ => None,
            }
        };

        // Idempotent path: the entry was already closed and dropped from memory,
        // so return the persisted disconnected summary. The lock is already
        // released here, so this await does not hold a `std::sync::MutexGuard`.
        let summary = match found {
            Some(summary) => summary,
            None => {
                return self
                    .persisted_disconnected_summary(&input.workspace_id, &input.session_id)
                    .await;
            }
        };

        // Persist terminal output and the final summary, then drop the in-memory
        // entry. The entry is removed unconditionally even when persistence
        // fails, so a persistence error cannot leak the session (#4). None of
        // these holds a `self.sessions` guard across an await.
        let flush_result = self.flush_session_history(&input.session_id).await;
        let update_result = self.terminal_history.update_session(&summary).await;
        self.sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?
            .remove(&input.session_id);
        flush_result?;
        update_result?;
        unfour_diag::log_operation_event(
            "ssh_disconnected",
            "ssh",
            "close_session",
            "ok",
            Some(started.elapsed().as_millis()),
            None,
            serde_json::json!({ "status": &summary.status }),
        );
        Ok(summary)
    }

    pub async fn cancel_reconnect(
        &self,
        input: SshReconnectCancelInput,
    ) -> AppResult<SshSessionSummary> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;

        #[cfg(feature = "ssh-native")]
        let native_handle = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            match sessions.get_mut(&input.session_id) {
                Some(state) if state.summary.workspace_id == input.workspace_id => {
                    state.intentional_close = true;
                    if let Some(cancel_tx) = state.cancel_tx.take() {
                        let _ = cancel_tx.send(true);
                    }
                    state.native_handle.take()
                }
                _ => None,
            }
        };

        // Close the native transport outside the mutex lock. Match
        // `close_session`: shut the channel writer, then send an SSH-level
        // disconnect so the transport is torn down gracefully instead of being
        // dropped mid-stream (#11).
        #[cfg(feature = "ssh-native")]
        if let Some(native) = native_handle {
            let _ = native.writer.close().await;
            let handle = native.handle.lock().await;
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "session closed", "en")
                .await;
        }

        // Mark disconnected, persist terminal output, then drop the in-memory
        // entry so the session map cannot grow without bound (#4). The entry is
        // removed unconditionally even when persistence fails, so a persistence
        // error cannot leak the session.
        let summary = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            let state =
                session_for_workspace_mut(&mut sessions, &input.workspace_id, &input.session_id)?;
            let now = Utc::now().to_rfc3339();
            state.intentional_close = true;
            state.summary.status = "disconnected".to_string();
            state.summary.reconnect_attempt = 0;
            state.summary.updated_at = now.clone();
            state
                .pending_output
                .push_str("SSH reconnect cancelled.\r\n");
            state.summary.clone()
        };
        let flush_result = self.flush_session_history(&input.session_id).await;
        let update_result = self.terminal_history.update_session(&summary).await;
        self.sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?
            .remove(&input.session_id);
        flush_result?;
        update_result?;
        Ok(summary)
    }

    pub async fn export_log(&self, input: SshLogExportInput) -> AppResult<SshLogExport> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;
        // Fast path: a live session still holds its structured event log in
        // memory, so export directly from there.
        if let Some(state) = self
            .sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?
            .get(&input.session_id)
        {
            if state.summary.workspace_id == input.workspace_id {
                return Ok(build_ssh_log_export(
                    &state.summary.session_id,
                    &state.events,
                ));
            }
        }
        // Closed session: the in-memory entry was dropped to bound memory, so
        // export the persisted, redacted terminal output instead.
        let history = self
            .terminal_history
            .hydrate(&input.workspace_id, &input.session_id)
            .await?;
        if history.is_empty() {
            return Err(AppError::NotFound("ssh session".to_string()));
        }
        Ok(build_ssh_log_export(&input.session_id, &history))
    }

    pub fn capability_summary(&self) -> serde_json::Value {
        let transport = if cfg!(feature = "ssh-native") {
            "russh-native"
        } else {
            "simulated"
        };
        serde_json::json!({
            "status": "terminal-streaming",
            "transport": transport,
            "features": [
                "connection-metadata-crud",
                "credential-ref-boundary",
                "password-auth-session",
                "private-key-auth-session",
                "no-auth-session",
                "host-key-tofu",
                "host-key-fingerprint-management",
                "graceful-close",
                "session-close",
                "redacted-log-export",
                "pty-channel",
                "terminal-streaming",
                "resize-propagation"
            ]
        })
    }

    // -----------------------------------------------------------------------
    // Host-key fingerprint management
    // -----------------------------------------------------------------------

    /// Look up the trusted host-key fingerprint for a host:port pair.
    pub async fn get_host_fingerprint(
        &self,
        input: SshHostKeyInput,
    ) -> AppResult<Option<SshHostFingerprintInfo>> {
        validate_workspace_id(&input.workspace_id)?;
        let host = input.host.trim().to_string();
        if host.is_empty() {
            return Err(AppError::Validation("host cannot be empty".to_string()));
        }
        if input.port == 0 {
            return Err(AppError::Validation("port cannot be 0".to_string()));
        }

        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        match host_key_store
            .get_fingerprint_info(&input.workspace_id, &host, input.port)
            .await?
        {
            Some((fingerprint, created_at)) => Ok(Some(SshHostFingerprintInfo {
                workspace_id: input.workspace_id,
                host,
                port: input.port,
                fingerprint,
                created_at,
            })),
            None => Ok(None),
        }
    }

    /// Remove the stored fingerprint for a host:port pair, allowing the next
    /// connection to establish a new trust (TOFU reset).
    pub async fn reset_host_fingerprint(&self, input: SshHostKeyInput) -> AppResult<bool> {
        validate_workspace_id(&input.workspace_id)?;
        let host = input.host.trim().to_string();
        if host.is_empty() {
            return Err(AppError::Validation("host cannot be empty".to_string()));
        }
        if input.port == 0 {
            return Err(AppError::Validation("port cannot be 0".to_string()));
        }

        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        host_key_store
            .delete_fingerprint(&input.workspace_id, &host, input.port)
            .await
    }

    /// List all stored host-key fingerprints in a workspace.
    pub async fn list_all_host_fingerprints(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<SshHostFingerprintInfo>> {
        validate_workspace_id(&workspace_id)?;
        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        let entries = host_key_store.list_all(&workspace_id).await?;
        Ok(entries
            .into_iter()
            .map(|entry| SshHostFingerprintInfo {
                workspace_id: entry.workspace_id,
                host: entry.host,
                port: entry.port.clamp(0, u16::MAX as i64) as u16,
                fingerprint: entry.fingerprint,
                created_at: entry.created_at,
            })
            .collect())
    }

    /// Import entries from OpenSSH known_hosts content.
    pub async fn import_known_hosts(
        &self,
        input: SshKnownHostsImportInput,
    ) -> AppResult<SshKnownHostsImportResult> {
        validate_workspace_id(&input.workspace_id)?;
        if input.content.trim().is_empty() {
            return Err(AppError::Validation(
                "known_hosts content cannot be empty".to_string(),
            ));
        }
        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        host_key_store
            .import_known_hosts(&input.workspace_id, &input.content)
            .await
    }

    /// Export stored fingerprints to OpenSSH known_hosts format.
    pub async fn export_known_hosts(
        &self,
        input: SshKnownHostsExportInput,
    ) -> AppResult<SshKnownHostsExportResult> {
        validate_workspace_id(&input.workspace_id)?;
        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        let (content, entry_count) = host_key_store
            .export_known_hosts(&input.workspace_id)
            .await?;
        Ok(SshKnownHostsExportResult {
            content,
            entry_count,
        })
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Resolve the credential reference to persist for a connection. A plaintext
    /// `secret` is written to the OS keychain (creating a new reference, or
    /// rotating the existing one) so SQLite only ever stores the reference.
    async fn resolve_credential_ref(
        &self,
        workspace_id: &str,
        auth_kind: &str,
        existing_ref: Option<String>,
        secret: Option<String>,
    ) -> AppResult<Option<String>> {
        let secret = secret.filter(|value| !value.is_empty());
        match auth_kind {
            // No authentication: never keep a stored secret.
            "none" => Ok(None),
            "password" => match secret {
                Some(secret) => Ok(Some(
                    self.store_secret(workspace_id, "ssh-password", existing_ref, secret)
                        .await?,
                )),
                // Editing without changing the password keeps the existing
                // reference; a brand-new password connection must supply one.
                None => match existing_ref {
                    Some(existing) => Ok(Some(existing)),
                    None => Err(AppError::Validation(
                        "password ssh auth requires a password".to_string(),
                    )),
                },
            },
            // Private-key passphrase is optional (unencrypted keys need none).
            "private-key" => match secret {
                Some(secret) => Ok(Some(
                    self.store_secret(workspace_id, "ssh-key-passphrase", existing_ref, secret)
                        .await?,
                )),
                None => Ok(existing_ref),
            },
            _ => Ok(existing_ref),
        }
    }

    /// Persist a plaintext secret to the keychain, rotating an existing
    /// reference when present so the stored reference stays stable.
    async fn store_secret(
        &self,
        workspace_id: &str,
        kind: &str,
        existing_ref: Option<String>,
        secret: String,
    ) -> AppResult<String> {
        match existing_ref {
            Some(existing) => {
                self.secret_store
                    .rotate_credential(workspace_id.to_string(), existing.clone(), secret)
                    .await?;
                Ok(existing)
            }
            None => {
                let metadata = self
                    .secret_store
                    .create_credential(
                        workspace_id.to_string(),
                        kind.to_string(),
                        format!("ssh {} credential", kind),
                        secret,
                    )
                    .await?;
                Ok(metadata.credential_ref)
            }
        }
    }

    async fn get_connection(&self, workspace_id: &str, id: &str) -> AppResult<SshConnection> {
        validate_workspace_id(workspace_id)?;
        validate_connection_id(id)?;

        let row = sqlx::query_as::<_, StoredSshConnection>(
            r#"
            SELECT
              c.id, c.workspace_id, c.name, c.host, c.port,
              sub.username, sub.auth_method, sub.config_json, c.credential_ref,
              c.created_at, c.updated_at, c.deleted_at, c.revision, c.sync_status, c.remote_id
            FROM connections c
            INNER JOIN ssh_connections sub ON sub.connection_id = c.id
            WHERE c.id = ?1 AND c.workspace_id = ?2
              AND c.connection_type = 'ssh' AND c.deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(stored_to_ssh_connection)
            .transpose()?
            .ok_or_else(|| AppError::NotFound("ssh connection".to_string()))
    }

    async fn close_sessions_for_connection(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();

        // Collect native handles under the lock, then disconnect outside it.
        #[cfg(feature = "ssh-native")]
        let native_handles: Vec<NativeSshHandle> = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            sessions
                .values()
                .filter(|state| {
                    state.summary.workspace_id == workspace_id
                        && state.summary.connection_id == connection_id
                        && is_live_status(&state.summary.status)
                })
                .filter_map(|state| state.native_handle.clone())
                .collect()
        };

        // Mark live sessions disconnected, capture their ids, and append the
        // close notice to the persisted terminal output.
        let session_ids: Vec<String> = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            let mut ids = Vec::new();
            for state in sessions.values_mut().filter(|state| {
                state.summary.workspace_id == workspace_id
                    && state.summary.connection_id == connection_id
                    && is_live_status(&state.summary.status)
            }) {
                #[cfg(feature = "ssh-native")]
                {
                    state.intentional_close = true;
                    if let Some(cancel_tx) = state.cancel_tx.take() {
                        let _ = cancel_tx.send(true);
                    }
                    state.native_handle.take();
                }

                state.summary.status = "disconnected".to_string();
                state.summary.reconnect_attempt = 0;
                state.summary.updated_at = now.clone();
                state.pending_output.push_str(
                    "SSH session closed because the connection was deleted.\r\n",
                );
                ids.push(state.summary.session_id.clone());
            }
            ids
        };

        // Flush buffered terminal output to the database before dropping entries.
        for id in &session_ids {
            let _ = self.flush_session_history(id).await;
        }

        // Drop the in-memory entries so the session map cannot grow without
        // bound across the process lifetime (#4).
        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            sessions.retain(|id, _| !session_ids.contains(id));
        }

        // Disconnect native handles outside the mutex lock.
        #[cfg(feature = "ssh-native")]
        for native in native_handles {
            // Close the channel first.
            let _ = native.writer.close().await;
            let handle = native.handle.lock().await;
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "connection deleted", "en")
                .await;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Simulated connect (non ssh-native path)
    // -----------------------------------------------------------------------

    #[cfg(not(feature = "ssh-native"))]
    async fn connect_simulated(
        &self,
        connection: &SshConnection,
        input: &SshConnectInput,
    ) -> AppResult<SshSessionSummary> {
        let now = Utc::now().to_rfc3339();
        let session_id = unfour_core::id::new_id();
        let cols = input.cols.unwrap_or(120).clamp(20, 300);
        let rows = input.rows.unwrap_or(32).clamp(8, 100);
        let summary = SshSessionSummary {
            session_id: session_id.clone(),
            workspace_id: connection.workspace_id.clone(),
            connection_id: connection.id.clone(),
            status: "connected".to_string(),
            reconnect_attempt: 0,
            auth_kind: connection.auth_kind.clone(),
            host: connection.host.clone(),
            username: connection.username.clone(),
            cols,
            rows,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let state = SshSessionState {
            summary: summary.clone(),
            events: vec![SshSessionEvent {
                session_id: session_id.clone(),
                kind: "output".to_string(),
                data: format!(
                    "Connected to {}@{} with {} auth. PTY {}x{} allocated.\r\n",
                    summary.username, summary.host, summary.auth_kind, cols, rows
                ),
                created_at: now,
            }],
            pending_output: String::new(),
            intentional_close: false,
        };

        self.sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?
            .insert(session_id, state);

        Ok(summary)
    }

    // -----------------------------------------------------------------------
    // Native connect (ssh-native feature path)
    // -----------------------------------------------------------------------

    #[cfg(feature = "ssh-native")]
    async fn connect_native(
        &self,
        connection: &SshConnection,
        input: &SshConnectInput,
    ) -> AppResult<SshSessionSummary> {
        let now = Utc::now().to_rfc3339();
        let session_id = unfour_core::id::new_id();
        let cols = input.cols.unwrap_or(120).clamp(20, 300);
        let rows = input.rows.unwrap_or(32).clamp(8, 100);
        let native_handle = self
            .open_native_transport(connection, cols, rows, input.secret.as_deref())
            .await?;
        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

        let summary = SshSessionSummary {
            session_id: session_id.clone(),
            workspace_id: connection.workspace_id.clone(),
            connection_id: connection.id.clone(),
            status: "connected".to_string(),
            reconnect_attempt: 0,
            auth_kind: connection.auth_kind.clone(),
            host: connection.host.clone(),
            username: connection.username.clone(),
            cols,
            rows,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let state = SshSessionState {
            summary: summary.clone(),
            events: vec![SshSessionEvent {
                session_id: session_id.clone(),
                kind: "output".to_string(),
                data: format!(
                    "Connected to {}@{} via native transport with {} auth. PTY {}x{} allocated.\r\n",
                    summary.username, summary.host, summary.auth_kind, cols, rows
                ),
                created_at: now,
            }],
            pending_output: String::new(),
            intentional_close: false,
            native_handle: Some(native_handle.clone()),
            cancel_tx: Some(cancel_tx),
        };

        self.sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?
            .insert(session_id.clone(), state);

        self.spawn_native_supervisor(session_id, connection.clone(), native_handle, cancel_rx);

        Ok(summary)
    }

    #[cfg(feature = "ssh-native")]
    async fn open_native_transport(
        &self,
        connection: &SshConnection,
        cols: u16,
        rows: u16,
        override_secret: Option<&str>,
    ) -> AppResult<NativeSshHandle> {
        let config = Arc::new(native_client_config());
        let handler = SshClientHandler {
            host_key_store: HostKeyStore::new(self.db.pool().clone()),
            workspace_id: connection.workspace_id.clone(),
            host: connection.host.clone(),
            port: connection.port,
        };
        let addr = format!("{}:{}", connection.host, connection.port);
        let timeout_duration = std::time::Duration::from_secs(15);
        let mut handle = match tokio::time::timeout(
            timeout_duration,
            russh::client::connect(config, addr.as_str(), handler),
        )
        .await
        {
            Ok(Ok(handle)) => handle,
            Ok(Err(error)) => {
                return Err(AppError::Config(format!(
                    "ssh connection to {}:{} failed: {}",
                    connection.host,
                    connection.port,
                    sanitize_ssh_error(&error)
                )));
            }
            Err(_) => {
                return Err(AppError::Config(format!(
                    "ssh connection to {}:{} timed out after {}s",
                    connection.host,
                    connection.port,
                    timeout_duration.as_secs()
                )));
            }
        };

        self.authenticate_native(&mut handle, connection, override_secret)
            .await?;
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|error| AppError::Config(format!("failed to open ssh channel: {}", error)))?;
        channel
            .request_pty(true, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
            .await
            .map_err(|error| AppError::Config(format!("failed to request ssh pty: {}", error)))?;
        channel
            .request_shell(true)
            .await
            .map_err(|error| AppError::Config(format!("failed to start ssh shell: {}", error)))?;

        let (reader, writer) = channel.split();
        Ok(NativeSshHandle {
            handle: Arc::new(tokio::sync::Mutex::new(handle)),
            writer: Arc::new(writer),
            reader: Arc::new(tokio::sync::Mutex::new(reader)),
        })
    }

    /// Open a fresh native connection, run a single command via `exec` (no PTY),
    /// capture stdout/stderr to EOF (bounded and timed), then disconnect.
    #[cfg(feature = "ssh-native")]
    async fn run_diagnostic_native(
        &self,
        connection: &SshConnection,
        command: &str,
        timeout: std::time::Duration,
    ) -> AppResult<SshDiagnosticResult> {
        let config = Arc::new(native_client_config());
        let handler = SshClientHandler {
            host_key_store: HostKeyStore::new(self.db.pool().clone()),
            workspace_id: connection.workspace_id.clone(),
            host: connection.host.clone(),
            port: connection.port,
        };
        let addr = format!("{}:{}", connection.host, connection.port);
        let connect_timeout = std::time::Duration::from_secs(15);
        let mut handle = match tokio::time::timeout(
            connect_timeout,
            russh::client::connect(config, addr.as_str(), handler),
        )
        .await
        {
            Ok(Ok(handle)) => handle,
            Ok(Err(error)) => {
                return Err(AppError::Config(format!(
                    "ssh connection to {}:{} failed: {}",
                    connection.host,
                    connection.port,
                    sanitize_ssh_error(&error)
                )));
            }
            Err(_) => {
                return Err(AppError::Config(format!(
                    "ssh connection to {}:{} timed out after {}s",
                    connection.host,
                    connection.port,
                    connect_timeout.as_secs()
                )));
            }
        };

        self.authenticate_native(&mut handle, connection, None)
            .await?;
        let mut channel = handle
            .channel_open_session()
            .await
            .map_err(|error| AppError::Config(format!("failed to open ssh channel: {}", error)))?;
        channel
            .exec(true, command.as_bytes())
            .await
            .map_err(|error| {
                AppError::Config(format!("failed to run ssh diagnostic command: {}", error))
            })?;

        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let mut exit_status: Option<i32> = None;
        let mut truncated = false;

        let capture = async {
            loop {
                match channel.wait().await {
                    Some(russh::ChannelMsg::Data { data }) => {
                        append_capped(&mut stdout, &data[..], &mut truncated);
                    }
                    Some(russh::ChannelMsg::ExtendedData { data, ext }) => {
                        if ext == 1 {
                            append_capped(&mut stderr, &data[..], &mut truncated);
                        } else {
                            append_capped(&mut stdout, &data[..], &mut truncated);
                        }
                    }
                    Some(russh::ChannelMsg::ExitStatus { exit_status: code }) => {
                        exit_status = Some(code as i32);
                    }
                    Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
        };

        let timed_out = tokio::time::timeout(timeout, capture).await.is_err();
        if timed_out {
            truncated = true;
        }

        let _ = channel.close().await;
        let _ = handle
            .disconnect(
                russh::Disconnect::ByApplication,
                "diagnostic complete",
                "en",
            )
            .await;

        let (stdout_text, _) = redact_sensitive_lines(&String::from_utf8_lossy(&stdout));
        let (stderr_text, _) = redact_sensitive_lines(&String::from_utf8_lossy(&stderr));

        Ok(SshDiagnosticResult {
            connection_id: connection.id.clone(),
            command: command.to_string(),
            stdout: stdout_text,
            stderr: stderr_text,
            exit_status,
            truncated,
        })
    }

    #[cfg(feature = "ssh-native")]
    async fn authenticate_native(
        &self,
        handle: &mut russh::client::Handle<SshClientHandler>,
        connection: &SshConnection,
        override_secret: Option<&str>,
    ) -> AppResult<()> {
        match connection.auth_kind.as_str() {
            "password" => {
                // A transient override (e.g. testing a not-yet-saved password)
                // takes precedence over the stored keychain credential and is
                // never persisted.
                let password = match override_secret {
                    Some(secret) => secret.to_string(),
                    None => {
                        let credential_ref =
                            connection.credential_ref.as_deref().ok_or_else(|| {
                                AppError::Validation(
                                    "password auth requires a credential reference".to_string(),
                                )
                            })?;
                        self.secret_store
                            .read_secret(
                                connection.workspace_id.clone(),
                                credential_ref.to_string(),
                            )
                            .await
                            .map_err(|error| {
                                // Surface the underlying cause (e.g. credential
                                // not found) so the user knows the stored secret
                                // is missing rather than facing an opaque
                                // failure. The credential for a password
                                // connection must be created via the dialog's
                                // "create credential" action before connecting.
                                AppError::Config(format!(
                                    "failed to read ssh credential from secret store \
                                     (create the credential before connecting): {}",
                                    error
                                ))
                            })?
                    }
                };
                let result = handle
                    .authenticate_password(connection.username.clone(), password)
                    .await
                    .map_err(|_| AppError::Config("ssh authentication failed".to_string()))?;
                if !result.success() {
                    return Err(AppError::Config(
                        "ssh authentication failed: invalid credentials".to_string(),
                    ));
                }
            }
            "private-key" => {
                let key_path = connection.key_path.as_deref().ok_or_else(|| {
                    AppError::Validation("private-key auth requires a key path".to_string())
                })?;
                let path = std::path::Path::new(key_path);
                if !path.exists() {
                    return Err(AppError::Config(format!(
                        "ssh private key file not found: {}",
                        key_path
                    )));
                }
                let private_key = match ssh_key::PrivateKey::read_openssh_file(path) {
                    Ok(key) => key,
                    Err(error) => {
                        let has_passphrase = match connection.credential_ref.as_deref() {
                            Some(credential_ref) => self
                                .secret_store
                                .read_secret(
                                    connection.workspace_id.clone(),
                                    credential_ref.to_string(),
                                )
                                .await
                                .is_ok(),
                            None => false,
                        };
                        if !has_passphrase {
                            return Err(AppError::Config(format!(
                                "failed to read ssh private key: {}. If the key is encrypted, save its passphrase as a credential.",
                                error
                            )));
                        }
                        let pem_bytes = std::fs::read(path).map_err(|io_error| {
                            AppError::Config(format!(
                                "failed to read ssh private key file: {}",
                                io_error
                            ))
                        })?;
                        ssh_key::PrivateKey::from_openssh(&pem_bytes).map_err(|key_error| {
                            AppError::Config(format!(
                                "failed to decrypt ssh private key (passphrase may be incorrect or format unsupported): {}",
                                key_error
                            ))
                        })?
                    }
                };
                let key = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(private_key), None);
                let result = handle
                    .authenticate_publickey(connection.username.clone(), key)
                    .await
                    .map_err(|_| {
                        AppError::Config("ssh public key authentication failed".to_string())
                    })?;
                if !result.success() {
                    return Err(AppError::Config(
                        "ssh public key authentication failed: key rejected by server".to_string(),
                    ));
                }
            }
            "none" => {
                let result = handle
                    .authenticate_none(connection.username.clone())
                    .await
                    .map_err(|_| AppError::Config("ssh authentication failed".to_string()))?;
                if !result.success() {
                    return Err(AppError::Config(
                        "ssh authentication failed: server rejected unauthenticated access"
                            .to_string(),
                    ));
                }
            }
            _ => {
                return Err(AppError::Validation(format!(
                    "unsupported ssh auth kind: {}",
                    connection.auth_kind
                )));
            }
        }
        Ok(())
    }

    #[cfg(feature = "ssh-native")]
    fn spawn_native_supervisor(
        &self,
        session_id: String,
        connection: SshConnection,
        initial_handle: NativeSshHandle,
        mut cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) {
        let service = self.clone();
        tokio::spawn(async move {
            let mut native = initial_handle;
            let mut persist_interval = tokio::time::interval(PERSIST_FLUSH_INTERVAL);
            persist_interval.tick().await;
            let mut emit_interval = tokio::time::interval(EMIT_FLUSH_INTERVAL);
            emit_interval.tick().await;
            // Pending terminal output coalesced between emits (see EMIT_* docs).
            let mut emit_buffer = String::new();
            loop {
                let channel_closed = loop {
                    let message = tokio::select! {
                        changed = cancel_rx.changed() => {
                            if changed.is_err() || *cancel_rx.borrow() {
                                service.flush_emit_buffer(&session_id, &mut emit_buffer);
                                return;
                            }
                            continue;
                        }
                        _ = persist_interval.tick() => {
                            // Persist on a detached task. Awaiting the database
                            // write here would stall the read loop, letting
                            // russh's bounded channel buffer fill under heavy
                            // output (e.g. a vim full-screen redraw). A full read
                            // buffer back-pressures the single SSH session task so
                            // it can no longer service outgoing keystroke writes,
                            // which makes interactive input appear frozen.
                            service.spawn_flush_session_history(&session_id);
                            continue;
                        }
                        _ = emit_interval.tick() => {
                            // Throttled flush of coalesced output to the frontend.
                            service.flush_emit_buffer(&session_id, &mut emit_buffer);
                            continue;
                        }
                        message = async {
                            // Only the supervisor reads, so this lock is
                            // uncontended; crucially it is a different lock from
                            // the write half, so blocking here never delays input.
                            let mut reader = native.reader.lock().await;
                            reader.wait().await
                        } => message,
                    };
                    match message {
                        Some(message) => {
                            if let Some(text) = terminal_output_from_channel_message(&message) {
                                // Coalesce for the frontend; emit on the throttle
                                // tick above or when the buffer grows large.
                                emit_buffer.push_str(&text);
                                if emit_buffer.len() >= EMIT_FLUSH_BYTES {
                                    service.flush_emit_buffer(&session_id, &mut emit_buffer);
                                }
                                // Buffer in memory (fast); flush to the database
                                // off the read path so persistence latency can
                                // never starve keystroke delivery (see above).
                                if service.buffer_session_output(&session_id, &text) {
                                    service.spawn_flush_session_history(&session_id);
                                }
                            } else if matches!(
                                message,
                                russh::ChannelMsg::Close | russh::ChannelMsg::Eof
                            ) {
                                service.flush_emit_buffer(&session_id, &mut emit_buffer);
                                break true;
                            }
                        }
                        None => {
                            service.flush_emit_buffer(&session_id, &mut emit_buffer);
                            break true;
                        }
                    }
                };

                if !channel_closed || !service.session_should_reconnect(&session_id) {
                    return;
                }
                service.set_session_health(
                    &session_id,
                    "degraded",
                    0,
                    "SSH connection lost. Preparing to reconnect.\r\n",
                );

                let mut reconnected = None;
                for (index, backoff_secs) in RECONNECT_BACKOFF_SECS.iter().enumerate() {
                    let attempt = (index + 1) as u8;
                    service.set_session_health(
                        &session_id,
                        "reconnecting",
                        attempt,
                        &format!("Reconnecting SSH session (attempt {attempt}/3).\r\n"),
                    );
                    tokio::select! {
                        changed = cancel_rx.changed() => {
                            if changed.is_err() || *cancel_rx.borrow() {
                                service.set_session_health(
                                    &session_id,
                                    "disconnected",
                                    0,
                                    "SSH reconnect cancelled.\r\n",
                                );
                                let _ = service.flush_session_history(&session_id).await;
                                return;
                            }
                        }
                        _ = tokio::time::sleep(std::time::Duration::from_secs(*backoff_secs)) => {}
                    }
                    if *cancel_rx.borrow() {
                        return;
                    }
                    let reconnect = service.open_native_transport(
                        &connection,
                        service.session_dimensions(&session_id).0,
                        service.session_dimensions(&session_id).1,
                        None,
                    );
                    let result = tokio::select! {
                        changed = cancel_rx.changed() => {
                            if changed.is_err() || *cancel_rx.borrow() {
                                return;
                            }
                            continue;
                        }
                        result = reconnect => result,
                    };
                    if let Ok(next_native) = result {
                        reconnected = Some(next_native);
                        break;
                    }
                }

                match reconnected {
                    Some(next_native) => {
                        if !service.install_reconnected_handle(&session_id, next_native.clone()) {
                            // The session was closed/cancelled while we were
                            // reconnecting, so this freshly opened transport has
                            // no owner. Disconnect it gracefully instead of
                            // dropping it mid-stream (#11). Entry cleanup is
                            // handled by the close/cancel path that raced us.
                            let _ = next_native.writer.close().await;
                            let handle = next_native.handle.lock().await;
                            let _ = handle
                                .disconnect(
                                    russh::Disconnect::ByApplication,
                                    "session closed",
                                    "en",
                                )
                                .await;
                            return;
                        }
                        service.set_session_health(
                            &session_id,
                            "connected",
                            0,
                            "SSH session reconnected.\r\n",
                        );
                        let _ = service.persist_session_summary(&session_id).await;
                        native = next_native;
                    }
                    None => {
                        service.set_session_health(
                            &session_id,
                            "failed",
                            3,
                            "SSH reconnect failed after 3 attempts.\r\n",
                        );
                        let _ = service.flush_session_history(&session_id).await;
                        let _ = service.persist_session_summary(&session_id).await;
                        service.clear_native_session_resources(&session_id);
                        // Drop the in-memory entry so a permanently failed session
                        // cannot leak (#4).
                        if let Ok(mut sessions) = service.sessions.lock() {
                            sessions.remove(&session_id);
                        }
                        return;
                    }
                }
            }
        });
    }

    #[cfg(feature = "ssh-native")]
    fn session_should_reconnect(&self, session_id: &str) -> bool {
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| sessions.get(session_id).map(should_reconnect))
            .unwrap_or(false)
    }

    #[cfg(feature = "ssh-native")]
    fn session_dimensions(&self, session_id: &str) -> (u16, u16) {
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| {
                sessions
                    .get(session_id)
                    .map(|state| (state.summary.cols, state.summary.rows))
            })
            .unwrap_or((120, 32))
    }

    #[cfg(feature = "ssh-native")]
    fn install_reconnected_handle(&self, session_id: &str, native_handle: NativeSshHandle) -> bool {
        let Ok(mut sessions) = self.sessions.lock() else {
            return false;
        };
        let Some(state) = sessions.get_mut(session_id) else {
            return false;
        };
        if state.intentional_close {
            return false;
        }
        state.native_handle = Some(native_handle);
        true
    }

    #[cfg(feature = "ssh-native")]
    fn clear_native_session_resources(&self, session_id: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(state) = sessions.get_mut(session_id) {
                state.native_handle = None;
                state.cancel_tx = None;
            }
        }
    }

    #[cfg(feature = "ssh-native")]
    fn set_session_health(&self, session_id: &str, status: &str, attempt: u8, message: &str) {
        let now = Utc::now().to_rfc3339();
        if let Ok(mut sessions) = self.sessions.lock() {
            let Some(state) = sessions.get_mut(session_id) else {
                return;
            };
            if state.intentional_close && status != "disconnected" {
                return;
            }
            state.summary.status = status.to_string();
            state.summary.reconnect_attempt = attempt;
            state.summary.updated_at = now.clone();
            record_session_event(state, SshSessionEvent {
                session_id: session_id.to_string(),
                kind: if status == "disconnected" || status == "failed" {
                    "close".to_string()
                } else {
                    "output".to_string()
                },
                data: message.to_string(),
                created_at: now,
            });
            state.pending_output.push_str(message);
        }
        self.emit_terminal_payload(session_id, message, Some(status), attempt);
    }

    /// Emit any coalesced output accumulated since the last flush, then clear
    /// the buffer. A no-op when the buffer is empty.
    #[cfg(feature = "ssh-native")]
    fn flush_emit_buffer(&self, session_id: &str, emit_buffer: &mut String) {
        if emit_buffer.is_empty() {
            return;
        }
        self.emit_terminal_payload(session_id, emit_buffer, None, 0);
        emit_buffer.clear();
    }

    #[cfg(feature = "ssh-native")]
    fn emit_terminal_payload(
        &self,
        session_id: &str,
        data: &str,
        status: Option<&str>,
        attempt: u8,
    ) {
        let callback = self
            .on_terminal_output
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        if let Some(callback) = callback {
            callback(
                serde_json::json!({
                    "sessionId": session_id,
                    "data": data,
                    "status": status,
                    "reconnectAttempt": attempt,
                })
                .to_string(),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

#[cfg(feature = "ssh-native")]
fn terminal_output_from_channel_message(message: &russh::ChannelMsg) -> Option<String> {
    match message {
        russh::ChannelMsg::Data { data } | russh::ChannelMsg::ExtendedData { data, .. } => {
            Some(String::from_utf8_lossy(data).to_string())
        }
        _ => None,
    }
}
fn stored_to_ssh_connection(row: StoredSshConnection) -> AppResult<SshConnection> {
    let config = parse_ssh_config(&row.id, &row.config_json)?;
    let port = decode_ssh_port(row.port)?;
    Ok(SshConnection {
        id: row.id,
        workspace_id: row.workspace_id,
        name: row.name,
        host: row.host,
        port,
        username: row.username,
        auth_kind: row.auth_method,
        key_path: config.key_path,
        credential_ref: row.credential_ref,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
        revision: row.revision,
        sync_status: row.sync_status,
        remote_id: row.remote_id,
    })
}

fn input_to_storage(input: &SshConnectionInput) -> AppResult<SshConnectionStorageInput> {
    let host = normalize_required(&input.host, "ssh host")?;
    let username = normalize_required(&input.username, "ssh username")?;
    let auth_method = input.auth_kind.trim().to_ascii_lowercase();
    if !matches!(auth_method.as_str(), "password" | "private-key" | "none") {
        return Err(AppError::Validation(format!(
            "unsupported ssh auth kind: {}",
            input.auth_kind
        )));
    }

    let port = input.port.unwrap_or(22);
    if port == 0 {
        return Err(AppError::Validation("ssh port cannot be 0".to_string()));
    }

    let key_path = empty_to_none(input.key_path.clone());
    if auth_method == "private-key" && key_path.is_none() {
        return Err(AppError::Validation(
            "private-key ssh auth requires a key path".to_string(),
        ));
    }

    Ok(SshConnectionStorageInput {
        host,
        port,
        username,
        auth_method,
        config: SshConnectionConfig { key_path },
    })
}

fn ssh_config_to_json(config: &SshConnectionConfig) -> AppResult<String> {
    serde_json::to_string(config).map_err(AppError::from)
}

fn parse_ssh_config(connection_id: &str, config_json: &str) -> AppResult<SshConnectionConfig> {
    serde_json::from_str::<SshConnectionConfig>(config_json).map_err(|error| {
        AppError::Config(format!(
            "invalid ssh_connections.config_json for connection {connection_id}: {error}"
        ))
    })
}

fn decode_ssh_port(port: i64) -> AppResult<u16> {
    if (1..=u16::MAX as i64).contains(&port) {
        Ok(port as u16)
    } else {
        Err(AppError::Config(format!(
            "ssh connection port out of range: {port}"
        )))
    }
}

fn validate_connection_ready_for_session(connection: &SshConnection) -> AppResult<()> {
    if connection.auth_kind == "password" && connection.credential_ref.is_none() {
        return Err(AppError::Validation(
            "password ssh session requires a stored password".to_string(),
        ));
    }
    if connection.auth_kind == "private-key" && connection.key_path.is_none() {
        return Err(AppError::Validation(
            "private-key ssh session requires a key path".to_string(),
        ));
    }
    Ok(())
}

/// Remove potentially sensitive details from SSH errors before surfacing them.
#[cfg(feature = "ssh-native")]
fn sanitize_ssh_error(error: &russh::Error) -> String {
    let msg = error.to_string();
    let lower = msg.to_ascii_lowercase();
    // Strip anything that could contain a password, passphrase, or key material.
    if lower.contains("password") || lower.contains("passphrase") || lower.contains("private key") {
        "ssh transport error".to_string()
    } else {
        msg
    }
}

/// Maximum bytes captured per stream (stdout/stderr) for a diagnostic command.
#[cfg(feature = "ssh-native")]
const SSH_DIAGNOSTIC_MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Append `data` to `buf`, capping at the diagnostic output limit and marking
/// `truncated` when the limit is reached.
#[cfg(feature = "ssh-native")]
fn append_capped(buf: &mut Vec<u8>, data: &[u8], truncated: &mut bool) {
    let remaining = SSH_DIAGNOSTIC_MAX_OUTPUT_BYTES.saturating_sub(buf.len());
    if remaining == 0 {
        *truncated = true;
        return;
    }
    if data.len() > remaining {
        buf.extend_from_slice(&data[..remaining]);
        *truncated = true;
    } else {
        buf.extend_from_slice(data);
    }
}

fn normalize_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "ssh connection name cannot be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 80 {
        return Err(AppError::Validation(
            "ssh connection name must be 80 characters or fewer".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn normalize_required(value: &str, label: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!("{} cannot be empty", label)));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AppError::Validation(format!(
            "{} cannot contain control characters",
            label
        )));
    }
    Ok(trimmed.to_string())
}

fn session_for_workspace_mut<'a>(
    sessions: &'a mut HashMap<String, SshSessionState>,
    workspace_id: &str,
    session_id: &str,
) -> AppResult<&'a mut SshSessionState> {
    sessions
        .get_mut(session_id)
        .filter(|state| state.summary.workspace_id == workspace_id)
        .ok_or_else(|| AppError::NotFound("ssh session".to_string()))
}

#[cfg(feature = "ssh-native")]
fn session_for_workspace<'a>(
    sessions: &'a HashMap<String, SshSessionState>,
    workspace_id: &str,
    session_id: &str,
) -> AppResult<&'a SshSessionState> {
    sessions
        .get(session_id)
        .filter(|state| state.summary.workspace_id == workspace_id)
        .ok_or_else(|| AppError::NotFound("ssh session".to_string()))
}

fn ensure_session_active(state: &SshSessionState) -> AppResult<()> {
    if state.summary.status != "connected" {
        return Err(AppError::Validation(
            "ssh session is not connected".to_string(),
        ));
    }
    Ok(())
}

/// Push a session event, dropping the oldest entries once the in-memory cap is
/// exceeded. Session events are only used for `export_log`; trimming them keeps
/// a long-lived session from growing the event vector without bound.
fn record_session_event(state: &mut SshSessionState, event: SshSessionEvent) {
    state.events.push(event);
    if state.events.len() > MAX_SESSION_EVENTS {
        let excess = state.events.len() - MAX_SESSION_EVENTS;
        state.events.drain(0..excess);
    }
}

/// Build a redacted session-log export from an event slice. Used for both live
/// sessions (events held in memory) and closed sessions (events hydrated from
/// the terminal-history store after the in-memory entry was dropped to bound
/// memory growth, see issue #4).
fn build_ssh_log_export(session_id: &str, events: &[SshSessionEvent]) -> SshLogExport {
    let mut redacted = false;
    let lines = events
        .iter()
        .map(|event| {
            let (data, event_redacted) = redact_ssh_log(&event.data);
            redacted |= event_redacted;
            format!("[{}] {} {}", event.created_at, event.kind, data)
        })
        .collect::<Vec<_>>();
    let content = lines.join("\n");
    // Persisted (closed) sessions already had line-level redaction applied at
    // append time; reflect that in the redacted flag so closed-session exports
    // stay accurate.
    if content.contains("<redacted>") {
        redacted = true;
    }
    SshLogExport {
        session_id: session_id.to_string(),
        filename: format!("ssh-session-{}.log", session_id),
        line_count: lines.len(),
        content,
        redacted,
    }
}

fn is_live_status(status: &str) -> bool {
    matches!(status, "connected" | "degraded" | "reconnecting")
}

#[cfg(any(feature = "ssh-native", test))]
fn should_reconnect(state: &SshSessionState) -> bool {
    !state.intentional_close && is_live_status(&state.summary.status)
}

#[cfg(feature = "ssh-native")]
fn native_client_config() -> russh::client::Config {
    let mut config = russh::client::Config::default();
    config.keepalive_interval = Some(KEEPALIVE_INTERVAL);
    config.keepalive_max = KEEPALIVE_MAX_MISSES;
    config.nodelay = true;
    config
}

fn redact_ssh_log(value: &str) -> (String, bool) {
    redact_sensitive_lines(value)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "ssh_tests/mod.rs"]
mod tests;
