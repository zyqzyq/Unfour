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

Future Pro cloud support must preserve this local-first model. Local SQLite
remains the runtime source of truth; cloud behavior should be implemented as a
periodic sync overlay that reconciles local workspace data, not as a
cloud-primary workspace provider that replaces local storage during normal app
use.

## Runtime Path Strategy

SQLite runtime paths are resolved by `crates/unfour-paths`, not by Tauri path
APIs. All Unfour data lives under `~/.unfour` on every platform so the desktop
app and standalone MCP process share the same local SQLite file at a stable,
predictable location, while the Tauri identifier `dev.unfour` remains only the
bundle/app identifier.

The current stable SQLite path is `~/.unfour/unfour.sqlite` on all platforms.

Do not replace this with Tauri `app_data_dir()`: Tauri derives that path from
`identifier = "dev.unfour"`, which would split data into a different
`dev.unfour` directory. `dev.unfour` is not treated as a legacy data directory
by the runtime path resolver.

Config and cache directories are also resolved by `unfour-paths` and all live
under `~/.unfour`:

- config: `~/.unfour/config`;
- cache: `~/.unfour/cache`;
- backups: `~/.unfour/backups`;
- logs: `~/.unfour/logs`;
- diagnostics: `~/.unfour/diagnostics`.

Runtime diagnostics are owned by `crates/unfour-diag`, not by
`tauri-plugin-log`. File logs use daily `unfour.log*` files under the logs
directory with a default 7-day retention window. Diagnostic bundles are written
under the diagnostics directory and may copy recent log files plus a manifest,
but must not copy the SQLite database or raw credential material. See
`docs/architecture/diagnostics.md`.

## SQLite Storage

The current SQLite-backed records include:

- app settings;
- workspaces;
- workspace settings;
- API requests;
- API history (local-only log, no sync fields);
- connection metadata (parent `connections` table plus `ssh_connections` /
  `database_connections` subtype tables);
- workspace-scoped SSH host-key trust records;
- terminal history;
- saved SQL (soft-deleted, sync fields reserved);
- local activity events.

Schema changes live in `crates/local-storage/migrations/`. Because v0.1 has
not shipped, the historical pre-release migrations are squashed into
`0001_initial_schema.sql`; future schema changes should add new numbered
migration files after that. Persistence code belongs in `crates/local-storage`
or the owning engine crate, not in frontend packages or Tauri command
adapters.

## Syncable Business Records

Syncable business records should have stable local identity and workspace
scope before any Pro sync layer is added:

- all syncable business records should have a stable `id`;
- all syncable business records should have `workspace_id`;
- important syncable records should have `created_at`;
- important syncable records should have `updated_at`;
- records whose deletion must propagate across devices should have nullable
  `deleted_at` instead of only hard-delete behavior.

Current local tables already reserve some forward-compatible fields such as
`revision`, `sync_status`, or `remote_id`. Those fields are not a requirement
for every OSS runtime table. Future sync metadata can be deferred to sync
metadata tables or a Pro-owned sync layer unless the OSS runtime directly needs
the field:

- `remote_id`
- `sync_version`
- `last_synced_at`
- `sync_status`
- `device_id` / `origin_device_id`

The first good candidates for future sync are durable workspace business data:

- `workspaces`
- `workspace_settings`
- `connections`
- `ssh_connections`
- `database_connections`
- `api_collections`
- `api_collection_folders`
- `api_requests`
- `api_environments`
- `saved_sql`

Data that can remain local-only for now:

- `api_history`
- `db_query_history`
- `ssh_terminal_history`
- `activity_events`
- diagnostics logs
- cache
- temporary runtime state

Future sync behavior must remain workspace-scoped and must not overwrite
secrets automatically.

## Connection Subtype Tables

`connections` is the parent row for a workspace-scoped connection. It holds
shared lifecycle and display metadata (`id`, `workspace_id`,
`connection_type`, `name`, `host`, `port`, `credential_ref`, timestamps,
`last_connected_at`, sync fields). Kind-specific core metadata lives in subtype
tables, while `config_json` is reserved for advanced or driver-specific
metadata:

- `ssh_connections(connection_id, username, auth_method, config_json)` — 1:1
  with `connections.id`, `ON DELETE CASCADE`. `config_json` stores advanced
  SSH metadata such as private-key path or future terminal/tunnel options, not
  passwords or passphrases.
- `database_connections(connection_id, driver, database_name, username,
  ssl_mode, read_only, config_json)` — 1:1 with `connections.id`,
  `ON DELETE CASCADE`. `config_json` stores advanced database metadata such as
  SQLite path, optional timeouts, default schema, or driver-specific options,
  not database passwords or credential material.

Engine services JOIN the parent with their subtype table on read and write
both rows on insert/update. `credential_ref` stays on the parent because it
is shared identity metadata. Tables that reference a connection by id
(`saved_sql.connection_id`, `db_query_history.connection_id`,
`ssh_terminal_history.connection_id`) point at the parent `connections.id`
and do not need to know which subtype the row belongs to. The schema enforces
same-workspace connection references; deleting a connection nulls nullable
history/snippet references and cascades terminal history rows.

## Single Active Environment

`api_environments.is_active` is constrained to a single active row per
workspace by the partial unique index
`uq_api_environments_active_per_workspace`. The application layer in
`http-engine` still wraps activate/deactivate in one transaction, but the
index is the source of truth: it enforces uniqueness at the statement level,
so the activate flow must deactivate other rows **before** activating the
target.

## Default Workspace

`workspace-engine` seeds the first default workspace during command-bus startup
when none exists. Workspaces created by users are inserted with
`is_default = 0`; the schema validates `is_default` as a boolean and does not
attempt to manage multiple default rows.

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

The keychain service name is currently `unfour`, and credential references use
the format `unfour:<workspace_id>:<kind>:<record_uuid>`. Keep that service name
stable across desktop, MCP, and packaging channels unless a migration plan
preserves access to existing credentials.

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
