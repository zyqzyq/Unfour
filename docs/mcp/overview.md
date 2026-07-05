# MCP Overview

`unfour-mcp` is a local stdio Model Context Protocol server. It exposes
workspace, API Client, database, SSH, activity, and system-health tools to MCP
clients.

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
workspace scoping, redaction, credential reference rules, MCP policy checks,
or high-risk confirmation checks.

## Protocol Shape

The server implements newline-delimited JSON-RPC over standard input and
standard output. Standard output is reserved for MCP messages; process errors
are written to standard error.

Implemented method families:

- `initialize`
- `tools/list`
- `tools/call`

The `initialize` response includes instructions for a diagnose-then-act flow:

1. Check `unfour.system.health`.
2. Review recent `unfour.activity.list`.
3. For API issues, inspect API history and saved request details.
4. For database issues, inspect saved database connections, schemas, and
   read-only query or explain output before executing a fix.
5. For host/service issues, start with SSH diagnostics, directory listings, and
   file reads before running commands or writing files.

## Safety Posture

Every MCP tool is evaluated against the target workspace's MCP policy before
execution. The default `auto` policy maps workspace environments as follows:

- `dev`: full access for ordinary development read/write actions.
- `test`: guarded access; write actions are allowed when not blocked by risk
  checks, and high-risk actions require confirmation.
- `prod`: read-only access, with safe SSH diagnostics allowed.

Explicit workspace policy can override the default environment mapping. Tools
also carry MCP behavior hints in `tools/list`:

- `readOnlyHint`
- `destructiveHint`
- `idempotentHint`
- `openWorldHint`

High-risk actions return a structured `CONFIRMATION_REQUIRED` result instead of
executing. The client must re-run the same tool call with `confirm=true` and
the returned `confirmation_text`. The confirmation text is bound to the exact
payload fingerprint, so changing the SQL, URL, command, path, or patch content
changes the required confirmation text.

All structured results use a common envelope:

```json
{
  "ok": true,
  "tool": "unfour.db.query_readonly",
  "environment": "dev",
  "risk_level": "read",
  "duration_ms": 12,
  "data": {},
  "warnings": [],
  "redactions": []
}
```

## Data Source

The standalone MCP process opens the same `unfour-paths` SQLite database used
by the desktop app: `~/.unfour/unfour.sqlite` on all platforms. This path
intentionally does not use Tauri's `app_data_dir()`, because the Tauri
identifier `dev.unfour` would resolve to a different directory. The MCP process
does not run schema migrations or create fallback workspaces. Start the desktop
app once before starting the MCP server if the local database does not exist
yet.

Credential values are resolved from the OS keychain under the same service name
as the desktop app. The MCP process reads credentials only when a tool needs to
open a saved database connection, send an API request, or use a saved SSH
connection. Connection creation tools may write supplied database passwords or
SSH secrets to the OS credential store through the command bus; raw credential
values are never returned.

## Current Non-Goals

The v0.1 MCP surface does not:

- accept ad-hoc database connection strings;
- open interactive SSH sessions;
- implement workflows;
- implement HTTP MCP transport;
- return raw secret values;
- attach to the running desktop process over IPC.

See also:

- `docs/mcp/tools.md`
- `docs/mcp/codex-setup.md`
- `docs/architecture/security-model.md`
