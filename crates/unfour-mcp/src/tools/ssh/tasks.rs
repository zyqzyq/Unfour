use std::collections::BTreeMap;

use serde_json::{json, Map, Value};
use unfour_core::models::{
    SshTask, SshTaskCancelInput, SshTaskCleanupInput, SshTaskDetail, SshTaskRun, SshTaskRunInput,
    SshTaskSaveInput, SshTaskStepInput, SshTasksReorderInput,
};

use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
use crate::sanitize::redact_json_in_place;

use super::super::confirmation::ensure_confirmed_if_guarded;
use super::super::policy::ToolPolicyEvaluation;
use super::super::{
    object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition,
};
use super::task_schemas::{id_schema, run_schema, save_schema, workspace_schema};

const MAX_TASK_LOG_CHARS: usize = 128 * 1024;

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        tool(
            "unfour.ssh.list_tasks",
            "List SSH Tasks",
            "Lists saved SSH task summaries for a workspace.",
            workspace_schema(),
            ToolAnnotations::local_read(),
            list_tasks,
        ),
        tool(
            "unfour.ssh.get_task",
            "Get SSH Task",
            "Returns one SSH task with steps and local connection binding. Sensitive fields inside step configuration are masked.",
            id_schema("taskId", false),
            ToolAnnotations::local_read(),
            get_task,
        ),
        tool(
            "unfour.ssh.save_task",
            "Save SSH Task",
            "Creates or updates a saved SSH task through the command bus. On update, omitted description and defaultConnectionId keep their current values; pass null for defaultConnectionId to clear it. Step types and configuration are validated by the SSH engine.",
            save_schema(),
            ToolAnnotations::local_write(),
            save_task,
        ),
        tool(
            "unfour.ssh.duplicate_task",
            "Duplicate SSH Task",
            "Duplicates a saved SSH task and its steps through the command bus.",
            id_schema("taskId", false),
            ToolAnnotations::local_write(),
            duplicate_task,
        ),
        tool(
            "unfour.ssh.reorder_tasks",
            "Reorder SSH Tasks",
            "Reorders all active SSH tasks in a workspace. taskIds must contain the exact active task set.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "taskIds": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["taskIds"],
                "additionalProperties": false
            }),
            ToolAnnotations::local_write(),
            reorder_tasks,
        ),
        tool(
            "unfour.ssh.delete_task",
            "Delete SSH Task",
            "Soft-deletes a saved SSH task. Guarded policy requires content-bound confirmation.",
            id_schema("taskId", true),
            ToolAnnotations::local_write_destructive(),
            delete_task,
        ),
        tool(
            "unfour.ssh.run_task",
            "Run SSH Task",
            "Starts a saved SSH task on its selected saved connection. Production/read-only policy blocks execution; guarded policy requires content-bound confirmation because a task may execute commands or transfer files.",
            run_schema(),
            ToolAnnotations::remote_action(),
            run_task,
        ),
        tool(
            "unfour.ssh.cancel_task_run",
            "Cancel SSH Task Run",
            "Cancels an active SSH task run through the command bus.",
            id_schema("runId", false),
            ToolAnnotations::remote_action(),
            cancel_task_run,
        ),
        tool(
            "unfour.ssh.list_task_runs",
            "List SSH Task Runs",
            "Lists saved run summaries for one SSH task without exposing local log paths.",
            id_schema("taskId", false),
            ToolAnnotations::local_read(),
            list_task_runs,
        ),
        tool(
            "unfour.ssh.read_task_run_log",
            "Read SSH Task Run Log",
            "Reads a capped SSH task run log. The SSH engine redacts configured secret inputs and the MCP response omits the local log path.",
            id_schema("runId", false),
            ToolAnnotations::local_read(),
            read_task_run_log,
        ),
        tool(
            "unfour.ssh.clear_task_runs",
            "Clear SSH Task Runs",
            "Deletes saved SSH task run records and local log files for one task or the workspace. Guarded policy requires content-bound confirmation.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "taskId": { "type": "string" },
                    "confirm": { "type": "boolean" },
                    "confirmationText": { "type": "string" },
                    "confirmation_text": { "type": "string" }
                },
                "additionalProperties": false
            }),
            ToolAnnotations::local_write_destructive(),
            clear_task_runs,
        ),
    ]
}

fn tool(
    name: &'static str,
    title: &'static str,
    description: &'static str,
    input_schema: Value,
    annotations: ToolAnnotations,
    handler: super::super::ToolHandler,
) -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name,
            title,
            description,
            input_schema,
            output_schema: json!({ "type": "object" }),
            annotations,
        },
        handler,
    }
}

fn list_tasks(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    object_with_allowed_keys(arguments, &["workspaceId"])?;
    let tasks = command_bus
        .list_ssh_tasks(&evaluation.workspace.workspace_id)
        .map_err(execution_error)?;
    Ok(task_list_result(&tasks))
}

fn get_task(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "taskId"])?;
    let task_id = required_string(&arguments, "taskId")?;
    let detail = command_bus
        .get_ssh_task(&evaluation.workspace.workspace_id, &task_id)
        .map_err(execution_error)?;
    Ok(json!({ "task": safe_task_detail(&detail), "source": "command-bus" }))
}

fn save_task(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "taskId",
            "name",
            "description",
            "defaultConnectionId",
            "steps",
        ],
    )?;
    let task_id = optional_string(&arguments, "taskId")?;
    let existing = match &task_id {
        Some(id) => Some(
            command_bus
                .get_ssh_task(&evaluation.workspace.workspace_id, id)
                .map_err(execution_error)?,
        ),
        None => None,
    };
    let steps = parse_steps(arguments.get("steps"))?;
    let input = SshTaskSaveInput {
        id: task_id,
        workspace_id: evaluation.workspace.workspace_id.clone(),
        name: required_string(&arguments, "name")?,
        description: parse_save_description(&arguments, existing.as_ref())?,
        default_connection_id: parse_save_default_connection_id(&arguments, existing.as_ref())?,
        steps,
    };
    let detail = command_bus.save_ssh_task(input).map_err(execution_error)?;
    Ok(json!({ "task": safe_task_detail(&detail), "source": "command-bus" }))
}

fn duplicate_task(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "taskId"])?;
    let task_id = required_string(&arguments, "taskId")?;
    let detail = command_bus
        .duplicate_ssh_task(&evaluation.workspace.workspace_id, &task_id)
        .map_err(execution_error)?;
    Ok(json!({ "task": safe_task_detail(&detail), "source": "command-bus" }))
}

fn reorder_tasks(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "taskIds"])?;
    let task_ids = parse_string_array(arguments.get("taskIds"), "taskIds")?;
    let tasks = command_bus
        .reorder_ssh_tasks(SshTasksReorderInput {
            workspace_id: evaluation.workspace.workspace_id.clone(),
            task_ids,
        })
        .map_err(execution_error)?;
    Ok(task_list_result(&tasks))
}

fn delete_task(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &confirmation_keys("taskId"))?;
    let task_id = required_string(&arguments, "taskId")?;
    ensure_confirmed_if_guarded(
        evaluation,
        &arguments,
        "SSH_DELETE_TASK",
        "Deleting an SSH task hides its saved workflow and requires confirmation under guarded policy.",
        json!({
            "tool": "unfour.ssh.delete_task",
            "workspaceId": evaluation.workspace.workspace_id,
            "taskId": task_id
        }),
    )?;
    command_bus
        .delete_ssh_task(&evaluation.workspace.workspace_id, &task_id)
        .map_err(execution_error)?;
    Ok(json!({ "deleted": true, "taskId": task_id, "source": "command-bus" }))
}

fn run_task(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "taskId",
            "connectionId",
            "inputs",
            "secretInputNames",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let task_id = required_string(&arguments, "taskId")?;
    let connection_id = optional_string(&arguments, "connectionId")?;
    let inputs = parse_inputs(arguments.get("inputs"))?;
    let secret_input_names = match arguments.get("secretInputNames") {
        None => Vec::new(),
        value => parse_string_array(value, "secretInputNames")?,
    };
    ensure_confirmed_if_guarded(
        evaluation,
        &arguments,
        "SSH_RUN_TASK",
        "Running a saved SSH task may execute multiple remote commands or file transfers and requires confirmation under guarded policy.",
        json!({
            "tool": "unfour.ssh.run_task",
            "workspaceId": evaluation.workspace.workspace_id,
            "taskId": task_id,
            "connectionId": connection_id,
            "inputs": inputs,
            "secretInputNames": secret_input_names
        }),
    )?;
    let run = command_bus
        .run_ssh_task(SshTaskRunInput {
            workspace_id: evaluation.workspace.workspace_id.clone(),
            task_id,
            connection_id,
            inputs,
            secret_input_names,
        })
        .map_err(execution_error)?;
    Ok(json!({ "run": safe_run(&run), "source": "command-bus" }))
}

fn cancel_task_run(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "runId"])?;
    let run = command_bus
        .cancel_ssh_task_run(SshTaskCancelInput {
            workspace_id: evaluation.workspace.workspace_id.clone(),
            run_id: required_string(&arguments, "runId")?,
        })
        .map_err(execution_error)?;
    Ok(json!({ "run": safe_run(&run), "source": "command-bus" }))
}

fn list_task_runs(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "taskId"])?;
    let task_id = required_string(&arguments, "taskId")?;
    let runs = command_bus
        .list_ssh_task_runs(&evaluation.workspace.workspace_id, &task_id)
        .map_err(execution_error)?;
    Ok(json!({
        "runs": runs.iter().map(safe_run).collect::<Vec<_>>(),
        "count": runs.len(),
        "source": "command-bus"
    }))
}

fn read_task_run_log(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "runId"])?;
    let run_id = required_string(&arguments, "runId")?;
    let log = command_bus
        .read_ssh_task_run_log(&evaluation.workspace.workspace_id, &run_id)
        .map_err(execution_error)?;
    let original_chars = log.chars().count();
    let truncated = original_chars > MAX_TASK_LOG_CHARS;
    let content = if truncated {
        log.chars().take(MAX_TASK_LOG_CHARS).collect::<String>()
    } else {
        log
    };
    Ok(json!({
        "runId": run_id,
        "content": content,
        "truncated": truncated,
        "returnedChars": content.chars().count(),
        "source": "command-bus"
    }))
}

fn clear_task_runs(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "taskId",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let task_id = optional_string(&arguments, "taskId")?;
    ensure_confirmed_if_guarded(
        evaluation,
        &arguments,
        "SSH_CLEAR_TASK_RUNS",
        "Clearing SSH task runs deletes local run records and log files and requires confirmation under guarded policy.",
        json!({
            "tool": "unfour.ssh.clear_task_runs",
            "workspaceId": evaluation.workspace.workspace_id,
            "taskId": task_id
        }),
    )?;
    let result = command_bus
        .clear_ssh_task_runs(SshTaskCleanupInput {
            workspace_id: evaluation.workspace.workspace_id.clone(),
            task_id,
        })
        .map_err(execution_error)?;
    Ok(json!({
        "deletedRuns": result.deleted_runs,
        "deletedLogs": result.deleted_logs,
        "source": "command-bus"
    }))
}

fn parse_steps(value: Option<&Value>) -> Result<Vec<SshTaskStepInput>, ToolCallError> {
    let Some(Value::Array(_)) = value else {
        return Err(ToolCallError::InvalidArguments(
            "argument `steps` must be an array".to_string(),
        ));
    };
    serde_json::from_value(value.cloned().unwrap_or_default()).map_err(|_| {
        ToolCallError::InvalidArguments(
            "argument `steps` contains an invalid SSH task step".to_string(),
        )
    })
}

fn parse_inputs(value: Option<&Value>) -> Result<BTreeMap<String, String>, ToolCallError> {
    match value {
        None => Ok(BTreeMap::new()),
        Some(Value::Object(_)) => serde_json::from_value(value.cloned().unwrap_or_default())
            .map_err(|_| {
                ToolCallError::InvalidArguments(
                    "argument `inputs` must map strings to strings".to_string(),
                )
            }),
        _ => Err(ToolCallError::InvalidArguments(
            "argument `inputs` must be an object".to_string(),
        )),
    }
}

fn parse_string_array(value: Option<&Value>, key: &str) -> Result<Vec<String>, ToolCallError> {
    let Some(Value::Array(values)) = value else {
        return Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be an array of non-empty strings"
        )));
    };
    values
        .iter()
        .map(|value| match value {
            Value::String(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
            _ => Err(ToolCallError::InvalidArguments(format!(
                "argument `{key}` must be an array of non-empty strings"
            ))),
        })
        .collect()
}

fn task_list_result(tasks: &[SshTask]) -> Value {
    json!({ "tasks": tasks, "count": tasks.len(), "source": "command-bus" })
}

fn safe_task_detail(detail: &SshTaskDetail) -> Value {
    let mut value = serde_json::to_value(detail).unwrap_or_else(|_| json!({}));
    redact_json_in_place(&mut value);
    value
}

fn safe_run(run: &SshTaskRun) -> Value {
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

fn required_string(arguments: &Map<String, Value>, key: &str) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a non-empty string"
        ))),
    }
}

fn optional_string(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => {
            Ok(Some(value.trim().to_string()))
        }
        _ => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a non-empty string when provided"
        ))),
    }
}

fn parse_save_description(
    arguments: &Map<String, Value>,
    existing: Option<&SshTaskDetail>,
) -> Result<String, ToolCallError> {
    match arguments.get("description") {
        None => Ok(existing
            .map(|detail| detail.task.description.clone())
            .unwrap_or_default()),
        Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) => Ok(value.clone()),
        _ => Err(ToolCallError::InvalidArguments(
            "argument `description` must be a string or null when provided".to_string(),
        )),
    }
}

fn parse_save_default_connection_id(
    arguments: &Map<String, Value>,
    existing: Option<&SshTaskDetail>,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get("defaultConnectionId") {
        None => Ok(existing
            .and_then(|detail| detail.local_binding.as_ref())
            .and_then(|binding| binding.default_connection_id.clone())),
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => {
            Ok(Some(value.trim().to_string()))
        }
        _ => Err(ToolCallError::InvalidArguments(
            "argument `defaultConnectionId` must be a non-empty string or null when provided"
                .to_string(),
        )),
    }
}

fn confirmation_keys(id: &'static str) -> Vec<&'static str> {
    vec![
        "workspaceId",
        id,
        "confirm",
        "confirmationText",
        "confirmation_text",
    ]
}

fn execution_error(error: CommandBusAdapterError) -> ToolCallError {
    ToolCallError::Execution {
        code: error.code,
        message: error.message,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use unfour_command_bus::{CurrentWorkspaceResult, ReadCommand, ReadCommandResult};
    use unfour_core::models::{SshTaskLocalBinding, SshTaskStep};

    use crate::tools::ToolRegistry;

    use super::*;

    struct TaskStub {
        environment_type: &'static str,
    }

    impl CommandBusAdapter for TaskStub {
        fn execute_read(
            &self,
            command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            assert_eq!(command, ReadCommand::CurrentWorkspace);
            Ok(ReadCommandResult::CurrentWorkspace(
                CurrentWorkspaceResult {
                    workspace_id: "workspace-1".to_string(),
                    workspace_name: "Workspace".to_string(),
                    environment_type: self.environment_type.to_string(),
                    mcp_policy: "auto".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                },
            ))
        }

        fn execute_saved_api_request(
            &self,
            _request_id: &str,
            _timeout_ms: Option<u64>,
        ) -> Result<unfour_core::models::ApiResponse, CommandBusAdapterError> {
            unreachable!("not used by SSH task tests")
        }

        fn list_db_connections(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<unfour_core::models::DatabaseConnection>, CommandBusAdapterError> {
            unreachable!("not used by SSH task tests")
        }

        fn get_db_schema(
            &self,
            _workspace_id: &str,
            _connection_id: &str,
        ) -> Result<unfour_core::models::DatabaseSchema, CommandBusAdapterError> {
            unreachable!("not used by SSH task tests")
        }

        fn execute_db_query(
            &self,
            _input: unfour_core::models::DatabaseQueryInput,
        ) -> Result<unfour_core::models::DatabaseQueryResult, CommandBusAdapterError> {
            unreachable!("not used by SSH task tests")
        }

        fn list_ssh_tasks(
            &self,
            workspace_id: &str,
        ) -> Result<Vec<SshTask>, CommandBusAdapterError> {
            assert_eq!(workspace_id, "workspace-1");
            Ok(vec![task()])
        }

        fn get_ssh_task(
            &self,
            workspace_id: &str,
            task_id: &str,
        ) -> Result<SshTaskDetail, CommandBusAdapterError> {
            assert_eq!(workspace_id, "workspace-1");
            assert_eq!(task_id, "task-1");
            Ok(detail())
        }

        fn save_ssh_task(
            &self,
            input: SshTaskSaveInput,
        ) -> Result<SshTaskDetail, CommandBusAdapterError> {
            assert_eq!(input.workspace_id, "workspace-1");
            assert_eq!(input.steps.len(), 1);
            assert_eq!(input.steps[0].step_type, "command");
            if input.id.as_deref() == Some("task-1") {
                assert_eq!(input.description, "Deploy service");
                assert_eq!(input.default_connection_id.as_deref(), Some("connection-1"));
            }
            Ok(detail())
        }

        fn run_ssh_task(
            &self,
            input: SshTaskRunInput,
        ) -> Result<SshTaskRun, CommandBusAdapterError> {
            assert_eq!(input.workspace_id, "workspace-1");
            assert_eq!(input.task_id, "task-1");
            Ok(run())
        }
    }

    fn task() -> SshTask {
        SshTask {
            id: "task-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            name: "Deploy".to_string(),
            description: "Deploy service".to_string(),
            sort_order: 0,
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
        }
    }

    fn detail() -> SshTaskDetail {
        SshTaskDetail {
            task: task(),
            steps: vec![SshTaskStep {
                id: "step-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                task_id: "task-1".to_string(),
                name: "Deploy".to_string(),
                step_type: "command".to_string(),
                position: 0,
                enabled: true,
                config_version: 1,
                config_json: json!({
                    "command": "deploy --token {{token}}",
                    "password": "literal-secret"
                }),
                created_at: String::new(),
                updated_at: String::new(),
                deleted_at: None,
            }],
            local_binding: Some(SshTaskLocalBinding {
                task_id: "task-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                default_connection_id: Some("connection-1".to_string()),
                last_used_connection_id: None,
                created_at: String::new(),
                updated_at: String::new(),
            }),
        }
    }

    fn run() -> SshTaskRun {
        SshTaskRun {
            id: "run-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            task_id: "task-1".to_string(),
            connection_id: Some("connection-1".to_string()),
            status: "running".to_string(),
            started_at: String::new(),
            finished_at: None,
            error_message: None,
            log_path: "C:/private/task.log".to_string(),
        }
    }

    #[test]
    fn task_detail_masks_sensitive_config_fields() {
        let registry = ToolRegistry::with_command_bus(Arc::new(TaskStub {
            environment_type: "dev",
        }));
        let result = registry
            .call("unfour.ssh.get_task", json!({ "taskId": "task-1" }))
            .unwrap();

        assert_eq!(result["isError"], false);
        assert!(!result.to_string().contains("literal-secret"));
        assert!(
            result["structuredContent"]["task"]["steps"][0]["configJson"]["password"]
                .as_str()
                .unwrap()
                .starts_with("[mask")
        );
    }

    #[test]
    fn save_task_parses_steps_and_uses_command_bus_adapter() {
        let registry = ToolRegistry::with_command_bus(Arc::new(TaskStub {
            environment_type: "dev",
        }));
        let result = registry
            .call(
                "unfour.ssh.save_task",
                json!({
                    "name": "Deploy",
                    "steps": [{
                        "name": "Deploy",
                        "stepType": "command",
                        "position": 0,
                        "enabled": true,
                        "configJson": { "command": "echo ok" }
                    }]
                }),
            )
            .unwrap();

        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"]["task"]["task"]["id"], "task-1");
    }

    #[test]
    fn save_task_update_preserves_omitted_description_and_default_connection() {
        let registry = ToolRegistry::with_command_bus(Arc::new(TaskStub {
            environment_type: "dev",
        }));
        let result = registry
            .call(
                "unfour.ssh.save_task",
                json!({
                    "taskId": "task-1",
                    "name": "Deploy",
                    "steps": [{
                        "name": "Deploy",
                        "stepType": "command",
                        "position": 0,
                        "enabled": true,
                        "configJson": { "command": "echo ok" }
                    }]
                }),
            )
            .unwrap();

        assert_eq!(result["isError"], false);
        assert_eq!(
            result["structuredContent"]["task"]["task"]["description"],
            "Deploy service"
        );
        assert_eq!(
            result["structuredContent"]["task"]["localBinding"]["defaultConnectionId"],
            "connection-1"
        );
    }

    #[test]
    fn guarded_task_run_requires_content_bound_confirmation() {
        let registry = ToolRegistry::with_command_bus(Arc::new(TaskStub {
            environment_type: "test",
        }));
        let arguments = json!({
            "taskId": "task-1",
            "connectionId": "connection-1",
            "inputs": { "token": "secret-value" },
            "secretInputNames": ["token"]
        });
        let first = registry
            .call("unfour.ssh.run_task", arguments.clone())
            .unwrap();

        assert_eq!(first["isError"], true);
        assert_eq!(
            first["structuredContent"]["error"]["code"],
            "CONFIRMATION_REQUIRED"
        );
        assert!(!first.to_string().contains("secret-value"));
        let confirmation = first["structuredContent"]["confirmation_text"]
            .as_str()
            .unwrap()
            .to_string();
        let mut confirmed = arguments;
        confirmed["confirm"] = json!(true);
        confirmed["confirmation_text"] = json!(confirmation);
        let second = registry.call("unfour.ssh.run_task", confirmed).unwrap();
        assert_eq!(second["isError"], false);
        assert!(second["structuredContent"]["run"].get("logPath").is_none());
    }
}
