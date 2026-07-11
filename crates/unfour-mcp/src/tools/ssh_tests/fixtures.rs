use std::sync::Arc;

use unfour_command_bus::{CurrentWorkspaceResult, ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiResponse, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult, DatabaseQuerySafety,
    DatabaseSchema, SshConnection, SshConnectionInput, SshDiagnosticInput, SshDiagnosticResult,
};

use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
use crate::tools::ToolRegistry;

/// Stub that runs diagnostics, echoing the validated command back.
pub(super) struct SshStubCommandBus;

impl CommandBusAdapter for SshStubCommandBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        match command {
            ReadCommand::CurrentWorkspace => Ok(ReadCommandResult::CurrentWorkspace(
                CurrentWorkspaceResult {
                    workspace_id: "ws-active".to_string(),
                    workspace_name: "Active".to_string(),
                    environment_type: "dev".to_string(),
                    mcp_policy: "guarded".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                },
            )),
            _ => Err(CommandBusAdapterError {
                code: "UNEXPECTED",
                message: "unexpected command",
            }),
        }
    }

    fn run_ssh_diagnostic(
        &self,
        input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        Ok(SshDiagnosticResult {
            connection_id: input.connection_id,
            command: input.command,
            stdout: "Filesystem Size Used Avail\n/dev/sda1 50G 20G 30G".to_string(),
            stderr: String::new(),
            exit_status: Some(0),
            truncated: false,
        })
    }

    fn run_ssh_command(
        &self,
        input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        if input.command.chars().any(char::is_control) {
            return Err(CommandBusAdapterError {
                code: "VALIDATION_ERROR",
                message: "control characters detected",
            });
        }

        if input.command.contains("__UNFOUR_MATCH_COUNT__") {
            let allow_multiple =
                input.command.contains("allow_multiple = True") || input.command.ends_with(" '1'");
            return Ok(SshDiagnosticResult {
                connection_id: input.connection_id,
                command: input.command,
                stdout: if allow_multiple {
                    "__UNFOUR_MATCH_COUNT__ 2\n__UNFOUR_PATCHED__ 2\n".to_string()
                } else {
                    "__UNFOUR_MATCH_COUNT__ 2\n".to_string()
                },
                stderr: String::new(),
                exit_status: if allow_multiple { Some(0) } else { Some(3) },
                truncated: false,
            });
        }

        Ok(SshDiagnosticResult {
            connection_id: input.connection_id,
            command: input.command.clone(),
            stdout: format!("ran: {}", input.command),
            stderr: String::new(),
            exit_status: Some(0),
            truncated: false,
        })
    }

    fn save_ssh_connection(
        &self,
        input: SshConnectionInput,
    ) -> Result<SshConnection, CommandBusAdapterError> {
        Ok(SshConnection {
            id: "created-ssh-1".to_string(),
            workspace_id: input.workspace_id,
            name: input.name,
            host: input.host,
            port: input.port.unwrap_or(22),
            username: input.username,
            auth_kind: input.auth_kind,
            key_path: input.key_path,
            credential_ref: input.credential_ref.or_else(|| {
                input
                    .secret
                    .map(|_| "unfour:ws-active:ssh-password:cred-1".to_string())
            }),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            deleted_at: None,
            revision: 1,
            sync_status: "local".to_string(),
            remote_id: None,
        })
    }

    fn execute_saved_api_request(
        &self,
        _request_id: &str,
        _timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        unreachable!()
    }

    fn list_db_connections(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        Ok(vec![])
    }

    fn get_db_schema(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        Ok(DatabaseSchema {
            connection_id: String::new(),
            tables: vec![],
        })
    }

    fn execute_db_query(
        &self,
        _input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        Ok(DatabaseQueryResult {
            columns: vec![],
            rows: vec![],
            affected_rows: 0,
            duration_ms: 0,
            safety: DatabaseQuerySafety {
                classification: "read".to_string(),
                requires_confirmation: false,
                confirmed: true,
                message: None,
            },
        })
    }
}

/// Stub that does NOT override `run_ssh_diagnostic`, so the trait default
/// (unsupported) is exercised ? mirrors a build without `ssh-native`.
pub(super) struct UnsupportedSshCommandBus;

impl CommandBusAdapter for UnsupportedSshCommandBus {
    fn execute_read(
        &self,
        _command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        Ok(ReadCommandResult::CurrentWorkspace(
            CurrentWorkspaceResult {
                workspace_id: "ws-active".to_string(),
                workspace_name: "Active".to_string(),
                environment_type: "dev".to_string(),
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
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        unreachable!()
    }

    fn list_db_connections(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        Ok(vec![])
    }

    fn get_db_schema(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        Ok(DatabaseSchema {
            connection_id: String::new(),
            tables: vec![],
        })
    }

    fn execute_db_query(
        &self,
        _input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        unreachable!()
    }
}

pub(super) fn registry() -> ToolRegistry {
    ToolRegistry::with_command_bus(Arc::new(SshStubCommandBus))
}
