# Project Structure

This document maps the active repository structure, package responsibilities,
crate responsibilities, and frontend-to-backend call chain.

## Top-Level Layout

```text
Unfour/
  apps/
    desktop/                 Tauri/Vite desktop application
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
    local-storage/           SQLite persistence and activity log
    http-engine/             API request execution and API persistence
    database-engine/         database connection, schema, and query service
    ssh-engine/              SSH connection and terminal session service
    workspace-engine/        workspace CRUD, environments, layout persistence
    secret-store/            OS keychain credential boundary
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
| `@unfour/app-shell` | Thin global shell composition layer. Accepts slots for toolbar, sidebar, main workspace, inspector, bottom panel, and status bar. |
| `@unfour/ui` | Shared UI primitives, shell helpers, states, menus, tabs, tree, data table, dialogs, and styling utilities. |
| `@unfour/command-client` | Typed Tauri `invoke` wrappers, shared frontend command types, and browser-development mock fallback. |
| `@unfour/workspace-core` | Zustand workspace store and workspace type re-exports. |
| `@unfour/workspace-local` | Reserved boundary for future local workspace lifecycle behavior; currently a compatibility package. |
| `@unfour/api-client` | API Client feature UI: requests, tabs, Send, responses, history, saved requests, collections, environments, import/export. |
| `@unfour/database` | Database feature UI: connections, schema tree, SQL editor, query results, table preview, query history. |
| `@unfour/ssh-terminal` | SSH Terminal feature UI: connections, sessions, xterm panes, split/search/logs, host-key trust. |
| `@unfour/desktop` | Desktop composition root that mounts shell and feature packages. |

## Rust Crates

| Crate | Role |
| --- | --- |
| `unfour-core` | Foundation crate for shared models, `AppError`, redaction, and reserved AI/sync contracts. |
| `unfour-local-storage` | SQLite setup, migrations, local persistence, and activity log. |
| `unfour-secret-store` | Credential reference service backed by OS keychain in production and in-memory storage in tests. |
| `unfour-http-engine` | API execution, environment resolution, saved requests, history, and persistence redaction. |
| `unfour-database-engine` | Database connection CRUD, schema browsing, SQL execution, table browsing, and SQL safety classification. |
| `unfour-ssh-engine` | SSH connection/session lifecycle, PTY events, host-key trust, reconnect behavior, and redacted log export. |
| `unfour-workspace-engine` | Workspace CRUD, active workspace state, environments, and layout persistence. |
| `unfour-command-bus` | Shared command entry point used by Tauri, MCP, and future adapters. |
| `unfour-mcp` | Local stdio MCP server that routes tools through the command bus. |
| `unfour` | Tauri adapter in `apps/desktop/src-tauri`. |

## Frontend Dependency Shape

```text
@unfour/ui                 no @unfour package dependencies
@unfour/command-client     no feature dependencies

@unfour/app-shell          -> ui
@unfour/workspace-core     -> command-client
@unfour/workspace-local    -> workspace-core

@unfour/api-client         -> command-client, ui
@unfour/database           -> command-client, ui, workspace-core
@unfour/ssh-terminal       -> command-client, ui, workspace-core

@unfour/desktop            -> app-shell, api-client, database,
                              ssh-terminal, workspace-core,
                              command-client, ui
```

Feature packages must not depend on each other or on `packages/app-shell`.

## Rust Dependency Shape

```text
unfour-core
  -> unfour-local-storage
       -> unfour-http-engine
       -> unfour-database-engine
       -> unfour-ssh-engine
       -> unfour-workspace-engine
  -> unfour-secret-store

unfour-command-bus
  -> unfour-core
  -> unfour-local-storage
  -> unfour-secret-store
  -> http, database, ssh, workspace engines

unfour-mcp
  -> unfour-command-bus
  -> unfour-core

unfour Tauri adapter
  -> unfour-command-bus
  -> core engine crates
```

## Frontend-To-Rust Call Chain

```text
React component
  -> feature hook or action
  -> @unfour/command-client function
  -> Tauri invoke(command, args) in desktop runtime
  -> browser mock fallback in Vite/browser development
  -> Rust #[tauri::command] adapter
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

The desktop adapter lives under `apps/desktop/src-tauri`. The product name is
Unfour and the repository package version is `0.1.0`. Release readiness must be
determined from the release verification documents, not from the version string
alone.

See also:

- `docs/architecture/package-boundaries.md`
- `docs/architecture/data-storage.md`
- `docs/architecture/security-model.md`
