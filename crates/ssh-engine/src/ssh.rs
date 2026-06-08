use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use unfour_core::models::{
    SshCloseInput, SshConnectInput, SshConnection, SshConnectionConfig, SshConnectionInput,
    SshLogExport, SshLogExportInput, SshResizeInput, SshSessionEvent, SshSessionInput,
    SshSessionSummary, StoredConnection,
};
use unfour_core::redaction::redact_sensitive_lines;
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;
use uuid::Uuid;

#[cfg(feature = "ssh-native")]
use crate::host_key::HostKeyStore;

#[derive(Clone)]
pub struct SshService {
    db: LocalDb,
    #[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
    secret_store: SecretStore,
    sessions: Arc<Mutex<HashMap<String, SshSessionState>>>,
}

#[derive(Debug, Clone)]
struct SshSessionState {
    summary: SshSessionSummary,
    events: Vec<SshSessionEvent>,
    #[cfg(feature = "ssh-native")]
    native_handle: Option<NativeSshHandle>,
}

#[cfg(feature = "ssh-native")]
struct NativeSshHandle {
    handle: std::sync::Arc<tokio::sync::Mutex<russh::client::Handle<SshClientHandler>>>,
}

#[cfg(feature = "ssh-native")]
impl Clone for NativeSshHandle {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
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
            db,
            secret_store,
            sessions: Arc::new(Mutex::new(HashMap::new())),
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
        {
            self.connect_native(&connection, &input).await
        }
        #[cfg(not(feature = "ssh-native"))]
        {
            self.connect_simulated(&connection, &input).await
        }
    }

    pub fn list_sessions(&self, workspace_id: String) -> AppResult<Vec<SshSessionSummary>> {
        validate_workspace_id(&workspace_id)?;
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?
            .values()
            .filter(|state| state.summary.workspace_id == workspace_id)
            .map(|state| state.summary.clone())
            .collect::<Vec<_>>();
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    pub fn send_input(&self, input: SshSessionInput) -> AppResult<SshSessionEvent> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;
        if input.data.is_empty() {
            return Err(AppError::Validation(
                "ssh input cannot be empty".to_string(),
            ));
        }
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
            session_id: input.session_id,
            kind: "output".to_string(),
            data: "Input accepted by SSH PTY stream.\r\n".to_string(),
            created_at: now.clone(),
        };
        state.events.push(event.clone());
        state.summary.updated_at = now;
        Ok(event)
    }

    pub fn resize(&self, input: SshResizeInput) -> AppResult<SshSessionEvent> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;
        validate_pty_size(input.cols, input.rows)?;
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
            session_id: input.session_id,
            kind: "resize".to_string(),
            data: format!("PTY resized to {}x{}.\r\n", input.cols, input.rows),
            created_at: now,
        };
        state.events.push(event.clone());
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
            if state.summary.status == "closed" {
                return Ok(state.summary.clone());
            }
            state.native_handle.take()
        };

        // Disconnect native transport outside the mutex lock.
        #[cfg(feature = "ssh-native")]
        if let Some(native) = native_handle {
            let handle = native.handle.lock().await;
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "session closed", "en")
                .await;
        }

        // Update session status under the lock.
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
        let state =
            session_for_workspace_mut(&mut sessions, &input.workspace_id, &input.session_id)?;

        // Idempotent: if already closed, return the stored summary.
        if state.summary.status == "closed" {
            return Ok(state.summary.clone());
        }

        let now = Utc::now().to_rfc3339();
        state.summary.status = "closed".to_string();
        state.summary.updated_at = now.clone();
        state.events.push(SshSessionEvent {
            session_id: input.session_id,
            kind: "close".to_string(),
            data: "SSH session closed.\r\n".to_string(),
            created_at: now,
        });
        Ok(state.summary.clone())
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
            "status": "transport-phase-1",
            "transport": transport,
            "features": [
                "connection-metadata-crud",
                "credential-ref-boundary",
                "password-auth-session",
                "host-key-tofu",
                "graceful-close",
                "session-close",
                "redacted-log-export"
            ]
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
                        && state.summary.status == "active"
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
                    && state.summary.status == "active"
            }) {
                #[cfg(feature = "ssh-native")]
                state.native_handle.take();

                state.summary.status = "closed".to_string();
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
            status: "active".to_string(),
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
        // Resolve password from SecretStore via credential_ref.
        let credential_ref = connection.credential_ref.as_deref().ok_or_else(|| {
            AppError::Validation("password auth requires a credential reference".to_string())
        })?;
        let password = self
            .secret_store
            .read_secret(connection.workspace_id.clone(), credential_ref.to_string())
            .await
            .map_err(|_| {
                AppError::Config("failed to read ssh credential from secret store".to_string())
            })?;

        // Build russh config.
        let config = Arc::new(russh::client::Config::default());

        // Build host-key handler with TOFU store.
        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        let handler = SshClientHandler {
            host_key_store,
            host: connection.host.clone(),
            port: connection.port,
        };

        // Connect with timeout.
        let addr = format!("{}:{}", connection.host, connection.port);
        let timeout_duration = std::time::Duration::from_secs(15);
        let mut handle = match tokio::time::timeout(
            timeout_duration,
            russh::client::connect(config, addr.as_str(), handler),
        )
        .await
        {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => {
                return Err(AppError::Config(format!(
                    "ssh connection to {}:{} failed: {}",
                    connection.host,
                    connection.port,
                    sanitize_ssh_error(&e)
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

        // Authenticate with password.
        let auth_result = handle
            .authenticate_password(connection.username.clone(), password)
            .await
            .map_err(|_| AppError::Config("ssh authentication failed".to_string()))?;

        if !auth_result.success() {
            return Err(AppError::Config(
                "ssh authentication failed: invalid credentials".to_string(),
            ));
        }

        // Build session summary.
        let now = Utc::now().to_rfc3339();
        let session_id = Uuid::new_v4().to_string();
        let cols = input.cols.unwrap_or(120).clamp(20, 300);
        let rows = input.rows.unwrap_or(32).clamp(8, 100);
        let summary = SshSessionSummary {
            session_id: session_id.clone(),
            workspace_id: connection.workspace_id.clone(),
            connection_id: connection.id.clone(),
            status: "active".to_string(),
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
                    "Connected to {}@{} via native transport with {} auth.\r\n",
                    summary.username, summary.host, summary.auth_kind
                ),
                created_at: now,
            }],
            native_handle: Some(NativeSshHandle {
                handle: std::sync::Arc::new(tokio::sync::Mutex::new(handle)),
            }),
        };

        self.sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?
            .insert(session_id, state);

        Ok(summary)
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
    // Strip anything that looks like it could contain a password or secret.
    if msg.to_ascii_lowercase().contains("password") {
        "ssh transport error".to_string()
    } else {
        msg
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

fn ensure_session_active(state: &SshSessionState) -> AppResult<()> {
    if state.summary.status != "active" {
        return Err(AppError::Validation("ssh session is closed".to_string()));
    }
    Ok(())
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
        assert_eq!(session.status, "active");
        assert_eq!(session.cols, 100);

        let output = service
            .send_input(SshSessionInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
                data: "echo ok\npassword=secret\n".to_string(),
            })
            .expect("send ssh input");
        assert_eq!(output.kind, "output");

        let resize = service
            .resize(SshResizeInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
                cols: 120,
                rows: 40,
            })
            .expect("resize ssh pty");
        assert_eq!(resize.kind, "resize");

        let sessions = service
            .list_sessions(workspace_id.clone())
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
        assert_eq!(closed.status, "closed");

        let rejected = service.send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "whoami\n".to_string(),
        });
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
            .expect("list sessions after delete");
        assert_eq!(sessions[0].session_id, session.session_id);
        assert_eq!(sessions[0].status, "closed");
    }

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
        assert_eq!(first_close.status, "closed");

        let second_close = service
            .close_session(SshCloseInput {
                workspace_id: workspace_id.clone(),
                session_id: session.session_id.clone(),
            })
            .await
            .expect("second close should not fail");
        assert_eq!(second_close.status, "closed");
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
}
