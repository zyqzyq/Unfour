use super::*;

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
    #[cfg(feature = "ssh-native")]
    pub(super) async fn authenticate_native(
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
    pub(super) fn spawn_native_supervisor(
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
    pub(super) fn session_should_reconnect(&self, session_id: &str) -> bool {
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| sessions.get(session_id).map(should_reconnect))
            .unwrap_or(false)
    }

    #[cfg(feature = "ssh-native")]
    pub(super) fn session_dimensions(&self, session_id: &str) -> (u16, u16) {
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
    pub(super) fn install_reconnected_handle(
        &self,
        session_id: &str,
        native_handle: NativeSshHandle,
    ) -> bool {
        let installed = {
            let Ok(mut sessions) = self.sessions.lock() else {
                return false;
            };
            let Some(state) = sessions.get_mut(session_id) else {
                return false;
            };
            if state.intentional_close {
                return false;
            }
            state.sftp = None;
            state.sftp_generation = state.sftp_generation.saturating_add(1);
            state.native_handle = Some(native_handle);
            true
        };
        self.cancel_sftp_transfers_for_session(session_id);
        installed
    }

    #[cfg(feature = "ssh-native")]
    pub(super) fn clear_native_session_resources(&self, session_id: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(state) = sessions.get_mut(session_id) {
                state.native_handle = None;
                state.cancel_tx = None;
                state.sftp = None;
                state.sftp_generation = state.sftp_generation.saturating_add(1);
            }
        }
        self.cancel_sftp_transfers_for_session(session_id);
    }

    #[cfg(feature = "ssh-native")]
    pub(super) fn set_session_health(
        &self,
        session_id: &str,
        status: &str,
        attempt: u8,
        message: &str,
    ) {
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
            record_session_event(
                state,
                SshSessionEvent {
                    session_id: session_id.to_string(),
                    kind: if status == "disconnected" || status == "failed" {
                        "close".to_string()
                    } else {
                        "output".to_string()
                    },
                    data: message.to_string(),
                    created_at: now,
                },
            );
            state.pending_output.push_str(message);
        }
        self.emit_terminal_payload(session_id, message, Some(status), attempt);
    }

    /// Emit any coalesced output accumulated since the last flush, then clear
    /// the buffer. A no-op when the buffer is empty.
    #[cfg(feature = "ssh-native")]
    pub(super) fn flush_emit_buffer(&self, session_id: &str, emit_buffer: &mut String) {
        if emit_buffer.is_empty() {
            return;
        }
        self.emit_terminal_payload(session_id, emit_buffer, None, 0);
        emit_buffer.clear();
    }

    #[cfg(feature = "ssh-native")]
    pub(super) fn emit_terminal_payload(
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

#[cfg(feature = "ssh-native")]
pub(super) fn terminal_output_from_channel_message(message: &russh::ChannelMsg) -> Option<String> {
    match message {
        russh::ChannelMsg::Data { data } | russh::ChannelMsg::ExtendedData { data, .. } => {
            Some(String::from_utf8_lossy(data).to_string())
        }
        _ => None,
    }
}
