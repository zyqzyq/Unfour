# Unfour MCP Server

`unfour-mcp` is a local stdio Model Context Protocol server. It exposes
workspace, connection, and API Debugger read-only capabilities plus the
ability to send previously saved API requests:

```text
Codex / MCP client
  -> unfour-mcp stdio server
  -> MCP tool handler
  -> command-bus adapter
  -> unfour-command-bus
  -> workspace / connection / API Debugger services
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

## Sensitive Data Redaction

All API tools apply a comprehensive sanitization layer before returning
results. The following field names are treated as sensitive (case-insensitive,
ignoring hyphens and underscores) and their values are replaced with
`[REDACTED]`:

`password`, `passwd`, `pwd`, `token`, `access_token`, `refresh_token`,
`api_key`, `apikey`, `secret`, `client_secret`, `authorization`, `cookie`,
`set-cookie`, `proxy-authorization`, `x-api-key`, `x-auth-token`,
`private_key`, `connection_string`.

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
```

## Current Non-Goals

This phase does not:

- create, edit, or delete API requests through MCP;
- support arbitrary URL requests (only saved `requestId`);
- execute database queries;
- execute SSH commands or open SSH sessions;
- implement `tail_log`;
- implement workflows;
- implement HTTP MCP transport;
- read secrets or environment variable values;
- attach to the running desktop process over IPC.
