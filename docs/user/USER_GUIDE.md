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

This guide documents the current v0.9.2 product capabilities. The published
v0.9.0 verification record remains the latest completed release evidence until
the v0.9.2 candidate is verified:

- The workspace shell is usable.
- API, SSH, and Database sidebars keep independent bounded widths and restore
  their own layout when switching modules or restarting the app.
- API debugging supports request editing, Send, response viewing, history,
  saved requests, collections, environments, import/export, and request
  scripts with test and console results.
- Local MCP is available through the stdio `unfour-mcp` server. It uses the
  same command bus and the same saved API, SSH, and database connections, so
  Codex and Cursor can reproduce issues, inspect logs and database state, and
  make fixes alongside the user. Real v0.9.0 client checks completed in both
  Codex and Cursor, including server start, initialization, tool discovery,
  tool calls, and access to real Unfour data/tools. MCP also supports per-call
  environment selection, saved-request script replay, and environment-variable
  set/delete tools subject to policy and confirmation checks.
- Stable Desktop builds show a one-time notice for anonymous active-install
  statistics and expose an opt-out under `Settings -> Privacy`; test/dev builds
  do not send telemetry. See [anonymous usage telemetry](../privacy/telemetry.md).
- Unfour's core desktop features are free and open source under Apache-2.0. An
  active Pro subscription unlocks Cloud Sync in the same application.
- SQLite, PostgreSQL, and MySQL database workflows are verified.
- Compatible MariaDB servers use the MySQL driver path where protocol and SQL
  behavior are compatible; independent MariaDB verification is not part of the
  published v0.9.0 release record.
- SSH Terminal workflows have completed release-level verification against a
  real SSH server.

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

The v0.9.0 GitHub browser sign-in, Desktop login,
`unfour://auth/callback`, and basic account state have been verified in the real
Desktop flow. Creem Test checkout, webhook, entitlement, and billing portal
have also been verified. Creem Production will be recorded after the first
successful real production transaction, webhook, entitlement, Desktop refresh,
Cloud Sync entitlement, and billing-portal flow; this pending record is not a
failed Production result and does not require repeating the Test validation.

Cloud Sync synchronizes the supported non-secret workspace data shown in the
app, including workspace settings, connection definitions, environments,
non-secret variables, API collections/folders/requests, and SSH tasks. Secrets,
SSH credentials and local key paths, database credentials and local file paths,
and history/runtime results remain local to each device.

Live multi-device Cloud Sync was verified in an earlier version. After Pro
capabilities were merged into the unified Unfour client, the v0.9.0 real
multi-device regression has not yet been recorded and remains `NOT VERIFIED`.
Single-device v0.9.0 coverage will be recorded during that same regression.

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

Codex and Cursor use their own coding tools to inspect, modify, and test code.
Unfour MCP supplies API, SSH/server, and database runtime evidence, including
re-checking the original symptom after a change in your selected environment.

1. Open the desktop app once. This creates `~/.unfour/unfour.sqlite`.
2. Open `Settings → MCP`.
3. Click `Configure Codex` or `Configure Cursor` for the client you want to
   connect.
4. Wait for configuration to complete.
5. Restart the corresponding client.

If one-click configuration is unavailable or you want to review the
configuration manually, see [docs/mcp/client-setup.md](../mcp/client-setup.md)
for the Manual / Advanced configuration steps.

Do not set `UNFOUR_MCP_STORAGE_MODE=ephemeral` for daily use. That mode is for
registry validation, CI, protocol smoke checks, and isolated tests; it uses an
empty in-memory workspace instead of the desktop app's saved data.

In the default `prod` environment, MCP is read-only. High-risk actions return
`CONFIRMATION_REQUIRED` and must be reviewed before Codex or Cursor retries
them.

That production policy is implemented, but its real v0.9.0 prod-workspace
verification remains `NOT VERIFIED`: permitted read-only calls, blocked writes,
`CONFIRMATION_REQUIRED`, `confirmation_text`/payload binding, and the confirmed
retry path still need to be exercised together. This does not change the
completed real Codex and Cursor client verification above.

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

The SSH Terminal supports saved SSH connections, credential references,
terminal sessions, PTY input/output, resize, search, redacted log export,
clipboard context actions, host-key trust, reconnect, SFTP remote files, and
serial command/upload/download tasks. These workflows have completed
release-level verification against a real SSH server.

Verify host-key fingerprints before trusting a new server. Review destructive
commands before running them, and use disposable data when testing SFTP or SSH
task workflows.

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

SQLite, PostgreSQL, and MySQL workflows are verified for the v0.9.0 release.
Compatible MariaDB servers use the MySQL driver path where protocol and SQL
behavior are compatible. This release record does not claim a separate
MariaDB-specific verification matrix. Use credential references for database
passwords, and review mutation SQL before confirming execution.

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
