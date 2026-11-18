//! Explicit current-terminal execution. Never falls back from exec or logs in.
use super::*;
use crate::ssh::temporary::terminal::TerminalLease;
use std::{collections::VecDeque, time::Duration};
use tokio::sync::watch;

mod output;
mod probe;
mod protocol;
use crate::ssh::terminal_wire::{Fragment, InputEncoding, Parser};
pub(in crate::ssh) use probe::run_probe;

const HANDSHAKE: Duration = Duration::from_secs(10);
const RECOVERY: Duration = Duration::from_secs(3);

pub(super) struct TerminalTaskDriver {
    lease: TerminalLease,
    parser: Parser,
    fragments: VecDeque<Fragment>,
    session: String,
    prepared: bool,
    restored: bool,
    uncertain: bool,
    submitted: bool,
    recovery_attempted: bool,
}

impl TerminalTaskDriver {
    pub(super) fn new(lease: TerminalLease) -> Self {
        Self {
            lease,
            parser: Parser::default(),
            fragments: VecDeque::new(),
            session: uuid::Uuid::new_v4().simple().to_string(),
            prepared: false,
            restored: false,
            uncertain: false,
            submitted: false,
            recovery_attempted: false,
        }
    }

    async fn next(&mut self) -> Result<Fragment, TaskStepError> {
        loop {
            if let Some(fragment) = self.fragments.pop_front() {
                return Ok(fragment);
            }
            if self.restored {
                return Err(shared::task_error("SSH_TASK_TERMINAL_NOT_READY"));
            }
            let bytes = self
                .lease
                .output
                .recv()
                .await
                .ok_or_else(|| shared::task_error("SSH_TASK_TERMINAL_NOT_READY"))?;
            self.ingest(&bytes.data);
        }
    }

    fn ingest(&mut self, bytes: &[u8]) {
        let restored = format!("{}:RESTORED", self.session);
        let parts = self.parser.push(bytes);
        self.restored |= parts
            .iter()
            .any(|part| matches!(part, Fragment::Frame(frame) if frame == &restored));
        self.fragments.extend(parts);
    }

    async fn send(&mut self, bytes: &[u8]) -> Result<(), TaskStepError> {
        for chunk in bytes.chunks(512) {
            // A restored marker can be queued immediately after a valid ACK.
            for _ in 0..self.lease.output.len() {
                let Ok(output) = self.lease.output.try_recv() else {
                    break;
                };
                self.ingest(&output.data);
                if self.restored {
                    break;
                }
            }
            if self.restored {
                return Err(shared::task_error("SSH_TASK_TERMINAL_NOT_READY"));
            }
            if !self.prepared {
                if self
                    .lease
                    .at_login_prompt()
                    .map_err(|_| shared::task_error("SSH_TASK_TRANSPORT_CLOSED"))?
                {
                    return Err(shared::task_error("SSH_TASK_INTERACTIVE_LOGIN_PROMPT"));
                }
                self.lease.mark_dirty();
            }
            self.lease
                .writer
                .data_bytes(chunk.to_vec())
                .await
                .map_err(|_| shared::task_error("SSH_TASK_TRANSPORT_CLOSED"))?;
        }
        Ok(())
    }

    async fn frame(&mut self, expected: &str) -> Result<(), TaskStepError> {
        loop {
            if let Fragment::Frame(frame) = self.next().await? {
                if frame == expected {
                    return Ok(());
                }
            }
        }
    }

    async fn prepare(&mut self) -> Result<(), TaskStepError> {
        if self.prepared {
            return Ok(());
        }
        self.send(protocol::BOOTSTRAP.as_bytes()).await?;
        let mut observation = super::command_step::stream::StreamBuffer::new();
        loop {
            match self.next().await? {
                Fragment::Frame(frame) if frame == protocol::STARTUP => break,
                Fragment::Data(data) => {
                    // Bootstrap/login bytes never enter task or terminal output.
                    observation.push(&data, "stdout", &mut |_| {});
                    if observation.has_login_prompt() {
                        return Err(shared::task_error("SSH_TASK_INTERACTIVE_LOGIN_PROMPT"));
                    }
                }
                _ => {}
            }
        }
        self.send(format!("{}\n", self.session).as_bytes()).await?;
        self.frame(&format!("{}:READY", self.session)).await?;
        self.prepared = true;
        Ok(())
    }

    async fn command(
        &mut self,
        command: &str,
        emit: &mut (dyn FnMut(DriverEvent) + Send),
    ) -> Result<TaskStepResult, TaskStepError> {
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        self.send(format!("{nonce}\n").as_bytes()).await?;
        self.frame(&format!("{nonce}:ACCEPT")).await?;
        for (line, expected) in InputEncoding::Octal.confirmed_payload(command, &nonce) {
            // Only the terminator permits execution. Delivery becomes uncertain
            // as soon as its write starts, even if that write later fails.
            self.submitted = line.starts_with(b"E:");
            self.send(&line).await?;
            loop {
                if let Fragment::Frame(frame) = self.next().await? {
                    if frame == expected {
                        break;
                    }
                    if frame == format!("{nonce}:INPUT_INVALID") {
                        // The child rejected the totals and is exiting on its own.
                        self.prepared = false;
                        self.submitted = false;
                        return Err(TaskStepError::InputUnconfirmed);
                    }
                    if frame.starts_with(&format!("{nonce}:")) {
                        return Err(TaskStepError::InputUnconfirmed);
                    }
                }
            }
        }
        let begin = format!("{nonce}:BEGIN");
        let status = format!("{nonce}:STATUS:");
        let end = format!("{nonce}:END");
        let mut started = false;
        let mut exit = None;
        let mut output = output::Output::default();
        loop {
            match self.next().await? {
                Fragment::Frame(frame) if frame == begin && !started => {
                    started = true;
                }
                Fragment::Frame(frame)
                    if started && frame.starts_with(&status) && exit.is_none() =>
                {
                    exit = frame[status.len()..].parse::<u8>().ok();
                    if exit.is_none() {
                        return Err(shared::task_error("SSH_TASK_TERMINAL_NOT_READY"));
                    }
                }
                Fragment::Frame(frame) if frame == end && started => {
                    output.finish(emit);
                    // STATUS alone is not completion: END is emitted only once
                    // cat drains the pipe to EOF, including inherited writers.
                    return match exit {
                        Some(0) => Ok(TaskStepResult { exit_code: Some(0) }),
                        Some(code) => Err(TaskStepError::Failed {
                            message: format!("Command exited with status {code}"),
                            exit_code: Some(i32::from(code)),
                        }),
                        None => Err(TaskStepError::OutcomeUncertain { cancelled: false }),
                    };
                }
                Fragment::Data(data) if started => output.push(&data, emit),
                Fragment::Frame(frame) if started => {
                    output.push(format!("\x1e{frame}\x1f").as_bytes(), emit)
                }
                _ => {}
            }
        }
    }

    async fn restore(&mut self, interrupt: bool) -> bool {
        if self.restored || !self.lease.dirty() {
            return true;
        }
        if self.recovery_attempted {
            return false;
        }
        self.recovery_attempted = true;
        if *self.lease.parent.borrow() {
            return false;
        }
        let expected = format!("{}:RESTORED", self.session);
        let attempt = async {
            if self.prepared {
                let sent = self.send(if interrupt { b"\x03" } else { b"Q\n" }).await;
                if self.restored {
                    return Ok(());
                }
                sent?;
            }
            // Until a correlated READY, the parent may still own stdin.
            // Listen for restoration without writing to that terminal.
            self.frame(&expected).await
        };
        self.restored = matches!(tokio::time::timeout(RECOVERY, attempt).await, Ok(Ok(())));
        self.restored
    }

    pub(super) async fn finish(mut self) -> bool {
        let restored = self.restore(self.uncertain || !self.prepared).await;
        self.lease.release(restored);
        restored
    }
}

impl TaskStepDriver for TerminalTaskDriver {
    async fn execute_step(
        &mut self,
        step: &SshTaskStep,
        cancel: &mut watch::Receiver<bool>,
        emit: &mut (dyn FnMut(DriverEvent) + Send),
    ) -> Result<TaskStepResult, TaskStepError> {
        if self.uncertain || self.restored {
            return Err(TaskStepError::OutcomeUncertain {
                cancelled: *cancel.borrow(),
            });
        }
        self.submitted = false;
        if step.step_type != "command" {
            return Err(shared::task_error("SSH_TASK_TERMINAL_COMMAND_ONLY"));
        }
        let config = parse_command_config(step.config_version, &step.config_json)
            .map_err(|_| shared::task_error("SSH_TASK_TERMINAL_NOT_READY"))?;
        let command = command_step::task_command(&config.command, &config.working_directory)
            .map_err(|_| shared::task_error("SSH_TASK_TERMINAL_NOT_READY"))?;
        let mut stopped = cancel.clone();
        let mut parent = self.lease.parent.clone();
        let mut overflow = self.lease.overflow.clone();
        if *stopped.borrow() || *parent.borrow() {
            return Err(TaskStepError::Cancelled);
        }
        let operation = async {
            tokio::time::timeout(HANDSHAKE, self.prepare())
                .await
                .map_err(|_| shared::task_error("SSH_TASK_TERMINAL_NOT_READY"))??;
            super::command_step::stream::emit_redacted("command", &format!("$ {command}\n"), emit);
            tokio::time::timeout(
                Duration::from_secs(config.timeout_seconds.clamp(1, 3600)),
                self.command(&command, emit),
            )
            .await
            .unwrap_or(Err(TaskStepError::InputUnconfirmed))
        };
        let result = tokio::select! {
            biased;
            _ = async { let _ = stopped.wait_for(|closed| *closed).await; } => Err(TaskStepError::Cancelled),
            _ = async { let _ = parent.wait_for(|closed| *closed).await; } => Err(TaskStepError::Cancelled),
            _ = async { let _ = overflow.wait_for(|full| *full).await; } => Err(shared::task_error("SSH_TASK_TERMINAL_OUTPUT_OVERFLOW")),
            result = operation => result,
        };
        let result = if self.submitted
            && matches!(
                &result,
                Err(TaskStepError::Cancelled
                    | TaskStepError::InputUnconfirmed
                    | TaskStepError::Failed {
                        exit_code: None,
                        ..
                    })
            ) {
            Err(TaskStepError::OutcomeUncertain {
                cancelled: *cancel.borrow() || *self.lease.parent.borrow(),
            })
        } else {
            result
        };
        // Only a correlated END+STATUS confirms this step and permits another
        // one. All other errors require interruption/restoration first.
        self.uncertain = !matches!(
            result,
            Ok(_)
                | Err(TaskStepError::Failed {
                    exit_code: Some(_),
                    ..
                })
        );
        if self.uncertain {
            let _ = self.restore(true).await;
        }
        result
    }
}
