# MCP Tools

This is the active tool reference for the local `unfour-mcp` server.

## Tool Safety Annotations

Every tool in `tools/list` carries MCP behavior hints:

- `readOnlyHint`: true when the tool does not mutate local app state.
- `destructiveHint`: always false for current tools.
- `idempotentHint`: true for repeatable reads.
- `openWorldHint`: true when the tool reaches a remote service or performs an
  external action.

Tools with `openWorldHint: true`:

- `unfour.api.send_request`
- `unfour.db.list_tables`
- `unfour.db.describe_table`
- `unfour.db.query_readonly`
- `unfour.db.test_connection`
- `unfour.ssh.run_diagnostic`

`unfour.api.send_request` is the only current tool with `readOnlyHint: false`
because it sends a saved request and records history.

## Tool Reference

| Tool | Input | Behavior |
| --- | --- | --- |
| `unfour.system.health` | `{}` | Returns command-bus and storage readiness. |
| `unfour.workspace.current` | `{}` | Returns the active workspace. |
| `unfour.workspace.list` | `{}` | Lists local workspaces and marks the active one. |
| `unfour.connection.list` | `{ "type": "optional" }` | Returns safe database and SSH connection summaries. `type` may be `all`, `api`, `database`, or `ssh`; default is `all`. |
| `unfour.activity.list` | `{ "workspaceId": "optional", "limit": "optional" }` | Lists recent redacted local activity events. Default limit is 50; max is 200. |
| `unfour.api.list_collections` | `{ "workspaceId": "optional" }` | Lists API request collections derived from saved request folders. |
| `unfour.api.list_requests` | `{ "workspaceId": "optional", "collectionId": "optional" }` | Lists saved API requests with sensitive URL parameters redacted. |
| `unfour.api.get_request` | `{ "requestId": "required", "includeBody": "optional bool" }` | Returns a saved API request with sensitive headers, query params, URL params, and body fields masked. |
| `unfour.api.send_request` | `{ "requestId": "required", "environmentId": "optional", "timeoutMs": "optional" }` | Sends a previously saved request and returns a masked response summary. Does not accept arbitrary URLs. |
| `unfour.api.list_history` | `{ "workspaceId": "optional", "limit": "optional" }` | Lists recent API request/response history. Default limit is 50; max is 200. |
| `unfour.api.get_history` | `{ "historyId": "required", "workspaceId": "optional" }` | Returns one history entry with request/response details masked. |
| `unfour.api.list_environments` | `{ "workspaceId": "optional" }` | Lists API environments and variables. Sensitive values are masked. |
| `unfour.db.list_connections` | `{ "workspaceId": "optional" }` | Lists saved database connections as safe summaries. |
| `unfour.db.list_tables` | `{ "connectionId": "required", "workspaceId": "optional", "limit": "optional" }` | Lists tables and views for a saved connection. Default limit is 200; max is 500. |
| `unfour.db.describe_table` | `{ "connectionId": "required", "tableName": "required", "schema": "optional", "workspaceId": "optional" }` | Describes a table's columns without reading table data. |
| `unfour.db.query_readonly` | `{ "connectionId": "required", "sql": "required", "limit": "optional", "workspaceId": "optional" }` | Executes one read-only SQL statement. Default limit is 100; max is 1000. |
| `unfour.db.test_connection` | `{ "connectionId": "required", "workspaceId": "optional" }` | Tests connectivity for a saved database connection and returns server metadata when available. |
| `unfour.ssh.run_diagnostic` | `{ "connectionId": "required", "command": "required", "workspaceId": "optional", "timeoutMs": "optional" }` | Runs a single allowlisted read-only diagnostic command on a saved SSH connection. Requires an `ssh-native` build. |

## API Client Tools

API tools inspect saved requests, history, collections, and environments through
the command bus.

Important limits:

- `unfour.api.send_request` only sends saved requests by `requestId`.
- It does not accept arbitrary URLs.
- `timeoutMs` is clamped to a maximum of 60,000 ms.
- Environment variables are resolved from the workspace environment.
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

## Database Tools

Database tools require saved `connectionId` values. Ad-hoc connection strings
are not accepted.

`unfour.db.query_readonly` allows only read-only SQL keywords:

- `SELECT`
- `WITH`
- `SHOW`
- `DESCRIBE`
- `DESC`
- `EXPLAIN`

The MCP layer rejects multi-statement SQL and forbidden write/control keywords
before the query reaches the command bus. Query results exceeding the response
cap are truncated and marked with `"truncated": true`.

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

## SSH Diagnostic Tool

`unfour.ssh.run_diagnostic` is the only SSH capability exposed over MCP. It is
not an interactive terminal and does not expose write/control operations.

The command is gated by a strict allowlist. The leading word must be a bare
allowlisted utility, such as:

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
body fields, activity details, and connection summaries.
