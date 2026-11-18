//! Bounded, memory-only byte operations on the user's already selected shell.
//! This is an internal service capability, not an arbitrary-command adapter.
use super::*;
use tokio_util::sync::CancellationToken;

#[cfg(feature = "ssh-native")]
mod driver;
#[cfg(feature = "ssh-native")]
pub(super) mod probe;
#[cfg(feature = "ssh-native")]
mod protocol;

/// Bytes intentionally have no Debug/Serialize implementation: callers own
/// response projection, redaction and persistence after parsing their protocol.
pub struct TerminalOperationResult {
    pub timing: unfour_core::models::TerminalOperationTiming,
    pub stdout: Vec<u8>,
    pub exit_code: i32,
    pub terminal_restored: bool,
}

/// Claim before pre-request scripts. Dropping an untouched claim releases it;
/// dropping a running dirty operation quarantines the terminal.
pub struct TemporaryTerminalOperation {
    #[cfg(feature = "ssh-native")]
    lease: terminal::TerminalLease,
}

impl SshService {
    pub fn claim_temporary_terminal_operation(
        &self,
        workspace: &str,
        handle: &str,
    ) -> AppResult<TemporaryTerminalOperation> {
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (workspace, handle);
            Err(failure("TEMPORARY_NATIVE_REQUIRED"))
        }
        #[cfg(feature = "ssh-native")]
        {
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
                || entry
                    .probe
                    .as_ref()
                    .is_some_and(|probe| probe.status == "running")
            {
                return Err(failure("SSH_TERMINAL_BUSY"));
            }
            terminal::claim(self, entry, workspace, handle)
                .map(|lease| TemporaryTerminalOperation { lease })
                .map_err(operation_error)
        }
    }
}

impl TemporaryTerminalOperation {
    pub async fn run(
        self,
        command: &str,
        timeout_ms: Option<u64>,
        cancellation: CancellationToken,
        max_output_bytes: usize,
    ) -> AppResult<TerminalOperationResult> {
        if command.is_empty() || command.contains('\0') || command.len() > 4 * 1024 * 1024 {
            return Err(failure("SSH_TERMINAL_INPUT_LIMIT"));
        }
        if max_output_bytes == 0 || max_output_bytes > 12 * 1024 * 1024 {
            return Err(failure("SSH_TERMINAL_OUTPUT_LIMIT"));
        }
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (timeout_ms, cancellation);
            Err(failure("TEMPORARY_NATIVE_REQUIRED"))
        }
        #[cfg(feature = "ssh-native")]
        driver::run(
            self.lease,
            command,
            timeout_ms,
            cancellation,
            max_output_bytes,
        )
        .await
    }
}

#[cfg(feature = "ssh-native")]
fn operation_error(error: AppError) -> AppError {
    match error {
        AppError::Validation(code) => failure(match code.as_str() {
            "SSH_TASK_TERMINAL_BUSY" => "SSH_TERMINAL_BUSY",
            "SSH_TASK_TERMINAL_RECOVERY_REQUIRED" => "SSH_TERMINAL_RECOVERY_REQUIRED",
            _ => "SSH_TERMINAL_NOT_CONNECTED",
        }),
        _ => failure("SSH_TERMINAL_NOT_CONNECTED"),
    }
}
