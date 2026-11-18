use super::bounded_stream::BoundedSftpStream;
use super::*;
use russh::{Channel, ChannelMsg};
use russh_sftp::{
    client::{error::Error as SftpError, Config, RawSftpSession},
    protocol::{FileAttributes, OpenFlags, StatusCode},
};
use std::future::Future;
use tokio::sync::watch;

const STEP_TIMEOUT: Duration = Duration::from_secs(5);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
const STDOUT: &[u8] = b"UNFOUR_A4_STDOUT\n";
const STDERR: &[u8] = b"UNFOUR_A4_STDERR\n";
const EXEC_OK: &str = "printf 'UNFOUR_A4_STDOUT\\n'; printf 'UNFOUR_A4_STDERR\\n' >&2; exit 0";
const EXEC_NONZERO: &str =
    "printf 'UNFOUR_A4_STDOUT\\n'; printf 'UNFOUR_A4_STDERR\\n' >&2; exit 23";
const FILE_CONTENT: &[u8] = b"unfour-a4-sftp-probe\n";
const OUTPUT_LIMIT: usize = 1024;
type ProbeChannel = Channel<russh::client::Msg>;

/// RawSftpSession's writer can remain blocked on SSH flow control after Drop.
/// The bounded bridge owns that I/O lifetime independently, and keeps an
/// explicit channel close handle which does not wait for data-window credit.
struct SftpBridge {
    task: tokio::task::JoinHandle<()>,
    writer: Arc<russh::ChannelWriteHalf<russh::client::Msg>>,
    closed: bool,
}
impl SftpBridge {
    fn new(channel: ProbeChannel) -> (tokio::io::DuplexStream, Self) {
        let (mut reader, writer) = channel.split();
        let writer = Arc::new(writer);
        let channel_writer = writer.clone();
        let (stream, relay) = tokio::io::duplex(4096);
        let task = tokio::spawn(async move {
            let (mut relay_read, mut relay_write) = tokio::io::split(relay);
            let mut channel_read = reader.make_reader();
            let mut channel_write = channel_writer.make_writer();
            tokio::select! {
                _ = tokio::io::copy(&mut channel_read, &mut relay_write) => {},
                _ = tokio::io::copy(&mut relay_read, &mut channel_write) => {},
            }
        });
        (
            stream,
            Self {
                task,
                writer,
                closed: false,
            },
        )
    }
    async fn close(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
        let _ = tokio::time::timeout(CLOSE_TIMEOUT, self.writer.close()).await;
        self.closed = true;
    }
}
impl Drop for SftpBridge {
    fn drop(&mut self) {
        self.task.abort();
        if !self.closed {
            let writer = self.writer.clone();
            tokio::spawn(async move {
                let _ = tokio::time::timeout(CLOSE_TIMEOUT, writer.close()).await;
            });
        }
    }
}

struct Control {
    parent: watch::Receiver<bool>,
    probe: watch::Receiver<bool>,
}
impl Control {
    fn cancelled(&self) -> bool {
        *self.parent.borrow()
            || *self.probe.borrow()
            || self.parent.has_changed().is_err()
            || self.probe.has_changed().is_err()
    }
    async fn stop(&self) {
        let mut parent = self.parent.clone();
        let mut probe = self.probe.clone();
        tokio::select! {
            _ = parent.wait_for(|value| *value) => {},
            _ = probe.wait_for(|value| *value) => {},
        }
    }
    async fn step<T>(
        &self,
        future: impl Future<Output = Result<T, &'static str>>,
    ) -> Result<T, &'static str> {
        tokio::select! {
            biased;
            _ = self.stop() => Err("CANCELLED"),
            result = tokio::time::timeout(STEP_TIMEOUT, future) => result.unwrap_or(Err("TIMEOUT")),
        }
    }
}

impl SshService {
    pub fn start_temporary_probe(
        &self,
        workspace: &str,
        handle: &str,
        kind: TemporaryProbeKind,
        confirmed: bool,
    ) -> AppResult<TemporaryProbeReport> {
        if kind == TemporaryProbeKind::Tcp {
            return Err(failure("TEMPORARY_PROBE_TARGET_REQUIRED"));
        }
        self.start_temporary_probe_inner(workspace, handle, kind, confirmed, None)
    }

    pub fn start_temporary_tcp_probe(
        &self,
        workspace: &str,
        handle: &str,
        host: &str,
        port: u16,
        confirmed: bool,
    ) -> AppResult<TemporaryProbeReport> {
        let host = crate::ssh::tunnel::target_host(host, port)?.to_owned();
        self.start_temporary_probe_inner(
            workspace,
            handle,
            TemporaryProbeKind::Tcp,
            confirmed,
            Some((host, port)),
        )
    }

    fn start_temporary_probe_inner(
        &self,
        workspace: &str,
        handle: &str,
        kind: TemporaryProbeKind,
        confirmed: bool,
        target: Option<(String, u16)>,
    ) -> AppResult<TemporaryProbeReport> {
        if !confirmed {
            return Err(failure("TEMPORARY_PROBE_CONFIRMATION_REQUIRED"));
        }
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| failure("TEMPORARY_RUNTIME_REQUIRED"))?;
        let (report, client, control, permits, terminal_lease) = {
            let mut queue = self
                .temporary
                .0
                .lock()
                .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
            let entry = queue.owned(workspace, handle)?;
            if entry.summary.status != "connected" || entry.summary.error_code.is_some() {
                return Err(failure("TEMPORARY_PROBE_NOT_CONNECTED"));
            }
            let parent = entry
                .cancel
                .as_ref()
                .filter(|cancel| !*cancel.borrow())
                .ok_or_else(|| failure("TEMPORARY_PROBE_NOT_CONNECTED"))?
                .subscribe();
            let client = entry
                .native
                .as_ref()
                .ok_or_else(|| failure("TEMPORARY_PROBE_NOT_CONNECTED"))?
                .handle
                .clone();
            if entry
                .probe
                .as_ref()
                .is_some_and(|report| report.status == "running")
            {
                return Err(failure("TEMPORARY_PROBE_BUSY"));
            }
            if entry
                .terminal
                .as_ref()
                .is_some_and(|owner| owner.api_request && entry.summary.terminal_task_busy)
            {
                return Err(failure("SSH_TASK_TERMINAL_BUSY"));
            }
            let terminal_lease = if matches!(
                kind,
                TemporaryProbeKind::Terminal
                    | TemporaryProbeKind::Transport
                    | TemporaryProbeKind::Curl
            ) {
                if entry
                    .task_runs
                    .iter()
                    .any(|record| record.run.status == "running")
                {
                    return Err(failure("SSH_TASK_TERMINAL_BUSY"));
                }
                let mut lease =
                    crate::ssh::temporary::terminal::claim(self, entry, workspace, handle)?;
                lease.curl_context = None;
                if kind == TemporaryProbeKind::Curl {
                    lease.short_curl_setup = false;
                }
                Some(lease)
            } else {
                None
            };
            let report = TemporaryProbeReport {
                run_id: Uuid::new_v4().to_string(),
                kind,
                status: "running".into(),
                started_at: Utc::now().to_rfc3339(),
                finished_at: None,
                checks: Vec::new(),
                probe_file: (kind == TemporaryProbeKind::Sftp)
                    .then(|| format!("/tmp/unfour-a4-{}", Uuid::new_v4())),
                cleanup: None,
            };
            let (cancel, probe) = watch::channel(false);
            entry.probe = Some(report.clone());
            entry.probe_cancel = Some(cancel);
            (
                report,
                client,
                Control { parent, probe },
                entry.pending_forwards.clone(),
                terminal_lease,
            )
        };
        let weak = Arc::downgrade(&self.temporary.0);
        let workspace = workspace.to_owned();
        let handle = handle.to_owned();
        let mut result = report.clone();
        runtime.spawn(async move {
            if result.kind == TemporaryProbeKind::Sftp {
                result.cleanup = Some("not-created".into());
            }
            match result.kind {
                TemporaryProbeKind::Exec => {
                    run_exec(&mut result, &client, &control, &permits).await
                }
                TemporaryProbeKind::Channels => {
                    run_channels(&mut result, &client, &control, &permits).await
                }
                TemporaryProbeKind::Sftp => {
                    run_sftp(&mut result, &client, &control, &permits).await
                }
                TemporaryProbeKind::Tcp => {
                    run_tcp(
                        &mut result,
                        &client,
                        &control,
                        &permits,
                        target.expect("TCP destination validated before spawn"),
                    )
                    .await
                }
                TemporaryProbeKind::Terminal => {
                    result.checks = crate::ssh::task::terminal::run_probe(
                        terminal_lease.expect("terminal mode owns its lease"),
                        control.probe.clone(),
                    )
                    .await;
                }
                TemporaryProbeKind::Transport | TemporaryProbeKind::Curl => {
                    result.checks = crate::ssh::temporary::terminal_operation::probe::run(
                        terminal_lease.expect("raw terminal probe owns its lease"),
                        control.probe.clone(),
                        result.kind,
                    )
                    .await;
                }
            }
            result.status = if control.cancelled()
                || result
                    .checks
                    .iter()
                    .any(|check| check.status == "cancelled")
            {
                "cancelled"
            } else {
                "completed"
            }
            .into();
            result.finished_at = Some(Utc::now().to_rfc3339());
            // A removed handle/workspace, or a different run, is never recreated.
            if let Some(queue) = weak.upgrade() {
                if let Ok(mut queue) = queue.lock() {
                    if let Ok(entry) = queue.owned(&workspace, &handle) {
                        if entry
                            .probe
                            .as_ref()
                            .is_some_and(|report| report.run_id == result.run_id)
                        {
                            entry.probe = Some(result);
                            entry.probe_cancel = None;
                        }
                    }
                }
            }
        });
        Ok(report)
    }
}

fn check(id: &str, code: &str, expected: Option<u32>, actual: Option<u32>) -> TemporaryProbeCheck {
    let status = match code {
        "OK" => "pass",
        "CANCELLED" => "cancelled",
        "CHANNEL_REJECTED"
        | "EXEC_REJECTED"
        | "SFTP_UNAVAILABLE"
        | "SSH_TUNNEL_FORWARD_PROHIBITED"
        | "SSH_TUNNEL_CHANNEL_UNSUPPORTED" => "unsupported",
        "TIMEOUT"
        | "INTERACTIVE_LOGIN_PROMPT"
        | "TRANSPORT_CLOSED"
        | "EXIT_STATUS_MISSING"
        | "SFTP_CLEANUP_FAILED"
        | "CHANNEL_BUSY"
        | "SFTP_PROTOCOL_ERROR"
        | "SSH_TUNNEL_TIMEOUT"
        | "SSH_TUNNEL_FORWARD_BUSY"
        | "SSH_TUNNEL_RESOURCE_SHORTAGE"
        | "SSH_TUNNEL_TRANSPORT_CLOSED"
        | "SSH_TUNNEL_FORWARD_FAILED" => "inconclusive",
        _ => "fail",
    };
    TemporaryProbeCheck {
        id: id.into(),
        status: status.into(),
        code: code.into(),
        expected_exit_code: expected,
        actual_exit_code: actual,
    }
}

async fn close(channel: &ProbeChannel) {
    let _ = tokio::time::timeout(CLOSE_TIMEOUT, channel.close()).await;
}

async fn open_channel(
    client: &Arc<russh::client::Handle<SshClientHandler>>,
    control: &Control,
    permits: &Arc<tokio::sync::Semaphore>,
) -> Result<ProbeChannel, &'static str> {
    open_probe_channel(client, control, permits, None).await
}

async fn open_probe_channel(
    client: &Arc<russh::client::Handle<SshClientHandler>>,
    control: &Control,
    permits: &Arc<tokio::sync::Semaphore>,
    target: Option<(String, u16)>,
) -> Result<ProbeChannel, &'static str> {
    if control.cancelled() {
        return Err("CANCELLED");
    }
    let permit = permits.clone().try_acquire_owned().map_err(|_| {
        if target.is_some() {
            "SSH_TUNNEL_FORWARD_BUSY"
        } else {
            "CHANNEL_BUSY"
        }
    })?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let client = client.clone();
    let mut parent = control.parent.clone();
    // Keep ownership of CHANNEL_OPEN until its ACK. Cancelling only the caller
    // would lose a late accepted channel. Pending opens share a bounded budget
    // with forwarding and disappear when the parent transport closes.
    tokio::spawn(async move {
        let _permit = permit;
        if sender.is_closed() {
            return;
        }
        let result = tokio::select! {
            biased;
            _ = parent.wait_for(|value| *value) => return,
            result = async {
                if let Some((host, port)) = target {
                    client.channel_open_direct_tcpip(&host, u32::from(port), "127.0.0.1", 0)
                        .await.map_err(|error| crate::ssh::tunnel::forward_open_failure_code(&error))
                } else {
                    client.channel_open_session().await.map_err(|error| match error {
                        russh::Error::ChannelOpenFailure(_) => "CHANNEL_REJECTED",
                        _ => "TRANSPORT_CLOSED",
                    })
                }
            } => result,
        };
        if let Err(Ok(channel)) = sender.send(result) {
            close(&channel).await;
        }
    });
    control
        .step(async { receiver.await.map_err(|_| "TRANSPORT_CLOSED")? })
        .await
}

async fn run_tcp(
    report: &mut TemporaryProbeReport,
    client: &Arc<russh::client::Handle<SshClientHandler>>,
    control: &Control,
    permits: &Arc<tokio::sync::Semaphore>,
    target: (String, u16),
) {
    let code = match open_probe_channel(client, control, permits, Some(target)).await {
        Ok(channel) => {
            // No payload is read or sent; only the explicit destination's TCP
            // channel acknowledgement is checked, then this channel is closed.
            let closed = tokio::time::timeout(CLOSE_TIMEOUT, channel.close()).await;
            if control.cancelled() {
                "CANCELLED"
            } else {
                match closed {
                    Ok(Ok(())) => "OK",
                    Err(_) => "SSH_TUNNEL_TIMEOUT",
                    Ok(Err(_)) => "SSH_TUNNEL_TRANSPORT_CLOSED",
                }
            }
        }
        Err("TIMEOUT") => "SSH_TUNNEL_TIMEOUT",
        Err("TRANSPORT_CLOSED") => "SSH_TUNNEL_TRANSPORT_CLOSED",
        Err(code) => code,
    };
    report.checks.push(check("tcp-forward", code, None, None));
}

#[derive(Default)]
struct ExecObservation {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit: Option<u32>,
    stdout_prompt: Option<crate::ssh::task::TaskOutputBuffer>,
    stderr_prompt: Option<crate::ssh::task::TaskOutputBuffer>,
}
impl ExecObservation {
    fn observe(&mut self, message: ChannelMsg) -> Result<bool, &'static str> {
        match message {
            ChannelMsg::Data { data } => {
                if self.stdout.len() + self.stderr.len() + data.len() > OUTPUT_LIMIT {
                    return Err("OUTPUT_LIMIT");
                }
                self.stdout.extend_from_slice(&data);
                self.stdout_prompt
                    .get_or_insert_with(crate::ssh::task::TaskOutputBuffer::new)
                    .observe(&data);
            }
            ChannelMsg::ExtendedData { data, ext: 1 } => {
                if self.stdout.len() + self.stderr.len() + data.len() > OUTPUT_LIMIT {
                    return Err("OUTPUT_LIMIT");
                }
                self.stderr.extend_from_slice(&data);
                self.stderr_prompt
                    .get_or_insert_with(crate::ssh::task::TaskOutputBuffer::new)
                    .observe(&data);
            }
            ChannelMsg::ExtendedData { .. } | ChannelMsg::ExitSignal { .. } => {
                return Err("OUTPUT_MISMATCH")
            }
            ChannelMsg::ExitStatus { exit_status } => self.exit = Some(exit_status),
            ChannelMsg::Failure => return Err("EXEC_REJECTED"),
            ChannelMsg::Close => return Ok(true),
            // EOF may precede the exit-status message.
            _ => {}
        }
        if self.stdout.len() + self.stderr.len() > OUTPUT_LIMIT {
            return Err("OUTPUT_LIMIT");
        }
        Ok(false)
    }
    fn code(&self, expected: u32) -> &'static str {
        match self.exit {
            None if self.has_login_prompt() => "INTERACTIVE_LOGIN_PROMPT",
            None => "EXIT_STATUS_MISSING",
            Some(actual) if actual != expected => "EXIT_STATUS_MISMATCH",
            _ if self.stdout != STDOUT || self.stderr != STDERR => "OUTPUT_MISMATCH",
            _ => "OK",
        }
    }

    fn has_login_prompt(&self) -> bool {
        self.stdout_prompt
            .as_ref()
            .is_some_and(|output| output.has_login_prompt())
            || self
                .stderr_prompt
                .as_ref()
                .is_some_and(|output| output.has_login_prompt())
    }

    fn completion_code(&self, outcome: Result<&'static str, &'static str>) -> &'static str {
        let code = outcome.unwrap_or_else(|code| code);
        if code == "TIMEOUT" && self.exit.is_none() && self.has_login_prompt() {
            "INTERACTIVE_LOGIN_PROMPT"
        } else {
            code
        }
    }
}

async fn exec_channel(
    channel: &mut ProbeChannel,
    control: &Control,
    id: &str,
    expected: u32,
) -> TemporaryProbeCheck {
    let mut observation = ExecObservation::default();
    let command = if expected == 23 {
        EXEC_NONZERO
    } else {
        EXEC_OK
    };
    let outcome = control
        .step(async {
            channel
                .exec(true, command.as_bytes())
                .await
                .map_err(|_| "TRANSPORT_CLOSED")?;
            while let Some(message) = channel.wait().await {
                if observation.observe(message)? {
                    break;
                }
            }
            Ok(observation.code(expected))
        })
        .await;
    close(channel).await;
    check(
        id,
        observation.completion_code(outcome),
        Some(expected),
        observation.exit,
    )
}

async fn run_exec(
    report: &mut TemporaryProbeReport,
    client: &Arc<russh::client::Handle<SshClientHandler>>,
    control: &Control,
    permits: &Arc<tokio::sync::Semaphore>,
) {
    for (id, expected) in [("exec-success", 0), ("exec-nonzero", 23)] {
        let result = match open_channel(client, control, permits).await {
            Ok(mut channel) => exec_channel(&mut channel, control, id, expected).await,
            Err(code) => check(id, code, Some(expected), None),
        };
        let stop = result.status != "pass";
        report.checks.push(result);
        if stop {
            break;
        }
    }
}

async fn run_channels(
    report: &mut TemporaryProbeReport,
    client: &Arc<russh::client::Handle<SshClientHandler>>,
    control: &Control,
    permits: &Arc<tokio::sync::Semaphore>,
) {
    let mut first = match open_channel(client, control, permits).await {
        Ok(channel) => channel,
        Err(code) => {
            report
                .checks
                .push(check("concurrent-channels", code, None, None));
            return;
        }
    };
    let mut second = match open_channel(client, control, permits).await {
        Ok(channel) => channel,
        Err(code) => {
            close(&first).await;
            report
                .checks
                .push(check("concurrent-channels", code, None, None));
            return;
        }
    };
    // Both additional channels coexist before either executes/closes. The
    // original interactive PTY remains owned by run_temporary throughout.
    let (a, b) = tokio::join!(
        exec_channel(&mut first, control, "concurrent-channels", 0),
        exec_channel(&mut second, control, "concurrent-channels", 0),
    );
    report.checks.push(if a.status != "pass" { a } else { b });
}

async fn run_sftp(
    report: &mut TemporaryProbeReport,
    client: &Arc<russh::client::Handle<SshClientHandler>>,
    control: &Control,
    permits: &Arc<tokio::sync::Semaphore>,
) {
    let mut channel = match open_channel(client, control, permits).await {
        Ok(channel) => channel,
        Err(code) => {
            report
                .checks
                .push(check("sftp-roundtrip", code, None, None));
            return;
        }
    };
    let accepted = control
        .step(async {
            channel
                .request_subsystem(true, "sftp")
                .await
                .map_err(|_| "TRANSPORT_CLOSED")?;
            loop {
                match channel.wait().await {
                    Some(ChannelMsg::Success) => return Ok(()),
                    Some(ChannelMsg::Failure) => return Err("SFTP_UNAVAILABLE"),
                    Some(ChannelMsg::Close) | None => return Err("TRANSPORT_CLOSED"),
                    _ => {}
                }
            }
        })
        .await;
    if let Err(code) = accepted {
        close(&channel).await;
        report
            .checks
            .push(check("sftp-roundtrip", code, None, None));
        return;
    }
    // Validate the frame length before the library parser allocates a packet.
    // Independently own the channel so even a blocked raw writer is reclaimed.
    let (stream, bridge) = SftpBridge::new(channel);
    let raw = RawSftpSession::new_with_config(
        BoundedSftpStream::new(stream),
        Config {
            max_packet_len: 4096,
            max_concurrent_writes: 1,
            request_timeout_secs: 5,
        },
    );
    if let Err(code) = control
        .step(async {
            raw.init()
                .await
                .map(|_| ())
                .map_err(|error| sftp_error(error, "SFTP_PROTOCOL_ERROR"))
        })
        .await
    {
        report
            .checks
            .push(check("sftp-roundtrip", code, None, None));
    } else {
        sftp_roundtrip(report, &raw, control).await;
    }
    drop(raw);
    bridge.close().await;
}

fn sftp_error(error: SftpError, fallback: &'static str) -> &'static str {
    match error {
        SftpError::Timeout => "TIMEOUT",
        SftpError::IO(_) | SftpError::UnexpectedBehavior(_) => "TRANSPORT_CLOSED",
        SftpError::Status(status) => match status.status_code {
            StatusCode::NoConnection | StatusCode::ConnectionLost => "TRANSPORT_CLOSED",
            StatusCode::OpUnsupported => "SFTP_UNAVAILABLE",
            StatusCode::Ok => "SFTP_PROTOCOL_ERROR",
            _ => fallback,
        },
        SftpError::UnexpectedPacket | SftpError::Limited(_) => "SFTP_PROTOCOL_ERROR",
    }
}

async fn sftp_roundtrip(
    report: &mut TemporaryProbeReport,
    raw: &RawSftpSession,
    control: &Control,
) {
    let path = report
        .probe_file
        .as_deref()
        .expect("SFTP generates its own path")
        .to_owned();
    if control.cancelled() {
        report
            .checks
            .push(check("sftp-roundtrip", "CANCELLED", None, None));
        return;
    }
    let mut attrs = FileAttributes::empty();
    attrs.permissions = Some(0o600);
    // Once CREATE is sent, await its bounded ACK even after cancellation. An
    // acknowledged exclusive create establishes ownership for safe cleanup.
    let opened = raw
        .open(
            &path,
            OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::EXCLUDE,
            attrs,
        )
        .await;
    let file = match opened {
        Ok(file) => file.handle,
        Err(error) => {
            let refused = matches!(&error, SftpError::Status(status) if !matches!(status.status_code, StatusCode::Ok | StatusCode::NoConnection | StatusCode::ConnectionLost));
            let code = if control.cancelled() {
                "CANCELLED"
            } else {
                sftp_error(error, "SFTP_CREATE_FAILED")
            };
            let mut result = check("sftp-roundtrip", code, None, None);
            if !refused {
                // Lost CREATE replies cannot establish ownership. Never delete
                // a possibly pre-existing file; retain only the generated path
                // for on-site reconciliation, and never claim cleanup succeeded.
                report.cleanup = Some("unverified".into());
                if code != "CANCELLED" {
                    result.status = "inconclusive".into();
                }
                report
                    .checks
                    .push(check("sftp-cleanup", "SFTP_CLEANUP_FAILED", None, None));
            }
            report.checks.insert(0, result);
            return;
        }
    };
    report.cleanup = Some("unverified".into());
    let outcome = control
        .step(async {
            raw.write(&file, 0, FILE_CONTENT.to_vec())
                .await
                .map_err(|error| sftp_error(error, "SFTP_WRITE_FAILED"))?;
            let mut received = Vec::with_capacity(FILE_CONTENT.len());
            // SFTP permits short reads. Accumulate only the fixed byte count, with
            // the overall step deadline bounding even a one-byte-at-a-time server.
            while received.len() < FILE_CONTENT.len() {
                let read = raw
                    .read(
                        &file,
                        received.len() as u64,
                        (FILE_CONTENT.len() - received.len() + 1) as u32,
                    )
                    .await
                    .map_err(|error| sftp_error(error, "SFTP_READ_FAILED"))?;
                if read.data.is_empty() || received.len() + read.data.len() > FILE_CONTENT.len() {
                    return Err("SFTP_MISMATCH");
                }
                received.extend_from_slice(&read.data);
            }
            if received != FILE_CONTENT {
                return Err("SFTP_MISMATCH");
            }
            // A full matching response plus EOF at the exact expected size proves
            // no truncated read or trailing byte was mistaken for a successful copy.
            match raw.read(&file, FILE_CONTENT.len() as u64, 1).await {
                Err(SftpError::Status(status)) if status.status_code == StatusCode::Eof => Ok("OK"),
                Err(error) => Err(sftp_error(error, "SFTP_READ_FAILED")),
                Ok(_) => Err("SFTP_MISMATCH"),
            }
        })
        .await;
    report.checks.push(check(
        "sftp-roundtrip",
        outcome.unwrap_or_else(|code| code),
        None,
        None,
    ));
    // Cleanup deliberately ignores cancellation, but is separately bounded.
    // A failed handle-close must not prevent removing our acknowledged file.
    let _ = tokio::time::timeout(CLOSE_TIMEOUT, raw.close(file)).await;
    let removed = matches!(
        tokio::time::timeout(CLOSE_TIMEOUT, raw.remove(path)).await,
        Ok(Ok(_))
    );
    report.cleanup = Some(if removed { "removed" } else { "unverified" }.into());
    report.checks.push(check(
        "sftp-cleanup",
        if removed { "OK" } else { "SFTP_CLEANUP_FAILED" },
        None,
        None,
    ));
}

#[cfg(test)]
mod tests;
