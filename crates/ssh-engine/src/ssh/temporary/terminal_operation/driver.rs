use super::{
    protocol::{self, Fragment, Parser},
    *,
};
use std::{collections::VecDeque, time::Duration};
use tokio::time::Instant;
use unfour_core::models::TerminalOperationTiming;
mod deadline;
use deadline::{Deadline, Stage};

pub(super) struct Driver<'a> {
    lease: &'a mut terminal::TerminalLease,
    parser: Parser,
    fragments: VecDeque<Fragment>,
    buffered_bytes: usize,
    session: String,
    recovery_attempted: bool,
    child_ready: bool,
    child_exiting: bool,
    child_restored: bool,
    deadline: Deadline,
    timing: TerminalOperationTiming,
}

impl<'a> Driver<'a> {
    fn new(lease: &'a mut terminal::TerminalLease, timeout_ms: Option<u64>) -> Self {
        Self {
            lease,
            parser: Parser::default(),
            fragments: VecDeque::new(),
            buffered_bytes: 0,
            session: Uuid::new_v4().simple().to_string(),
            recovery_attempted: false,
            child_ready: false,
            child_exiting: false,
            child_restored: false,
            deadline: Deadline::new(timeout_ms),
            timing: TerminalOperationTiming::default(),
        }
    }

    async fn next(&mut self) -> Result<Fragment, &'static str> {
        loop {
            self.deadline.check()?;
            if let Some(part) = self.fragments.pop_front() {
                self.buffered_bytes -= fragment_cost(&part);
                return Ok(part);
            }
            if self.child_restored {
                return Err("SSH_TERMINAL_NOT_READY");
            }
            let bytes = tokio::time::timeout_at(self.deadline.at(), self.lease.output.recv())
                .await
                .map_err(|_| self.deadline.error())?
                .ok_or("SSH_TERMINAL_TRANSPORT_CLOSED")?;
            self.ingest(&bytes.data)?;
        }
    }

    fn ingest(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        let parts = self.parser.push(bytes);
        // Even a batch exceeding the memory budget may contain proof that the
        // child already exited. Remember it before rejecting buffered output.
        if parts.iter().any(|part| matches!(part, Fragment::Frame(frame) if frame == &format!("{}:RESTORED", self.session))) {
            self.child_restored = true;
        }
        for part in parts {
            self.buffered_bytes += fragment_cost(&part);
            if self.buffered_bytes > 512 * 1024 {
                return Err("SSH_TERMINAL_OUTPUT_LIMIT");
            }
            self.fragments.push_back(part);
        }
        Ok(())
    }

    fn check_login_prompt(&self) -> Result<(), &'static str> {
        if self
            .lease
            .at_login_prompt()
            .map_err(|_| "SSH_TERMINAL_TRANSPORT_CLOSED")?
        {
            return Err("SSH_TERMINAL_LOGIN_REQUIRED");
        }
        Ok(())
    }

    async fn send(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        for chunk in bytes.chunks(512) {
            self.deadline.check()?;
            // Observe a child exit already queued behind an ACK before sending
            // more payload. Snapshot the queue length so active noise cannot
            // turn this check into an unbounded drain.
            for _ in 0..self.lease.output.len() {
                let Ok(bytes) = self.lease.output.try_recv() else {
                    break;
                };
                self.ingest(&bytes.data)?;
                if self.child_restored {
                    break;
                }
            }
            if self.child_restored {
                return Err("SSH_TERMINAL_NOT_READY");
            }
            self.deadline.check()?;
            if !self.child_ready {
                self.check_login_prompt()?;
                self.lease.mark_dirty();
            }
            tokio::time::timeout_at(
                self.deadline.at(),
                self.lease.writer.data_bytes(chunk.to_vec()),
            )
            .await
            .map_err(|_| self.deadline.error())?
            .map_err(|_| "SSH_TERMINAL_TRANSPORT_CLOSED")?;
        }
        Ok(())
    }

    async fn frame(&mut self, expected: &str) -> Result<(), &'static str> {
        loop {
            if let Fragment::Frame(frame) = self.next().await? {
                if frame == expected {
                    return Ok(());
                }
            }
        }
    }

    async fn input_frame(&mut self, nonce: &str, expected: &str) -> Result<(), &'static str> {
        let invalid = format!("{nonce}:INPUT_INVALID");
        let line_prefix = format!("{nonce}:INPUT:");
        let end_prefix = format!("{nonce}:INPUT_END:");
        loop {
            if let Fragment::Frame(frame) = self.next().await? {
                if frame == expected {
                    return Ok(());
                }
                if frame == invalid {
                    // The child rejects mismatched final totals before eval
                    // and exits itself. Do not interrupt its parent shell.
                    self.child_exiting = true;
                    return Err("SSH_TERMINAL_PROTOCOL_ERROR");
                }
                if frame.starts_with(&line_prefix) || frame.starts_with(&end_prefix) {
                    return Err("SSH_TERMINAL_PROTOCOL_ERROR");
                }
            }
        }
    }

    async fn handshake_frame(
        &mut self,
        prompt: &mut crate::ssh::login_prompt::LoginPrompt,
    ) -> Result<String, &'static str> {
        loop {
            let fragment = self.next().await?;
            // The native reader also saw any prompt prefix from before the claim.
            self.check_login_prompt()?;
            match fragment {
                Fragment::Frame(frame) => return Ok(frame),
                Fragment::Data(bytes) => {
                    if prompt.observe(&bytes) {
                        return Err("SSH_TERMINAL_LOGIN_REQUIRED");
                    }
                }
            }
        }
    }

    async fn prepare(&mut self) -> Result<protocol::InputEncoding, &'static str> {
        self.send(protocol::BOOTSTRAP.as_bytes()).await?;
        let mut prompt = crate::ssh::login_prompt::LoginPrompt::default();
        while self.handshake_frame(&mut prompt).await? != protocol::STARTUP {}
        self.deadline.enter(Stage::Ready);
        self.send(format!("{}\n", self.session).as_bytes()).await?;
        loop {
            let frame = self.handshake_frame(&mut prompt).await?;
            if let Some(capabilities) = frame.strip_prefix(&format!("{}:", self.session)) {
                let encoding = match capabilities {
                    protocol::READY_CAPABILITIES => Some(protocol::InputEncoding::Base64),
                    protocol::OCTAL_CAPABILITIES => Some(protocol::InputEncoding::Octal),
                    _ => None,
                };
                if let Some(encoding) = encoding {
                    self.child_ready = true;
                    return Ok(encoding);
                }
            }
            if frame.starts_with(&format!("{}:READY", self.session)) {
                return Err("SSH_TERMINAL_PROTOCOL_ERROR");
            }
            for (suffix, error) in [
                ("BASE64_MISSING", "SSH_TERMINAL_BASE64_MISSING"),
                ("PRINTF_UNSUPPORTED", "SSH_TERMINAL_PRINTF_UNSUPPORTED"),
            ] {
                if frame == format!("{}:{suffix}", self.session) {
                    self.child_exiting = true;
                    return Err(error);
                }
            }
        }
    }

    async fn execute(
        &mut self,
        command: &str,
        max_bytes: usize,
        cancellation: &CancellationToken,
    ) -> Result<(Vec<u8>, i32), &'static str> {
        if cancellation.is_cancelled() {
            return Err("SSH_TERMINAL_CANCELLED");
        }
        let prepare_start = Instant::now();
        let encoding = self.prepare().await?;
        let nonce = Uuid::new_v4().simple().to_string();
        if cancellation.is_cancelled() {
            return Err("SSH_TERMINAL_CANCELLED");
        }
        self.deadline.enter(Stage::Accept);
        self.send(format!("{nonce}\n").as_bytes()).await?;
        self.frame(&format!("{nonce}:ACCEPT")).await?;
        self.deadline.enter(Stage::InputAck);
        let input_start = Instant::now();
        self.timing.prepare_ms = (input_start - prepare_start).as_millis();
        let mut ack_wait = Duration::ZERO;
        for (line, expected) in encoding.confirmed_payload(command, &nonce) {
            if cancellation.is_cancelled() {
                return Err("SSH_TERMINAL_CANCELLED");
            }
            self.send(&line).await?;
            // Only one short canonical line may be outstanding. Confirm its
            // consumed bytes, and the E terminator's totals, before advancing.
            // Encoded content stays inside this exclusive memory-only stream;
            // never include acknowledgements in diagnostics or error messages.
            let waiting = Instant::now();
            self.input_frame(&nonce, &expected).await?;
            ack_wait += waiting.elapsed();
            self.timing.input_ack_count += 1;
            self.deadline.enter(Stage::InputAck);
        }
        let execution_start = Instant::now();
        self.timing.input_ms = (execution_start - input_start).as_millis();
        self.timing.input_ack_wait_ms = ack_wait.as_millis();
        self.deadline.enter(Stage::Begin);
        let begin = format!("{nonce}:BEGIN");
        let end = format!("{nonce}:END:");
        let mut started = false;
        let mut encoded = Vec::new();
        let max_encoded = (max_bytes + 128).div_ceil(3) * 4;
        loop {
            match self.next().await? {
                Fragment::Frame(frame) if frame == begin && !started => {
                    started = true;
                    self.deadline.enter(Stage::End);
                }
                Fragment::Frame(frame) if started && frame.starts_with(&end) => {
                    if frame != format!("{end}0") {
                        return Err("SSH_TERMINAL_PROTOCOL_ERROR");
                    }
                    let result = protocol::decode(&encoded, &nonce, max_bytes);
                    self.timing.execution_and_output_ms = execution_start.elapsed().as_millis();
                    return result;
                }
                Fragment::Data(bytes) if started => {
                    for byte in bytes {
                        if matches!(byte, b'\r' | b'\n') {
                            continue;
                        }
                        if !byte.is_ascii_alphanumeric() && !matches!(byte, b'+' | b'/' | b'=') {
                            return Err("SSH_TERMINAL_PROTOCOL_ERROR");
                        }
                        if encoded.len() >= max_encoded {
                            return Err("SSH_TERMINAL_OUTPUT_LIMIT");
                        }
                        encoded.push(byte);
                    }
                }
                Fragment::Frame(_) if started => return Err("SSH_TERMINAL_PROTOCOL_ERROR"),
                _ => {}
            }
        }
    }

    async fn restore(&mut self, interrupt: bool) -> Result<(), &'static str> {
        if !self.lease.dirty() || self.child_restored {
            return Ok(());
        }
        if *self.lease.parent.borrow() {
            return Err("SSH_TERMINAL_TRANSPORT_CLOSED");
        }
        if self.recovery_attempted {
            return Err("SSH_TERMINAL_RESTORE_TIMEOUT");
        }
        self.recovery_attempted = true;
        self.deadline.enter(Stage::Restore);
        let expected = format!("{}:RESTORED", self.session);
        // A child may have exited while an earlier stage was waiting. Inspect
        // already-buffered frames before writing to what may now be its parent.
        if self
            .fragments
            .iter()
            .any(|part| matches!(part, Fragment::Frame(frame) if frame == &expected))
        {
            self.child_restored = true;
            return Ok(());
        }
        for _ in 0..self.lease.output.len() {
            self.deadline.check()?;
            let Ok(bytes) = self.lease.output.try_recv() else {
                break;
            };
            if self
                .parser
                .push(&bytes.data)
                .iter()
                .any(|part| matches!(part, Fragment::Frame(frame) if frame == &expected))
            {
                self.child_restored = true;
                return Ok(());
            }
        }
        // An unacknowledged bootstrap may still be the parent login/menu. Never
        // inject Ctrl-C there. A prerequisite failure exits its child itself;
        // wait for that child's correlated restore instead of interrupting it.
        if self.child_ready && !self.child_exiting {
            let sent = self.send(if interrupt { b"\x03" } else { b"Q\n" }).await;
            if self.child_restored {
                return Ok(());
            }
            sent?;
        }
        self.frame(&expected).await
    }
}

fn fragment_cost(part: &Fragment) -> usize {
    (match part {
        Fragment::Data(bytes) => bytes.len(),
        Fragment::Frame(frame) => frame.len(),
    }) + 64
}

pub(super) async fn run(
    lease: terminal::TerminalLease,
    command: &str,
    timeout_ms: Option<u64>,
    cancellation: CancellationToken,
    max_output_bytes: usize,
) -> AppResult<TerminalOperationResult> {
    let mut lease = lease;
    let (result, restoration, timing) = run_report_borrowed(
        &mut lease,
        command,
        timeout_ms,
        cancellation,
        max_output_bytes,
    )
    .await;
    lease.release(restoration.is_ok());
    result
        .map(|(stdout, exit_code)| TerminalOperationResult {
            stdout,
            exit_code,
            timing,
            terminal_restored: restoration.is_ok(),
        })
        .map_err(failure)
}

/// Run an independently restored operation without releasing the exclusive
/// claim. Callers must stop after restoration fails, and release the claim.
pub(super) async fn run_report_borrowed(
    lease: &mut terminal::TerminalLease,
    command: &str,
    timeout_ms: Option<u64>,
    cancellation: CancellationToken,
    max_output_bytes: usize,
) -> (
    Result<(Vec<u8>, i32), &'static str>,
    Result<(), &'static str>,
    TerminalOperationTiming,
) {
    let mut driver = Driver::new(lease, timeout_ms);
    let parent = driver.lease.parent.clone();
    let overflow = driver.lease.overflow.clone();
    if cancellation.is_cancelled() || *parent.borrow() {
        return (Err("SSH_TERMINAL_CANCELLED"), Ok(()), driver.timing);
    }
    let result = await_result(
        driver.execute(command, max_output_bytes, &cancellation),
        &cancellation,
        parent,
        overflow,
    )
    .await;
    // Keep a fully received response even if the shell restoration is uncertain.
    let restoring = Instant::now();
    let restoration = driver.restore(result.is_err()).await;
    driver.timing.restore_ms = restoring.elapsed().as_millis();
    if restoration.is_ok() {
        driver.lease.mark_clean();
    }
    (result, restoration, driver.timing)
}

async fn await_result(
    operation: impl std::future::Future<Output = Result<(Vec<u8>, i32), &'static str>>,
    cancellation: &CancellationToken,
    mut parent: tokio::sync::watch::Receiver<bool>,
    mut overflow: tokio::sync::watch::Receiver<bool>,
) -> Result<(Vec<u8>, i32), &'static str> {
    tokio::select! {
        biased;
        // A complete buffered result remains authoritative if cancellation is
        // observed simultaneously. execute checks cancellation before dispatch.
        result = operation => result,
        _ = cancellation.cancelled() => Err("SSH_TERMINAL_CANCELLED"),
        _ = async { let _ = parent.wait_for(|closed| *closed).await; } => Err("SSH_TERMINAL_TRANSPORT_CLOSED"),
        _ = async { let _ = overflow.wait_for(|full| *full).await; } => Err("SSH_TERMINAL_OUTPUT_LIMIT"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    #[tokio::test]
    async fn complete_decoded_response_wins_same_tick_cancel_and_parent_close() {
        for close_parent in [false, true] {
            let cancel = CancellationToken::new();
            if !close_parent {
                cancel.cancel();
            }
            let (_parent, parent) = tokio::sync::watch::channel(close_parent);
            let (_overflow, overflow) = tokio::sync::watch::channel(false);
            let complete = async {
                protocol::decode(
                    STANDARD.encode(b"HTTP data\x1en:STATUS:0\x1f").as_bytes(),
                    "n",
                    100,
                )
            };
            assert_eq!(
                await_result(complete, &cancel, parent, overflow)
                    .await
                    .unwrap(),
                (b"HTTP data".to_vec(), 0)
            );
        }
    }
    #[tokio::test]
    async fn incomplete_response_loses_to_cancellation_without_success() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let (_parent, parent) = tokio::sync::watch::channel(false);
        let (_overflow, overflow) = tokio::sync::watch::channel(false);
        assert_eq!(
            await_result(std::future::pending(), &cancel, parent, overflow).await,
            Err("SSH_TERMINAL_CANCELLED")
        );
    }
}
