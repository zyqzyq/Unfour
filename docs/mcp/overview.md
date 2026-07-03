# MCP Overview

`unfour-mcp` is a local stdio Model Context Protocol server. It exposes a
small set of workspace, API Client, database, SSH diagnostic, activity, and
system-health tools to MCP clients.

## Architecture

```text
MCP client
  -> unfour-mcp stdio server
  -> MCP tool handler
  -> command-bus adapter
  -> unfour-command-bus
  -> workspace / API / database / SSH / activity services
  -> structured MCP result
```

The MCP layer is an adapter. It must not bypass command-bus safety,
workspace scoping, redaction, credential reference rules, SQL read-only policy,
or SSH diagnostic allowlists.

## Protocol Shape

The server implements newline-delimited JSON-RPC over standard input and
standard output. Standard output is reserved for MCP messages; process errors
are written to standard error.

Implemented method families:

- `initialize`
- `tools/list`
- `tools/call`

The `initialize` response includes instructions for a diagnostic flow:

1. Check `unfour.system.health`.
2. Review recent `unfour.activity.list`.
3. For API issues, inspect API history and saved request details.
4. For database issues, inspect saved database connections, schemas, and
   read-only query output.
5. For host/service issues, use the SSH diagnostic tool with allowlisted
   read-only commands.

## Safety Posture

Most MCP tools are read-only local diagnostics. Exceptions and open-world tools
are explicitly annotated in `tools/list`:

- `readOnlyHint`
- `destructiveHint`
- `idempotentHint`
- `openWorldHint`

No MCP tool performs destructive operations. `unfour.api.send_request` can send
a previously saved API request and record history. Database tools can reach
remote databases but only through saved connections and read-only operations.
SSH exposes only a single read-only diagnostic command tool with a strict
allowlist.

## Data Source

The standalone MCP process opens the same `unfour-paths` SQLite database used
by the desktop app: `dirs::data_dir()/Unfour/unfour.sqlite` (on Windows,
`%APPDATA%\Unfour\unfour.sqlite`). This path intentionally does not use
Tauri's `app_data_dir()`, because the Tauri identifier `dev.unfour` would
resolve to a different directory. The MCP process does not run schema
migrations or create fallback workspaces. Start the desktop app once before
starting the MCP server if the local database does not exist yet.

Credential values are resolved from the OS keychain under the same service name
as the desktop app. The MCP process reads credentials only when a tool needs to
open a saved database connection or run an SSH diagnostic command. It never
creates, rotates, or deletes credentials.

## Current Non-Goals

The v0.1 MCP surface does not:

- create, edit, or delete API requests;
- send arbitrary URLs;
- execute database writes;
- accept ad-hoc database connection strings;
- open interactive SSH sessions;
- run arbitrary SSH commands;
- perform SSH write/control operations;
- implement workflows;
- implement HTTP MCP transport;
- return raw secret values;
- attach to the running desktop process over IPC.

See also:

- `docs/mcp/tools.md`
- `docs/mcp/codex-setup.md`
- `docs/architecture/security-model.md`
