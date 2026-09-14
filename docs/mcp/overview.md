# MCP Overview

`unfour-mcp` is a local stdio Model Context Protocol server. It exposes
workspace, API Client, database, SSH, activity, and system-health tools to
Codex and Cursor.

## Coding Tools and Runtime Tools

The coding client owns:

- repository inspection;
- code edits;
- code-level tests;
- git and source-control actions.

Unfour MCP provides:

- workspace and environment context;
- API requests and runtime responses;
- SSH/server evidence;
- database state;
- policy-controlled runtime actions;
- runtime re-checks and verification through existing tools.

The coding agent edits code with its own tools. Unfour MCP complements those
tools with controlled runtime access; it does not manage or edit the repository.
The user controls the workspace, environment, risky actions, and final decision.

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

The single server applies the unified core + Cloud Sync migration chain and
installs `SyncOutboxHook` on its writable Command Bus. MCP mutations therefore
reach the same Workspace Domain Command coordinator as desktop and enqueue
outbox rows inside the Command Bus-owned SQLite transaction when the workspace
has Cloud Sync enabled. Local-only workspaces remain local-only.

## Protocol Shape

The server implements newline-delimited JSON-RPC over standard input and
standard output. Standard output is reserved for MCP messages; process errors
are written to standard error.

Implemented method families:

- `initialize`
- `tools/list`
- `tools/call`
- `ping`
- `notifications/cancelled`

### Cancellation and Long Calls

The stdio reader remains responsive while a tool executes. One business call
runs at a time, with at most 16 queued calls; excess calls receive
`MCP_SERVER_BUSY`. `ping`, `tools/list` and cancellation do not wait for that
execution slot. Duplicate in-flight IDs are rejected. Tool notifications without
request IDs do not execute business actions.

Clients may send `notifications/cancelled` with `params.requestId`, following
the [MCP cancellation schema](https://modelcontextprotocol.io/specification/2025-06-18/schema#cancellednotification).
Only the matching in-flight or queued call is cancelled. Unknown, completed,
or initialize IDs have no cancellation effect. The server suppresses the
cancelled call's response and does not respond to the notification itself.

API, database and one-shot SSH execution futures have an independent 120-second
safety deadline. API `timeoutMs: 0` disables the HTTP timeout; positive values
are passed through to the HTTP layer. Neither disables the MCP deadline. The stdio watchdog also bounds the time waiting for a
tool response. On expiration it reports `MCP_CALL_TIMEOUT` and signals cancellation.
If the adapter's execution deadline wins the race, its normal structured tool
error is returned instead. A worker retains the single execution slot until it
actually finishes; cancelled or timed-out workers are never replaced with an
unbounded number of detached workers.

Saved API replay cancellation uses the Command Bus execution ID and existing
API cancellation token, allowing up to two seconds for cleanup. Other API,
database and one-shot SSH calls drop the awaited execution future, propagating
cancellation to async I/O where supported. This does not guarantee server-side
SQL cancellation or termination of an already-started remote SSH process.
Completed writes and script changes already committed are not rolled back.
Local metadata transactions already executing are allowed to finish; cancellation
is checked before invoking the handler. SSH task runs remain asynchronous and
must be stopped with `unfour.ssh.cancel_task_run` after they have started.
Inspect state before retrying a cancelled mutation.

On stdin EOF, queued/completing calls have a two-second drain window, then the
active call is cancelled and the runtime shuts down with its existing bounded
cleanup. Broken stdout also cancels the active call. Idle timeout only applies
when no tool is running. An uncooperative worker can keep business calls busy,
but cannot block ping, tool discovery or process shutdown.

This adds no HTTP transport, interactive SSH, Flow runner, automatic
troubleshooting workflow, policy-editing tools, raw credential/known_hosts
access, or lower-priority CRUD/import/export/batch capabilities.

The `initialize` response includes instructions for a diagnose → act → verify flow:

1. Check `unfour.system.health`.
2. Review recent `unfour.activity.list`.
3. For API issues, inspect workspace context, API history, and saved request
   details to reproduce or inspect the visible API symptom.
4. For database issues, inspect saved database connections, schemas, and
   read-only query or explain output before executing a fix.
5. For host/service issues, inspect `unfour.ssh.list_history` for recent
   workspace-scoped commands, then start with SSH diagnostics, directory
   listings, file reads, or saved task inspection before running commands,
   writing files, or starting an SSH task. If asked to turn recent commands
   into a reusable task, draft steps from history and wait for user
   confirmation instead of saving or running a task automatically.
6. Summarize the evidence and propose the next action for the user's review.
7. If the coding client changes code using its own coding tools, re-run the
   original runtime check against the updated backend in the user-selected
   workspace and environment, subject to the same policy and confirmation checks.
8. Verify that the original symptom is gone and relevant runtime and database
   state is consistent; report any checks that could not be completed.

Code changes in step 7 belong to the coding client, not Unfour MCP. Re-checking
requires the updated code to be running in the target environment; a local code
edit alone does not establish that a runtime issue is fixed.

This diagnose → act → verify sequence is Unfour's troubleshooting loop: Codex or
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

The tool registry is fail-closed: every registered tool must have an explicit
capability and risk classification, registry completeness is tested, and
unknown/unclassified tools are denied. API request, collection, environment,
environment-variable deletes, and database/SSH connection deletes are classified
as destructive. Connection deletion confirmations include workspace, connection
ID and record revision. Metadata updates preserve omitted fields and credentials;
their results expose only IDs. Database history and stored SSH fingerprints are
read-only projections, and SSH connection tests use the existing Command Bus path.

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

For `unfour.api.send_request`, `environmentId` is a per-call override. The
selected environment is fixed for request-variable resolution and the entire
pre-request/post-response script lifecycle without changing Desktop's active
environment. If no override is provided, the current active environment is
used. Saved request replay executes its saved scripts through the shared
command-bus script execution path; scripted replays are treated as writes for
policy purposes because scripts may persist environment mutations.

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
resolve to a different directory. The MCP process runs the idempotent unified
migration chain but does not create a missing normal-storage database or seed
fallback workspaces. Start the desktop app once before starting the MCP server
if the local database does not exist yet.

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

The current Unfour MCP surface does not:

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
