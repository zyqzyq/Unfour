# Unfour User Guide

This document is for people using the app. Implementation details live in
`docs/architecture`, `docs/mcp`, `docs/testing`, and `docs/release`.

## Current MVP

Unfour opens into a single workspace surface:

- Workspaces on the left
- Tool tabs across the top
- API client, SSH terminal, and database panels in the center
- Local-first storage by default

## Current Capabilities

The app is currently an MVP workbench:

- The workspace shell is usable.
- API debugging supports request editing, Send, response viewing, history,
  saved requests, collections, environments, import, and export.
- SQLite database workflows are usable.
- PostgreSQL and MySQL/MariaDB database workflows are experimental and should be
  verified against your own database before relying on them.
- SSH Terminal workflows are experimental until the live SSH verification gate
  is completed.
- AI calls and cloud sync are planned, not visible product features.

## API Client

1. Select a workspace.
2. Open `API Client`.
3. Add workspace environment variables, such as `base_url` and `source`.
4. Use variables in requests with `{{base_url}}` syntax.
5. Choose an HTTP method.
6. Enter the URL.
7. Add query parameters and headers.
8. Edit the request body for non-GET requests.
9. Click `Send`.
10. Review status, duration, response body, and history.

Saved requests are stored inside the active workspace.

## SSH Terminal

The SSH Terminal is experimental. It supports saved SSH connections, credential
references, terminal sessions, PTY input/output, resize, search, redacted log
export, host-key trust, and reconnect behavior in the current implementation.

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
- `packages/app-shell` provides a thin shell slot wrapper. The desktop app
  composes the current workspace window while the module split is in progress.
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
