# MCP Overview

`unfour-mcp` is a local stdio Model Context Protocol server. It exposes
workspace, API Client, database, SSH, activity, and system-health tools to
Codex and Cursor.

## Architecture

```text
Codex or Cursor
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

The Community server constructs the writable adapter with empty Command Bus
extensions. Edition composition may instead use
`LocalCommandBusAdapter::default_storage_with_extensions` to install
`TransactionalCommandHook` implementations. MCP Environment create, update,
and delete operations, including the legacy API Environment compatibility
methods, then reach the same Workspace Domain Command coordinator and run each
hook inside the Command Bus-owned SQLite transaction. The adapter does not
define or install Pro hooks itself.

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
5. For host/service issues, inspect `unfour.ssh.list_history` for recent
   workspace-scoped commands, then start with SSH diagnostics, directory
   listings, file reads, or saved task inspection before running commands,
   writing files, or starting an SSH task. If asked to turn recent commands
   into a reusable task, draft steps from history and wait for user
   confirmation instead of saving or running a task automatically.

This diagnose-then-act sequence is Unfour's troubleshooting loop: Codex or
Cursor can use saved API, SSH, and database connections to reproduce an issue,
inspect logs and database state, and then act with the user's review. The
server does not automatically run a complete troubleshooting playbook; the
user and Codex or Cursor coordinate the steps.

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

High-risk actions return a `CONFIRMATION_REQUIRED` error result instead of
executing. The client must re-run the same tool call with `confirm=true` and
the returned `confirmation_text`. The confirmation text is bound to the exact
payload fingerprint, so changing the SQL, URL, command, path, or patch content
changes the required confirmation text.

## Result Contract

Successful `tools/call` results keep business data and Unfour call metadata in
separate layers:

```text
structuredContent  = the tool's business value; must match that tool's outputSchema
_meta              = Unfour call metadata (environment, riskLevel, durationMs, tool)
content[].text     = the same business JSON, for clients that only read text
isError            = false
```

Example success:

```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"connections\":[],\"count\":0,\"source\":\"command-bus\"}"
    }
  ],
  "structuredContent": {
    "connections": [],
    "count": 0,
    "source": "command-bus"
  },
  "_meta": {
    "tool": "unfour.db.list_connections",
    "environment": "dev",
    "riskLevel": "low",
    "durationMs": 3
  },
  "isError": false
}
```

`outputSchema` describes only the success business value. Do not put
`ok`, `tool`, `environment`, `risk_level`, `duration_ms`, `data`, `warnings`,
or `redactions` into `structuredContent` unless a specific tool's own schema
defines that field.

Error, policy-blocked, and confirmation-required calls keep `isError: true`
and put a machine-readable JSON payload in `content[].text`. They omit
`structuredContent` so they cannot fail the success `outputSchema`. Call
metadata still appears in `_meta`. Confirmation payloads include
`CONFIRMATION_REQUIRED`, `reason`, `confirmation_text`, and
`confirmation_hint`.

## Data Source

The standalone MCP process opens the same `unfour-paths` SQLite database used
by the desktop app. The stable default is `~/.unfour/unfour.sqlite` on all
platforms; `UNFOUR_STORAGE_PROFILE` / `UNFOUR_DATA_DIR` select the same
alternate roots as the desktop app (see
`docs/architecture/data-storage.md`). This path intentionally does not use
Tauri's `app_data_dir()`, because the Tauri identifier `dev.unfour` would
resolve to a different directory. The MCP process does not run schema
migrations or create fallback workspaces. Start the desktop app once before
starting the MCP server if the local database does not exist yet.

Credential values are resolved from the OS keychain under the same service name
as the desktop app. The MCP process reads credentials only when a tool needs to
open a saved database connection, send an API request, or use a saved SSH
connection. Connection creation tools may write supplied database passwords or
SSH secrets to the OS credential store through the command bus; raw credential
values are never returned.

### Ephemeral registry mode

For MCP registry validation, CI, protocol smoke tests, and isolated integration
tests only, set:

```text
UNFOUR_MCP_STORAGE_MODE=ephemeral
```

The mode uses `LocalCommandBusAdapter::ephemeral()` with in-memory SQLite and
the in-memory secret store. It does not open or create
`~/.unfour/unfour.sqlite`, read the OS credential store, or require a desktop
workspace. The real tool registry still handles `initialize`,
`notifications/initialized`, and `tools/list`; tool execution remains subject
to the normal command-bus policy and may still reach external services when a
tool is explicitly called.

Do not set this variable for normal Codex or Cursor usage. Codex and Cursor
should use the default mode so MCP can read the desktop's real,
workspace-scoped data.

## Current Non-Goals

The current Community MCP surface does not:

- accept ad-hoc database connection strings;
- open interactive SSH sessions;
- ship an automatic troubleshooting playbook or workflow runner;
- implement HTTP MCP transport;
- return raw secret values;
- attach to the running desktop process over IPC.

See also:

- `docs/mcp/tools.md`
- `docs/mcp/client-setup.md`
- `docs/mcp/codex-setup.md`
- `docs/architecture/security-model.md`
