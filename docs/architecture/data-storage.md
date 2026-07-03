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

## Runtime Path Strategy

SQLite runtime paths are resolved by `crates/unfour-paths`, not by Tauri path
APIs. The stable product data directory is named `Unfour` so the desktop app
and standalone MCP process share the same local SQLite file, while the Tauri
identifier `dev.unfour` remains only the bundle/app identifier.

The current stable SQLite path strategy is:

- Windows: `%APPDATA%\Unfour\unfour.sqlite`.
- macOS/Linux: `dirs::data_dir()/Unfour/unfour.sqlite`.

Do not replace this with Tauri `app_data_dir()`: Tauri derives that path from
`identifier = "dev.unfour"`, which would split data into a different
`dev.unfour` directory. `dev.unfour` is not treated as a legacy data directory
by the runtime path resolver.

Config and cache directories are also resolved by `unfour-paths`:

- config: `dirs::config_dir()/Unfour`, falling back to
  `<product_data_dir>/config`;
- cache: `dirs::cache_dir()/Unfour`, falling back to
  `<product_data_dir>/cache`;
- backups: `<product_data_dir>/backups`.

Unfour has not introduced a file logging module. Runtime path governance does
not currently include a logs directory or `tauri-plugin-log`.

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

Tables that carry these fields today: `workspaces`, `workspace_settings`,
`api_requests`, `api_collections`, `api_collection_folders`, `api_environments`,
`connections`, `saved_sql`.

Two local-only log tables intentionally do **not** carry sync fields, because
they are append-only local trails that are never synced:

- `api_history` — request log (mirrors `db_query_history`).
- `db_query_history` — SQL execution log.

`api_collection_folders` and `saved_sql` carry sync metadata in the initial
schema. `saved_sql` uses soft delete fields so saved snippets can be retained
without active-list visibility.

Cloud sync is not part of the v0.1 public release readiness criteria. Future
sync behavior must remain workspace-scoped and must not overwrite secrets
automatically.

## Connection Subtype Tables

`connections` is the parent row for a workspace-scoped connection. It holds
identity and sync metadata (`id`, `workspace_id`, `name`, `credential_ref`,
timestamps, sync fields), while kind-specific configuration lives in subtype
tables:

- `ssh_connections(connection_id, config_json)` — 1:1 with `connections.id`,
  `ON DELETE CASCADE`.
- `database_connections(connection_id, config_json)` — 1:1 with
  `connections.id`, `ON DELETE CASCADE`.

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
