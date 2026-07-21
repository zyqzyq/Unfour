use super::*;

impl SshService {
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
    pub(super) fn buffer_session_output(&self, session_id: &str, output: &str) -> bool {
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
    pub(super) fn spawn_flush_session_history(&self, session_id: &str) {
        let service = self.clone();
        let session_id = session_id.to_string();
        tokio::spawn(async move {
            let _ = service.flush_session_history(&session_id).await;
        });
    }

    pub(super) async fn flush_session_history(&self, session_id: &str) -> AppResult<()> {
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

    pub(super) async fn persist_session_summary(&self, session_id: &str) -> AppResult<()> {
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
    pub(super) async fn persisted_disconnected_summary(
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

        #[cfg(feature = "ssh-native")]
        self.cancel_sftp_transfers_for_session(&input.session_id);

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
                    state.sftp = None;
                    state.sftp_generation = state.sftp_generation.saturating_add(1);
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
        self.cancel_sftp_transfers_for_session(&input.session_id);

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
                    state.sftp = None;
                    state.sftp_generation = state.sftp_generation.saturating_add(1);
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
    // Simulated connect (non ssh-native path)
    // -----------------------------------------------------------------------

    #[cfg(not(feature = "ssh-native"))]
    pub(super) async fn connect_simulated(
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
}
