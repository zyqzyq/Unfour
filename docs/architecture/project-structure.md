# Project Structure

This document maps the active repository structure, package responsibilities,
crate responsibilities, and frontend-to-backend call chain.

## Top-Level Layout

```text
Unfour/
  apps/
    desktop/                 thin Tauri/Vite desktop binary and entrypoint
  packages/
    api-client/              API Client frontend module
    app-shell/               global shell composition and mount slots
    command-client/          typed Tauri command wrappers and shared TS types
    database/                Database frontend module
    ssh-terminal/            SSH Terminal frontend module
    ui/                      shared UI primitives and layout helpers
    workspace-core/          shared frontend workspace state
    workspace-local/         reserved local workspace lifecycle boundary
  crates/
    unfour-core/             shared Rust models, errors, redaction helpers
    unfour-paths/            shared runtime path resolution
    unfour-diag/             structured logging and diagnostic bundles
    local-storage/           SQLite persistence and activity log
    http-engine/             API request execution and API persistence
    database-engine/         database connection, schema, and query service
    ssh-engine/              SSH connection and terminal session service
    workspace-engine/        workspace CRUD, environments, layout persistence
    secret-store/            OS keychain credential boundary
    unfour-app/              shared Tauri composition layer
    unfour-command-bus/      reusable Rust command entry point
    unfour-mcp/              local stdio MCP server adapter
  docs/
    agents/                  AI agent onboarding and execution protocol
    architecture/            package, storage, security, and structure docs
    mcp/                     MCP overview, tools, and Codex setup
    testing/                 release verification and manual test cases
    release/                 release checklist, distribution, signing
    ui/                      active design-system and interaction docs
    user/                    user-facing guide
    archive/                 historical progress, checkpoint, and task docs
```

## Frontend Packages

| Package | Role |
| --- | --- |
| `@unfour/app-shell` | Frontend desktop workbench composition root. Owns global shell wiring, workspace switcher, module navigation, layout slots, command palette, diagnostics actions, and mounts the API Client, SSH Terminal, and Database modules without owning their internal feature logic. |
| `@unfour/ui` | Shared UI primitives, shell helpers, states, menus, tabs, tree, data table, dialogs, and styling utilities. |
| `@unfour/command-client` | Typed Tauri `invoke` wrappers, shared frontend command types, and browser-development mock fallback. |
| `@unfour/workspace-core` | Zustand workspace store and workspace type re-exports. |
| `@unfour/workspace-local` | OSS local workspace lifecycle boundary; currently a compatibility/transitional package reserved for recent workspace, import/export, persistence lifecycle, and migration behavior. |
| `@unfour/api-client` | API Client feature UI: requests, tabs, Send, responses, history, saved requests, collections, environments, import/export. |
| `@unfour/database` | Database feature UI: connections, schema tree, SQL editor, query results, table preview, query history. |
| `@unfour/ssh-terminal` | SSH Terminal feature UI: connections, sessions, xterm panes, split/search/logs, host-key trust. |
| `@unfour/desktop` | Thin desktop frontend entrypoint that mounts `@unfour/app-shell`. |

## Rust Crates

| Crate | Role |
| --- | --- |
| `unfour-core` | Foundation crate for shared models, `AppError`, redaction, and reserved AI/sync contracts. |
| `unfour-paths` | Stable runtime path resolution shared by the desktop app and standalone MCP server. |
| `unfour-diag` | Structured logging, log retention, correlation IDs, and diagnostic bundle export. |
| `unfour-local-storage` | SQLite setup, migrations, local persistence, and activity log. |
| `unfour-secret-store` | Credential reference service backed by OS keychain in production and in-memory storage in tests. |
| `unfour-http-engine` | API execution, environment resolution, saved requests, history, and persistence redaction. |
| `unfour-database-engine` | Database connection CRUD, schema browsing, SQL execution, table browsing, and SQL safety classification. |
| `unfour-ssh-engine` | SSH connection/session lifecycle, PTY events, host-key trust, reconnect behavior, and redacted log export. |
| `unfour-workspace-engine` | Workspace CRUD, active workspace state, environments, and layout persistence. |
| `unfour-app` | Shared Tauri composition layer for plugins, command-bus setup, managed `AppState`, commands, and edition-independent wiring. |
| `unfour-command-bus` | Shared command entry point used by Tauri, MCP, and future adapters. |
| `unfour-mcp` | Local stdio MCP server that routes tools through the command bus. |
| `unfour` | Thin Tauri desktop binary and edition adapter in `apps/desktop/src-tauri`. |

## Frontend Dependency Shape

```text
@unfour/ui                 no @unfour package dependencies
@unfour/command-client     no feature dependencies

@unfour/app-shell          -> api-client, database, ssh-terminal,
                              command-client, workspace-core, ui
@unfour/workspace-core     -> command-client
@unfour/workspace-local    -> workspace-core

@unfour/api-client         -> command-client, ui
@unfour/database           -> command-client, ui, workspace-core
@unfour/ssh-terminal       -> command-client, ui, workspace-core

@unfour/desktop            -> app-shell
```

Feature packages must not depend on each other, on `packages/app-shell`, on
`packages/workspace-local`, or on any future Pro sync package. App-shell and
edition composition layers choose local or Pro sync capabilities; feature
packages consume only workspace contracts from `workspace-core`.

## Rust Dependency Shape

```text
unfour-core
unfour-paths

unfour-diag -> unfour-core, unfour-paths

unfour-local-storage -> unfour-core, unfour-diag
unfour-secret-store -> unfour-core, unfour-diag
unfour-http-engine -> unfour-core, unfour-local-storage, unfour-diag
unfour-database-engine -> unfour-core, unfour-local-storage, unfour-diag
unfour-ssh-engine -> unfour-core, unfour-local-storage, unfour-diag
unfour-workspace-engine -> unfour-core, unfour-local-storage

unfour-command-bus
  -> unfour-core, unfour-diag, unfour-local-storage, unfour-secret-store
  -> http, database, ssh, workspace engines

unfour-app
  -> unfour-command-bus, unfour-core, unfour-diag, unfour-local-storage,
     unfour-paths, unfour-secret-store

unfour-mcp
  -> unfour-command-bus, unfour-core, unfour-diag, unfour-paths

unfour Tauri binary
  -> unfour-app
```

## Frontend-To-Rust Call Chain

```text
React component
  -> feature hook or action
  -> @unfour/command-client function
  -> Tauri invoke(command, args) in desktop runtime
  -> browser mock fallback in Vite/browser development
  -> Rust #[tauri::command] adapter in crates/unfour-app
  -> CommandBus method
  -> service
  -> driver or local store
```

MCP tools use the same backend path starting at the MCP adapter:

```text
MCP client
  -> unfour-mcp stdio server
  -> tool handler
  -> command-bus adapter
  -> CommandBus method
  -> service
  -> driver or local store
```

## Tauri Configuration Snapshot

The desktop binary wrapper lives under `apps/desktop/src-tauri`. Shared Tauri
composition lives in `crates/unfour-app`: plugins, command-bus setup,
`AppState`, command adapters, and edition-independent wiring. The product name
is Unfour and the repository package version is `0.1.0`. Release readiness
must be determined from the release verification documents, not from the
version string alone.

See also:

- `docs/architecture/package-boundaries.md`
- `docs/architecture/data-storage.md`
- `docs/architecture/diagnostics.md`
- `docs/architecture/security-model.md`
