# Unfour MCP Server

`unfour-mcp` is a local stdio Model Context Protocol server. It now verifies
the first real read-only integration path:

```text
Codex / MCP client
  -> unfour-mcp stdio server
  -> MCP tool handler
  -> command-bus adapter
  -> unfour-command-bus
  -> workspace / connection read services
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

The standalone MCP process opens the same app data SQLite database used by
the desktop app in read-only mode. On Windows this resolves to:

```text
%APPDATA%\com.unfour.workspace\unfour-workspace.sqlite
```

The MCP process does not run migrations, seed workspaces, or write fallback
workspace settings. If the desktop database does not exist yet, start the
desktop app once before starting the MCP server.

The workspace model does not currently persist a workspace root, so
`workspaceRoot` is `null`. There is no API connection business model yet, so
`type: "api"` returns an empty list instead of treating saved API requests as
connections.

Example workspace result:

```json
{
  "workspaceId": "generated-workspace-id",
  "workspaceName": "Default Workspace",
  "workspaceRoot": null,
  "mode": "local",
  "source": "command-bus"
}
```

Example connection result:

```json
{
  "connections": [
    {
      "id": "connection-id",
      "name": "Development Database",
      "type": "database",
      "workspaceId": "workspace-id",
      "safeSummary": {
        "host": "localhost",
        "databaseType": "postgres"
      }
    }
  ],
  "count": 1,
  "source": "command-bus"
}
```

## Safety

Connection output is built from allowlisted summary DTOs. It may contain only
`host`, `databaseType`, and `apiBaseUrl`. A second recursive filter removes
sensitive keys before MCP serialization.

The server does not return passwords, tokens, private keys, credential
references, Authorization headers, cookies, database connection strings,
database usernames, SSH usernames, SSH key paths, or SQLite paths. Command-bus
failures return a stable error code and generic message without forwarding
underlying storage or credential details.

## Mock Tools

The phase-one mock tools remain available and are implemented separately from
the real tools:

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
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"unfour.connection.list","arguments":{"type":"all"}}}
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

## Current Non-Goals

This phase does not:

- send API Debugger requests;
- execute database queries;
- execute SSH commands or open SSH sessions;
- implement `tail_log`;
- implement workflows;
- implement HTTP MCP transport;
- read secrets or environment variables;
- attach to the running desktop process over IPC.
