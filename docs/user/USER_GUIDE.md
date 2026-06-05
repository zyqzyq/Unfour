# Unfour Workspace User Guide

This document is for people using the app. It avoids implementation details that belong in `docs/engineering`.

## Current Preview

Unfour Workspace opens into a single workspace surface:

- Workspaces on the left
- Tool tabs across the top
- API client, SSH terminal, and database panels in the center
- Local-first storage by default

## Current Progress

The app is currently an MVP workbench:

- The workspace shell is usable.
- API debugging is the most complete tool today.
- SQLite database workflows have a first usable version.
- SSH is still a preview screen while the real session engine is being built.
- AI calls and cloud sync are not visible product features yet; the codebase only reserves their future integration points.

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

The SSH screen is present as a preview. Real connection support is planned for the SSH MVP.

## Database

The database screen can save workspace-scoped database connections.

1. Open `Database`.
2. Click `+` to create a connection.
3. Choose `SQLite`.
4. Enter a local SQLite file path.
5. Click `Save`.
6. Select the saved connection and click `Test`.
7. Review tables and columns in `Schema`.
8. Write SQL in `SQL Editor`.
9. Click `Run`.
10. Review result rows, affected rows, and duration.

PostgreSQL and MySQL/MariaDB connection metadata can be prepared, but live credential-backed connections are still reserved for the next implementation phase.

## Code Architecture Overview

This section explains the code layout in human terms. It is here so a reader can understand how the app is assembled without reading the engineering design docs first.

Unfour has two halves:

- The desktop window and interface are written with React and TypeScript.
- The secure local capabilities are written in Rust inside Tauri.

The frontend is responsible for what you see and edit:

- `apps/desktop/src/App.tsx` mounts the composed desktop shell.
- `packages/app-shell` builds the main workspace window, left resource area, tabs, and current API/SSH/Database panels.
- `packages/command-client` is the bridge used by React to call Rust commands. It also contains browser-only mocks so the interface can run during frontend development.
- `packages/workspace` keeps temporary UI state such as the active workspace, active tab, and sidebar state.
- `packages/ui` contains reusable interface primitives.
- `packages/api-debugger`, `packages/database`, and `packages/terminal` contain feature-specific frontend logic.

The Rust backend is responsible for actions that should not live only in the browser:

- `apps/desktop/src-tauri/src/lib.rs` starts the Tauri app and registers all commands.
- `apps/desktop/src-tauri/src/commands.rs` exposes thin Tauri commands.
- `apps/desktop/src-tauri/src/command_bus.rs` routes commands to the correct service. This is the same path future AI, CLI, or sync automation should use.
- `crates/local-storage` opens and migrates the local SQLite database and records local activity.
- `crates/workspace-engine` handles workspace data.
- `crates/http-engine` sends HTTP requests and stores API history/templates.
- `crates/database-engine` stores database connections and currently runs SQLite test/schema/query actions.
- `crates/ssh-engine` is the reserved boundary for real SSH sessions.
- `crates/secret-store` is the reserved boundary for OS keychain or Stronghold credentials.

The important idea is that API, SSH, and Database are not separate apps. They share the same Workspace, tabs, local database, local activity trail, credential boundary, and future sync model.

## Data And Privacy

The app is local-first. Workspace metadata is stored locally. High-value actions such as writes, credential changes, external API sends, SSH session lifecycle events, and future AI-triggered actions keep redacted local activity records for troubleshooting. Routine reads and UI layout changes are not treated as activity events. Secret storage is reserved for OS keychain/Stronghold integration; until that work lands, do not place long-lived secrets into saved request bodies.

## Documentation Split

- This guide explains how to use the app.
- `docs/engineering` explains how the app is built.
- `AGENTS.md` explains how coding agents should work in this repository.
