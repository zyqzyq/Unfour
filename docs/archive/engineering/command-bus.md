# Command Bus

The Command Bus is the stable backend contract for manual UI actions and future automated actions.

The reusable Rust entry point is `crates/unfour-command-bus`. Tauri commands
and the stdio MCP server are adapters over that crate.

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

## Current Command Families

The exact Tauri command names live in the Tauri adapter and
`@unfour/command-client`. The current command bus covers these families:

- Workspace list/create/rename/delete/set-active operations.
- Workspace environment and layout reads/writes.
- API send, history, saved-request, duplicate, delete, and collection reads.
- Credential create/inspect/rotate/delete operations through credential refs.
- Database connection save/delete/test, schema, query, and table-browse paths.
- SSH connection save/delete, session connect/list/history/input/resize/close,
  reconnect cancellation, log export, host-key, and known_hosts paths.
- System health and safe read commands for MCP/future AI adapters.

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

AI adapters should also use the same safety policy as human-triggered commands:

- Routine local reads may run without a confirmation dialog.
- Writes, destructive operations, sensitive exports, and data sent to third-party AI services require confirmation.
- Confirmed AI-triggered writes and external side effects must write redacted local activity records.
- Prompts, responses, secrets, request/response bodies, and query result rows must not be stored in activity details.
