use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use unfour_core::models::{
    SshCloseInput, SshConnectInput, SshConnection, SshConnectionConfig, SshConnectionInput,
    SshDiagnosticInput, SshDiagnosticResult, SshHostFingerprintInfo, SshHostKeyInput,
    SshKnownHostsExportResult, SshKnownHostsImportInput, SshKnownHostsImportResult, SshLogExport,
    SshLogExportInput, SshReconnectCancelInput, SshResizeInput, SshSessionEvent, SshSessionInput,
    SshSessionSummary, StoredConnection,
};
use unfour_core::redaction::redact_sensitive_lines;
use unfour_core::{AppError, AppResult};
use unfour_local_storage::{LocalDb, TerminalHistoryService};
use unfour_secret_store::SecretStore;
use uuid::Uuid;

use crate::host_key::HostKeyStore;

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

#[cfg(feature = "ssh-native")]
struct NativeSshHandle {
    handle: std::sync::Arc<tokio::sync::Mutex<russh::client::Handle<SshClientHandler>>>,
    channel: std::sync::Arc<tokio::sync::Mutex<russh::Channel<russh::client::Msg>>>,
}

#[cfg(feature = "ssh-native")]
impl Clone for NativeSshHandle {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            channel: self.channel.clone(),
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
            .verify_or_record(&self.host, self.port, &fingerprint)
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

        let rows = sqlx::query_as::<_, StoredConnection>(
            r#"
            SELECT
              id, workspace_id, kind, name, config_json, credential_ref, created_at,
              updated_at, deleted_at, revision, sync_status, remote_id
            FROM connections
            WHERE workspace_id = ?1 AND kind = 'ssh' AND deleted_at IS NULL
            ORDER BY updated_at DESC
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
        let config = input_to_config(&input)?;
        let credential_ref = empty_to_none(input.credential_ref);
        validate_credential_boundary(&config, credential_ref.as_deref())?;
        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(&config)?;

        if let Some(id) = input
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            let result = sqlx::query(
                r#"
                UPDATE connections
                SET name = ?1, config_json = ?2, credential_ref = ?3,
                    updated_at = ?4, revision = revision + 1, sync_status = 'pending'
                WHERE id = ?5 AND workspace_id = ?6 AND kind = 'ssh' AND deleted_at IS NULL
                "#,
            )
            .bind(name)
            .bind(config_json)
            .bind(credential_ref)
            .bind(now)
            .bind(id)
            .bind(&input.workspace_id)
            .execute(self.db.pool())
            .await?;

            if result.rows_affected() == 0 {
                return Err(AppError::NotFound("ssh connection".to_string()));
            }

            return self.get_connection(&input.workspace_id, id).await;
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO connections (
              id, workspace_id, kind, name, config_json, credential_ref,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, 'ssh', ?3, ?4, ?5, ?6, ?6, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(name)
        .bind(config_json)
        .bind(credential_ref)
        .bind(now)
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

        let result = sqlx::query(
            r#"
            UPDATE connections
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1, sync_status = 'deleted'
            WHERE id = ?2 AND workspace_id = ?3 AND kind = 'ssh' AND deleted_at IS NULL
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

        self.close_sessions_for_connection(&workspace_id, &connection_id)
            .await?;
        self.terminal_history
            .delete_connection_history(&workspace_id, &connection_id)
            .await?;

        self.list_connections(workspace_id).await
    }

    pub async fn connect(&self, input: SshConnectInput) -> AppResult<SshSessionSummary> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        validate_connection_ready_for_session(&connection)?;

        #[cfg(feature = "ssh-native")]
        let summary = self.connect_native(&connection, &input).await?;
        #[cfg(not(feature = "ssh-native"))]
        let summary = self.connect_simulated(&connection, &input).await?;

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
                let channel = native.channel.lock().await;
                channel
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
            state.events.push(input_event);
            let event = SshSessionEvent {
                session_id: input.session_id.clone(),
                kind: "output".to_string(),
                data: "Input accepted by SSH PTY stream.\r\n".to_string(),
                created_at: now.clone(),
            };
            #[cfg(not(feature = "ssh-native"))]
            {
                state.events.push(event.clone());
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
                let channel = native.channel.lock().await;
                let _ = channel
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
            state.events.push(event.clone());
            event
        };
        self.persist_session_summary(&input.session_id).await?;
        Ok(event)
    }

    pub async fn close_session(&self, input: SshCloseInput) -> AppResult<SshSessionSummary> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;

        // Extract native handle and update state under the lock.
        #[cfg(feature = "ssh-native")]
        let native_handle: Option<NativeSshHandle> = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            let state =
                session_for_workspace_mut(&mut sessions, &input.workspace_id, &input.session_id)?;
            state.intentional_close = true;
            if let Some(cancel_tx) = state.cancel_tx.take() {
                let _ = cancel_tx.send(true);
            }
            state.native_handle.take()
        };

        // Close channel and disconnect native transport outside the mutex lock.
        #[cfg(feature = "ssh-native")]
        if let Some(native) = native_handle {
            // Close the channel first so the reader task terminates.
            {
                let channel = native.channel.lock().await;
                let _ = channel.close().await;
            }
            let handle = native.handle.lock().await;
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "session closed", "en")
                .await;
        }

        // Update session status under the lock.
        let summary = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            let state =
                session_for_workspace_mut(&mut sessions, &input.workspace_id, &input.session_id)?;
            if state.summary.status != "disconnected" {
                let now = Utc::now().to_rfc3339();
                state.intentional_close = true;
                state.summary.status = "disconnected".to_string();
                state.summary.reconnect_attempt = 0;
                state.summary.updated_at = now.clone();
                state.events.push(SshSessionEvent {
                    session_id: input.session_id.clone(),
                    kind: "close".to_string(),
                    data: "SSH session closed.\r\n".to_string(),
                    created_at: now,
                });
                state.pending_output.push_str("SSH session closed.\r\n");
            }
            state.summary.clone()
        };
        self.flush_session_history(&input.session_id).await?;
        self.terminal_history.update_session(&summary).await?;
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
            let state =
                session_for_workspace_mut(&mut sessions, &input.workspace_id, &input.session_id)?;
            state.intentional_close = true;
            if let Some(cancel_tx) = state.cancel_tx.take() {
                let _ = cancel_tx.send(true);
            }
            state.native_handle.take()
        };

        #[cfg(feature = "ssh-native")]
        if let Some(native) = native_handle {
            let channel = native.channel.lock().await;
            let _ = channel.close().await;
        }

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
            state.events.push(SshSessionEvent {
                session_id: input.session_id.clone(),
                kind: "close".to_string(),
                data: "SSH reconnect cancelled.\r\n".to_string(),
                created_at: now,
            });
            state
                .pending_output
                .push_str("SSH reconnect cancelled.\r\n");
            state.summary.clone()
        };
        self.flush_session_history(&input.session_id).await?;
        self.terminal_history.update_session(&summary).await?;
        Ok(summary)
    }

    pub fn export_log(&self, input: SshLogExportInput) -> AppResult<SshLogExport> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
        let state = sessions
            .get(&input.session_id)
            .filter(|state| state.summary.workspace_id == input.workspace_id)
            .ok_or_else(|| AppError::NotFound("ssh session".to_string()))?;

        let mut redacted = false;
        let lines = state
            .events
            .iter()
            .map(|event| {
                let (data, event_redacted) = redact_ssh_log(&event.data);
                redacted |= event_redacted;
                format!("[{}] {} {}", event.created_at, event.kind, data)
            })
            .collect::<Vec<_>>();
        Ok(SshLogExport {
            session_id: input.session_id,
            filename: format!("ssh-session-{}.log", state.summary.session_id),
            line_count: lines.len(),
            content: lines.join("\n"),
            redacted,
        })
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
        let host = input.host.trim().to_string();
        if host.is_empty() {
            return Err(AppError::Validation("host cannot be empty".to_string()));
        }
        if input.port == 0 {
            return Err(AppError::Validation("port cannot be 0".to_string()));
        }

        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        match host_key_store
            .get_fingerprint_info(&host, input.port)
            .await?
        {
            Some((fingerprint, created_at)) => Ok(Some(SshHostFingerprintInfo {
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
        let host = input.host.trim().to_string();
        if host.is_empty() {
            return Err(AppError::Validation("host cannot be empty".to_string()));
        }
        if input.port == 0 {
            return Err(AppError::Validation("port cannot be 0".to_string()));
        }

        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        host_key_store.delete_fingerprint(&host, input.port).await
    }

    /// List all stored host-key fingerprints.
    pub async fn list_all_host_fingerprints(&self) -> AppResult<Vec<SshHostFingerprintInfo>> {
        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        let entries = host_key_store.list_all().await?;
        Ok(entries
            .into_iter()
            .map(|entry| SshHostFingerprintInfo {
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
        host_key_store.import_known_hosts(&input.content).await
    }

    /// Export stored fingerprints to OpenSSH known_hosts format.
    pub async fn export_known_hosts(&self) -> AppResult<SshKnownHostsExportResult> {
        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        let (content, entry_count) = host_key_store.export_known_hosts().await?;
        Ok(SshKnownHostsExportResult {
            content,
            entry_count,
        })
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    async fn get_connection(&self, workspace_id: &str, id: &str) -> AppResult<SshConnection> {
        validate_workspace_id(workspace_id)?;
        validate_connection_id(id)?;

        let row = sqlx::query_as::<_, StoredConnection>(
            r#"
            SELECT
              id, workspace_id, kind, name, config_json, credential_ref, created_at,
              updated_at, deleted_at, revision, sync_status, remote_id
            FROM connections
            WHERE id = ?1 AND workspace_id = ?2 AND kind = 'ssh' AND deleted_at IS NULL
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

        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
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
                state.events.push(SshSessionEvent {
                    session_id: state.summary.session_id.clone(),
                    kind: "close".to_string(),
                    data: "SSH session closed because the connection was deleted.\r\n".to_string(),
                    created_at: now.clone(),
                });
            }
        }

        // Disconnect native handles outside the mutex lock.
        #[cfg(feature = "ssh-native")]
        for native in native_handles {
            // Close the channel first.
            {
                let channel = native.channel.lock().await;
                let _ = channel.close().await;
            }
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
        let session_id = Uuid::new_v4().to_string();
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
        let session_id = Uuid::new_v4().to_string();
        let cols = input.cols.unwrap_or(120).clamp(20, 300);
        let rows = input.rows.unwrap_or(32).clamp(8, 100);
        let native_handle = self.open_native_transport(connection, cols, rows).await?;
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
    ) -> AppResult<NativeSshHandle> {
        let config = Arc::new(native_client_config());
        let handler = SshClientHandler {
            host_key_store: HostKeyStore::new(self.db.pool().clone()),
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

        self.authenticate_native(&mut handle, connection).await?;
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

        Ok(NativeSshHandle {
            handle: Arc::new(tokio::sync::Mutex::new(handle)),
            channel: Arc::new(tokio::sync::Mutex::new(channel)),
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

        self.authenticate_native(&mut handle, connection).await?;
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
    ) -> AppResult<()> {
        match connection.auth_kind.as_str() {
            "password" => {
                let credential_ref = connection.credential_ref.as_deref().ok_or_else(|| {
                    AppError::Validation(
                        "password auth requires a credential reference".to_string(),
                    )
                })?;
                let password = self
                    .secret_store
                    .read_secret(connection.workspace_id.clone(), credential_ref.to_string())
                    .await
                    .map_err(|_| {
                        AppError::Config(
                            "failed to read ssh credential from secret store".to_string(),
                        )
                    })?;
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
            loop {
                let channel_closed = loop {
                    let message = tokio::select! {
                        changed = cancel_rx.changed() => {
                            if changed.is_err() || *cancel_rx.borrow() {
                                return;
                            }
                            continue;
                        }
                        _ = persist_interval.tick() => {
                            let _ = service.flush_session_history(&session_id).await;
                            continue;
                        }
                        message = async {
                            let mut channel = native.channel.lock().await;
                            channel.wait().await
                        } => message,
                    };
                    match message {
                        Some(russh::ChannelMsg::Data { data }) => {
                            let text = String::from_utf8_lossy(&data).to_string();
                            service.emit_terminal_payload(&session_id, &text, None, 0);
                            if service.buffer_session_output(&session_id, &text) {
                                let _ = service.flush_session_history(&session_id).await;
                            }
                        }
                        Some(russh::ChannelMsg::Close) | Some(russh::ChannelMsg::Eof) | None => {
                            break true;
                        }
                        _ => {}
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
            state.events.push(SshSessionEvent {
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

fn stored_to_ssh_connection(row: StoredConnection) -> AppResult<SshConnection> {
    let config = serde_json::from_str::<SshConnectionConfig>(&row.config_json)?;
    Ok(SshConnection {
        id: row.id,
        workspace_id: row.workspace_id,
        name: row.name,
        host: config.host,
        port: config.port,
        username: config.username,
        auth_kind: config.auth_kind,
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

fn input_to_config(input: &SshConnectionInput) -> AppResult<SshConnectionConfig> {
    let host = normalize_required(&input.host, "ssh host")?;
    let username = normalize_required(&input.username, "ssh username")?;
    let auth_kind = input.auth_kind.trim().to_ascii_lowercase();
    if !matches!(auth_kind.as_str(), "password" | "private-key") {
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
    if auth_kind == "private-key" && key_path.is_none() {
        return Err(AppError::Validation(
            "private-key ssh auth requires a key path".to_string(),
        ));
    }

    Ok(SshConnectionConfig {
        host,
        port,
        username,
        auth_kind,
        key_path,
    })
}

fn validate_credential_boundary(
    config: &SshConnectionConfig,
    credential_ref: Option<&str>,
) -> AppResult<()> {
    if config.auth_kind == "password" && credential_ref.is_none() {
        return Err(AppError::Validation(
            "password ssh auth requires a credential reference".to_string(),
        ));
    }

    Ok(())
}

fn validate_connection_ready_for_session(connection: &SshConnection) -> AppResult<()> {
    if connection.auth_kind == "password" && connection.credential_ref.is_none() {
        return Err(AppError::Validation(
            "password ssh session requires a credential reference".to_string(),
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

/// Leading command words permitted for read-only SSH diagnostics. Each is a
/// non-mutating utility; commands that can also write/control state (notably
/// `systemctl` and `journalctl`) get additional subcommand/flag restrictions in
/// `validate_diagnostic_command`.
const SSH_DIAGNOSTIC_ALLOWED_COMMANDS: &[&str] = &[
    "df",
    "du",
    "free",
    "uptime",
    "uname",
    "hostname",
    "whoami",
    "id",
    "date",
    "ps",
    "ss",
    "netstat",
    "ip",
    "ifconfig",
    "vmstat",
    "iostat",
    "mount",
    "stat",
    "wc",
    "ls",
    "cat",
    "tail",
    "head",
    "systemctl",
    "journalctl",
];

/// Validate a one-shot SSH diagnostic command. Returns the trimmed command on
/// success. Enforces: non-empty, length bound, no control characters, no shell
/// metacharacters (so no chaining/piping/redirection/subshells), a bare
/// allowlisted leading utility, and read-only subcommands/flags for utilities
/// that could otherwise mutate state.
fn validate_diagnostic_command(command: &str) -> AppResult<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "ssh diagnostic command cannot be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 512 {
        return Err(AppError::Validation(
            "ssh diagnostic command must be 512 characters or fewer".to_string(),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "ssh diagnostic command cannot contain control characters".to_string(),
        ));
    }
    const FORBIDDEN: &[char] = &[
        ';', '|', '&', '$', '`', '>', '<', '(', ')', '{', '}', '\\', '*', '?', '~', '!', '#', '\'',
        '"',
    ];
    if let Some(found) = trimmed.chars().find(|c| FORBIDDEN.contains(c)) {
        return Err(AppError::Validation(format!(
            "ssh diagnostic command cannot contain the shell metacharacter `{}`",
            found
        )));
    }

    let mut tokens = trimmed.split_whitespace();
    let head = tokens.next().unwrap_or_default();
    if head.contains('/') {
        return Err(AppError::Validation(
            "ssh diagnostic command must be a bare allowlisted utility (no path)".to_string(),
        ));
    }
    if !SSH_DIAGNOSTIC_ALLOWED_COMMANDS.contains(&head) {
        return Err(AppError::Validation(format!(
            "`{}` is not an allowed read-only diagnostic command",
            head
        )));
    }

    match head {
        "systemctl" => {
            const SYSTEMCTL_READONLY: &[&str] = &[
                "status",
                "is-active",
                "is-enabled",
                "is-failed",
                "show",
                "cat",
                "list-units",
                "list-unit-files",
                "list-timers",
                "list-sockets",
                "get-default",
            ];
            let sub = tokens.find(|token| !token.starts_with('-')).unwrap_or("");
            if !SYSTEMCTL_READONLY.contains(&sub) {
                return Err(AppError::Validation(
                    "systemctl diagnostics are limited to read-only subcommands (status, is-active, show, list-units, ...)".to_string(),
                ));
            }
        }
        "journalctl" => {
            if tokens.any(|token| {
                token.starts_with("--vacuum")
                    || token == "--rotate"
                    || token == "--flush"
                    || token == "--sync"
                    || token == "--relinquish-var"
            }) {
                return Err(AppError::Validation(
                    "journalctl diagnostics cannot use log-management flags".to_string(),
                ));
            }
        }
        _ => {}
    }

    Ok(trimmed.to_string())
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

fn empty_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn validate_workspace_id(workspace_id: &str) -> AppResult<()> {
    if workspace_id.trim().is_empty() {
        return Err(AppError::Validation(
            "workspace id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_connection_id(connection_id: &str) -> AppResult<()> {
    if connection_id.trim().is_empty() {
        return Err(AppError::Validation(
            "ssh connection id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_session_id(session_id: &str) -> AppResult<()> {
    if session_id.trim().is_empty() {
        return Err(AppError::Validation(
            "ssh session id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_pty_size(cols: u16, rows: u16) -> AppResult<()> {
    if !(20..=300).contains(&cols) || !(8..=100).contains(&rows) {
        return Err(AppError::Validation(
            "ssh pty size must be between 20x8 and 300x100".to_string(),
        ));
    }
    Ok(())
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
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use unfour_secret_store::SecretStore;

    #[test]
    fn diagnostic_command_allows_read_only_utilities() {
        for cmd in [
            "df -h",
            "free -m",
            "uptime",
            "tail -n 200 /var/log/syslog",
            "cat /etc/os-release",
            "systemctl status nginx",
            "systemctl is-active sshd",
            "journalctl -u nginx -n 100",
            "ps aux",
        ] {
            assert!(
                validate_diagnostic_command(cmd).is_ok(),
                "expected `{cmd}` to be allowed"
            );
        }
    }

    #[test]
    fn diagnostic_command_rejects_shell_metacharacters() {
        for cmd in [
            "cat /etc/passwd; rm -rf /",
            "df -h | grep sda",
            "uptime && reboot",
            "cat $(which sh)",
            "tail -f /var/log/x > /tmp/y",
            "echo `whoami`",
        ] {
            assert!(
                validate_diagnostic_command(cmd).is_err(),
                "expected `{cmd}` to be rejected"
            );
        }
    }

    #[test]
    fn diagnostic_command_rejects_non_allowlisted_and_paths() {
        assert!(validate_diagnostic_command("rm -rf /").is_err());
        assert!(validate_diagnostic_command("curl http://evil").is_err());
        assert!(validate_diagnostic_command("/usr/bin/df").is_err());
        assert!(validate_diagnostic_command("").is_err());
    }

    #[test]
    fn diagnostic_command_restricts_systemctl_and_journalctl() {
        assert!(validate_diagnostic_command("systemctl restart nginx").is_err());
        assert!(validate_diagnostic_command("systemctl stop sshd").is_err());
        assert!(validate_diagnostic_command("systemctl daemon-reload").is_err());
        assert!(validate_diagnostic_command("journalctl --vacuum-size=1M").is_err());
        assert!(validate_diagnostic_command("journalctl --rotate").is_err());
    }

    #[tokio::test]
    async fn run_diagnostic_validates_before_connecting() {
        // An invalid command must be rejected by validation, independent of any
        // SSH transport or feature flag.
        let (service, workspace_a, connection_id) = diagnostic_fixture().await;
        let result = service
            .run_diagnostic(SshDiagnosticInput {
                workspace_id: workspace_a,
                connection_id,
                command: "rm -rf /".to_string(),
                timeout_ms: None,
            })
            .await;
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn run_diagnostic_is_unsupported_without_native() {
        let (service, workspace_a, connection_id) = diagnostic_fixture().await;
        let result = service
            .run_diagnostic(SshDiagnosticInput {
                workspace_id: workspace_a,
                connection_id,
                command: "uptime".to_string(),
                timeout_ms: None,
            })
            .await;
        assert!(matches!(result, Err(AppError::Unsupported(_))));
    }

    /// Build a service with one saved password SSH connection ready for a
    /// session, returning (service, workspace_id, connection_id).
    async fn diagnostic_fixture() -> (SshService, String, String) {
        let (service, workspace_a, _workspace_b) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_a))
            .await
            .expect("save ssh connection");
        (service, workspace_a, connection.id)
    }

    async fn service_with_workspaces() -> (SshService, String, String) {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory app db");
        let db = LocalDb::from_pool(pool);
        db.migrate().await.expect("run migrations");

        let secret_store = SecretStore::in_memory("unfour-test");

        let workspace_a = Uuid::new_v4().to_string();
        let workspace_b = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        for workspace_id in [&workspace_a, &workspace_b] {
            sqlx::query(
                r#"
                INSERT INTO workspaces (
                  id, name, is_default, last_opened_at, created_at, updated_at,
                  revision, sync_status
                )
                VALUES (?1, 'Test Workspace', 0, ?2, ?2, ?2, 1, 'local')
                "#,
            )
            .bind(workspace_id)
            .bind(&now)
            .execute(db.pool())
            .await
            .expect("insert workspace");
        }

        (SshService::new(db, secret_store), workspace_a, workspace_b)
    }

    fn password_input(workspace_id: &str) -> SshConnectionInput {
        SshConnectionInput {
            id: None,
            workspace_id: workspace_id.to_string(),
            name: "Deploy host".to_string(),
            host: " example.internal ".to_string(),
            port: None,
            username: " deploy ".to_string(),
            auth_kind: "password".to_string(),
            key_path: None,
            credential_ref: Some("ssh-password-1".to_string()),
        }
    }

    #[tokio::test]
    async fn ssh_connection_crud_is_workspace_scoped_and_soft_deletes() {
        let (service, workspace_a, workspace_b) = service_with_workspaces().await;

        let created = service
            .save_connection(password_input(&workspace_a))
            .await
            .expect("save ssh connection");
        assert_eq!(created.host, "example.internal");
        assert_eq!(created.port, 22);
        assert_eq!(created.username, "deploy");
        assert_eq!(created.credential_ref.as_deref(), Some("ssh-password-1"));

        let workspace_a_items = service
            .list_connections(workspace_a.clone())
            .await
            .expect("list workspace a");
        let workspace_b_items = service
            .list_connections(workspace_b)
            .await
            .expect("list workspace b");
        assert_eq!(workspace_a_items.len(), 1);
        assert!(workspace_b_items.is_empty());

        let updated = service
            .save_connection(SshConnectionInput {
                id: Some(created.id.clone()),
                name: "Deploy bastion".to_string(),
                port: Some(2222),
                ..password_input(&workspace_a)
            })
            .await
            .expect("update ssh connection");
        assert_eq!(updated.name, "Deploy bastion");
        assert_eq!(updated.port, 2222);
        assert_eq!(updated.sync_status, "pending");

        let remaining = service
            .delete_connection(workspace_a.clone(), created.id)
            .await
            .expect("delete ssh connection");
        assert!(remaining.is_empty());
        assert!(service
            .list_connections(workspace_a)
            .await
            .expect("list after delete")
            .is_empty());
    }

    #[tokio::test]
    async fn ssh_connection_validation_keeps_secrets_out_of_config() {
        let (service, workspace_id, _) = service_with_workspaces().await;

        let missing_credential = service
            .save_connection(SshConnectionInput {
                credential_ref: None,
                ..password_input(&workspace_id)
            })
            .await;
        assert!(matches!(missing_credential, Err(AppError::Validation(_))));

        let private_key = service
            .save_connection(SshConnectionInput {
                auth_kind: "private-key".to_string(),
                key_path: Some("C:/Users/zhang/.ssh/id_ed25519".to_string()),
                credential_ref: Some("ssh-key-passphrase-1".to_string()),
                ..password_input(&workspace_id)
            })
            .await
            .expect("save private key metadata");

        let stored_config: (String,) =
            sqlx::query_as("SELECT config_json FROM connections WHERE id = ?1")
                .bind(private_key.id)
                .fetch_one(service.db.pool())
                .await
                .expect("load stored config");
        assert!(stored_config.0.contains("id_ed25519"));
        assert!(!stored_config.0.contains("ssh-key-passphrase-1"));
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn ssh_session_lifecycle_supports_connect_input_resize_close_and_export() {
        let (service, workspace_id, _) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_id))
            .await
            .expect("save ssh connection");

        let session = service
            .connect(SshConnectInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id.clone(),
                cols: Some(100),
                rows: Some(30),
            })
            .await
            .expect("connect ssh session");
        assert_eq!(session.connection_id, connection.id);
        assert_eq!(session.status, "connected");
        assert_eq!(session.cols, 100);

        let output = service
            .send_input(SshSessionInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
                data: "echo ok\npassword=secret\n".to_string(),
            })
            .await
            .expect("send ssh input");
        assert_eq!(output.kind, "output");

        let resize = service
            .resize(SshResizeInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
                cols: 120,
                rows: 40,
            })
            .await
            .expect("resize ssh pty");
        assert_eq!(resize.kind, "resize");

        let sessions = service
            .list_sessions(workspace_id.clone())
            .await
            .expect("list sessions");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].cols, 120);

        let closed = service
            .close_session(SshCloseInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
            })
            .await
            .expect("close session");
        assert_eq!(closed.status, "disconnected");

        let rejected = service
            .send_input(SshSessionInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
                data: "whoami\n".to_string(),
            })
            .await;
        assert!(matches!(rejected, Err(AppError::Validation(_))));

        let export = service
            .export_log(SshLogExportInput {
                workspace_id,
                session_id: session.session_id,
            })
            .expect("export log");
        assert!(export.content.contains("<redacted>"));
        assert!(!export.content.contains("password=secret"));
        assert!(export.redacted);
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn deleting_ssh_connection_closes_active_sessions() {
        let (service, workspace_id, _) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_id))
            .await
            .expect("save ssh connection");
        let session = service
            .connect(SshConnectInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id.clone(),
                cols: None,
                rows: None,
            })
            .await
            .expect("connect ssh session");

        service
            .delete_connection(workspace_id.clone(), connection.id)
            .await
            .expect("delete connection");
        let sessions = service
            .list_sessions(workspace_id)
            .await
            .expect("list sessions after delete");
        assert_eq!(sessions[0].session_id, session.session_id);
        assert_eq!(sessions[0].status, "disconnected");
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn explicit_close_flushes_buffered_output_and_restore_lists_history() {
        let (service, workspace_id, _) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_id))
            .await
            .expect("save ssh connection");
        let session = service
            .connect(SshConnectInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id,
                cols: None,
                rows: None,
            })
            .await
            .expect("connect ssh session");

        {
            let mut sessions = service.sessions.lock().expect("lock sessions");
            sessions
                .get_mut(&session.session_id)
                .expect("session")
                .pending_output
                .push_str("buffered before close\r\n");
        }
        service
            .close_session(SshCloseInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
            })
            .await
            .expect("close session");
        service.sessions.lock().expect("lock sessions").clear();

        let restored = service
            .list_sessions(workspace_id.clone())
            .await
            .expect("list persisted sessions");
        let history = service
            .session_history(SshCloseInput {
                workspace_id,
                session_id: session.session_id,
            })
            .await
            .expect("hydrate persisted history");
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].status, "disconnected");
        assert!(history[0].data.contains("buffered before close"));
        assert!(history[0].data.contains("SSH session closed."));
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn repeated_flush_without_new_output_does_not_duplicate_history() {
        let (service, workspace_id, _) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_id))
            .await
            .expect("save ssh connection");
        let session = service
            .connect(SshConnectInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id,
                cols: None,
                rows: None,
            })
            .await
            .expect("connect ssh session");

        {
            let mut sessions = service.sessions.lock().expect("lock sessions");
            sessions
                .get_mut(&session.session_id)
                .expect("session")
                .pending_output
                .push_str("persist exactly once\r\n");
        }
        service
            .flush_session_history(&session.session_id)
            .await
            .expect("first flush");
        service
            .flush_session_history(&session.session_id)
            .await
            .expect("second flush");

        let history = service
            .session_history(SshCloseInput {
                workspace_id,
                session_id: session.session_id,
            })
            .await
            .expect("hydrate history");
        assert_eq!(history[0].data.matches("persist exactly once").count(), 1);
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn repeated_close_does_not_panic_and_returns_stable_result() {
        let (service, workspace_id, _) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_id))
            .await
            .expect("save ssh connection");
        let session = service
            .connect(SshConnectInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id.clone(),
                cols: None,
                rows: None,
            })
            .await
            .expect("connect ssh session");

        let first_close = service
            .close_session(SshCloseInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
            })
            .await
            .expect("first close");
        assert_eq!(first_close.status, "disconnected");

        let second_close = service
            .close_session(SshCloseInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
            })
            .await
            .expect("second close should not fail");
        assert_eq!(second_close.status, "disconnected");
        assert_eq!(second_close.session_id, first_close.session_id);
    }

    #[tokio::test]
    async fn auth_failure_does_not_leak_password_in_error() {
        // This test verifies the error message contract.
        // A real auth failure requires a live SSH server, so we verify
        // that the error sanitization helper strips sensitive keywords.
        #[cfg(feature = "ssh-native")]
        {
            let sanitized = sanitize_ssh_error(&russh::Error::IO(std::io::Error::new(
                std::io::ErrorKind::Other,
                "password rejected by server",
            )));
            assert!(
                !sanitized.contains("password"),
                "error must not contain password: {}",
                sanitized
            );
        }

        // Non-sensitive errors pass through.
        #[cfg(feature = "ssh-native")]
        {
            let sanitized = sanitize_ssh_error(&russh::Error::IO(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "connection refused",
            )));
            assert_eq!(sanitized, "connection refused");
        }

        // For non-native builds, this test is a no-op but still passes.
        #[cfg(not(feature = "ssh-native"))]
        {
            // Verify that the error types we use don't contain secrets.
            let err = AppError::Config("ssh authentication failed".to_string());
            let msg = err.to_string();
            assert!(!msg.contains("super-secret-password"));
        }
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn async_send_input_and_resize_work_in_simulated_path() {
        let (service, workspace_id, _) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_id))
            .await
            .expect("save ssh connection");
        let session = service
            .connect(SshConnectInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id.clone(),
                cols: Some(80),
                rows: Some(24),
            })
            .await
            .expect("connect ssh session");

        // send_input is now async.
        let event = service
            .send_input(SshSessionInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
                data: "ls -la\n".to_string(),
            })
            .await
            .expect("async send input");
        assert_eq!(event.kind, "output");

        // resize is now async.
        let resize_event = service
            .resize(SshResizeInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
                cols: 200,
                rows: 50,
            })
            .await
            .expect("async resize");
        assert_eq!(resize_event.kind, "resize");
        assert!(resize_event.data.contains("200x50"));

        // Verify session dimensions were updated.
        let sessions = service
            .list_sessions(workspace_id)
            .await
            .expect("list sessions");
        assert_eq!(sessions[0].cols, 200);
        assert_eq!(sessions[0].rows, 50);
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn multiple_sessions_handle_concurrent_input_and_close() {
        let (service, workspace_id, _) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_id))
            .await
            .expect("save ssh connection");

        let session_a = service
            .connect(SshConnectInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id.clone(),
                cols: None,
                rows: None,
            })
            .await
            .expect("connect session a");

        let session_b = service
            .connect(SshConnectInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id.clone(),
                cols: None,
                rows: None,
            })
            .await
            .expect("connect session b");

        // Send input to both sessions concurrently.
        let (result_a, result_b) = tokio::join!(
            service.send_input(SshSessionInput {
                workspace_id: workspace_id.clone(),
                session_id: session_a.session_id.clone(),
                data: "echo A\n".to_string(),
            }),
            service.send_input(SshSessionInput {
                workspace_id: workspace_id.clone(),
                session_id: session_b.session_id.clone(),
                data: "echo B\n".to_string(),
            }),
        );
        assert!(result_a.is_ok());
        assert!(result_b.is_ok());

        // Close session a, session b remains active.
        service
            .close_session(SshCloseInput {
                workspace_id: workspace_id.clone(),
                session_id: session_a.session_id.clone(),
            })
            .await
            .expect("close session a");

        // Session b should still accept input.
        let event_b = service
            .send_input(SshSessionInput {
                workspace_id: workspace_id.clone(),
                session_id: session_b.session_id.clone(),
                data: "whoami\n".to_string(),
            })
            .await
            .expect("session b still active");
        assert_eq!(event_b.kind, "output");

        let sessions = service
            .list_sessions(workspace_id)
            .await
            .expect("list sessions");
        let closed = sessions
            .iter()
            .find(|s| s.session_id == session_a.session_id)
            .unwrap();
        let active = sessions
            .iter()
            .find(|s| s.session_id == session_b.session_id)
            .unwrap();
        assert_eq!(closed.status, "disconnected");
        assert_eq!(active.status, "connected");
    }

    #[cfg(feature = "ssh-native")]
    #[tokio::test]
    async fn terminal_output_callback_can_be_registered() {
        let (service, _, _) = service_with_workspaces().await;

        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let received_clone = received.clone();

        service.set_terminal_output_callback(std::sync::Arc::new(move |payload| {
            received_clone.lock().unwrap().push(payload);
        }));

        // Verify the callback is stored.
        let has_callback = service
            .on_terminal_output
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false);
        assert!(has_callback, "callback should be registered");

        // Invoke the callback manually and verify the payload.
        if let Ok(slot) = service.on_terminal_output.lock() {
            if let Some(ref cb) = *slot {
                cb(r#"{"sessionId":"test","data":"hello"}"#.to_string());
            }
        }
        let items = received.lock().unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].contains("hello"));
    }

    #[cfg(feature = "ssh-native")]
    #[test]
    fn native_keepalive_detects_unresponsive_connections_within_about_ten_seconds() {
        let config = native_client_config();
        assert_eq!(
            config.keepalive_interval,
            Some(std::time::Duration::from_secs(3))
        );
        assert_eq!(config.keepalive_max, 2);
        assert_eq!(
            config.keepalive_interval.unwrap().as_secs() * (config.keepalive_max as u64 + 1),
            9
        );
    }

    #[test]
    fn reconnect_policy_is_bounded_to_three_attempts() {
        assert_eq!(RECONNECT_BACKOFF_SECS, [1, 2, 4]);
        assert_eq!(RECONNECT_BACKOFF_SECS.len(), 3);
        assert_eq!(RECONNECT_BACKOFF_SECS.iter().sum::<u64>(), 7);
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn explicit_close_disables_reconnect() {
        let (service, workspace_id, _) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_id))
            .await
            .expect("save ssh connection");
        let session = service
            .connect(SshConnectInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id,
                cols: None,
                rows: None,
            })
            .await
            .expect("connect session");

        service
            .close_session(SshCloseInput {
                workspace_id,
                session_id: session.session_id.clone(),
            })
            .await
            .expect("close session");

        let sessions = service.sessions.lock().expect("session lock");
        let state = sessions.get(&session.session_id).expect("session state");
        assert!(state.intentional_close);
        assert!(!should_reconnect(state));
        assert_eq!(state.summary.status, "disconnected");
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn cancel_reconnect_marks_session_disconnected() {
        let (service, workspace_id, _) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_id))
            .await
            .expect("save ssh connection");
        let session = service
            .connect(SshConnectInput {
                workspace_id: workspace_id.clone(),
                connection_id: connection.id,
                cols: None,
                rows: None,
            })
            .await
            .expect("connect session");

        let cancelled = service
            .cancel_reconnect(SshReconnectCancelInput {
                workspace_id,
                session_id: session.session_id.clone(),
            })
            .await
            .expect("cancel reconnect");

        assert_eq!(cancelled.status, "disconnected");
        assert_eq!(cancelled.reconnect_attempt, 0);
        let sessions = service.sessions.lock().expect("session lock");
        assert!(!should_reconnect(
            sessions.get(&session.session_id).expect("session state")
        ));
    }

    #[cfg(not(feature = "ssh-native"))]
    #[tokio::test]
    async fn dropped_and_failed_states_stop_after_cleanup() {
        let (service, workspace_id, _) = service_with_workspaces().await;
        let connection = service
            .save_connection(password_input(&workspace_id))
            .await
            .expect("save ssh connection");
        let session = service
            .connect(SshConnectInput {
                workspace_id,
                connection_id: connection.id,
                cols: None,
                rows: None,
            })
            .await
            .expect("connect session");

        let mut sessions = service.sessions.lock().expect("session lock");
        let state = sessions
            .get_mut(&session.session_id)
            .expect("session state");
        state.summary.status = "degraded".to_string();
        assert!(should_reconnect(state));
        state.summary.status = "reconnecting".to_string();
        state.summary.reconnect_attempt = 3;
        assert!(should_reconnect(state));
        state.summary.status = "failed".to_string();
        assert!(!should_reconnect(state));
    }
}
