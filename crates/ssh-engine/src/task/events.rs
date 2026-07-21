use super::*;
use std::io::Write;

const MAX_TASK_LOG_BYTES: u64 = 10 * 1024 * 1024;

pub(super) struct TaskLogWriter {
    file: std::fs::File,
    bytes_written: u64,
    truncated: bool,
}

impl TaskLogWriter {
    pub(super) fn create(path: &std::path::Path) -> AppResult<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .map_err(|error| AppError::Config(format!("failed to create SSH task log: {error}")))?;
        Ok(Self {
            file,
            bytes_written: 0,
            truncated: false,
        })
    }

    pub(super) fn write_event(&mut self, event: &SshTaskRunEvent) {
        if self.truncated {
            return;
        }
        let line = match event.kind.as_str() {
            "run" => format!(
                "[{}] run {}\n",
                event.created_at,
                event.status.as_deref().unwrap_or("updated")
            ),
            "step" => format!(
                "[{}] step '{}' {}{}\n",
                event.created_at,
                event.step_name.as_deref().unwrap_or("Unnamed step"),
                event.status.as_deref().unwrap_or("updated"),
                event
                    .error
                    .as_deref()
                    .map(|error| format!(": {error}"))
                    .unwrap_or_default()
            ),
            "output" => {
                let (data, _) = unfour_core::redaction::redact_sensitive_lines(
                    event.data.as_deref().unwrap_or_default(),
                );
                format!(
                    "[{}] {} {}",
                    event.created_at,
                    event.stream.as_deref().unwrap_or("stdout"),
                    data
                )
            }
            "transfer" => format!(
                "[{}] transfer {} {}/{} bytes\n",
                event.created_at,
                event.direction.as_deref().unwrap_or("unknown"),
                event.transferred_bytes.unwrap_or(0),
                event.total_bytes.unwrap_or(0)
            ),
            _ => return,
        };
        self.write_bounded(line.as_bytes());
    }

    fn write_bounded(&mut self, bytes: &[u8]) {
        let remaining = MAX_TASK_LOG_BYTES.saturating_sub(self.bytes_written) as usize;
        if remaining == 0 {
            self.write_truncation_marker();
            return;
        }
        let write_len = remaining.min(bytes.len());
        if self.file.write_all(&bytes[..write_len]).is_ok() {
            self.bytes_written += write_len as u64;
        }
        if write_len < bytes.len() {
            self.write_truncation_marker();
        }
    }

    fn write_truncation_marker(&mut self) {
        if self.truncated {
            return;
        }
        self.truncated = true;
        let _ = self.file.write_all(b"\n[log truncated at 10 MiB]\n");
        let _ = self.file.flush();
    }
}

pub(super) fn run_event(
    run_id: &str,
    task_id: &str,
    status: &str,
    error: Option<String>,
) -> SshTaskRunEvent {
    SshTaskRunEvent {
        run_id: run_id.to_string(),
        task_id: task_id.to_string(),
        kind: "run".to_string(),
        step_id: None,
        step_name: None,
        step_type: None,
        position: None,
        status: Some(status.to_string()),
        stream: None,
        data: None,
        exit_code: None,
        duration_ms: None,
        direction: None,
        transferred_bytes: None,
        total_bytes: None,
        bytes_per_second: None,
        error,
        created_at: Utc::now().to_rfc3339(),
    }
}

pub(super) fn step_event(
    run_id: &str,
    task_id: &str,
    step: &SshTaskStep,
    status: &str,
    duration_ms: Option<u64>,
    exit_code: Option<i32>,
    error: Option<String>,
) -> SshTaskRunEvent {
    SshTaskRunEvent {
        run_id: run_id.to_string(),
        task_id: task_id.to_string(),
        kind: "step".to_string(),
        step_id: Some(step.id.clone()),
        step_name: Some(step.name.clone()),
        step_type: Some(step.step_type.clone()),
        position: Some(step.position),
        status: Some(status.to_string()),
        stream: None,
        data: None,
        exit_code,
        duration_ms,
        direction: None,
        transferred_bytes: None,
        total_bytes: None,
        bytes_per_second: None,
        error,
        created_at: Utc::now().to_rfc3339(),
    }
}

pub(super) fn output_event(
    run_id: &str,
    task_id: &str,
    step: &SshTaskStep,
    stream: &str,
    data: String,
) -> SshTaskRunEvent {
    SshTaskRunEvent {
        run_id: run_id.to_string(),
        task_id: task_id.to_string(),
        kind: "output".to_string(),
        step_id: Some(step.id.clone()),
        step_name: Some(step.name.clone()),
        step_type: Some(step.step_type.clone()),
        position: Some(step.position),
        status: None,
        stream: Some(stream.to_string()),
        data: Some(data),
        exit_code: None,
        duration_ms: None,
        direction: None,
        transferred_bytes: None,
        total_bytes: None,
        bytes_per_second: None,
        error: None,
        created_at: Utc::now().to_rfc3339(),
    }
}

pub(super) fn transfer_event(
    run_id: &str,
    task_id: &str,
    step: &SshTaskStep,
    progress: &TaskTransferProgress,
) -> SshTaskRunEvent {
    SshTaskRunEvent {
        run_id: run_id.to_string(),
        task_id: task_id.to_string(),
        kind: "transfer".to_string(),
        step_id: Some(step.id.clone()),
        step_name: Some(step.name.clone()),
        step_type: Some(step.step_type.clone()),
        position: Some(step.position),
        status: Some("running".to_string()),
        stream: None,
        data: None,
        exit_code: None,
        duration_ms: None,
        direction: Some(progress.direction.clone()),
        transferred_bytes: Some(progress.transferred_bytes),
        total_bytes: Some(progress.total_bytes),
        bytes_per_second: Some(progress.bytes_per_second),
        error: None,
        created_at: Utc::now().to_rfc3339(),
    }
}

#[cfg(feature = "ssh-native")]
impl SshService {
    pub(super) fn emit_task_run_event(&self, event: &SshTaskRunEvent) {
        let callback = self.on_task_run.lock().ok().and_then(|slot| slot.clone());
        if let Some(callback) = callback {
            if let Ok(payload) = serde_json::to_string(event) {
                callback(payload);
            }
        }
    }
}
