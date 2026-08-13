# MCP Tools

This is the active tool reference for the local `unfour-mcp` server.

## Tool Safety Annotations

Every tool in `tools/list` carries MCP behavior hints:

- `readOnlyHint`: true when the tool does not mutate local app state.
- `destructiveHint`: true for tools that can delete, overwrite, or otherwise
  make destructive changes.
- `idempotentHint`: true for repeatable reads.
- `openWorldHint`: true when the tool reaches a remote service or performs an
  external action.

Tools with `openWorldHint: true`:

- `unfour.api.send_request`
- `unfour.db.list_tables`
- `unfour.db.describe_table`
- `unfour.db.query_readonly`
- `unfour.db.execute`
- `unfour.db.explain`
- `unfour.db.test_connection`
- `unfour.ssh.run_diagnostic`
- `unfour.ssh.exec`
- `unfour.ssh.read_file`
- `unfour.ssh.write_file`
- `unfour.ssh.patch_file`
- `unfour.ssh.list_dir`
- `unfour.ssh.run_task`
- `unfour.ssh.cancel_task_run`

Write-capable tools are also checked by workspace policy at call time. The
default `auto` mapping is dev = full access, test = guarded, prod = read-only.
High-risk calls return `CONFIRMATION_REQUIRED` with a content-bound
`confirmation_text`; re-run the same call with `confirm=true` and that exact
text to execute.

## Tool Reference

| Tool | Input | Behavior |
| --- | --- | --- |
| `unfour.system.health` | `{}` | Returns command-bus and storage readiness. |
| `unfour.workspace.current` | `{}` | Returns the active workspace. |
| `unfour.workspace.list` | `{}` | Lists local workspaces and marks the active one. |
| `unfour.workspace.list_variables` | `{ "workspaceId": "optional" }` | Lists workspace-global variables. Explicit secrets and sensitive keys are masked. |
| `unfour.workspace.create_variable` | `{ "workspaceId": "optional", "key": "required", "value": "required", "isSecret": "optional", "isEnabled": "optional", "description": "optional", "sortOrder": "optional" }` | Creates one workspace-global variable. Returned sensitive values are masked. |
| `unfour.workspace.update_variable` | `{ "workspaceId": "optional", "variableId": "required", "key": "required", "value": "required", "isSecret": "optional", "isEnabled": "optional", "description": "optional", "sortOrder": "optional" }` | Updates one workspace-global variable. Omitted optional fields keep their current values. |
| `unfour.workspace.replace_variables` | `{ "workspaceId": "optional", "variables": "required array" }` | Replaces the exact workspace-global variable set; omitted records are soft-deleted. |
| `unfour.workspace.delete_variable` | `{ "workspaceId": "optional", "variableId": "required", "confirm": "optional", "confirmation_text": "optional" }` | Soft-deletes one workspace-global variable; guarded policy requires confirmation. |
| `unfour.connection.list` | `{ "type": "optional" }` | Returns safe database and SSH connection summaries. `type` may be `all`, `api`, `database`, or `ssh`; default is `all`. |
| `unfour.activity.list` | `{ "workspaceId": "optional", "limit": "optional" }` | Lists recent redacted local activity events. Default limit is 50; max is 200. |
| `unfour.api.list_collections` | `{ "workspaceId": "optional" }` | Lists API request collections derived from saved request folders. |
| `unfour.api.list_requests` | `{ "workspaceId": "optional", "collectionId": "optional" }` | Lists saved API requests with sensitive URL parameters redacted. |
| `unfour.api.get_request` | `{ "requestId": "required", "includeBody": "optional bool" }` | Returns a saved API request with sensitive headers, query params, URL params, and body fields masked. |
| `unfour.api.send_request` | `{ "requestId": "optional", "method": "optional", "url": "optional", "headers": "optional", "query": "optional", "body": "optional", "workspaceId": "optional", "environmentId": "optional", "timeoutMs": "optional" }` | Sends a saved request or an ad-hoc request and returns a masked response summary. Non-read methods are blocked by prod policy. |
| `unfour.api.create_request` | `{ "workspaceId": "optional", "collectionId": "optional", "parentId": "optional", "parentFolderId": "optional", "name": "required", "method": "required", "url": "required", "headers": "optional", "query": "optional", "body": "optional", "bodyKind": "optional", "auth": "optional", "authJson": "optional" }` | Creates a saved API request in an allowed workspace. `parentFolderId` is the current folder field; `parentId` is accepted as a compatibility alias. |
| `unfour.api.update_request` | `{ "requestId": "required", "workspaceId": "optional", "collectionId": "optional", "parentId": "optional", "parentFolderId": "optional", "name": "optional", "method": "optional", "url": "optional", "headers": "optional", "query": "optional", "body": "optional", "bodyKind": "optional", "auth": "optional", "authJson": "optional" }` | Updates a saved API request. Omitted fields keep their current values. `parentFolderId` is the current folder field; `parentId` is accepted as a compatibility alias. |
| `unfour.api.delete_request` | `{ "requestId": "required", "workspaceId": "optional", "confirm": "optional", "confirmation_text": "optional" }` | Soft-deletes a saved API request after confirmation. |
| `unfour.api.create_collection` | `{ "workspaceId": "optional", "name": "required" }` | Creates an API collection/folder. |
| `unfour.api.update_collection` | `{ "collectionId": "required", "workspaceId": "optional", "name": "required" }` | Renames an API collection/folder. |
| `unfour.api.delete_collection` | `{ "collectionId": "required", "workspaceId": "optional", "confirm": "optional", "confirmation_text": "optional" }` | Deletes an API collection/folder after confirmation. |
| `unfour.api.list_history` | `{ "workspaceId": "optional", "limit": "optional" }` | Lists recent API request/response history. Default limit is 50; max is 200. |
| `unfour.api.get_history` | `{ "historyId": "required", "workspaceId": "optional" }` | Returns one history entry with request/response details masked. |
| `unfour.api.list_environments` | `{ "workspaceId": "optional" }` | Lists API environments and variables. Sensitive values are masked. |
| `unfour.api.create_environment` | `{ "workspaceId": "optional", "name": "required" }` | Creates an empty API environment in an allowed workspace. |
| `unfour.api.update_environment` | `{ "environmentId": "required", "workspaceId": "optional", "name": "required", "variables": "required array" }` | Updates an API environment name and variables. Sensitive values are masked in the result. |
| `unfour.api.delete_environment` | `{ "environmentId": "required", "workspaceId": "optional", "confirm": "optional", "confirmation_text": "optional" }` | Soft-deletes an API environment; guarded policy requires confirmation. |
| `unfour.db.create_connection` | `{ "workspaceId": "optional", "name": "required", "driver": "required", "host": "optional", "port": "optional", "database": "optional", "username": "optional", "sslMode": "optional", "sqlitePath": "optional", "credentialRef": "optional", "password": "optional", "credentialLabel": "optional", "readOnly": "optional" }` | Creates a saved database connection. If `password` is supplied, it is written to the OS credential store and only the resulting credential reference is persisted. |
| `unfour.db.list_connections` | `{ "workspaceId": "optional" }` | Lists saved database connections as safe summaries. |
| `unfour.db.list_tables` | `{ "connectionId": "required", "workspaceId": "optional", "limit": "optional" }` | Lists tables and views for a saved connection. Default limit is 200; max is 500. |
| `unfour.db.describe_table` | `{ "connectionId": "required", "tableName": "required", "schema": "optional", "workspaceId": "optional" }` | Describes a table's columns without reading table data. |
| `unfour.db.query_readonly` | `{ "connectionId": "required", "sql": "required", "limit": "optional", "workspaceId": "optional" }` | Executes one read-only SQL statement. Default limit is 100; max is 1000. |
| `unfour.db.execute` | `{ "connectionId": "required", "sql": "required", "workspaceId": "optional", "limit": "optional", "catalog": "optional", "schema": "optional", "timeoutMs": "optional", "dryRun": "optional", "transaction": "optional", "confirm": "optional", "confirmationText": "optional", "confirmation_text": "optional" }` | Executes one SQL statement when policy allows. High-risk writes such as `DELETE` without `WHERE` require confirmation. |
| `unfour.db.explain` | `{ "connectionId": "required", "sql": "required", "workspaceId": "optional", "limit": "optional", "catalog": "optional", "schema": "optional", "timeoutMs": "optional" }` | Runs `EXPLAIN` for a read-only statement or an existing explain query. |
| `unfour.db.test_connection` | `{ "connectionId": "required", "workspaceId": "optional" }` | Tests connectivity for a saved database connection and returns server metadata when available. |
| `unfour.ssh.create_connection` | `{ "workspaceId": "optional", "name": "required", "host": "required", "port": "optional", "username": "required", "authKind": "required", "keyPath": "optional", "credentialRef": "optional", "secret": "optional" }` | Creates a saved SSH connection. If `secret` is supplied for password or private-key auth, it is written to the OS credential store and only the resulting credential reference is persisted. |
| `unfour.ssh.list_connections` | `{ "workspaceId": "optional" }` | Lists saved SSH connections as safe summaries. |
| `unfour.ssh.list_history` | `{ "workspaceId": "optional", "connectionId": "optional", "query": "optional", "since": "optional RFC 3339", "until": "optional RFC 3339", "limit": "optional" }` | Lists structured SSH command history for the selected workspace. Default limit is 50; max is 200. Sensitive commands are excluded or replaced with `[redacted command]`. Terminal buffers and session logs are never returned. |
| `unfour.ssh.run_diagnostic` | `{ "connectionId": "required", "command": "required", "workspaceId": "optional", "timeoutMs": "optional" }` | Runs a single allowlisted read-only diagnostic command on a saved SSH connection. Requires an `ssh-native` build. |
| `unfour.ssh.exec` | `{ "connectionId": "required", "command": "required", "workspaceId": "optional", "cwd": "optional", "env": "optional", "timeoutMs": "optional", "confirm": "optional", "confirmation_text": "optional" }` | Executes one non-interactive SSH command when policy allows. High-risk commands require confirmation. |
| `unfour.ssh.read_file` | `{ "connectionId": "required", "path": "required", "workspaceId": "optional", "offset": "optional", "limit": "optional", "tailLines": "optional", "timeoutMs": "optional" }` | Reads a capped remote file slice or tail. |
| `unfour.ssh.write_file` | `{ "connectionId": "required", "path": "required", "content": "required", "workspaceId": "optional", "mode": "optional", "timeoutMs": "optional", "confirm": "optional", "confirmation_text": "optional" }` | Writes or appends a remote file when policy allows. Test workspaces and sensitive paths require confirmation. |
| `unfour.ssh.patch_file` | `{ "connectionId": "required", "path": "required", "search": "required", "replace": "required", "workspaceId": "optional", "timeoutMs": "optional", "confirm": "optional", "confirmation_text": "optional" }` | Applies a search/replace patch. Multiple matches, test workspaces, and sensitive paths require confirmation. |
| `unfour.ssh.list_dir` | `{ "connectionId": "required", "path": "required", "workspaceId": "optional", "limit": "optional", "timeoutMs": "optional" }` | Lists one remote directory with size and modified-time metadata. |
| `unfour.ssh.list_tasks` | `{ "workspaceId": "optional" }` | Lists saved SSH task summaries. |
| `unfour.ssh.get_task` | `{ "workspaceId": "optional", "taskId": "required" }` | Returns one task, its steps, and local binding with sensitive configuration fields masked. |
| `unfour.ssh.save_task` | `{ "workspaceId": "optional", "taskId": "optional", "name": "required", "description": "optional", "defaultConnectionId": "optional", "steps": "required array" }` | Creates or updates a task. On update, omitted `description` and `defaultConnectionId` keep their current values; pass `null` for `defaultConnectionId` to clear it. Supported step types are `command`, `upload`, and `download`. |
| `unfour.ssh.duplicate_task` | `{ "workspaceId": "optional", "taskId": "required" }` | Duplicates a saved task and its steps. |
| `unfour.ssh.reorder_tasks` | `{ "workspaceId": "optional", "taskIds": "required array" }` | Reorders the exact active task set. |
| `unfour.ssh.delete_task` | `{ "workspaceId": "optional", "taskId": "required", "confirm": "optional", "confirmation_text": "optional" }` | Soft-deletes a task; guarded policy requires confirmation. |
| `unfour.ssh.run_task` | `{ "workspaceId": "optional", "taskId": "required", "connectionId": "optional", "inputs": "optional object", "secretInputNames": "optional array", "confirm": "optional", "confirmation_text": "optional" }` | Starts a task run. Guarded policy requires confirmation; production/read-only policy blocks execution. |
| `unfour.ssh.cancel_task_run` | `{ "workspaceId": "optional", "runId": "required" }` | Cancels an active task run when execution policy allows. |
| `unfour.ssh.list_task_runs` | `{ "workspaceId": "optional", "taskId": "required" }` | Lists task run summaries without local log paths. |
| `unfour.ssh.read_task_run_log` | `{ "workspaceId": "optional", "runId": "required" }` | Reads a redacted task log capped at 128 KiB characters. |
| `unfour.ssh.clear_task_runs` | `{ "workspaceId": "optional", "taskId": "optional", "confirm": "optional", "confirmation_text": "optional" }` | Deletes task run records and local logs for one task or the workspace; guarded policy requires confirmation. |

## Workspace Variable Tools

Workspace-global variables are distinct from variables attached to an API
environment. All reads and mutation results mask values when `isSecret=true`
or the key matches the shared sensitive-key policy. The replace operation uses
the same exact-set semantics as the desktop command: variables omitted from the
submitted array are soft-deleted. For `update_variable`, and for replace items
that include an existing `id`, omitted optional fields such as `isSecret`
keep their current values instead of resetting to create defaults.

## API Client Tools

API tools inspect saved requests, history, collections, and environments through
the command bus. Write-capable API tools create, update, or delete saved
request metadata only when the workspace policy allows it.

Important limits:

- `unfour.api.send_request` can send either a saved `requestId` or an ad-hoc
  `method` plus `url`.
- Saved and ad-hoc requests both use the command bus so history, masking, and
  credential handling stay in one path.
- `timeoutMs` is clamped to a maximum of 60,000 ms.
- Environment variables are resolved from the workspace environment.
- Delete operations require the confirmation handshake.
- Sensitive request and response fields are masked before returning to the MCP
  client.

Example `send_request` result:

```json
{
  "ok": true,
  "status": 200,
  "statusText": "OK",
  "durationMs": 123,
  "headers": [
    { "name": "Content-Type", "value": "application/json" }
  ],
  "bodyPreview": "{\"ok\":true}",
  "bodyType": "json",
  "sizeBytes": 1024,
  "truncated": false,
  "source": "command-bus"
}
```

Example confirmation-required result:

```json
{
  "ok": false,
  "tool": "unfour.api.delete_request",
  "environment": "dev",
  "risk_level": "high",
  "requires_confirmation": true,
  "confirmation_text": "API_DELETE_REQUEST:1a2b3c4d"
}
```

## Database Tools

Database tools operate on saved connection records. `unfour.db.create_connection`
creates those records through the command bus; it accepts structured connection
fields rather than ad-hoc connection strings, and it never returns passwords or
credential references. When `password` is provided, the command bus stores it in
the OS credential store and persists only a credential reference.

Most database tools require saved `connectionId` values. Ad-hoc connection
strings are not accepted.

`unfour.db.query_readonly` and `unfour.db.explain` allow only read-only SQL
keywords:

- `SELECT`
- `WITH`
- `SHOW`
- `DESCRIBE`
- `DESC`
- `EXPLAIN`

`unfour.db.execute` accepts one SQL statement. In dev it can run ordinary
mutations. In test it is guarded by policy and confirmation for high-risk
statements. In prod it is blocked by read-only policy. These statements require
confirmation before execution:

- `DELETE` without `WHERE`
- `UPDATE` without `WHERE`
- `DROP`, `TRUNCATE`, or `ALTER`
- statements whose risk cannot be classified confidently

Set `dryRun=true` to return the detected risk and execution plan without
running the statement. Query results exceeding the response cap are truncated
and marked with `"truncated": true`.

Use `catalog` and `schema` to pass database context where a driver supports it,
and `timeoutMs` to cap execution time. Confirmation can be supplied as either
`confirmationText` or `confirmation_text`; tool responses return
`confirmation_text`.

Example `query_readonly` result:

```json
{
  "ok": true,
  "connectionId": "conn-1",
  "columns": [
    { "name": "id", "dataType": "integer" },
    { "name": "email", "dataType": "varchar" }
  ],
  "rows": [
    [1, "user@example.com"]
  ],
  "rowCount": 1,
  "durationMs": 42,
  "truncated": false,
  "source": "command-bus"
}
```

## SSH Tools

SSH tools use saved SSH connections and the command bus. They are
non-interactive and require an `ssh-native` build for real remote execution.

`unfour.ssh.create_connection` creates saved SSH connection metadata. It accepts
`authKind` values `password`, `private-key`, or `none`; password auth requires
either `secret` or an existing `credentialRef`, and private-key auth requires
`keyPath`. Returned summaries never include secrets, private-key paths, or
credential references.

`unfour.ssh.list_history` reads structured command history already recorded for
the selected workspace. It defaults to the active MCP workspace, always scopes
the query to that workspace, and never returns history from another workspace.
Optional filters are `connectionId`, `query`, `since`, `until`, and `limit`.
The result is a list of commands with connection summary, `executedAt`, and
`cwd` / `exitCode` / `durationMs` when those fields were recorded. It does not
return terminal buffers or full session logs. Persisted redacted rows are
omitted; any remaining command that still matches credential markers is
replaced with `[redacted command]`.

Use history to inspect what the user recently ran, then draft a reusable SSH
Task for confirmation. Do not call `unfour.ssh.save_task` or
`unfour.ssh.run_task` unless the user explicitly asks to save or run the draft.
`unfour.ssh.save_task` already provides a safe write path when that confirmation
happens; this release does not add a separate auto-create tool.

`unfour.ssh.run_diagnostic` is gated by a strict allowlist. The leading word
must be a bare allowlisted utility, such as:

```text
df du free uptime uname hostname whoami id date ps ss netstat ip ifconfig
vmstat iostat mount stat wc ls cat tail head grep systemctl journalctl
docker podman kubectl
```

Shell metacharacters are rejected, so chaining, piping, redirection, wildcard
expansion, command substitution, and subshells are not available.

Additional restrictions:

- `systemctl` is limited to read-only subcommands such as `status`,
  `is-active`, `is-enabled`, `show`, `cat`, `list-units`, and similar reads.
- `journalctl` log-management flags are rejected.
- `docker` and `podman` are limited to read-only subcommands such as `ps`,
  `logs`, `inspect`, `images`, `version`, `info`, `stats`, `top`, `port`, and
  `diff`.
- `kubectl` is limited to read-only subcommands such as `get`, `describe`,
  `logs`, `top`, `version`, `api-resources`, `explain`, and `cluster-info`.

Captured output is line-redacted by the SSH engine. Streams are capped and the
command runs under a timeout.

`unfour.ssh.exec` allows broader one-shot commands in dev and guarded test
workspaces. Prod permits only read-only diagnostic commands. These command
families require confirmation before execution:

- `rm`, `rm -rf`, and other delete commands.
- `kill`, `reboot`, `shutdown`, and related process/power controls.
- service stop/restart commands.
- `docker rm`, `docker compose down`, `podman rm`, and `kubectl delete`.
- `curl ... | sh` or `curl ... | bash`.
- obvious writes to system paths such as `/etc`, `/usr`, or `C:\Windows`.

`unfour.ssh.read_file` returns a capped file slice. `unfour.ssh.write_file`
returns only path, mode, byte count, and command status. `unfour.ssh.patch_file`
performs search/replace without returning full file content; if the search text
matches multiple locations, it returns a confirmation request before replacing
all matches.

SSH task metadata operations use the same task service and command-bus methods
as the desktop UI. Task execution is asynchronous and returns a run summary.
Because a saved task can contain multiple remote commands and transfers,
`unfour.ssh.run_task` requires the normal confirmation handshake under guarded
policy. Run summaries omit local log paths, and log reads are capped.

## Sensitive Data Masking

MCP results are consumed by an LLM client, so sensitive values must not leave
the local machine in usable form.

Sensitive fields include:

```text
password passwd pwd token access_token refresh_token api_key apikey secret
client_secret authorization cookie set-cookie proxy-authorization x-api-key
x-auth-token private_key connection_string database_url credential_ref
```

Sensitive values are replaced with partial-mask descriptors that expose useful
diagnostic shape while hiding the secret. Examples:

```text
[mask kind=jwt scheme=Bearer len=872 fp=a1b2c3]
[mask kind=prefixed:sk len=51 fp=9f8e7d]
```

Masking applies to HTTP headers, URL query parameters, JSON request/response
body fields, activity details, connection summaries, SSH task configuration,
and SSH command history. SSH history additionally omits persisted redacted
rows and replaces any remaining credential-marker commands with
`[redacted command]`.
