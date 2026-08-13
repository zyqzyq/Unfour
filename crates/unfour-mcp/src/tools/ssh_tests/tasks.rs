use std::sync::Arc;

use serde_json::json;
use unfour_command_bus::{CurrentWorkspaceResult, ReadCommand, ReadCommandResult};
use unfour_core::models::{
    SshTask, SshTaskDetail, SshTaskLocalBinding, SshTaskRun, SshTaskRunInput, SshTaskSaveInput,
    SshTaskStep,
};

use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
use crate::tools::ToolRegistry;

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

    fn list_ssh_tasks(&self, workspace_id: &str) -> Result<Vec<SshTask>, CommandBusAdapterError> {
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

    fn run_ssh_task(&self, input: SshTaskRunInput) -> Result<SshTaskRun, CommandBusAdapterError> {
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
