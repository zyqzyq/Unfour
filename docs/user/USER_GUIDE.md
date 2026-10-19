# Unfour User Guide

This document is for people using the app. Implementation layout, MCP
boundaries, and release evidence are listed in
[Documentation Split](#documentation-split).

## Current Product

Unfour opens into a single workspace surface:

- Workspaces on the left
- Tool tabs across the top
- API client, SSH terminal, database, and Flow panels in the center
- Local-first storage by default

## Current Capabilities

This guide documents v0.11.0, released as Standard stable on 2026-10-10.
See the [Release Notes](../release/v0.11.0-release-notes.md). Version-scoped
results, including checks that remain `NOT VERIFIED`, are collected in
[release verification](../testing/release-verification.md#v0110-final-release-verification-record).
This guide does not restate those results. Unrun v0.11.0 native, manual, and
live-service checks remain `NOT VERIFIED`.

- The workspace shell is usable. API, SSH, and Database sidebars keep
  independent bounded widths and restore their own layout when switching
  modules or restarting the app.
- API debugging supports request editing, Send/cancel, response viewing,
  history, saved requests, collections, environments, import/export, request
  timeouts, multipart text/file form-data, and request scripts with test and
  console results. File selections are transient and must be reselected when
  reopening saved or historical multipart requests.
- SQL Editor runs a script as one batch on one physical connection, with
  whole-script confirmation, per-statement results, and Stop that waits for the
  current statement. Switching modules keeps unsaved editor drafts. Closing a
  Query tab with unsaved user SQL asks for confirmation. Saved SQL opens without
  replacing the current editor.
- API save and history show loading, error, retry, and in-dialog save-failure
  feedback. Repeat save submits stay blocked while a collection or folder create
  is pending.
- SSH Close All, Close Others, and close-to-left/right ask once and disconnect
  only the confirmed sessions.
- Truncated workspace names show in full on hover. Workspace safety-tier and
  variable-environment wording is distinct from delete, which states that local
  API, SSH, and Database resources are removed.
- The About page includes GitHub Discussions and issue links for feedback.
- Local MCP is available through the stdio `unfour-mcp` server. It uses the
  same command bus and the same saved API, SSH, database, and Flow data, so
  Codex and Cursor can reproduce issues, inspect logs and database state, and
  make fixes alongside the user. MCP can save, run, cancel, and query Flows.
  It does not generate a troubleshooting playbook. Calls follow workspace
  policy and confirmation, including per-call environment selection and
  saved-request script replay. See [Connect Codex or Cursor](#connect-codex-or-cursor-mcp).
- Stable Desktop builds show a one-time notice for anonymous active-install
  statistics and expose an opt-out under `Settings -> Privacy`; test/dev builds
  do not send telemetry. See [anonymous usage telemetry](../privacy/telemetry.md).
- Unfour's core desktop features are free and open source under Apache-2.0. An
  active Pro subscription unlocks Cloud Sync in the same application.
- SQLite, PostgreSQL, and MySQL are supported. Compatible MariaDB servers use
  the MySQL driver path. A separate MariaDB verification matrix is not part of
  the published record.

## Workspace Exchange and Local Backups

In the Workspace menu, choose import to create a new Workspace, or export for
the currently selected Workspace. Both actions are also available in the command
palette. Export includes variables/environments, API collections/folders/requests,
SSH/Database connection settings, SSH tasks, Saved SQL, and Flow definitions.

- **Share definitions:** recognized secrets are removed and device paths are
  omitted by default. Review scripts, SQL, and free text for hardcoded credentials
  before sharing. Fill missing credentials and local paths before using the copy.
- **Encrypted backup:** the entire export file is password-encrypted. Use at
  least 12 characters and keep the password separately; it cannot be recovered.
  You can include saved SSH/Database passwords, SSH key passphrases, secret
  variables, and API authentication values. Paths are included by default and can
  be omitted independently. Literal secrets in bodies, scripts, SQL, and Flow
  defaults remain subject to redaction even in an encrypted backup.

Import a plain or encrypted file, unlock it if needed, then review the name,
resource counts, exclusions, missing fields, and local paths. Paths can be edited
or replaced by prefix and optionally checked before confirming. Import creates
and activates an independent copy with fresh IDs and remapped references; it
does not merge into or overwrite an existing Workspace. Preview and cancellation
create no Workspace. Failed validation must leave no partial imported data.

Restored SSH/Database credentials receive new local OS credential-store references.
Supported API and variable secrets return to their existing local storage fields;
this does not add at-rest encryption to SQLite. Private-key files, SQLite database
files, and upload/download file contents are not embedded. Select multipart files
again and verify paths before connecting or running imported tasks. History,
activity, tabs/layout, host trust, and Cloud Sync bindings are excluded; the new
Workspace defaults to MCP disabled and is not automatically cloud-bound.
You can explicitly select MCP permissions in the import preview. After import,
Workspace security settings let you save environment type and MCP permissions
independently. Auto uses DEV full access, TEST guarded, and PROD read-only;
the UI shows effective permissions and warns about PROD protection overrides.

This is a local backup and transfer feature, separate from Pro Cloud Sync.
v0.11.0 does not support sensitive-data Cloud Sync. Flow definitions are part
of this exchange. Flow run history stays on the device and is not exported or
synchronized. See the [format and security boundaries](../architecture/workspace-bundle.md).

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
   sync status in `Settings → Cloud Sync`. First account activation on a device
   turns Global Sync on; an existing Off preference is left unchanged.

The v0.9.0 account, Creem, and Cloud Sync results, and the checks that remain
`NOT VERIFIED`, are recorded in
[release verification](../testing/release-verification.md). This guide does not
restate them. An unrecorded production or multi-device check stays
`NOT VERIFIED`.

Cloud Sync synchronizes the supported non-secret workspace data shown in the
app, including workspace settings, connection definitions, environments,
non-secret variables, API collections/folders/requests, and SSH tasks. Secrets,
SSH credentials and local key paths, database credentials and local file paths,
history, runtime results, and Flow definitions and run history remain local to
each device.

## Connect Codex or Cursor (MCP)

Unfour provides a local stdio MCP server for Codex and Cursor. It uses the same
command bus as the desktop app and reads the workspace data and saved
connections from the local storage. MCP can save, run, cancel, and query
Flows. It does not generate a troubleshooting playbook. Capability boundaries,
environment permissions, and confirmation are defined in the
[MCP overview](../mcp/overview.md).

Codex and Cursor use their own coding tools to inspect, modify, and test code.
Unfour MCP supplies API, SSH, database, and Flow runtime access, including
re-checking the original symptom after a change in your selected environment.

1. Open the desktop app once. This creates `~/.unfour/unfour.sqlite`.
2. Open `Settings → MCP`.
3. Click `Configure Codex` or `Configure Cursor` for the client you want to
   connect.
4. Wait for configuration to complete.
5. Restart the corresponding client.
6. Use `Settings → MCP → Example Prompt`, copy it, and paste it into that
   client with a description of the issue.

If one-click configuration is unavailable, or a Windows path needs the
`cmd.exe /c` wrapper, follow
[client setup](../mcp/client-setup.md). Do not set
`UNFOUR_MCP_STORAGE_MODE=ephemeral` for daily use.

In the default `prod` mapping, MCP is read-only except safe SSH diagnostics.
High-risk actions return `CONFIRMATION_REQUIRED` and must be reviewed before
retry. Real production-policy client checks remain `NOT VERIFIED`; see
[release verification](../testing/release-verification.md).

## Flow

1. Open a workspace that already contains the saved API requests, SSH tasks, or
   database connections the runbook should use.
2. Open `Flow` and choose `New Flow`.
3. Name the Flow. On the canvas, insert `API Request`, `SSH Task`,
   `Database Query`, `Condition`, `Wait`, or `Wait Until` nodes.
4. Choose `Save`.
5. Choose `Run`, review the variable environment and inputs, and confirm. A
   run may send requests, execute remote commands, and modify databases.
6. Open `Run history` to inspect a run. `Cancel run` stops later scheduling;
   completed remote effects are not rolled back.

Deleting a Flow keeps its run history and the referenced API, SSH, and database
resources. Exporting the Workspace includes Flow definitions and omits run
history. Flow is not part of Cloud Sync. Codex or Cursor can save, run, cancel,
and query the same Flows through MCP; they do not write a playbook for you.

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
serial command/upload/download tasks. Recorded SSH checks and the checks that
remain `NOT VERIFIED` are in
[release verification](../testing/release-verification.md).

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
9. Click `Run All`. A script runs as one batch on one connection. Use
   `Run Selected` to execute only highlighted SQL. Mutation SQL
   still requires confirmation for the executed script, and Stop waits for the
   current statement.
10. Review result rows, affected rows, and duration.

SQLite, PostgreSQL, and MySQL are supported. Compatible MariaDB servers use the
MySQL driver path. Recorded database checks, and the separate MariaDB matrix
that the published record does not claim, are in
[release verification](../testing/release-verification.md). Use credential
references for database passwords, and review mutation SQL before confirming
execution.

## Where The Code Lives

Package ownership, the command-bus path, and crate responsibilities are in
[project structure](../architecture/project-structure.md) and
[package boundaries](../architecture/package-boundaries.md). This guide does not
repeat that layout.

## Data And Privacy

The app is local-first. Workspace metadata is stored locally. High-value actions
such as writes, credential changes, external API sends, SSH session lifecycle
events, and MCP or Flow actions that write state, call external systems, or send
data keep redacted local activity records for troubleshooting. Routine reads and
UI layout changes are not treated as activity events. Use credential references
for SSH and database secrets where available; do not place long-lived secrets in
workspace environment variables or saved request bodies.

## Documentation Split

| Question | Document |
| --- | --- |
| How do I use the app? | This guide |
| What can MCP do, and what must be confirmed? | [MCP overview](../mcp/overview.md) |
| How do I connect an installed client? | [Client setup](../mcp/client-setup.md) |
| What does each tool accept? | [MCP tools](../mcp/tools.md) |
| How is the app built? | [Architecture](../architecture/project-structure.md) |
| Which release checks passed? | [Release verification](../testing/release-verification.md) |
