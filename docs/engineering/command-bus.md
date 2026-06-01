# Command Bus

The Command Bus is the stable backend contract for manual UI actions and future automated actions.

## Why

If domain logic lives directly in `#[tauri::command]`, future AI/MCP/CLI/cloud runners would need to duplicate behavior. The bus keeps one execution path:

```text
Tauri IPC Adapter
MCP Adapter
Local CLI Adapter
AI Agent Adapter
Cloud Workflow Adapter
        -> CommandBus -> Service -> Driver
```

## Current Commands

- `workspace_list`
- `workspace_create`
- `workspace_rename`
- `workspace_delete`
- `workspace_set_active`
- `workspace_environment_get`
- `workspace_environment_update`
- `api_send_request`
- `api_history_list`
- `api_request_save`
- `api_saved_requests`
- `database_connections_list`
- `database_connection_save`
- `database_connection_delete`
- `database_connection_test`
- `database_schema_get`
- `database_query_execute`
- `system_health`

## Rules

- Commands return structured `AppError` values.
- Dangerous future commands must define capability metadata and confirmation policy.
- Command input/output types must be serializable and avoid raw secrets.
- API requests resolve workspace variables in Rust before execution.
- Long-running streaming work should use events/channels rather than blocking commands.

## Reserved AI Contract

`ai_reserved::AppCommand` names the future high-level operation families:

- Workspace
- API
- SSH
- Database

The first AI implementation should call the bus, not DOM-click the UI.
