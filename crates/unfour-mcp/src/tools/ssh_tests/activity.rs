use std::sync::{Arc, Mutex};

use serde_json::json;
use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiResponse, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult, DatabaseSchema,
    SshConnection, SshConnectionInput, SshDiagnosticInput, SshDiagnosticResult,
};

use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
use crate::output_schema::assert_success_matches_output_schema;
use crate::tools::ToolRegistry;

use super::fixtures::SshStubCommandBus;

struct ActivityBus {
    calls: Mutex<Vec<String>>,
}

impl ActivityBus {
    fn kept(input: SshDiagnosticInput) -> SshDiagnosticResult {
        SshDiagnosticResult {
            warnings: vec!["ACTIVITY_RECORD_FAILED".into()],
            connection_id: input.connection_id,
            command: input.command,
            stdout: "status-kept".into(),
            stderr: "stderr-kept".into(),
            exit_status: Some(7),
            truncated: false,
        }
    }
}

impl CommandBusAdapter for ActivityBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        SshStubCommandBus.execute_read(command)
    }

    fn run_ssh_diagnostic(
        &self,
        input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        self.calls.lock().unwrap().push(input.command.clone());
        Ok(Self::kept(input))
    }

    fn run_ssh_command(
        &self,
        input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        self.calls.lock().unwrap().push(input.command.clone());
        Ok(Self::kept(input))
    }

    fn save_ssh_connection(
        &self,
        input: SshConnectionInput,
    ) -> Result<SshConnection, CommandBusAdapterError> {
        SshStubCommandBus.save_ssh_connection(input)
    }

    fn execute_saved_api_request(
        &self,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        SshStubCommandBus.execute_saved_api_request(request_id, timeout_ms)
    }

    fn list_db_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        SshStubCommandBus.list_db_connections(workspace_id)
    }

    fn get_db_schema(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        SshStubCommandBus.get_db_schema(workspace_id, connection_id)
    }

    fn execute_db_query(
        &self,
        input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        SshStubCommandBus.execute_db_query(input)
    }
}

fn assert_kept(result: &serde_json::Value) {
    assert_eq!(result["isError"], false);
    let content = &result["structuredContent"];
    assert_eq!(
        content["stdout"].as_str().or(content["content"].as_str()),
        Some("status-kept")
    );
    assert_eq!(content["stderr"], "stderr-kept");
    assert_eq!(content["warnings"], json!(["ACTIVITY_RECORD_FAILED"]));
    assert!(!result.to_string().contains("audit sink"));
    assert!(!result.to_string().contains("secret-file-body"));
}

#[test]
fn command_results_keep_output_and_pass_activity_warnings() {
    let bus = Arc::new(ActivityBus {
        calls: Mutex::new(Vec::new()),
    });
    let registry = ToolRegistry::with_command_bus(bus.clone());
    let blocked = registry
        .call(
            "unfour.ssh.exec",
            json!({ "connectionId": "conn-1", "command": "rm -rf /tmp/app" }),
        )
        .unwrap();
    assert_eq!(blocked["isError"], true);
    assert_eq!(
        crate::response::error_json(&blocked)["error"]["code"],
        "CONFIRMATION_REQUIRED"
    );
    assert!(bus.calls.lock().unwrap().is_empty());

    let diagnostic = registry
        .call(
            "unfour.ssh.run_diagnostic",
            json!({ "connectionId": "conn-1", "command": "uptime" }),
        )
        .unwrap();
    assert_kept(&diagnostic);
    assert_eq!(diagnostic["structuredContent"]["exitStatus"], 7);
    assert_success_matches_output_schema(&registry, "unfour.ssh.run_diagnostic", &diagnostic);

    let executed = registry
        .call(
            "unfour.ssh.exec",
            json!({ "connectionId": "conn-1", "command": "echo marker-9f3c" }),
        )
        .unwrap();
    assert_kept(&executed);
    assert_eq!(executed["structuredContent"]["exitCode"], 7);
    assert_success_matches_output_schema(&registry, "unfour.ssh.exec", &executed);

    let read = registry
        .call(
            "unfour.ssh.read_file",
            json!({ "connectionId": "conn-1", "path": "/tmp/notes.txt" }),
        )
        .unwrap();
    assert_kept(&read);
    assert_success_matches_output_schema(&registry, "unfour.ssh.read_file", &read);

    let write = registry
        .call(
            "unfour.ssh.write_file",
            json!({
                "connectionId": "conn-1",
                "path": "/tmp/notes.txt",
                "content": "secret-file-body"
            }),
        )
        .unwrap();
    assert_kept(&write);
    assert_eq!(
        write["structuredContent"]["bytes"],
        "secret-file-body".len()
    );
    assert_success_matches_output_schema(&registry, "unfour.ssh.write_file", &write);
    assert_eq!(bus.calls.lock().unwrap().len(), 4);
}
