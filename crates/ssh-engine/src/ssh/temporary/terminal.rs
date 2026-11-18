//! Exclusive, memory-only access to an existing PTY. Its native reader remains
//! the only reader; task consumers receive a bounded copy instead of racing it.
use super::*;
use tokio::sync::{mpsc, watch, OwnedMutexGuard, OwnedSemaphorePermit, Semaphore};

pub(super) struct TerminalOwner {
    id: String,
    output: mpsc::UnboundedSender<TerminalChunk>,
    budget: Arc<Semaphore>,
    overflow: watch::Sender<bool>,
}

pub(in crate::ssh) struct TerminalChunk {
    pub data: Vec<u8>,
    _permit: OwnedSemaphorePermit,
}

pub(in crate::ssh) struct TerminalLease {
    service: SshService,
    workspace: String,
    handle: String,
    id: String,
    gate: Option<OwnedMutexGuard<()>>,
    dirty: bool,
    completed: bool,
    pub writer: Arc<russh::ChannelWriteHalf<russh::client::Msg>>,
    pub parent: watch::Receiver<bool>,
    pub output: mpsc::UnboundedReceiver<TerminalChunk>,
    pub overflow: watch::Receiver<bool>,
}

impl SshService {
    #[cfg(test)]
    pub(in crate::ssh) fn claim_terminal(
        &self,
        workspace: &str,
        handle: &str,
    ) -> AppResult<TerminalLease> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        let entry = queue.owned(workspace, handle)?;
        if entry
            .task_runs
            .iter()
            .any(|run| run.run.status == "running")
        {
            return Err(failure("SSH_TASK_TERMINAL_BUSY"));
        }
        claim(self, entry, workspace, handle)
    }

    pub(super) async fn terminal_input_guard(
        &self,
        workspace: &str,
        handle: &str,
    ) -> AppResult<OwnedMutexGuard<()>> {
        let (gate, mut paused) = {
            let mut queue = self
                .temporary
                .0
                .lock()
                .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
            let entry = queue.owned(workspace, handle)?;
            input_available(entry)?;
            (entry.input_gate.clone(), entry.input_paused.subscribe())
        };
        let guard = tokio::select! {
            biased;
            _ = async { let _ = paused.wait_for(|paused| *paused).await; } => return Err(failure("SSH_TASK_TERMINAL_BUSY")),
            guard = gate.lock_owned() => guard,
        };
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        input_available(queue.owned(workspace, handle)?)?;
        Ok(guard)
    }
}

fn input_available(entry: &Entry) -> AppResult<()> {
    if entry.summary.terminal_recovery_required {
        return Err(failure("SSH_TASK_TERMINAL_RECOVERY_REQUIRED"));
    }
    if entry.terminal.is_some() {
        return Err(failure("SSH_TASK_TERMINAL_BUSY"));
    }
    Ok(())
}

pub(super) fn claim(
    service: &SshService,
    entry: &mut Entry,
    workspace: &str,
    handle: &str,
) -> AppResult<TerminalLease> {
    input_available(entry)?;
    let native = entry
        .native
        .as_ref()
        .ok_or_else(|| failure("TEMPORARY_NOT_CONNECTED"))?;
    let cancel = entry
        .cancel
        .as_ref()
        .ok_or_else(|| failure("TEMPORARY_NOT_CONNECTED"))?;
    if entry.summary.status != "connected" || entry.summary.error_code.is_some() || *cancel.borrow()
    {
        return Err(failure("TEMPORARY_NOT_CONNECTED"));
    }
    let gate = entry
        .input_gate
        .clone()
        .try_lock_owned()
        .map_err(|_| failure("SSH_TASK_TERMINAL_BUSY"))?;
    // Account payload plus a minimum per-message cost: both byte volume and
    // a malicious one-byte packet flood remain bounded without false overflow
    // merely because a short control frame is split into many SSH packets.
    let (output, receiver) = mpsc::unbounded_channel();
    let (overflow, observed) = watch::channel(false);
    let id = Uuid::new_v4().simple().to_string();
    entry.terminal = Some(TerminalOwner {
        id: id.clone(),
        output,
        budget: Arc::new(Semaphore::new(512 * 1024)),
        overflow,
    });
    entry.input_paused.send_replace(true);
    entry.summary.terminal_task_busy = true;
    Ok(TerminalLease {
        service: service.clone(),
        workspace: workspace.into(),
        handle: handle.into(),
        id,
        gate: Some(gate),
        dirty: false,
        completed: false,
        writer: native.writer.clone(),
        parent: cancel.subscribe(),
        output: receiver,
        overflow: observed,
    })
}

/// Returning true also suppresses display after overflow or quarantine. A
/// dropped consumer must never expose encoded commands/protocol to the PTY UI.
pub(super) fn route(entry: &mut Entry, message: &russh::ChannelMsg) -> bool {
    let bytes: &[u8] = match message {
        russh::ChannelMsg::Data { data } | russh::ChannelMsg::ExtendedData { data, .. } => {
            data.as_ref()
        }
        _ => return entry.terminal.is_some(),
    };
    entry.login_prompt.observe(bytes);
    let Some(owner) = &entry.terminal else {
        return false;
    };
    for chunk in bytes.chunks(16 * 1024) {
        let Ok(permit) = owner
            .budget
            .clone()
            .try_acquire_many_owned(chunk.len().max(64) as u32)
        else {
            owner.overflow.send_replace(true);
            break;
        };
        if owner
            .output
            .send(TerminalChunk {
                data: chunk.to_vec(),
                _permit: permit,
            })
            .is_err()
        {
            owner.overflow.send_replace(true);
            break;
        }
    }
    true
}

impl TerminalLease {
    /// The UI may already have drained the PTY text. The native reader records
    /// the current prompt under this same lock, including after lease acquisition.
    pub(in crate::ssh) fn at_login_prompt(&self) -> AppResult<bool> {
        let mut queue = self
            .service
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        Ok(queue
            .owned(&self.workspace, &self.handle)?
            .login_prompt
            .active())
    }

    pub(in crate::ssh) fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    // Keep the exclusive claim across consecutive probes, while ensuring a
    // cancellation between restored operations does not quarantine a clean PTY.
    pub(in crate::ssh) fn mark_clean(&mut self) {
        self.dirty = false;
    }
    pub(in crate::ssh) fn release(&mut self, restored: bool) {
        if self.completed {
            return;
        }
        if let Ok(mut queue) = self.service.temporary.0.lock() {
            if let Ok(entry) = queue.owned(&self.workspace, &self.handle) {
                if entry
                    .terminal
                    .as_ref()
                    .is_some_and(|owner| owner.id == self.id)
                {
                    entry.summary.terminal_task_busy = false;
                    if restored || !self.dirty {
                        entry.terminal = None;
                        entry.input_paused.send_replace(false);
                    } else {
                        entry.summary.terminal_recovery_required = true;
                    }
                }
            }
        }
        self.gate.take();
        self.completed = true;
    }
    pub(in crate::ssh) fn dirty(&self) -> bool {
        self.dirty
    }
}
impl Drop for TerminalLease {
    fn drop(&mut self) {
        self.release(false);
    }
}
