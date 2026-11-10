use super::*;

#[cfg(feature = "ssh-native")]
pub(super) enum NativeConnectError {
    Failed(String),
    TimedOut,
}

impl SshService {
    // -----------------------------------------------------------------------
    // Native connect (ssh-native feature path)
    // -----------------------------------------------------------------------

    #[cfg(feature = "ssh-native")]
    pub(super) async fn connect_native(
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
            warnings: Vec::new(),
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
            command_line: SshCommandLineTracker::new(),
            pending_output: String::new(),
            history_flush_running: false,
            intentional_close: false,
            native_handle: Some(native_handle.clone()),
            cancel_tx: Some(cancel_tx),
            sftp: None,
            sftp_generation: 0,
        };

        self.sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?
            .insert(session_id.clone(), state);

        self.spawn_native_supervisor(session_id, connection.clone(), native_handle, cancel_rx);

        Ok(summary)
    }

    #[cfg(feature = "ssh-native")]
    pub(super) async fn open_native_transport(
        &self,
        connection: &SshConnection,
        cols: u16,
        rows: u16,
        override_secret: Option<&str>,
    ) -> AppResult<NativeSshHandle> {
        let handle = self
            .open_authenticated_connection(connection, override_secret)
            .await?;
        let (channel, _) = open_terminal_channel(&handle, cols, rows, false).await?;

        let (reader, writer) = channel.split();
        Ok(NativeSshHandle {
            handle: Arc::new(tokio::sync::Mutex::new(handle)),
            writer: Arc::new(writer),
            reader: Arc::new(tokio::sync::Mutex::new(reader)),
        })
    }

    /// Shared dial/host-key boundary. Channel consumers own PTY, exec and SFTP
    /// setup; routing changes must preserve the endpoint's host-key identity.
    #[cfg(feature = "ssh-native")]
    pub(super) async fn dial_native_connection(
        &self,
        connection: &SshConnection,
    ) -> Result<russh::client::Handle<SshClientHandler>, NativeConnectError> {
        let config = Arc::new(native_client_config());
        let handler = SshClientHandler {
            temporary_trust: None,
            host_key_store: HostKeyStore::new(self.db.pool().clone()),
            workspace_id: connection.workspace_id.clone(),
            host: connection.host.clone(),
            port: connection.port,
        };
        let addr = format!("{}:{}", connection.host, connection.port);
        let timeout_duration = std::time::Duration::from_secs(15);
        match tokio::time::timeout(
            timeout_duration,
            russh::client::connect(config, addr.as_str(), handler),
        )
        .await
        {
            Ok(Ok(handle)) => Ok(handle),
            Ok(Err(error)) => Err(NativeConnectError::Failed(sanitize_ssh_error(&error))),
            Err(_) => Err(NativeConnectError::TimedOut),
        }
    }

    #[cfg(feature = "ssh-native")]
    pub(super) async fn open_authenticated_connection(
        &self,
        connection: &SshConnection,
        override_secret: Option<&str>,
    ) -> AppResult<russh::client::Handle<SshClientHandler>> {
        let mut handle =
            self.dial_native_connection(connection)
                .await
                .map_err(|error| match error {
                    NativeConnectError::Failed(message) => AppError::Config(format!(
                        "ssh connection to {}:{} failed: {}",
                        connection.host, connection.port, message
                    )),
                    NativeConnectError::TimedOut => AppError::Config(format!(
                        "ssh connection to {}:{} timed out after 15s",
                        connection.host, connection.port
                    )),
                })?;

        self.authenticate_native(&mut handle, connection, override_secret)
            .await?;
        Ok(handle)
    }
}

#[cfg(feature = "ssh-native")]
pub(super) async fn open_terminal_channel(
    handle: &russh::client::Handle<SshClientHandler>,
    cols: u16,
    rows: u16,
    confirm_requests: bool,
) -> AppResult<(russh::Channel<russh::client::Msg>, String)> {
    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|error| AppError::Config(format!("failed to open ssh channel: {}", error)))?;
    channel
        .request_pty(true, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await
        .map_err(|error| AppError::Config(format!("failed to request ssh pty: {}", error)))?;
    let mut prelude = String::new();
    if confirm_requests {
        confirm_terminal_request(&mut channel, &mut prelude).await?;
    }
    channel
        .request_shell(true)
        .await
        .map_err(|error| AppError::Config(format!("failed to start ssh shell: {}", error)))?;
    if confirm_requests {
        confirm_terminal_request(&mut channel, &mut prelude).await?;
    }
    Ok((channel, prelude))
}

#[cfg(feature = "ssh-native")]
async fn confirm_terminal_request(
    channel: &mut russh::Channel<russh::client::Msg>,
    prelude: &mut String,
) -> AppResult<()> {
    loop {
        match channel.wait().await {
            Some(russh::ChannelMsg::Success) => return Ok(()),
            Some(
                russh::ChannelMsg::Failure | russh::ChannelMsg::Close | russh::ChannelMsg::Eof,
            )
            | None => return Err(AppError::Config("ssh terminal request rejected".into())),
            Some(message) => {
                if let Some(text) =
                    native_supervisor::terminal_output_from_channel_message(&message)
                {
                    append_pending_output(prelude, &text);
                }
            }
        }
    }
}
