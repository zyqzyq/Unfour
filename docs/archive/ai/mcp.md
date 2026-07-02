# Unfour MCP Server

`unfour-mcp` is a local stdio Model Context Protocol server. It exposes
workspace, connection, API Debugger, and database read-only capabilities:

```text
Codex / MCP client
  -> unfour-mcp stdio server
  -> MCP tool handler
  -> command-bus adapter
  -> unfour-command-bus
  -> workspace / connection / API Debugger / database services
  -> structured MCP result
```

The server implements newline-delimited JSON-RPC over standard input and
output with `initialize`, `tools/list`, and `tools/call`. Standard output is
reserved for MCP messages; process errors are written to standard error.

## Recommended Diagnostic Flow

The `initialize` response returns an `instructions` string that gives MCP
clients a suggested backend-troubleshooting flow:

1. `unfour.system.health` — confirm the store is ready.
2. `unfour.activity.list` — see what changed recently before a failure started.
3. API issues — `unfour.api.list_history` → `unfour.api.get_history` to find the
   first failing request and inspect masked auth, then `unfour.api.send_request`
   to replay a saved request.
4. Database issues — `unfour.db.list_connections`, `unfour.db.list_tables`,
   `unfour.db.describe_table`, and `unfour.db.query_readonly`
   (SELECT/WITH/SHOW/EXPLAIN only).
5. Host/service issues — `unfour.ssh.run_diagnostic` with read-only commands
   (`df`, `free`, `journalctl`, `grep`, `docker logs`, `kubectl get/logs`, ...).

## Tool Annotations

Every tool in `tools/list` carries MCP behavior hints so a client can reason
about safety without parsing descriptions:

- `readOnlyHint` — `true` when the tool does not mutate any state.
- `destructiveHint` — always `false`; no tool performs destructive operations.
- `idempotentHint` — `true` for repeatable reads.
- `openWorldHint` — `true` when the tool reaches a system outside the local app
  data store (a remote database or SSH host) or performs an external action.

The only tools with `openWorldHint: true` are the database tools that connect to
a remote server (`list_tables`, `describe_table`, `query_readonly`,
`test_connection`), `unfour.ssh.run_diagnostic`, and `unfour.api.send_request`
(the one tool with `readOnlyHint: false`, since it issues an HTTP request and
records history). All local-store reads are `readOnlyHint: true`,
`openWorldHint: false`.

## Tools

| Tool | Input | Current behavior |
| --- | --- | --- |
| `unfour.workspace.current` | `{}` | Returns the active workspace from the command bus. |
| `unfour.workspace.list` | `{}` | Lists all local workspaces, marking which one is active. |
| `unfour.connection.list` | `{ "type": "all" }` | Returns safe database and SSH connection summaries. The optional type is `all`, `api`, `database`, or `ssh`; the default is `all`. |
| `unfour.api.list_collections` | `{ "workspaceId": "optional" }` | Lists API request collections (derived from folder paths). |
| `unfour.api.list_requests` | `{ "workspaceId": "optional", "collectionId": "optional" }` | Lists saved API requests with sensitive URL parameters redacted. |
| `unfour.api.get_request` | `{ "requestId": "required", "includeBody": "optional bool" }` | Returns a saved API request with sensitive headers, query params, URL params, and body fields redacted. |
| `unfour.api.send_request` | `{ "requestId": "required", "environmentId": "optional", "timeoutMs": "optional" }` | Sends a previously saved API request and returns the response summary with sensitive data masked. |
| `unfour.api.list_history` | `{ "workspaceId": "optional", "limit": "optional" }` | Lists recent API request/response history with sensitive URL parameters masked. Default limit 50, max 200. Useful for diagnosing when a request started failing. |
| `unfour.api.get_history` | `{ "historyId": "required", "workspaceId": "optional" }` | Returns a single history entry's request/response detail with sensitive headers, query params, and body fields masked. |
| `unfour.api.list_environments` | `{ "workspaceId": "optional" }` | Lists API environments and their variables. Sensitive variable values are masked; non-sensitive values (e.g. base URLs) are shown verbatim. |
| `unfour.db.list_connections` | `{ "workspaceId": "optional" }` | Lists saved database connections as safe summaries (no passwords or connection strings). |
| `unfour.db.list_tables` | `{ "connectionId": "required", "workspaceId": "optional", "limit": "optional" }` | Lists tables and views for a saved database connection. Default limit 200, max 500. |
| `unfour.db.describe_table` | `{ "connectionId": "required", "tableName": "required", "schema": "optional", "workspaceId": "optional" }` | Describes a table's columns (name, type, nullable, primaryKey). Does not read table data. |
| `unfour.db.query_readonly` | `{ "connectionId": "required", "sql": "required", "limit": "optional", "workspaceId": "optional" }` | Executes a read-only SQL query. Only SELECT, WITH, SHOW, DESCRIBE, DESC, EXPLAIN are allowed. Default limit 100, max 1000. |
| `unfour.db.test_connection` | `{ "connectionId": "required", "workspaceId": "optional" }` | Tests connectivity to a saved database connection and returns success plus server version when available. |
| `unfour.activity.list` | `{ "workspaceId": "optional", "limit": "optional" }` | Lists recent workspace activity events (workspace/connection/API/database/SSH changes) newest first, with sensitive fields in event details masked. Default limit 50, max 200. Useful for diagnosing what changed before a failure started. |
| `unfour.ssh.run_diagnostic` | `{ "connectionId": "required", "command": "required", "workspaceId": "optional", "timeoutMs": "optional" }` | Runs a single read-only diagnostic command on a saved SSH connection and returns captured stdout/stderr. Only a fixed allowlist of read-only utilities is permitted; shells, pipes, redirection, chaining, and write/control operations are rejected. Output is line-redacted. Requires an `ssh-native` build. |
| `unfour.system.health` | `{}` | Returns command-bus and storage readiness for diagnostics. |

### API Debugger Tools

The API Debugger tools allow MCP clients to inspect and execute saved API
requests through the command bus. All tools route through
`unfour-command-bus` — the MCP layer never accesses the API Debugger
internal storage or UI state directly.

**Collections** are derived from the `folder_path` of saved API requests.
Requests without a folder path are grouped under a "General" collection
with an empty `id`.

**`unfour.api.send_request`** only sends previously saved requests by
`requestId`. It does not accept arbitrary URLs. The `timeoutMs` parameter
is clamped to a maximum of 60,000 ms (60 seconds). Environment variables
are resolved from the workspace's current environment.

Example list_collections result:

```json
{
  "collections": [
    { "id": "Users", "name": "Users", "requestCount": 3, "workspaceId": "ws-1" },
    { "id": "", "name": "General", "requestCount": 1, "workspaceId": "ws-1" }
  ],
  "count": 2,
  "source": "command-bus"
}
```

Example send_request result:

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

### Database Tools

The Database tools allow MCP clients to inspect database connections, browse
table schemas, and execute read-only SQL queries through the command bus.
All tools route through `unfour-command-bus` — the MCP layer never accesses
the Database module internal storage directly.

All database tools require a saved `connectionId`. Ad-hoc connection strings
are not accepted.

**`unfour.db.list_connections`** returns safe summaries without passwords,
tokens, usernames, credential references, or connection strings. Each summary
includes `id`, `name`, `databaseType`, `host`, `port`, `database`, and
`workspaceId`.

**`unfour.db.list_tables`** returns tables and views with their schema, kind,
and column count. The `limit` parameter defaults to 200 and is clamped to a
maximum of 500. When the result is truncated, the response includes
`"truncated": true` and `totalTables` reflects the untruncated count.

**`unfour.db.describe_table`** returns column metadata (name, data type,
nullable, primary key) for a single table. The optional `schema` parameter
filters by schema name (e.g. `"public"`). If the table is not found, a
structured `TABLE_NOT_FOUND` error is returned.

**`unfour.db.query_readonly`** executes a single read-only SQL statement.
The MCP layer enforces a strict allowlist before the query reaches the
command bus:

Allowed keywords: `SELECT`, `WITH`, `SHOW`, `DESCRIBE`, `DESC`, `EXPLAIN`.

Forbidden keywords include: `INSERT`, `UPDATE`, `DELETE`, `DROP`, `ALTER`,
`CREATE`, `TRUNCATE`, `REPLACE`, `MERGE`, `GRANT`, `REVOKE`, `VACUUM`,
`ANALYZE`, `CALL`, `EXEC`, `EXECUTE`, `COPY`, `LOAD`, `ATTACH`, `DETACH`,
`PRAGMA`, `BEGIN`, `COMMIT`, `ROLLBACK`.

Leading SQL comments (`--`, `/* */`) are stripped before keyword extraction
to prevent bypass. Multi-statement SQL (containing `;` between statements)
is rejected. The `limit` parameter defaults to 100 and is clamped to 1000.
Query results exceeding 20 KB are truncated with `"truncated": true`.

Example list_connections result:

```json
{
  "connections": [
    {
      "id": "conn-1",
      "name": "Dev Database",
      "databaseType": "postgres",
      "host": "localhost",
      "port": 5432,
      "database": "app_dev",
      "workspaceId": "ws-1"
    }
  ],
  "count": 1,
  "source": "command-bus"
}
```

Example query_readonly result:

```json
{
  "ok": true,
  "connectionId": "conn-1",
  "columns": [
    { "name": "id", "dataType": "integer" },
    { "name": "email", "dataType": "varchar" }
  ],
  "rows": [
    [1, "user@example.com"],
    [2, "other@example.com"]
  ],
  "rowCount": 2,
  "durationMs": 42,
  "truncated": false,
  "source": "command-bus"
}
```

### Activity Tool

`unfour.activity.list` returns the recent local activity trail recorded by the
command bus (workspace, connection, API, database, and SSH lifecycle events),
newest first. Results are scoped to the active workspace unless `workspaceId`
is provided. `limit` defaults to 50 and is clamped to a maximum of 200.

Event `details` are stored as already-redacted summaries; the MCP layer applies
an additional recursive masking pass over the details payload before returning
it, so sensitive fields (passwords, tokens, credential references, etc.) are
masked even if a future writer records one by mistake.

This tool is read-only diagnostic context. It helps an AI client answer
"what changed just before this started failing?" by correlating activity
timestamps with API history or database/connection state.

Example list result:

```json
{
  "activity": [
    {
      "id": "evt-2",
      "workspaceId": "ws-1",
      "action": "database.connection.create",
      "target": "conn-1",
      "details": { "name": "Prod DB" },
      "createdAt": "2026-06-20T01:00:00Z"
    }
  ],
  "count": 1,
  "source": "command-bus"
}
```

### SSH Diagnostics Tool

`unfour.ssh.run_diagnostic` runs a single, read-only command on a saved SSH
connection and returns its captured `stdout`/`stderr` and exit status. It is the
only SSH capability exposed over MCP — there is no interactive shell, no session
state, and no write/control access.

The command is gated by a strict allowlist before it ever reaches the host:

- The leading word must be a bare allowlisted read-only utility (no path):
  `df`, `du`, `free`, `uptime`, `uname`, `hostname`, `whoami`, `id`, `date`,
  `ps`, `ss`, `netstat`, `ip`, `ifconfig`, `vmstat`, `iostat`, `mount`, `stat`,
  `wc`, `ls`, `cat`, `tail`, `head`, `grep`, `systemctl`, `journalctl`,
  `docker`, `podman`, `kubectl`.
- Shell metacharacters are rejected entirely (`; | & $ \` > < ( ) { } \ * ? ~ !
  # ' "`), so chaining, piping, redirection, and subshells are impossible. As a
  consequence `grep` is limited to literal patterns without those characters and
  without spaces (e.g. `grep ERROR /var/log/app.log`); piping into `grep` is not
  possible.
- `systemctl` is restricted to read-only subcommands (`status`, `is-active`,
  `is-enabled`, `is-failed`, `show`, `cat`, `list-units`, `list-unit-files`,
  `list-timers`, `list-sockets`, `get-default`); start/stop/restart/enable/etc.
  are rejected.
- `journalctl` log-management flags (`--vacuum*`, `--rotate`, `--flush`,
  `--sync`, `--relinquish-var`) are rejected.
- `docker` and `podman` are restricted to read-only subcommands (`ps`, `logs`,
  `inspect`, `images`, `version`, `info`, `stats`, `top`, `port`, `diff`);
  `run`, `exec`, `rm`, `stop`, `kill`, and other mutating verbs are rejected.
- `kubectl` is restricted to read-only subcommands (`get`, `describe`, `logs`,
  `top`, `version`, `api-resources`, `explain`, `cluster-info`); mutating verbs
  (`apply`, `delete`, `edit`, `scale`, `exec`, `cp`, `port-forward`, ...) and
  `config` (which can expose credentials) are rejected.
- For `docker`, `podman`, and `kubectl` the read-only subcommand must come
  **first** (e.g. `kubectl get pods -n prod`, not `kubectl -n prod get pods`).
  Global flags before the subcommand are rejected so a value-taking flag cannot
  smuggle a mutating verb in as its argument.

Captured output is line-redacted by the SSH engine (the same redaction applied
to terminal history): any line that looks like a sensitive `key: value` /
`key=value` pair is replaced. This is best-effort over free-form command output;
the allowlist is the primary control. Each stream is capped (64 KB) and the whole
command runs under a timeout (default 15s, max 60s); `truncated` is `true` when a
cap or the timeout is hit.

This tool relies on the `ssh-native` feature, which is **enabled by default** in
`unfour-mcp`, so a plain `cargo build -p unfour-mcp` produces a binary that can
run SSH diagnostics. If you build with `--no-default-features`, the tool is still
listed but returns `COMMAND_BUS_OPERATION_UNSUPPORTED`. Live SSH transport
remains pending end-to-end verification against a real server.

Example result:

```json
{
  "connectionId": "conn-1",
  "command": "df -h",
  "stdout": "Filesystem      Size  Used Avail Use% Mounted on\n/dev/sda1        50G   20G   30G  40% /",
  "stderr": "",
  "exitStatus": 0,
  "truncated": false,
  "source": "command-bus"
}
```

## Sensitive Data Masking

All API tools apply a sanitization layer before returning results. The
following field names are treated as sensitive (case-insensitive, ignoring
hyphens and underscores):

`password`, `passwd`, `pwd`, `token`, `access_token`, `refresh_token`,
`api_key`, `apikey`, `secret`, `client_secret`, `authorization`, `cookie`,
`set-cookie`, `proxy-authorization`, `x-api-key`, `x-auth-token`,
`private_key`, `connection_string`, `database_url`, `credential_ref`.

Because MCP results are consumed by an LLM (potentially cloud-hosted), these
values must not leave the machine in usable form. Instead of replacing the
whole value, sensitive values are replaced with a **partial-mask descriptor**
that exposes diagnostic *shape* while hiding the usable secret. This lets a
client diagnose the common auth failures (wrong scheme, truncated/malformed
token, expired JWT, wrong-environment key, mismatched tokens across fields)
without exfiltrating the credential.

The descriptor has the form `[mask kind=… scheme=… len=… fp=…]` where:

- `kind` — structural classification: `jwt`, `basic`, `uuid`, `hex`,
  `prefixed:<p>` (a non-secret leading prefix such as `sk`/`ghp`), or `opaque`.
- `scheme` — the auth scheme word (`Bearer`, `Basic`, `Digest`, …) when present.
- `len` — character length of the secret material.
- `fp` — a short, deterministic, non-cryptographic fingerprint (FNV-1a) of the
  secret. It lets a client check whether two fields hold the *same* secret
  (e.g. request header vs. environment variable) without revealing either.

Examples: `[mask kind=jwt scheme=Bearer len=872 fp=a1b2c3]`,
`[mask kind=prefixed:sk len=51 fp=9f8e7d]`.

Masking is applied to:

- HTTP request and response headers
- URL query parameters
- JSON request and response body fields

This is the MCP-layer (LLM-facing) policy. The narrower persistence-layer
redaction in `unfour-core` (`<redacted>` for the five auth headers) is
separate and unchanged.

Body previews are truncated to 20 KB. When truncation occurs the result
includes `"truncated": true`.

## Data Source

The standalone MCP process opens the same app data SQLite database used by
the desktop app. The database is opened in read-write mode to support
`send_request` history recording, but the MCP tool interface only exposes
read-only operations plus sending saved requests — no create, edit, or
delete tools are exposed.

Because the desktop app and the MCP process can open the database
concurrently, all connections set a 5-second `busy_timeout` to avoid spurious
"database is locked" failures under contention.

Database connection credentials are resolved from the OS keychain under the
same service name the desktop app uses (`unfour`). The MCP only reads
credentials to open database connections; it never creates, rotates, or deletes
them. On platforms where keychain items are ACL'd per-application (notably
macOS), the first credential read from the MCP process may require user
approval.

On Windows this resolves to:

```text
%APPDATA%\Unfour\unfour.sqlite
```

The MCP process does not run migrations, seed workspaces, or write fallback
workspace settings. If the desktop database does not exist yet, start the
desktop app once before starting the MCP server.

## Windows Build And Run

From the repository root:

```powershell
cargo build -p unfour-mcp
.\target\debug\unfour-mcp.exe
```

The process waits for one JSON-RPC message per input line. Closing standard
input shuts it down.

Manual smoke check:

```powershell
@'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"manual-check","version":"0.1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"unfour.workspace.current","arguments":{}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"unfour.api.list_collections","arguments":{}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"unfour.api.list_requests","arguments":{}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"unfour.db.list_connections","arguments":{}}}
'@ | .\target\debug\unfour-mcp.exe
```

## Codex MCP Configuration

Prefer the absolute path to the prebuilt executable:

```toml
[mcp_servers.unfour]
command = "E:\\project\\unfour\\target\\debug\\unfour-mcp.exe"
args = []
```

Build the executable before starting Codex:

```powershell
cargo build -p unfour-mcp
```

The default build already includes native SSH transport, so
`unfour.ssh.run_diagnostic` works out of the box. To build a binary without it,
use `cargo build -p unfour-mcp --no-default-features`.

Example Codex prompts:

```text
请通过 unfour MCP 列出当前 workspace 中保存的 API requests。
请通过 unfour MCP 获取 requestId 为 xxx 的 API 请求详情。
请通过 unfour MCP 发送 requestId 为 xxx 的 API 请求，并总结响应状态、耗时和 bodyPreview。
请通过 unfour MCP 列出当前 workspace 的数据库连接。
请通过 unfour MCP 查看 connectionId 为 xxx 的数据库表列表。
请通过 unfour MCP 描述 users 表结构。
请通过 unfour MCP 对 connectionId 为 xxx 执行只读查询：select id, email from users limit 10，并总结结果。
请通过 unfour MCP 列出最近的 API 请求历史，找出最早开始返回非 2xx 的请求。
请通过 unfour MCP 获取 historyId 为 xxx 的历史详情，对照请求/响应头里的 auth 掩码信息判断鉴权问题。
请通过 unfour MCP 测试 connectionId 为 xxx 的数据库连通性。
请通过 unfour MCP 列出当前 workspace 的 API 环境与变量（敏感值会被掩码）。
请通过 unfour MCP 列出最近的 workspace 活动事件，结合 API 历史判断"故障开始前发生了什么变化"。
请通过 unfour MCP 在 connectionId 为 xxx 的 SSH 主机上执行只读诊断：df -h，并判断磁盘是否快满。
请通过 unfour MCP 在 connectionId 为 xxx 上查看 systemctl status nginx，判断服务是否异常。
请通过 unfour MCP 检查系统健康状态。
```

## Current Non-Goals

This phase does not:

- create, edit, or delete API requests through MCP;
- support arbitrary URL requests (only saved `requestId`);
- execute database write operations (INSERT, UPDATE, DELETE, DDL);
- accept ad-hoc database connection strings (only saved `connectionId`);
- open interactive SSH sessions, run arbitrary (non-allowlisted) SSH commands,
  or perform any SSH write/control operation (only the read-only allowlisted
  `unfour.ssh.run_diagnostic` is exposed);
- implement workflows;
- implement HTTP MCP transport;
- return raw secret values (sensitive headers, tokens, passwords, and sensitive
  environment variables are masked — never returned in usable form);
- attach to the running desktop process over IPC.
