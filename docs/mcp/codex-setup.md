# Codex MCP Setup

Use this guide to build and register the local Unfour MCP server with Codex or
another MCP client that supports stdio servers.

## Build

From the repository root:

```powershell
cargo build -p unfour-mcp
```

The default build includes native SSH transport, so SSH diagnostic, exec, file,
and directory tools are available. To build without native SSH support:

```powershell
cargo build -p unfour-mcp --no-default-features
```

In a no-default-features build, SSH tools remain listed but return an
unsupported-operation error when remote execution is required.

## Manual Smoke Check

The process waits for one JSON-RPC message per input line. Closing standard
input shuts it down.

```powershell
@'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"manual-check","version":"0.1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"unfour.workspace.current","arguments":{}}}
'@ | .\target\debug\unfour-mcp.exe
```

Start the desktop app once before running this smoke check if the local Unfour
SQLite database has not been created yet.

## Codex Configuration

Use the absolute path to the built executable. The exact path depends on your
build/install location and is shown automatically in the app under
**Settings → MCP** (the "MCP command" value). For a release install it is the
sidecar bundled next to the app; for a dev build it is the `target/debug`
binary. A dev build looks like:

```toml
[mcp_servers.unfour]
command = "D:\\project\\Unfour\\apps\\desktop\\target\\debug\\unfour-mcp.exe"
args = []
```

On non-Windows platforms, use the absolute path to the platform-specific
`unfour-mcp` binary shown in Settings → MCP.

## Process Lifetime

The stdio server exits when the client closes stdin, disconnects stdout, or
sends a termination signal. As a defensive backstop for desktop clients that
keep completed-task transports open, the packaged server also exits after five
minutes without an MCP protocol message. A later MCP session starts a fresh
sidecar process.

Set `UNFOUR_MCP_IDLE_TIMEOUT_SECS` in the MCP server environment to change this
period. Set it to `0` to disable idle shutdown. Values above 86,400 seconds are
capped at one day.

## Example Prompts

```text
Use the Unfour MCP server to list saved API requests in the current workspace.
Use the Unfour MCP server to inspect the history entry with id <history-id>.
Use the Unfour MCP server to list database connections.
Use the Unfour MCP server to describe the users table for connection <id>.
Use the Unfour MCP server to run a read-only query: select id, email from users limit 10.
Use the Unfour MCP server to explain this query on connection <id>: select * from users where email = 'me@example.com'.
Use the Unfour MCP server to dry-run this database fix on connection <id>: update users set active = true where id = 42.
Use the Unfour MCP server to list recent workspace activity.
Use the Unfour MCP server to run the read-only SSH diagnostic command df -h on connection <id>.
Use the Unfour MCP server to list /var/log on SSH connection <id>.
Use the Unfour MCP server to read the last 20000 bytes of /var/log/app.log on SSH connection <id>.
Use the Unfour MCP server to check system health.
```

For high-risk requests, the first call returns `CONFIRMATION_REQUIRED` with a
`confirmation_text`. Re-run only after reviewing the target workspace,
command/SQL/path, and payload, passing `confirm=true` and that exact
confirmation text.

See `docs/mcp/tools.md` for the current tool list and safety constraints.
