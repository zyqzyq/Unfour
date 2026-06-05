# Architecture

Unfour Workspace is a local-first Tauri 2 app built around a shared Workspace model. SSH, database, and API workflows reuse the same resource tree, tab system, credential boundary, local activity trail, and future sync model.

## Runtime Split

- React frontend owns layout, tabs, forms, editor surfaces, terminal rendering, table rendering, and user feedback.
- Rust backend owns filesystem access, SQLite, credential references, HTTP execution, future SSH/database connections, logs, and policy checks.
- Tauri IPC is an adapter layer. It should remain thin.

## Command Flow

```text
React UI
  -> Tauri command
  -> CommandBus
  -> Service
  -> Adapter / Driver
```

The same `CommandBus` shape is reserved for later AI, MCP, CLI, workflow runner, and cloud automation adapters.

## Current Modules

- `command_bus`: single backend entrypoint for app actions.
- `workspace`: workspace CRUD, active workspace state, and workspace-scoped environment variables.
- `api_client`: HTTP/HTTPS execution, workspace variable resolution, saved requests, request history.
- `local_db`: SQLite connection and migrations.
- `activity_log`: append-only local activity trail with redacted summaries for high-value actions.
- `ssh`: reserved boundary for `russh` sessions and event streaming. The dependency is available behind the `ssh-native` feature and uses the `ring` backend to avoid NASM on Windows.
- `database`: `sqlx`-backed database connection metadata, SQLite connection tests, SQLite schema browsing, and SQLite SQL execution. PostgreSQL/MySQL live connections remain behind the credential boundary for the next phase.
- `secret_store`: reserved boundary for OS keychain/Stronghold credential refs.
- `ai_reserved`: command/capability types for future AI invocation.
- `sync_reserved`: local-first sync metadata policy.

## Current Progress

- Foundation and API MVP are implemented enough to validate the Workspace, Command Bus, local SQLite, history, saved request, local activity, and environment variable model.
- Database MVP has moved beyond a preview: SQLite connection metadata, connection testing, schema browsing, and SQL execution are implemented. PostgreSQL/MySQL live connections are still reserved until credential storage is implemented.
- SSH is still a preview surface with `xterm` rendering and a reserved Rust service boundary. Real auth, PTY, streaming, resize, and session cleanup remain.
- AI and cloud sync are intentionally not user-facing yet. The current work only preserves their data fields and command boundaries.

## Frontend Shape

- `apps/desktop`: Tauri/Vite desktop application entry. It mounts the composed workspace shell.
- `packages/app-shell`: workspace shell, title bar, sidebars, tabs, and current panel composition.
- `packages/command-client`: typed Tauri command adapter, shared frontend command types, and browser-dev mocks.
- `packages/workspace`: UI state for active workspace/tab/sidebar.
- `packages/ui`: shared shadcn-style primitives and styling helpers.
- `packages/api-debugger`, `packages/database`, and `packages/terminal`: feature-specific frontend logic that should not depend on each other directly.

## Workspace Shape

- Root `package.json` orchestrates workspace commands through pnpm filters.
- `pnpm-workspace.yaml` includes `apps/*` and `packages/*`.
- Root `Cargo.toml` defines a Cargo workspace for `apps/desktop/src-tauri` and `crates/*`.
- `apps/desktop/src-tauri` is the Tauri adapter and composition layer.
- `crates/unfour-core` owns shared models, errors, redaction, sync policy, and reserved AI contracts.
- `crates/local-storage` owns SQLite setup and local activity logging.
- `crates/http-engine`, `crates/database-engine`, `crates/ssh-engine`, `crates/workspace-engine`, and `crates/secret-store` own backend capability modules.

## Design Constraint

Do not create independent "API app", "SSH app", and "Database app" islands. New features should plug into Workspace resources, tabs, history, and credential references.
