# Data Storage

Unfour is local-first. The active source of truth is local SQLite plus OS
credential storage.

## Workspace Scope

Workspace is the top-level product boundary. A workspace owns:

- API saved requests and request history.
- SSH and database connection metadata.
- Workspace environment variables.
- Layout and tab restore state.
- Local activity events.
- Future sync metadata.

Every persisted business record must carry `workspace_id` unless it is truly
global application configuration.

## SQLite Storage

The desktop app stores local data in the Tauri app data directory. The current
SQLite-backed records include:

- app settings;
- workspaces;
- workspace settings;
- API requests;
- API history;
- connection metadata;
- SSH host-key trust records;
- terminal history;
- saved SQL;
- local activity events.

Schema changes live in `crates/local-storage/migrations/`. Persistence code
belongs in `crates/local-storage` or the owning engine crate, not in frontend
packages or Tauri command adapters.

## Reserved Sync Fields

Core local records reserve sync-related fields from the beginning:

- `id`
- `workspace_id`
- `created_at`
- `updated_at`
- `deleted_at`
- `revision`
- `sync_status`
- `remote_id`

Cloud sync is not part of the v0.1 public release readiness criteria. Future
sync behavior must remain workspace-scoped and must not overwrite secrets
automatically.

## Workspace Environments

Workspace environments are ordinary workspace data. API requests resolve
placeholders such as `{{base_url}}` in URL, headers, query parameters, and body
before sending.

Environment values are not encrypted. Do not store long-lived secrets in
workspace environment variables. Use credential references for passwords,
private-key passphrases, database passwords, and API tokens when a feature
supports them.

## Credential Boundary

SQLite records may store `credential_ref`, but must never store raw secret
material such as passwords, API tokens, or SSH private-key passphrases.

`crates/secret-store` is the credential boundary:

- production builds use OS keychain backends;
- tests can use an in-memory backend;
- metadata commands may return credential references and labels, but not raw
  secret values.

## Local Activity

`activity_events` is a local troubleshooting and safety trail. It is not an
enterprise audit log.

Record redacted summaries for:

- workspace, environment, saved request, connection, credential, and SSH
  session lifecycle writes;
- external API sends;
- database SQL that requires confirmation;
- future AI/workflow actions that write local state, execute external side
  effects, or send local data outside the app.

Do not record routine reads, UI layout noise, terminal resize events, request
bodies, response bodies, SQL result rows, passwords, tokens, private-key
passphrases, or raw AI prompts/responses in activity details.

## Concurrency

The desktop app and standalone MCP server can open the same local database.
Database access should use busy-timeout behavior to avoid avoidable
`database is locked` failures under normal contention.
