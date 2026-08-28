# Connect Codex and Cursor to Unfour MCP

This guide is for users of an installed Unfour release. It does not require a
Rust or Cargo development build.

Before configuring Codex or Cursor:

1. Open the Unfour desktop app once. This creates the local database at
   `~/.unfour/unfour.sqlite`.
2. Open `Settings → MCP` and copy the command shown by the app. Microsoft
   Store/MSIX installs show the stable `unfour-mcp.exe` execution alias;
   Standard installs show the absolute path to the installed sidecar.
3. Replace `PASTE_COMMAND_FROM_SETTINGS_MCP` in one configuration below
   with that command. For Windows absolute paths, preserve the escaping
   required by TOML or JSON.
4. Start the desktop app before connecting Codex or Cursor, then restart the
   configured client after saving its configuration.

The server uses local stdio transport, so `args` must remain empty. It runs
through the same command bus as the desktop app and uses the user's saved API,
SSH, and database connections. Do not invent a CLI install command or use a
development `target/debug` path for an installed release. Do not replace the
MSIX alias with a path under `C:\Program Files\WindowsApps`; that directory is
versioned and changes during installation and Store upgrades.

Codex and Cursor can use those saved connections to reproduce issues, inspect
logs and database state, and then act with the user's review. The steps are
worked through together; Unfour provides diagnostic tools rather than an
automatic troubleshooting playbook or workflow runner.

## Codex

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

## Cursor

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
