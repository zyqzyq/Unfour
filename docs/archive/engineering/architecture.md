# Architecture

Unfour is a local-first Tauri 2 app built around a shared Workspace model. SSH, database, and API workflows reuse the same resource tree, tab system, credential boundary, local activity trail, and future sync model.

## Runtime Split

- React frontend owns layout, tabs, forms, editor surfaces, terminal rendering, table rendering, and user feedback.
- Rust backend owns filesystem access, SQLite, credential references, HTTP
  execution, SSH/database connections, logs, and policy checks.
- Tauri IPC is an adapter layer. It should remain thin.

## Command Flow

```text
React UI
  -> Tauri command
  -> CommandBus
  -> Service
  -> Adapter / Driver
```

The same `CommandBus` shape backs the shipped MCP adapter (`crates/unfour-mcp`) and is reserved for later AI, CLI, workflow runner, and cloud automation adapters.

## Current Modules

- `command_bus`: single backend entrypoint for app actions.
- `workspace`: workspace CRUD, active workspace state, and workspace-scoped environment variables.
- `api_client`: HTTP/HTTPS execution, workspace variable resolution, saved requests, request history.
- `local_db`: SQLite connection and migrations.
- `activity_log`: append-only local activity trail with redacted summaries for high-value actions.
- `ssh`: SSH sessions, host-key handling, reconnect behavior, terminal event
  streaming, and log export. Live SSH server verification remains a release
  gate.
- `database`: `sqlx`-backed database connection metadata, SQLite/PostgreSQL/MySQL
  connection tests, schema browsing, SQL execution, and table browsing.
- `secret_store`: OS keychain-backed credential references in production and
  in-memory credentials for tests.
- `ai_reserved`: command/capability types for future AI invocation.
- `sync_reserved`: local-first sync metadata policy.

## Current Progress

- Foundation and API MVP are implemented enough to validate the Workspace, Command Bus, local SQLite, history, saved request, local activity, and environment variable model.
- Database MVP has moved beyond a preview: SQLite/PostgreSQL/MySQL connection,
  schema, SQL execution, and table-browse paths are implemented. Live database
  behavior should be re-verified when those paths change.
- SSH has connection metadata, password/private-key session paths, PTY
  streaming, resize, close, redacted log export, host-key trust, and reconnect
  behavior. The full live SSH server journey remains experimental until
  release-gate verification is completed.
- A local MCP server (`crates/unfour-mcp`) now exposes read-only tools to AI clients; in-app AI and cloud sync remain not user-facing yet, preserving their data fields and command boundaries.

## Frontend Shape

- `apps/desktop`: Tauri/Vite desktop application entry. It mounts the composed workspace shell.
- `packages/app-shell`: thin shell slot wrapper. `apps/desktop` owns most
  desktop composition while the module split is still in progress.
- `packages/command-client`: typed Tauri command adapter, shared frontend command types, and browser-dev mocks.
- `packages/workspace-core`: UI state for active workspace/tab/sidebar.
- `packages/workspace-local`: compatibility boundary for future local workspace persistence, import/export, recent-workspace, and migration implementations.
- `packages/ui`: shared shadcn-style primitives and styling helpers.
- `packages/api-client`, `packages/database`, and `packages/ssh-terminal`: feature-specific frontend logic that should not depend on each other directly.

## Workspace Shape

- Root `package.json` orchestrates workspace commands through pnpm filters.
- `pnpm-workspace.yaml` includes `apps/*` and `packages/*`.
- Root `Cargo.toml` defines a Cargo workspace for `apps/desktop/src-tauri` and `crates/*`.
- `apps/desktop/src-tauri` is the Tauri adapter and composition layer.
- `crates/unfour-core` owns shared models, errors, redaction, sync policy, and reserved AI contracts.
- `crates/local-storage` owns SQLite setup and local activity logging.
- `crates/unfour-command-bus` owns the reusable command entry point for Tauri,
  MCP, and future adapters.
- `crates/http-engine`, `crates/database-engine`, `crates/ssh-engine`,
  `crates/workspace-engine`, and `crates/secret-store` own backend capability
  modules.
- `crates/unfour-mcp` owns the local stdio MCP server and routes real tools
  through the command bus.

## Design Constraint

Do not create independent "API app", "SSH app", and "Database app" islands. New features should plug into Workspace resources, tabs, history, and credential references.
