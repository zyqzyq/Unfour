# Workspace Model

Workspace is the top-level product boundary.

## Scope

A workspace owns:

- API saved requests and history
- SSH/database connection metadata
- Environment variables
- Layout and tab restore state
- Local activity events
- Future sync state

## Persistence

The app uses local SQLite in the Tauri app data directory. Core records reserve sync fields from day one:

- `id`
- `workspace_id`
- `created_at`
- `updated_at`
- `deleted_at`
- `revision`
- `sync_status`
- `remote_id`

Global records, such as `app_settings`, do not require `workspace_id`.

## Current Tables

- `app_settings`
- `workspaces`
- `workspace_settings`
- `api_requests`
- `api_history`
- `connections`
- `activity_events` for local activity summaries

## Environments

Workspace environments are stored in `workspace_settings.env_json` as an array of enabled key/value pairs. API requests resolve placeholders such as `{{base_url}}` in URL, headers, query parameters, and body before sending.

Environment values are ordinary workspace data, not secret storage. Do not put
long-lived secrets in workspace environment variables. Connection passwords and
SSH private-key passphrases should be stored through credential references where
the feature supports them.

## Credential Boundary

Connection tables may store `credential_ref`, but not secret material.
`SecretStore` is the credential boundary: production builds use OS keychain
backends, while tests can use an in-memory backend.

## Sync Posture

The current source of truth is local SQLite. Cloud sync will be optional and workspace-scoped. Initial conflict policy is "keep both versions"; no secret should be overwritten automatically.
