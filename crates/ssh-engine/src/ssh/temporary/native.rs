use super::*;
use tokio::sync::{oneshot, watch};

#[derive(Clone)]
pub(crate) struct TrustPrompt {
    pub(super) queue: std::sync::Weak<Mutex<Queue>>,
    pub(super) handle: String,
}
impl TrustPrompt {
    pub(crate) async fn verify(
        &self,
        store: &HostKeyStore,
        workspace: &str,
        host: &str,
        port: u16,
        fingerprint: &str,
    ) -> bool {
        // Read existing trust, never persist an endpoint/authorization binding.
        match store.get_fingerprint(workspace, host, port).await {
            Ok(Some(expected)) => return expected == fingerprint,
            Err(_) => return false,
            Ok(None) => {}
        }
        let (tx, rx) = oneshot::channel();
        let Some(queue) = self.queue.upgrade() else {
            return false;
        };
        {
            let Ok(mut queue) = queue.lock() else {
                return false;
            };
            let Ok(entry) = queue.owned(workspace, &self.handle) else {
                return false;
            };
            // Rekey must match the explicitly approved key for this target.
            if let Some(expected) = &entry.trusted_fingerprint {
                return expected == fingerprint;
            }
            entry.summary.fingerprint = Some(fingerprint.into());
            entry.summary.status = "awaiting-trust".into();
            entry.trust = Some(tx);
        }
        matches!(
            tokio::time::timeout(Duration::from_secs(120), rx).await,
            Ok(Ok(true))
        )
    }
}

impl SshService {
    pub fn trust_temporary(
        &self,
        workspace: &str,
        handle: &str,
        fingerprint: &str,
    ) -> AppResult<()> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        let entry = queue.owned(workspace, handle)?;
        if entry.summary.status != "awaiting-trust"
            || entry.summary.fingerprint.as_deref() != Some(fingerprint)
        {
            return Err(failure("TEMPORARY_TRUST_STATE"));
        }
        let tx = entry
            .trust
            .take()
            .ok_or_else(|| failure("TEMPORARY_TRUST_STATE"))?;
        tx.send(true)
            .map_err(|_| failure("TEMPORARY_TRUST_STATE"))?;
        entry.trusted_fingerprint = Some(fingerprint.into());
        entry.summary.status = "connecting".into();
        Ok(())
    }

    pub fn start_temporary(&self, workspace: &str, handle: &str) -> AppResult<()> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| failure("TEMPORARY_RUNTIME_REQUIRED"))?;
        let (request, cancel_rx) = {
            let mut queue = self
                .temporary
                .0
                .lock()
                .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
            queue.sweep(Instant::now());
            let entry = queue.owned(workspace, handle)?;
            if entry.summary.status != "claimed" {
                return Err(failure("TEMPORARY_CONSUMED"));
            }
            let request = entry
                .request
                .take()
                .ok_or_else(|| failure("TEMPORARY_EXPIRED"))?;
            let (tx, rx) = watch::channel(false);
            entry.cancel = Some(tx);
            entry.summary.status = "connecting".into();
            (request, rx)
        };
        let service = self.clone();
        let workspace = workspace.to_owned();
        let handle = handle.to_owned();
        runtime.spawn(async move {
            service
                .run_temporary(workspace, handle, request, cancel_rx)
                .await;
        });
        Ok(())
    }

    async fn run_temporary(
        &self,
        workspace: String,
        id: String,
        request: TemporaryRequest,
        mut cancel: watch::Receiver<bool>,
    ) {
        // Transport is retained outside the setup future so cancellation during
        // auth/PTY setup can explicitly disconnect it, including late success.
        let mut transport = None;
        let setup = self.setup_temporary(&workspace, &id, &request, &mut transport);
        let result = tokio::select! {
            biased;
            _ = cancel.changed() => Err(failure("TEMPORARY_CANCELLED")),
            result = setup => result,
        };
        drop(request);
        let result = match result {
            Ok(native) => {
                let accepted = if let Ok(mut queue) = self.temporary.0.lock() {
                    if let Ok(entry) = queue.owned(&workspace, &id) {
                        entry.native = Some(native.clone());
                        entry.summary.status = "connected".into();
                        entry.summary.fingerprint = None;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if accepted {
                    let mut reader = native.reader.lock().await;
                    loop {
                        let message = tokio::select! {
                            biased;
                            _ = cancel.changed() => break,
                            message = reader.wait() => message,
                        };
                        match message {
                            Some(message) => {
                                if let Some(data) =
                                    native_supervisor::terminal_output_from_channel_message(
                                        &message,
                                    )
                                {
                                    if let Ok(mut queue) = self.temporary.0.lock() {
                                        if let Ok(entry) = queue.owned(&workspace, &id) {
                                            append_pending_output(&mut entry.output, &data);
                                        }
                                    }
                                }
                                if matches!(
                                    message,
                                    russh::ChannelMsg::Eof | russh::ChannelMsg::Close
                                ) {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
                let _ = tokio::time::timeout(Duration::from_secs(2), native.writer.close()).await;
                Ok(())
            }
            Err(error) => Err(error),
        };
        if let Some(transport) = transport {
            let _ = tokio::time::timeout(Duration::from_secs(2), async {
                transport
                    .lock()
                    .await
                    .disconnect(russh::Disconnect::ByApplication, "closed", "en")
                    .await
            })
            .await;
        }
        let deadline = Instant::now() + RETENTION;
        if let Ok(mut queue) = self.temporary.0.lock() {
            if let Ok(entry) = queue.owned(&workspace, &id) {
                entry.native = None;
                entry.cancel = None;
                entry.trust = None;
                entry.trusted_fingerprint = None;
                entry.summary.fingerprint = None;
                entry.summary.status = "disconnected".into();
                // All setup errors below are fixed codes, never driver text.
                entry.summary.error_code = result.err().map(|error| match error {
                    AppError::Validation(code) => code,
                    _ => "TEMPORARY_FAILED".into(),
                });
                entry.deadline = deadline;
            }
        }
        self.temporary
            .schedule_sweep(&tokio::runtime::Handle::current(), deadline);
    }

    async fn setup_temporary(
        &self,
        workspace: &str,
        id: &str,
        request: &TemporaryRequest,
        transport: &mut Option<Arc<tokio::sync::Mutex<russh::client::Handle<SshClientHandler>>>>,
    ) -> AppResult<NativeSshHandle> {
        let handler = SshClientHandler {
            pinned_fingerprint: None,
            host_key_store: HostKeyStore::new(self.db.pool().clone()),
            workspace_id: workspace.into(),
            host: request.host.to_string(),
            port: request.port,
            temporary_trust: Some(TrustPrompt {
                queue: Arc::downgrade(&self.temporary.0),
                handle: id.into(),
            }),
        };
        // Includes up to 120s of explicit host-trust waiting. Authentication
        // gets its own 30s bound; a single PTY + shell gets 15s total.
        let mut config = native_client_config();
        // A setup timeout must own its outcome. Normal-session keepalives
        // otherwise disconnect a slow authentication/channel after nine seconds.
        // Temporary sessions perform no background liveness probes or reconnect.
        config.keepalive_interval = None;
        let handle = tokio::time::timeout(
            Duration::from_secs(150),
            russh::client::connect(
                Arc::new(config),
                (request.host.as_str(), request.port),
                handler,
            ),
        )
        .await
        .map_err(|_| failure("TEMPORARY_TRANSPORT_TIMEOUT"))?
        .map_err(|_| failure("TEMPORARY_TRANSPORT_OR_TRUST_FAILED"))?;
        let handle = Arc::new(tokio::sync::Mutex::new(handle));
        *transport = Some(handle.clone());
        let authenticated = tokio::time::timeout(Duration::from_secs(30), async {
            handle
                .lock()
                .await
                .authenticate_password(request.username.to_string(), request.password.to_string())
                .await
        })
        .await
        .map_err(|_| failure("TEMPORARY_AUTH_TIMEOUT"))?
        .map_err(|_| failure("TEMPORARY_AUTH_FAILED"))?;
        if !authenticated.success() {
            return Err(failure("TEMPORARY_AUTH_FAILED"));
        }
        let (channel, prelude) = tokio::time::timeout(Duration::from_secs(15), async {
            let guard = handle.lock().await;
            super::super::native_transport::open_terminal_channel(&guard, 120, 32, true).await
        })
        .await
        .map_err(|_| failure("TEMPORARY_CHANNEL_TIMEOUT"))?
        .map_err(|_| failure("TEMPORARY_CHANNEL_FAILED"))?;
        {
            let mut queue = self
                .temporary
                .0
                .lock()
                .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
            append_pending_output(&mut queue.owned(workspace, id)?.output, &prelude);
        }
        let (reader, writer) = channel.split();
        Ok(NativeSshHandle {
            handle,
            reader: Arc::new(tokio::sync::Mutex::new(reader)),
            writer: Arc::new(writer),
        })
    }

    fn temporary_native(&self, workspace: &str, handle: &str) -> AppResult<NativeSshHandle> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        queue
            .owned(workspace, handle)?
            .native
            .clone()
            .ok_or_else(|| failure("TEMPORARY_NOT_CONNECTED"))
    }
    pub async fn temporary_input(&self, input: SshSessionInput) -> AppResult<SshSessionEvent> {
        if input.data.len() > 64 * 1024 {
            return Err(failure("TEMPORARY_INPUT_TOO_LARGE"));
        }
        let native = self.temporary_native(&input.workspace_id, &input.session_id)?;
        tokio::time::timeout(
            Duration::from_secs(5),
            native.writer.data_bytes(input.data.into_bytes()),
        )
        .await
        .map_err(|_| failure("TEMPORARY_IO_TIMEOUT"))?
        .map_err(|_| failure("TEMPORARY_IO_FAILED"))?;
        Ok(SshSessionEvent {
            session_id: input.session_id,
            kind: "input".into(),
            data: String::new(),
            created_at: Utc::now().to_rfc3339(),
        })
    }
    pub async fn temporary_resize(&self, input: SshResizeInput) -> AppResult<SshSessionEvent> {
        let native = self.temporary_native(&input.workspace_id, &input.session_id)?;
        tokio::time::timeout(
            Duration::from_secs(5),
            native.writer.window_change(
                input.cols.clamp(20, 300) as u32,
                input.rows.clamp(8, 100) as u32,
                0,
                0,
            ),
        )
        .await
        .map_err(|_| failure("TEMPORARY_IO_TIMEOUT"))?
        .map_err(|_| failure("TEMPORARY_IO_FAILED"))?;
        Ok(SshSessionEvent {
            session_id: input.session_id,
            kind: "resize".into(),
            data: String::new(),
            created_at: Utc::now().to_rfc3339(),
        })
    }
}
