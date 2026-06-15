# Unfour MCP Server

`unfour-mcp` is the first local Model Context Protocol (MCP) server skeleton for
Unfour. This version exists only to verify that an MCP client can start a local
stdio process, initialize it, discover tools, and call deterministic mock tools.

## Current Scope

The server implements newline-delimited JSON-RPC over standard input and output
with these MCP methods:

- `initialize`
- `tools/list`
- `tools/call`

The server advertises MCP protocol version `2025-06-18` and the `tools`
capability. Standard output is reserved for MCP messages; process errors are
written to standard error.

## Mock Tools

| Tool | Purpose |
| --- | --- |
| `unfour.mock.ping` | Returns `pong` and echoes a supplied string. |
| `unfour.mock.workspace_current` | Returns fixed mock workspace metadata. |
| `unfour.mock.echo` | Returns a supplied JSON value. |

Tool results include both MCP `structuredContent` and a JSON-serialized text
content block for client compatibility. The tools do not read application state,
the filesystem, environment variables, credentials, or network resources.

## Build and Run

From the repository root:

```bash
cargo build -p unfour-mcp
cargo run -p unfour-mcp
```

An MCP client should configure the executable as a stdio server. During local
development, the equivalent command and arguments are:

```text
command: cargo
args: run -p unfour-mcp
working directory: <path-to-unfour>
```

For a prebuilt binary, use:

```text
target/debug/unfour-mcp
```

The process waits for one JSON-RPC message per input line and writes one
JSON-RPC response per output line. Closing standard input shuts down the server.

## Manual Smoke Check

PowerShell can send a minimal request sequence:

```powershell
@'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"manual-check","version":"0.1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"unfour.mock.ping","arguments":{"message":"hello"}}}
'@ | cargo run -q -p unfour-mcp
```

## Explicit Non-Goals

This version does not:

- connect to the Unfour Command Bus;
- call API Debugger, Database, SSH, Terminal, or Workspace services;
- implement HTTP or any other MCP transport;
- implement workflows, resources, prompts, sampling, or system commands;
- read or return passwords, tokens, private keys, environment variables, or
  other sensitive data.

## Next Phase

A later phase can define an explicit MCP-to-Command-Bus adapter, permission and
confirmation rules, audit and redaction behavior, and narrowly scoped tools for
API, Database, SSH, and Workspace capabilities. Those integrations should be
added only after their security and package boundaries are reviewed.
