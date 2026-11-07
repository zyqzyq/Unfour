//! Terminal output, reconnect policy and session resource installation.
use super::*;

impl SshService {
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
            state.command_line.reset();
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
            append_pending_output(&mut state.pending_output, message);
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
