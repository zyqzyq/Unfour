use super::*;

#[cfg(feature = "ssh-native")]
pub(super) enum NativeConnectError {
    Failed(russh::Error),
    TimedOut,
}

#[cfg(feature = "ssh-native")]
struct ShutdownOnDrop(Option<std::net::TcpStream>);

#[cfg(feature = "ssh-native")]
impl Drop for ShutdownOnDrop {
    fn drop(&mut self) {
        if let Some(socket) = self.0.take() {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
    }
}

#[cfg(feature = "ssh-native")]
pub(super) async fn connect_transport(
    host: &str,
    port: u16,
    config: Arc<russh::client::Config>,
    handler: SshClientHandler,
) -> Result<russh::client::Handle<SshClientHandler>, russh::Error> {
    let socket = tokio::net::TcpStream::connect((host, port)).await?;
    socket.set_nodelay(config.nodelay)?;
    let socket = socket.into_std()?;
    // russh runs key exchange in a separate task before returning a handle.
    // Close its socket if the caller times out or cancels while awaiting it.
    let mut guard = ShutdownOnDrop(Some(socket.try_clone()?));
    let handle =
        russh::client::connect_stream(config, tokio::net::TcpStream::from_std(socket)?, handler)
            .await?;
    guard.0.take();
    Ok(handle)
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
        temporary: Option<&TemporarySshTunnel>,
    ) -> AppResult<SshSessionSummary> {
        let now = Utc::now().to_rfc3339();
        let session_id = unfour_core::id::new_id();
        let cols = input.cols.unwrap_or(120).clamp(20, 300);
        let rows = input.rows.unwrap_or(32).clamp(8, 100);
        let (native_handle, prelude) = self
            .open_native_transport(connection, cols, rows, input.secret.as_deref(), temporary)
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
                    "Connected to {}@{} via native transport with {} auth. PTY {}x{} allocated.\r\n{}",
                    summary.username, summary.host, summary.auth_kind, cols, rows, prelude
                ),
                created_at: now,
            }],
            command_line: SshCommandLineTracker::new(),
            pending_output: prelude,
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

        self.spawn_native_supervisor(
            session_id,
            connection.clone(),
            native_handle,
            cancel_rx,
            temporary.is_none(),
        );

        Ok(summary)
    }

    #[cfg(feature = "ssh-native")]
    pub(super) async fn open_native_transport(
        &self,
        connection: &SshConnection,
        cols: u16,
        rows: u16,
        override_secret: Option<&str>,
        temporary: Option<&TemporarySshTunnel>,
    ) -> AppResult<(NativeSshHandle, String)> {
        let (handle, channel, prelude) = if let Some(tunnel) = temporary {
            let forwarded = async {
                let handle = self
                    .open_forwarded_connection(connection, override_secret, tunnel)
                    .await?;
                let (channel, prelude) = tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    open_terminal_channel(&handle, cols, rows, true),
                )
                .await
                .map_err(|_| AppError::Validation("SSH_TUNNEL_TIMEOUT".into()))??;
                Ok::<_, AppError>((handle, channel, prelude))
            };
            tokio::select! {
                biased;
                _ = tunnel.closed() => return Err(AppError::Validation("SSH_TUNNEL_TEMPORARY_UNAVAILABLE".into())),
                result = forwarded => result?,
            }
        } else {
            let handle = self
                .open_authenticated_connection(connection, override_secret)
                .await?;
            let (channel, prelude) = open_terminal_channel(&handle, cols, rows, false).await?;
            (handle, channel, prelude)
        };

        let (reader, writer) = channel.split();
        Ok((
            NativeSshHandle {
                handle: Arc::new(handle),
                writer: Arc::new(writer),
                reader: Arc::new(tokio::sync::Mutex::new(reader)),
            },
            prelude,
        ))
    }

    #[cfg(feature = "ssh-native")]
    pub(super) async fn open_forwarded_connection(
        &self,
        connection: &SshConnection,
        override_secret: Option<&str>,
        tunnel: &TemporarySshTunnel,
    ) -> AppResult<russh::client::Handle<SshClientHandler>> {
        let connect = async {
            let (established, completed) = tokio::sync::oneshot::channel::<()>();
            let stream = tunnel
                .connect_for_handshake(&connection.host, connection.port, async move {
                    if completed.await.is_ok() {
                        std::future::pending::<()>().await;
                    }
                })
                .await?;
            let verified = Arc::new(std::sync::OnceLock::new());
            let mut handler = self.saved_host_handler(connection, None);
            handler.verified_fingerprint = Some(verified.clone());
            let mut handle = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                russh::client::connect_stream(Arc::new(native_client_config()), stream, handler),
            )
            .await
            .map_err(|_| AppError::Validation("SSH_TUNNEL_TIMEOUT".into()))?
            .map_err(|err| AppError::Config(sanitize_ssh_error(&err)))?;
            let _ = established.send(());
            let pin = verified
                .get()
                .ok_or_else(|| AppError::Validation("SSH_TUNNEL_HOST_KEY_UNTRUSTED".into()))?;
            self.check_forwarded_target(connection, pin).await?;
            tokio::time::timeout(
                std::time::Duration::from_secs(30),
                self.authenticate_native(&mut handle, connection, override_secret),
            )
            .await
            .map_err(|_| AppError::Validation("SSH_TUNNEL_TIMEOUT".into()))??;
            self.check_forwarded_target(connection, pin).await?;
            Ok(handle)
        };
        tokio::select! {
            biased;
            _ = tunnel.closed() => Err(AppError::Validation("SSH_TUNNEL_TEMPORARY_UNAVAILABLE".into())),
            result = connect => result,
        }
    }

    #[cfg(feature = "ssh-native")]
    async fn check_forwarded_target(&self, target: &SshConnection, pin: &str) -> AppResult<()> {
        // Draft tests use a synthetic connection with revision zero.
        if target.revision != 0 {
            let current = self
                .get_connection(&target.workspace_id, &target.id)
                .await?;
            if current.revision != target.revision
                || current.credential_ref != target.credential_ref
                || current.key_path != target.key_path
            {
                return Err(AppError::Validation("SSH_TUNNEL_ROUTE_CHANGED".into()));
            }
        }
        let current = HostKeyStore::new(self.db.pool().clone())
            .get_fingerprint(&target.workspace_id, &target.host, target.port)
            .await?;
        match current.as_deref() {
            Some(expected) if expected == pin => Ok(()),
            Some(_) => Err(AppError::Validation("SSH_TUNNEL_HOST_KEY_CHANGED".into())),
            None => Err(AppError::Validation("SSH_TUNNEL_HOST_KEY_UNTRUSTED".into())),
        }
    }

    /// Shared dial/host-key boundary. Channel consumers own PTY, exec and SFTP
    /// setup; routing changes must preserve the endpoint's host-key identity.
    #[cfg(feature = "ssh-native")]
    pub(super) async fn dial_native_connection(
        &self,
        connection: &SshConnection,
    ) -> Result<russh::client::Handle<SshClientHandler>, NativeConnectError> {
        self.dial_saved_host(connection, None).await
    }

    #[cfg(feature = "ssh-native")]
    pub(super) async fn dial_saved_host(
        &self,
        connection: &SshConnection,
        pinned_fingerprint: Option<String>,
    ) -> Result<russh::client::Handle<SshClientHandler>, NativeConnectError> {
        let config = Arc::new(native_client_config());
        let handler = self.saved_host_handler(connection, pinned_fingerprint);
        let host = connection
            .host
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(&connection.host);
        let timeout_duration = std::time::Duration::from_secs(15);
        let dial = connect_transport(host, connection.port, config, handler);
        match tokio::time::timeout(timeout_duration, dial).await {
            Ok(Ok(handle)) => Ok(handle),
            Ok(Err(error)) => Err(NativeConnectError::Failed(error)),
            Err(_) => Err(NativeConnectError::TimedOut),
        }
    }

    #[cfg(feature = "ssh-native")]
    fn saved_host_handler(
        &self,
        connection: &SshConnection,
        pinned_fingerprint: Option<String>,
    ) -> SshClientHandler {
        SshClientHandler {
            temporary_trust: None,
            pinned_fingerprint,
            verified_fingerprint: None,
            host_key_store: HostKeyStore::new(self.db.pool().clone()),
            workspace_id: connection.workspace_id.clone(),
            host: connection.host.clone(),
            port: connection.port,
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
                    NativeConnectError::Failed(error) => AppError::Config(format!(
                        "ssh connection to {}:{} failed: {}",
                        connection.host,
                        connection.port,
                        sanitize_ssh_error(&error)
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
