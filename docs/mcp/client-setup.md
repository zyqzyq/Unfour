# Connect Codex and Cursor to Unfour MCP

This guide is for users of an installed Unfour release. It does not require a
Rust or Cargo development build.

## Preferred: one-click configuration

1. Open the Unfour desktop app once. This creates the local database at
   `~/.unfour/unfour.sqlite`.
2. Open `Settings → MCP`.
3. Use `Configure Codex` or `Configure Cursor` for the client you want to
   connect.
4. After configuration completes, restart the corresponding client.

The Settings page detects the runtime command for the installed build and shows
the configuration status for each client. Microsoft Store/MSIX installs use the
stable `unfour-mcp.exe` execution alias; Standard installs use the absolute path
to the installed sidecar. The one-click actions preserve the existing client
configuration according to the client's integration and report success or an
error in the Settings page.

Codex and Cursor can use those saved connections to reproduce issues, inspect
logs and database state, and then act with the user's review. The steps are
worked through together; Unfour provides diagnostic tools rather than an
automatic troubleshooting playbook or workflow runner.

The coding client uses its own tools to inspect, edit, and test code; Unfour MCP
provides runtime evidence and re-checks after a change. You keep control of the
workspace, environment, risky actions, and final decision.

After restarting your configured client, use `Settings → MCP → Example Prompt`
and `Copy example prompt`, then paste it into that client and describe your
issue. Copying only places text on the clipboard; it does not launch a client,
call tools, send network requests, or change the workspace or MCP policy.

The server uses local stdio transport, so `args` must remain empty. Do not
invent a CLI install command or use a development `target/debug` path for an
installed release. Do not replace the MSIX alias with a path under
`C:\Program Files\WindowsApps`; that directory is versioned and changes during
installation and Store upgrades.

## Manual / Advanced configuration

Use this fallback when one-click configuration is unavailable or when you need
to review and edit the client files yourself:

1. Open `Settings → MCP` and use `Copy command`.
2. Replace `PASTE_COMMAND_FROM_SETTINGS_MCP` in the configuration below with
   that command. For Windows absolute paths, preserve the escaping required by
   TOML or JSON.
3. Save the configuration and restart the configured client.

### Codex

Add this entry to the Codex TOML configuration:

```toml
[mcp_servers.unfour]
command = "PASTE_COMMAND_FROM_SETTINGS_MCP"
args = []
```

For a Microsoft Store/MSIX install, the command value is the stable alias:

```toml
[mcp_servers.unfour]
command = "unfour-mcp.exe"
args = []
```

### Cursor

Add this server to `.cursor/mcp.json` in a project or `~/.cursor/mcp.json` for
your user configuration:

```json
{
  "mcpServers": {
    "unfour": {
      "command": "PASTE_COMMAND_FROM_SETTINGS_MCP",
      "args": []
    }
  }
}
```

For a Microsoft Store/MSIX install, use `"unfour-mcp.exe"` as the `command`
value. Do not use the versioned physical path under `WindowsApps`.

In the default `prod` environment, MCP is read-only. High-risk actions do not
run immediately: the server returns `CONFIRMATION_REQUIRED` with confirmation
details, and Codex or Cursor must retry only after the target and payload
have been reviewed.

Do not set `UNFOUR_MCP_STORAGE_MODE=ephemeral` for daily use. It is intended for
registry validation, CI, protocol smoke checks, and isolated tests, and uses an
empty in-memory workspace instead of the desktop app's saved data.

See [MCP Overview](overview.md) for the protocol and safety model, and
[MCP Tools](tools.md) for the current tool list.
