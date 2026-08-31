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

Cloud Sync preserves this local-first model. Local SQLite
remains the runtime source of truth; cloud behavior should be implemented as a
periodic sync overlay that reconciles local workspace data, not as a
cloud-primary workspace provider that replaces local storage during normal app
use.

## Runtime Path Strategy

SQLite runtime paths are resolved by `crates/unfour-paths`, not by Tauri path
APIs. The desktop app and standalone MCP process share the same resolver so they
open one predictable local SQLite file. The Tauri identifier `dev.unfour`
remains only the bundle/app identifier.

Do not replace this with Tauri `app_data_dir()`: Tauri derives that path from
`identifier = "dev.unfour"`, which would split data into a different
`dev.unfour` directory. `dev.unfour` is not treated as a legacy data directory
by the runtime path resolver.

### Storage profiles

`unfour-paths` selects a product data root by storage profile. Profiles use
**sibling** directories under the user home — never nested children such as
`~/.unfour/dev`:

| Profile | Product data root | SQLite |
| --- | --- | --- |
| `stable` | `~/.unfour` | `~/.unfour/unfour.sqlite` |
| `test` | `~/.unfour-test` | `~/.unfour-test/unfour.sqlite` |
| `dev` | `~/.unfour-dev` | `~/.unfour-dev/unfour.sqlite` |

Stable keeps the historical layout with zero migration. The database file name
remains `unfour.sqlite` for every profile. Logs, backups, diagnostics, config,
and cache all follow the selected product root.

Under product root `<root>`:

- SQLite: `<root>/unfour.sqlite`
- backups: `<root>/backups`
- logs: `<root>/logs`
- diagnostics: `<root>/diagnostics`
- config / cache: follow `<root>` (same product-data tree as today's stable
  layout)

### Release channel and storage profile

Release identity and local data isolation are separate axes:

- `UNFOUR_RELEASE_CHANNEL` is a build-time input and accepts only `test` or
  `stable`. It controls Unfour release metadata and supplies the default
  storage profile for that compiled artifact. It never accepts `dev`.
- `UNFOUR_STORAGE_PROFILE` is a runtime local-data override and accepts only
  `dev`, `test`, or `stable`. It does not change the product, distribution
  type, keychain namespace, package identity, or any service address.
- `UNFOUR_DATA_DIR` is the highest-priority complete product-tree override and
  must be absolute.

All non-empty values are validated. Invalid values return an error instead of
falling back to another profile.

### Resolution priority

`initialize_unfour_storage()`, `resolve_unfour_paths()`,
`default_database_path()`, and storage diagnostics all use the same resolver:

1. `UNFOUR_DATA_DIR` — absolute path that replaces the entire product tree
   (CI / sandboxes). Relative values are rejected.
2. `UNFOUR_STORAGE_PROFILE` — runtime `dev` | `test` | `stable`.
3. Compile-time `UNFOUR_RELEASE_CHANNEL` (`stable` → stable, `test` → test).

The root Tauri launcher always exports a channel to the complete child process
and Cargo graph. Local `pnpm tauri dev` defaults to Test, while
`pnpm tauri build` defaults to Stable. Use `pnpm tauri build:test` for an
isolated Test-channel bundle. Formal Standard Release CI explicitly sets
`UNFOUR_RELEASE_CHANNEL=stable` and the exact build commit. Direct Cargo/Tauri
invocations without the variable emit a warning and consistently compile as
Test.

Common commands:

```bash
pnpm tauri dev
UNFOUR_STORAGE_PROFILE=dev pnpm tauri dev
pnpm tauri build
pnpm tauri build:test
UNFOUR_RELEASE_CHANNEL=stable UNFOUR_BUILD_COMMIT=<exact-sha> pnpm tauri build
```

The last command describes the formal CI build contract. A local Stable build
uses the Stable data root but is not a verified or publishable release artifact.

Callers (desktop, MCP, command-bus) keep using the existing public path APIs;
they do not need separate profile arguments.

### Explicit non-goals

Storage profiles isolate the local product data tree only. They do **not**
change:

- OS keychain / `SECRET_STORE_NAMESPACE` (service name stays `unfour`);
- product, distribution type, or package identity;
- service or updater endpoints.

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
- workspace-local active environment state;
- workspace variables, environments, and normalized environment variables;
- API requests;
- API history (local-only log, no sync fields);
- connection metadata (parent `connections` table plus `ssh_connections` /
  `database_connections` subtype tables);
- workspace-scoped SSH host-key trust records;
- terminal session-output history;
- SSH command history (user-executed commands only, persisted after remote
  echo, with conservative secret redaction; multiline bracketed pastes are
  skipped because their embedded lines are buffered by the remote line editor
  rather than individually executed);
- saved SQL (soft-deleted, sync fields reserved);
- local activity events.

Core schema changes live in `crates/local-storage/migrations/`; Cloud Sync
schema changes live in `crates/unfour-cloud-sync-storage/migrations/`. The
single runtime applies both through `unfour_cloud_sync_storage::migrate` and
shares sqlx's default `_sqlx_migrations` table, so migration versions must be
globally unique. sqlx parses the digits
before the first `_` as the version; `0001_core_init.sql` and
`0001_pro_init.sql` both become version `1` and collide.

All new migration files must use a UTC timestamp version plus the unified
`core` marker inside the description:

```text
YYYYMMDDHHMMSS_core_description.sql
```

The Cloud Sync directory still contains immutable historical `_pro_`
migrations. Their names and checksums are compatibility data and must not be
rewritten; new Cloud Sync migrations use `_core_`.

The version must be pure digits before the first `_`. Do not add local
`0001_xxx.sql` / `0002_xxx.sql` migrations, and do not put the marker first
as `core_YYYYMMDDHHMMSS_xxx.sql` because sqlx would parse `core` as the
version. Both embedded migrators behind the unified entry point must enable
sqlx `set_ignore_missing(true)` so each migration set can ignore the other's
records in `_sqlx_migrations`. This only handles missing/unknown records; it
does not permit changing the checksum of an already applied migration.

Do not rename, delete, or edit the content of already-published migrations.
If a published schema needs correcting, add a new compatible migration instead.
Before adding or reviewing migrations, run `pnpm run check:migrations`. For a
nonstandard Cloud Sync migration fixture, set
`UNFOUR_CLOUD_SYNC_MIGRATIONS_DIR` to its migrations directory.

`crates/local-storage` owns the base schema;
`crates/unfour-cloud-sync-storage` owns independent `cloud_sync_` tables for
account binding, sync state, remote IDs, outbox, and conflict metadata. Avoid
adding Cloud Sync-only columns to base tables unless there is a strong reason
and local-only paths can safely ignore the change. Persistence code belongs in
those storage crates or the owning engine crate, not in frontend packages or
Tauri command adapters.

## Syncable Business Records

Syncable business records have stable local identity and workspace scope so
the optional Cloud Sync overlay can operate transactionally:

- all syncable business records should have a stable `id`;
- all syncable business records should have `workspace_id`;
- important syncable records should have `created_at`;
- important syncable records should have `updated_at`;
- records whose deletion must propagate across devices should have nullable
  `deleted_at` instead of only hard-delete behavior.

Current local tables already reserve some forward-compatible fields such as
`revision`, `sync_status`, or `remote_id`. Those fields are not a requirement
for every local runtime table. Future sync metadata can remain in
Cloud Sync-owned metadata tables unless the local runtime directly needs the
field:

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
- `workspace_variables`
- `workspace_environments`
- `workspace_environment_variables`
- `saved_sql`

Data that can remain local-only for now:

- `api_history`
- `db_query_history`
- `ssh_terminal_history`
- `ssh_command_history` (local-only, 200 rows per connection, echoed commands only)
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

## Active Environment State

The current environment is local workspace state in `workspace_local_state`,
not a property of a syncable environment record. Its
`active_environment_id` is validated against `workspace_environments` in the
same workspace. Deleting the active environment falls back to the first
available environment by sort order, or to no environment when none remain.

## Workspace Business Data And Local Usage State

The syncable Workspace business fields are `id`, `name`, `environment_type`,
`mcp_policy`, `created_at`, `updated_at`, deletion state, and the business
`revision`. Workspace domain snapshots and external apply payloads contain
only these fields.

The following values are device-local usage state and are never part of the
Workspace sync protocol:

- `active_workspace_id` in `app_settings`;
- `active_environment_id` in `workspace_local_state`;
- `last_opened_at` on the local Workspace row;
- `is_default` on the local Workspace row.

Activating a Workspace updates `active_workspace_id` and `last_opened_at`
locally. It does not change `updated_at` or `revision`, produce a
`DomainMutation`, or invoke transactional mutation hooks. Setting the
default Workspace likewise changes only the local preference. External
Workspace apply preserves these local fields and does not select a different
active Workspace when applying an upsert. Deleting the active Workspace may
still choose a local fallback so the runtime never points at a deleted row.

## Default Workspace

`workspace-engine` seeds the first default workspace during command-bus startup
when none exists. Workspaces created by users are inserted with
`is_default = 0`; the schema validates `is_default` as a boolean and does not
attempt to manage multiple default rows. `is_default` is a device-local
preference, so Cloud Sync Workspace upserts do not read or write it.

## Workspace Environments

Workspace variables and environments are ordinary workspace data. Environment
variables override workspace variables with the same key. API requests resolve
placeholders such as `{{base_url}}` in URL, auth metadata, headers, query
parameters, and body before sending. Resolution is workspace-scoped and an
environment ID is rejected unless it belongs to the supplied workspace.

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
