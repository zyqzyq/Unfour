use serde_json::{json, Value};
use unfour_core::models::{SshTask, SshTaskDetail, SshTaskRun};

use crate::sanitize::redact_json_in_place;

pub(super) fn task_list_result(tasks: &[SshTask]) -> Value {
    json!({ "tasks": tasks, "count": tasks.len(), "source": "command-bus" })
}

pub(super) fn safe_task_detail(detail: &SshTaskDetail) -> Value {
    let mut value = serde_json::to_value(detail).unwrap_or_else(|_| json!({}));
    redact_json_in_place(&mut value);
    value
}

pub(super) fn safe_run(run: &SshTaskRun) -> Value {
    json!({
        "id": run.id,
        "workspaceId": run.workspace_id,
        "taskId": run.task_id,
        "connectionId": run.connection_id,
        "status": run.status,
        "startedAt": run.started_at,
        "finishedAt": run.finished_at,
        "errorMessage": run.error_message
    })
}
