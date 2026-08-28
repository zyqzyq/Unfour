# Unfour User Guide

This document is for people using the app. Implementation details live in
`docs/architecture`, `docs/mcp`, `docs/testing`, and `docs/release`.

## Current Product

Unfour opens into a single workspace surface:

- Workspaces on the left
- Tool tabs across the top
- API client, SSH terminal, and database panels in the center
- Local-first storage by default

## Current Capabilities

The current published release is `v0.9.0`:

- The workspace shell is usable.
- API debugging supports request editing, Send, response viewing, history,
  saved requests, collections, environments, import/export, and request
  scripts with test and console results.
- Local MCP is available through the stdio `unfour-mcp` server. It uses the
  same command bus and the same saved API, SSH, and database connections, so
  Codex and Cursor can reproduce issues, inspect logs and database state, and
  make fixes alongside the user.
- Unfour's core desktop features are free and open source under Apache-2.0. An
  active Pro subscription unlocks Cloud Sync in the same application.
- SQLite database workflows are usable.
- PostgreSQL and MySQL/MariaDB database workflows are experimental and should be
  verified against your own database before relying on them.
- SSH Terminal workflows are experimental until the live SSH verification gate
  is completed.

## Account and Cloud Sync

Unfour is one application and one product. Cloud Sync is available in that same
application when the signed-in account has an active Pro entitlement for Cloud
Sync; there is no separate client for Pro.

1. Open `Settings → Account` and choose `Sign in`.
2. Complete the GitHub sign-in in your browser. After the
   `unfour://auth/callback` link returns to Unfour, the account status and plan
   are refreshed.
3. Open `Settings → Cloud Sync`. Cloud Sync is available only when the account
   has the required active entitlement. If it is unavailable, refresh the
   account status or use the account and billing action in `Settings → Account`.
4. Enable Cloud Sync for a workspace from its workspace actions, then review
   sync status in `Settings → Cloud Sync`.

Cloud Sync synchronizes the supported non-secret workspace data shown in the
app, including workspace settings, connection definitions, environments,
non-secret variables, API collections/folders/requests, and SSH tasks. Secrets,
SSH credentials and local key paths, database credentials and local file paths,
and history/runtime results remain local to each device.

Troubleshooting is a core product loop. Unfour does not include an automatic
troubleshooting playbook: the user and Codex or Cursor work through the steps
together with the available diagnostic tools. Those tools are not a workflow
runner.

## Connect Codex or Cursor (MCP)

Unfour provides a local stdio MCP server for Codex and Cursor. It uses the same
command bus as the desktop app and reads the workspace data and saved
connections from the local storage.

This gives Codex and Cursor the same troubleshooting surface as the desktop
app: they can use saved connections to reproduce an issue, inspect logs or
database state, and then act with your review. The steps remain collaborative;
the server provides diagnostic tools, not an automatic troubleshooting
playbook.

1. Open the desktop app once. This creates `~/.unfour/unfour.sqlite`.
2. In `Settings → MCP`, copy the MCP command shown by the app. Microsoft
   Store/MSIX installations use the stable `unfour-mcp.exe` alias; Standard
   installations show the absolute sidecar path.
3. Follow the [installed-user MCP setup](../mcp/client-setup.md) and paste that
   command into the Codex or Cursor configuration.
4. Start or restart Codex or Cursor after saving its configuration.

Do not set `UNFOUR_MCP_STORAGE_MODE=ephemeral` for daily use. That mode is for
registry validation, CI, protocol smoke checks, and isolated tests; it uses an
empty in-memory workspace instead of the desktop app's saved data.

In the default `prod` environment, MCP is read-only. High-risk actions return
`CONFIRMATION_REQUIRED` and must be reviewed before Codex or Cursor retries
them.

## API Client

1. Select a workspace.
2. Open `API Client`.
3. Add workspace environment variables, such as `base_url` and `source`.
4. Use variables in requests with `{{base_url}}` syntax.
5. Choose an HTTP method.
6. Enter the URL.
7. Add query parameters and headers.
8. Edit the request body for non-GET requests.
9. Optionally add a pre-request or post-response script in `Scripts`.
10. Click `Send`.
11. Review status, duration, response body, script tests/console, and history.

Saved requests are stored inside the active workspace.

## SSH Terminal

The SSH Terminal is experimental. It supports saved SSH connections, credential
references, terminal sessions, PTY input/output, resize, search, redacted log
export, clipboard context actions, host-key trust, reconnect, SFTP remote files,
and serial command/upload/download tasks in the current implementation.

Use non-critical hosts until the live SSH verification gate is completed. The
full password/private-key, host-key, history restore, keepalive, and reconnect
journey still needs release-level manual verification against a reachable SSH
server.

## Database

The database screen can save workspace-scoped database connections.

1. Open `Database`.
2. Click `+` to create a connection.
3. Choose `SQLite`.
4. Enter a local SQLite file path.
5. Click `Save`.
6. Select the saved connection and click `Connect`.
7. Review tables and columns in `Schema`.
8. Write SQL in `SQL Editor`.
9. Click `Run`.
10. Review result rows, affected rows, and duration.

PostgreSQL and MySQL/MariaDB workflows are experimental. You can create
connections using credential references and use the same connect, schema,
query, and table-preview flow, but live behavior depends on your database
environment. Re-verify before using them for important work.

## Code Architecture Overview

This section explains the code layout in human terms. It is here so a reader can understand how the app is assembled without reading the engineering design docs first.

Unfour has two halves:

- The desktop window and interface are written with React and TypeScript.
- The secure local capabilities are written in Rust inside Tauri.

The frontend is responsible for what you see and edit:

- `apps/desktop/src/App.tsx` mounts the composed desktop shell.

- `packages/app-shell provides` the frontend desktop workbench composition root.
  It wires the workspace switcher, module navigation, layout slots, command
  palette, diagnostics actions, and mounts the API Client, Database, and SSH
  Terminal modules without owning their feature internals.
- `packages/command-client` is the bridge used by React to call Rust commands. It also contains browser-only mocks so the interface can run during frontend development.
- `packages/workspace-core` keeps temporary UI state such as the active workspace, active tab, and sidebar state.
- `packages/workspace-local` is the frontend boundary reserved for local workspace persistence and currently provides a compatibility re-export.
- `packages/ui` contains reusable interface primitives.
- `packages/api-client`, `packages/database`, and `packages/ssh-terminal` contain feature-specific frontend logic.

The Rust backend is responsible for actions that should not live only in the browser:

- Rust/Tauri commands are registered and shared through `crates/unfour-app`.
- `apps/desktop/src-tauri` is the thin desktop binary wrapper.
- `crates/unfour-command-bus` routes commands to the correct service. Tauri,
  MCP, and future AI/CLI automation should use this same command boundary.
- `crates/local-storage` opens and migrates the local SQLite database and records local activity.
- `crates/workspace-engine` handles workspace data.
- `crates/http-engine` sends HTTP requests and stores API history/templates.
- `crates/database-engine` stores database connections and runs database
  test/schema/query/table-browse actions.
- `crates/ssh-engine` owns SSH sessions, host-key trust, reconnect behavior,
  and terminal log export.
- `crates/secret-store` owns OS keychain-backed credential references in
  production and in-memory credentials for tests.

The important idea is that API, SSH, and Database are not separate apps. They share the same Workspace, tabs, local database, local activity trail, credential boundary, and future sync model.

## Data And Privacy

The app is local-first. Workspace metadata is stored locally. High-value actions
such as writes, credential changes, external API sends, SSH session lifecycle
events, and future AI-triggered actions keep redacted local activity records for
troubleshooting. Routine reads and UI layout changes are not treated as activity
events. Use credential references for SSH and database secrets where available;
do not place long-lived secrets in workspace environment variables or saved
request bodies.

## Documentation Split

- This guide explains how to use the app.
- `docs/architecture` explains how the app is built.
- `docs/mcp` explains the local MCP server.
- `docs/testing` and `docs/release` explain release verification.
- `AGENTS.md` explains how coding agents should work in this repository.
