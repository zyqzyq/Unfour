use std::sync::Arc;

use serde_json::json;
use unfour_command_bus::{
    CurrentWorkspaceResult, ReadCommand, ReadCommandResult, WorkspaceListResult, WorkspaceSummary,
};
use unfour_core::models::{
    ApiResponse, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult, DatabaseSchema,
    SshCommandHistoryEntry, SshCommandHistoryQuery, SshConnection,
};

use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
use crate::tools::ToolRegistry;

struct HistoryStub {
    environment_type: &'static str,
    mcp_policy: &'static str,
    active_workspace_id: &'static str,
}

impl Default for HistoryStub {
    fn default() -> Self {
        Self {
            environment_type: "dev",
            mcp_policy: "auto",
            active_workspace_id: "workspace-1",
        }
    }
}

impl HistoryStub {
    fn registry(self) -> ToolRegistry {
        ToolRegistry::with_command_bus(Arc::new(self))
    }
}

impl CommandBusAdapter for HistoryStub {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        match command {
            ReadCommand::CurrentWorkspace => {
                Ok(ReadCommandResult::CurrentWorkspace(current_workspace(
                    self.active_workspace_id,
                    self.environment_type,
                    self.mcp_policy,
                )))
            }
            ReadCommand::ListWorkspaces => Ok(ReadCommandResult::Workspaces(WorkspaceListResult {
                workspaces: vec![
                    workspace_summary(
                        "workspace-1",
                        true,
                        self.active_workspace_id == "workspace-1",
                        self.environment_type,
                        self.mcp_policy,
                    ),
                    workspace_summary("workspace-2", false, false, "dev", "auto"),
                ],
                active_workspace_id: self.active_workspace_id.to_string(),
                count: 2,
                source: "command-bus".to_string(),
            })),
            _ => Err(CommandBusAdapterError {
                code: "UNEXPECTED",
                message: "unexpected command",
            }),
        }
    }

    fn list_ssh_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<SshConnection>, CommandBusAdapterError> {
        Ok(all_connections()
            .into_iter()
            .filter(|connection| connection.workspace_id == workspace_id)
            .collect())
    }

    fn list_ssh_command_history(
        &self,
        query: SshCommandHistoryQuery,
    ) -> Result<Vec<SshCommandHistoryEntry>, CommandBusAdapterError> {
        assert!(
            !query.include_redacted,
            "MCP history reads must not request redacted commands"
        );
        let search = query
            .search
            .as_deref()
            .map(str::to_ascii_lowercase)
            .filter(|value| !value.is_empty());
        let mut entries = all_entries()
            .into_iter()
            .filter(|entry| {
                entry.workspace_id == query.workspace_id
                    && query
                        .connection_id
                        .as_deref()
                        .is_none_or(|connection_id| entry.connection_id == connection_id)
                    && (query.include_redacted || !entry.redacted)
                    && search
                        .as_deref()
                        .is_none_or(|needle| entry.command.to_ascii_lowercase().contains(needle))
                    && query
                        .since
                        .as_deref()
                        .is_none_or(|since| entry.executed_at.as_str() >= since)
                    && query
                        .until
                        .as_deref()
                        .is_none_or(|until| entry.executed_at.as_str() <= until)
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            right
                .executed_at
                .cmp(&left.executed_at)
                .then(right.id.cmp(&left.id))
        });
        let limit = query.limit.unwrap_or(50).clamp(1, 200) as usize;
        entries.truncate(limit);
        Ok(entries)
    }

    fn execute_saved_api_request(
        &self,
        _request_id: &str,
        _timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        unreachable!("not used by SSH history tests")
    }

    fn list_db_connections(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        unreachable!("not used by SSH history tests")
    }

    fn get_db_schema(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        unreachable!("not used by SSH history tests")
    }

    fn execute_db_query(
        &self,
        _input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        unreachable!("not used by SSH history tests")
    }
}

fn current_workspace(
    workspace_id: &str,
    environment_type: &str,
    mcp_policy: &str,
) -> CurrentWorkspaceResult {
    CurrentWorkspaceResult {
        workspace_id: workspace_id.to_string(),
        workspace_name: workspace_id.to_string(),
        environment_type: environment_type.to_string(),
        mcp_policy: mcp_policy.to_string(),
        workspace_root: None,
        mode: "local".to_string(),
        source: "command-bus".to_string(),
    }
}

fn workspace_summary(
    id: &str,
    is_default: bool,
    is_active: bool,
    environment_type: &str,
    mcp_policy: &str,
) -> WorkspaceSummary {
    WorkspaceSummary {
        id: id.to_string(),
        name: id.to_string(),
        is_default,
        is_active,
        environment_type: environment_type.to_string(),
        mcp_policy: mcp_policy.to_string(),
        last_opened_at: None,
    }
}

fn connection(
    id: &str,
    workspace_id: &str,
    name: &str,
    host: &str,
    username: &str,
) -> SshConnection {
    SshConnection {
        id: id.to_string(),
        workspace_id: workspace_id.to_string(),
        name: name.to_string(),
        host: host.to_string(),
        port: 22,
        username: username.to_string(),
        auth_kind: "password".to_string(),
        key_path: None,
        credential_ref: Some("unfour:workspace-1:ssh-password:cred-1".to_string()),
        created_at: "2026-08-13T00:00:00Z".to_string(),
        updated_at: "2026-08-13T00:00:00Z".to_string(),
        deleted_at: None,
        revision: 1,
        sync_status: "local".to_string(),
        remote_id: None,
    }
}

fn entry(
    id: &str,
    workspace_id: &str,
    connection_id: &str,
    command: &str,
    executed_at: &str,
    cwd: Option<&str>,
    exit_code: Option<i32>,
    redacted: bool,
) -> SshCommandHistoryEntry {
    SshCommandHistoryEntry {
        id: id.to_string(),
        workspace_id: workspace_id.to_string(),
        connection_id: connection_id.to_string(),
        session_id: Some("session-1".to_string()),
        command: command.to_string(),
        cwd: cwd.map(str::to_string),
        exit_code,
        duration_ms: Some(1200),
        redacted,
        executed_at: executed_at.to_string(),
    }
}

fn all_connections() -> Vec<SshConnection> {
    vec![
        connection(
            "connection-a",
            "workspace-1",
            "API host",
            "api.example.test",
            "deploy",
        ),
        connection(
            "connection-b",
            "workspace-2",
            "Other host",
            "other.example.test",
            "root",
        ),
    ]
}

fn all_entries() -> Vec<SshCommandHistoryEntry> {
    vec![
        entry(
            "hist-1",
            "workspace-1",
            "connection-a",
            "cd /srv/api",
            "2026-08-13T10:00:00Z",
            Some("/home/deploy"),
            Some(0),
            false,
        ),
        entry(
            "hist-2",
            "workspace-1",
            "connection-a",
            "git pull",
            "2026-08-13T10:00:05Z",
            Some("/srv/api"),
            Some(0),
            false,
        ),
        entry(
            "hist-3",
            "workspace-1",
            "connection-a",
            "docker compose build api",
            "2026-08-13T10:00:20Z",
            Some("/srv/api"),
            Some(0),
            false,
        ),
        entry(
            "hist-4",
            "workspace-1",
            "connection-a",
            "docker compose up -d api",
            "2026-08-13T10:01:00Z",
            Some("/srv/api"),
            Some(0),
            false,
        ),
        entry(
            "hist-5",
            "workspace-1",
            "connection-a",
            "docker compose logs --tail=100 api",
            "2026-08-13T10:01:10Z",
            Some("/srv/api"),
            Some(0),
            false,
        ),
        entry(
            "hist-secret",
            "workspace-1",
            "connection-a",
            "curl -H 'Authorization: Bearer leaked-token' https://example.test",
            "2026-08-13T09:59:00Z",
            None,
            Some(0),
            false,
        ),
        entry(
            "hist-redacted",
            "workspace-1",
            "connection-a",
            "<redacted>",
            "2026-08-13T09:58:00Z",
            None,
            None,
            true,
        ),
        entry(
            "hist-other-ws",
            "workspace-2",
            "connection-b",
            "rm -rf /srv/other",
            "2026-08-13T11:00:00Z",
            Some("/srv/other"),
            Some(0),
            false,
        ),
    ]
}

#[test]
fn list_history_is_registered_as_local_read() {
    let definition = HistoryStub::default()
        .registry()
        .definitions()
        .into_iter()
        .find(|definition| definition.name == "unfour.ssh.list_history")
        .expect("history tool should be registered");
    assert!(definition.annotations.read_only_hint);
    assert!(!definition.annotations.open_world_hint);
    assert!(!definition.input_schema["properties"]
        .as_object()
        .unwrap()
        .contains_key("includeRedacted"));
}

#[test]
fn list_history_returns_workspace_scoped_structured_commands() {
    let result = HistoryStub::default()
        .registry()
        .call("unfour.ssh.list_history", json!({}))
        .expect("history list should succeed");

    assert_eq!(result["isError"], false);
    let content = &result["structuredContent"];
    assert_eq!(content["workspaceId"], "workspace-1");
    assert_eq!(content["source"], "command-bus");
    assert_eq!(content["count"], 6);
    let history = content["history"].as_array().unwrap();
    assert_eq!(history[0]["command"], "docker compose logs --tail=100 api");
    assert_eq!(history[0]["connection"]["id"], "connection-a");
    assert_eq!(history[0]["connection"]["name"], "API host");
    assert_eq!(history[0]["connection"]["host"], "api.example.test");
    assert_eq!(history[0]["cwd"], "/srv/api");
    assert_eq!(history[0]["exitCode"], 0);
    assert_eq!(history[0]["durationMs"], 1200);
    assert!(history[0].get("sessionId").is_none());
    assert!(content.to_string().contains("cd /srv/api"));
    assert!(!content.to_string().contains("rm -rf /srv/other"));
    assert!(!content.to_string().contains("session-1"));
    assert!(!content.to_string().contains("credential_ref"));
    assert!(!content.to_string().contains("ssh-password:cred-1"));
}

#[test]
fn list_history_does_not_leak_other_workspace_when_workspace_id_is_set() {
    let result = HistoryStub::default()
        .registry()
        .call(
            "unfour.ssh.list_history",
            json!({ "workspaceId": "workspace-2" }),
        )
        .expect("history list should succeed");

    let commands = result["structuredContent"]["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["command"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(commands, vec!["rm -rf /srv/other"]);
    assert_eq!(result["structuredContent"]["workspaceId"], "workspace-2");
    assert!(!result.to_string().contains("git pull"));
}

#[test]
fn list_history_filters_by_connection_and_returns_empty_for_unknown_connection() {
    let registry = HistoryStub::default().registry();
    let filtered = registry
        .call(
            "unfour.ssh.list_history",
            json!({ "connectionId": "connection-a" }),
        )
        .expect("connection filter should succeed");
    assert!(filtered["structuredContent"]["history"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entry| entry["connection"]["id"] == "connection-a"));

    let missing = registry
        .call(
            "unfour.ssh.list_history",
            json!({ "connectionId": "missing-connection" }),
        )
        .expect("unknown connection should succeed");
    assert_eq!(missing["isError"], false);
    assert_eq!(missing["structuredContent"]["count"], 0);
    assert_eq!(missing["structuredContent"]["history"], json!([]));

    let other_workspace_connection = registry
        .call(
            "unfour.ssh.list_history",
            json!({ "connectionId": "connection-b" }),
        )
        .expect("out-of-workspace connection should succeed");
    assert_eq!(other_workspace_connection["structuredContent"]["count"], 0);
}

#[test]
fn list_history_applies_query_limit_and_time_range() {
    let registry = HistoryStub::default().registry();
    let limited = registry
        .call("unfour.ssh.list_history", json!({ "limit": 2 }))
        .expect("limit should succeed");
    assert_eq!(limited["structuredContent"]["count"], 2);
    assert_eq!(
        limited["structuredContent"]["history"][0]["command"],
        "docker compose logs --tail=100 api"
    );
    assert_eq!(
        limited["structuredContent"]["history"][1]["command"],
        "docker compose up -d api"
    );

    let searched = registry
        .call("unfour.ssh.list_history", json!({ "query": "git" }))
        .expect("query should succeed");
    assert_eq!(searched["structuredContent"]["count"], 1);
    assert_eq!(
        searched["structuredContent"]["history"][0]["command"],
        "git pull"
    );

    let ranged = registry
        .call(
            "unfour.ssh.list_history",
            json!({
                "since": "2026-08-13T10:00:20Z",
                "until": "2026-08-13T10:01:00Z"
            }),
        )
        .expect("time range should succeed");
    let commands = ranged["structuredContent"]["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["command"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        vec!["docker compose up -d api", "docker compose build api"]
    );
}

#[test]
fn list_history_redacts_sensitive_commands_and_omits_persisted_redacted_rows() {
    let result = HistoryStub::default()
        .registry()
        .call("unfour.ssh.list_history", json!({}))
        .expect("history list should succeed");
    let serialized = result.to_string();
    assert!(!serialized.contains("leaked-token"));
    assert!(!serialized.contains("Authorization: Bearer"));
    assert!(!serialized.contains("<redacted>"));

    let commands = result["structuredContent"]["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["command"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(commands.contains(&"[redacted command]"));
    assert!(!commands.contains(&"<redacted>"));
}

#[test]
fn list_history_rejects_include_redacted_and_invalid_time_range() {
    let registry = HistoryStub::default().registry();
    assert!(registry
        .call(
            "unfour.ssh.list_history",
            json!({ "includeRedacted": true }),
        )
        .is_err());
    assert!(registry
        .call(
            "unfour.ssh.list_history",
            json!({
                "since": "2026-08-13T12:00:00Z",
                "until": "2026-08-13T10:00:00Z"
            }),
        )
        .is_err());
    // Chronologically inverted even though the raw strings sort the other way.
    assert!(registry
        .call(
            "unfour.ssh.list_history",
            json!({
                "since": "2026-08-13T11:00:00Z",
                "until": "2026-08-13T18:00:00+08:00"
            }),
        )
        .is_err());
    assert!(registry
        .call("unfour.ssh.list_history", json!({ "since": "yesterday" }))
        .is_err());
}

#[test]
fn list_history_accepts_mixed_utc_offset_time_range() {
    // 20:00+08:00 is 12:00Z: lexicographically "20:00..." sorts after
    // "13:00...Z", but the range is chronologically valid and must pass.
    let result = HistoryStub::default()
        .registry()
        .call(
            "unfour.ssh.list_history",
            json!({
                "since": "2026-08-13T20:00:00+08:00",
                "until": "2026-08-13T13:00:00Z"
            }),
        )
        .expect("mixed-offset range should be accepted");
    assert_eq!(result["isError"], false);
}

#[test]
fn list_history_allows_prod_read_and_blocks_disabled_policy() {
    let prod = HistoryStub {
        environment_type: "prod",
        mcp_policy: "auto",
        active_workspace_id: "workspace-1",
    }
    .registry()
    .call("unfour.ssh.list_history", json!({}))
    .expect("prod read should succeed");
    assert_eq!(prod["isError"], false);
    assert_eq!(prod["structuredContent"]["risk_level"], "low");
    assert!(prod["structuredContent"]["count"].as_u64().unwrap() > 0);

    let disabled = HistoryStub {
        environment_type: "dev",
        mcp_policy: "disabled",
        active_workspace_id: "workspace-1",
    }
    .registry()
    .call("unfour.ssh.list_history", json!({}))
    .expect("disabled policy should be structured");
    assert_eq!(disabled["isError"], true);
    assert_eq!(
        disabled["structuredContent"]["error"]["code"],
        "WORKSPACE_POLICY_BLOCKED"
    );
}

#[test]
fn list_history_returns_empty_list_when_workspace_has_no_matching_rows() {
    let result = HistoryStub {
        environment_type: "dev",
        mcp_policy: "auto",
        active_workspace_id: "workspace-1",
    }
    .registry()
    .call(
        "unfour.ssh.list_history",
        json!({
            "query": "this-command-does-not-exist"
        }),
    )
    .expect("empty history should succeed");
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["count"], 0);
    assert_eq!(result["structuredContent"]["history"], json!([]));
}
