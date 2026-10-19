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
error in the Settings page. They write the client entry only. They do not start
a troubleshooting run or change workspace policy.

After restarting your configured client, use `Settings → MCP → Example Prompt`
and `Copy example prompt`, then paste it into that client and describe your
issue. Copying only places text on the clipboard; it does not launch a client,
call tools, send network requests, or change the workspace or MCP policy.

`unfour-mcp` itself takes no CLI arguments. Codex should keep `args = []`. On
Windows, Cursor still launches stdio MCP servers through `cmd.exe`, so a
command path that contains spaces must be wrapped as `cmd.exe /c` with the
absolute path as a single argument; one-click Cursor configuration writes that
wrapper automatically. Do not invent a CLI install command or use a development
`target/debug` path for an installed release. Do not replace the MSIX alias
with a path under `C:\Program Files\WindowsApps`; that directory is versioned
and changes during installation and Store upgrades.

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

If the pasted Windows command contains spaces (for example
`D:\Program Files\Unfour\unfour-mcp.exe`), wrap it so Cursor does not split the
path:

```json
{
  "mcpServers": {
    "unfour": {
      "command": "cmd.exe",
      "args": ["/c", "D:\\Program Files\\Unfour\\unfour-mcp.exe"]
    }
  }
}
```

For a Microsoft Store/MSIX install, use `"unfour-mcp.exe"` as the `command`
value. Do not use the versioned physical path under `WindowsApps`.

Do not set `UNFOUR_MCP_STORAGE_MODE=ephemeral` for daily use. That mode is for
registry validation and isolated tests; see
[Ephemeral registry mode](overview.md#ephemeral-registry-mode).

Capability boundaries, the default prod read-only mapping, confirmation, and
the limit that MCP does not generate a troubleshooting playbook are defined in
[MCP Overview](overview.md). Tool arguments are in [MCP Tools](tools.md).
Day-to-day use of the connected client is in the
[user guide](../user/USER_GUIDE.md#connect-codex-or-cursor-mcp).
