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

## Real Tools

| Tool | Input | Current behavior |
| --- | --- | --- |
| `unfour.workspace.current` | `{}` | Returns the active workspace from the command bus. |
| `unfour.connection.list` | `{ "type": "all" }` | Returns safe database and SSH connection summaries. The optional type is `all`, `api`, `database`, or `ssh`; the default is `all`. |
| `unfour.api.list_collections` | `{ "workspaceId": "optional" }` | Lists API request collections (derived from folder paths). |
| `unfour.api.list_requests` | `{ "workspaceId": "optional", "collectionId": "optional" }` | Lists saved API requests with sensitive URL parameters redacted. |
| `unfour.api.get_request` | `{ "requestId": "required", "includeBody": "optional bool" }` | Returns a saved API request with sensitive headers, query params, URL params, and body fields redacted. |
| `unfour.api.send_request` | `{ "requestId": "required", "environmentId": "optional", "timeoutMs": "optional" }` | Sends a previously saved API request and returns the response summary with sensitive data redacted. |
| `unfour.db.list_connections` | `{ "workspaceId": "optional" }` | Lists saved database connections as safe summaries (no passwords or connection strings). |
| `unfour.db.list_tables` | `{ "connectionId": "required", "workspaceId": "optional", "limit": "optional" }` | Lists tables and views for a saved database connection. Default limit 200, max 500. |
| `unfour.db.describe_table` | `{ "connectionId": "required", "tableName": "required", "schema": "optional", "workspaceId": "optional" }` | Describes a table's columns (name, type, nullable, primaryKey). Does not read table data. |
| `unfour.db.query_readonly` | `{ "connectionId": "required", "sql": "required", "limit": "optional", "workspaceId": "optional" }` | Executes a read-only SQL query. Only SELECT, WITH, SHOW, DESCRIBE, DESC, EXPLAIN are allowed. Default limit 100, max 1000. |

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

## Sensitive Data Redaction

All API tools apply a comprehensive sanitization layer before returning
results. The following field names are treated as sensitive (case-insensitive,
ignoring hyphens and underscores) and their values are replaced with
`[REDACTED]`:

`password`, `passwd`, `pwd`, `token`, `access_token`, `refresh_token`,
`api_key`, `apikey`, `secret`, `client_secret`, `authorization`, `cookie`,
`set-cookie`, `proxy-authorization`, `x-api-key`, `x-auth-token`,
`private_key`, `connection_string`, `database_url`, `credential_ref`.

Redaction is applied to:

- HTTP request and response headers
- URL query parameters
- JSON request and response body fields

Body previews are truncated to 20 KB. When truncation occurs the result
includes `"truncated": true`.

## Data Source

The standalone MCP process opens the same app data SQLite database used by
the desktop app. The database is opened in read-write mode to support
`send_request` history recording, but the MCP tool interface only exposes
read-only operations plus sending saved requests — no create, edit, or
delete tools are exposed.

On Windows this resolves to:

```text
%APPDATA%\com.unfour.workspace\unfour-workspace.sqlite
```

The MCP process does not run migrations, seed workspaces, or write fallback
workspace settings. If the desktop database does not exist yet, start the
desktop app once before starting the MCP server.

## Mock Tools

The phase-one mock tools remain available for testing and are implemented
separately from the real tools:

| Tool | Purpose |
| --- | --- |
| `unfour.mock.ping` | Returns `pong` and echoes a supplied string. |
| `unfour.mock.workspace_current` | Returns fixed mock workspace metadata. |
| `unfour.mock.echo` | Returns a supplied JSON value. |

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

Example Codex prompts:

```text
请通过 unfour MCP 列出当前 workspace 中保存的 API requests。
请通过 unfour MCP 获取 requestId 为 xxx 的 API 请求详情。
请通过 unfour MCP 发送 requestId 为 xxx 的 API 请求，并总结响应状态、耗时和 bodyPreview。
请通过 unfour MCP 列出当前 workspace 的数据库连接。
请通过 unfour MCP 查看 connectionId 为 xxx 的数据库表列表。
请通过 unfour MCP 描述 users 表结构。
请通过 unfour MCP 对 connectionId 为 xxx 执行只读查询：select id, email from users limit 10，并总结结果。
```

## Current Non-Goals

This phase does not:

- create, edit, or delete API requests through MCP;
- support arbitrary URL requests (only saved `requestId`);
- execute database write operations (INSERT, UPDATE, DELETE, DDL);
- accept ad-hoc database connection strings (only saved `connectionId`);
- execute SSH commands or open SSH sessions;
- implement `tail_log`;
- implement workflows;
- implement HTTP MCP transport;
- read secrets or environment variable values;
- attach to the running desktop process over IPC.
