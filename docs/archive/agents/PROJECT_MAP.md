# PROJECT_MAP.md

> Architecture/reference snapshot. Current package and crate status lives in
> `docs/project/PACKAGE_STATUS.md`. If this file and `PACKAGE_STATUS.md`
> disagree on current status, treat `PACKAGE_STATUS.md` as authoritative.

## Top-Level Directory Structure

```
unfour/
  AGENTS.md                  -- AI agent rules and architecture constraints
  README.md                  -- Project overview and commands
  Cargo.toml                 -- Rust workspace root
  Cargo.lock
  pnpm-workspace.yaml        -- pnpm workspace: apps/*, packages/*
  package.json               -- Root scripts and shared dependencies
  tsconfig.base.json         -- Shared TypeScript config
  pnpm-lock.yaml
  .gitignore
  apps/
    desktop/                 -- Tauri 2 desktop application
  packages/
    api-client/              -- API Debugger frontend module
    app-shell/               -- Global shell composition layer
    command-client/          -- Typed Tauri command wrappers + shared types
    database/                -- Database frontend module
    ssh-terminal/            -- Terminal/SSH frontend module
    ui/                      -- Shared UI primitives (leaf package)
    workspace-core/          -- Shared workspace Zustand store
    workspace-local/         -- Compatibility boundary for future local workspace behavior
  crates/
    unfour-core/             -- Foundation: models, errors, redaction
    local-storage/           -- SQLite persistence (LocalDb, ActivityLog)
    http-engine/             -- HTTP/API client service
    database-engine/         -- Database connection + query service
    ssh-engine/              -- SSH connection + session service
    workspace-engine/        -- Workspace lifecycle + layout service
    secret-store/            -- OS keychain credential service
    unfour-command-bus/      -- Reusable Rust command entry point
    unfour-mcp/              -- Local stdio MCP server
  docs/
    agents/                  -- AI agent documentation
    architecture/            -- Package and module boundary specs
    decisions/               -- ADRs
    engineering/             -- Architecture, progress, security, task tracking
    ui/                      -- Design tokens, guidelines, component inventory
    user/                    -- End-user guide
    roadmap.md               -- Version milestone roadmap
  artifacts/                 -- Build or design artifacts
  dist/                      -- Frontend build output
  target/                    -- Rust build output
  node_modules/
```

## Frontend Packages

### `@unfour/ui` (packages/ui)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Source layout | Shared primitives and stateless layout helpers under `src/` |
| Internal deps | None (leaf package) |
| External deps | @radix-ui/react-dialog, @radix-ui/react-dropdown-menu, @radix-ui/react-slot, class-variance-authority, clsx, lucide-react, react, tailwind-merge |

**Exports:** Badge, Button/ButtonProps, DataTable/DataTableColumn, Dialog (full set), IconButton, Input, ContextMenu/DropdownMenu (full set), Select/SelectOption, AppShellFrame/BottomPanel/CommandPalette/GlobalToolbar/MainWorkspace/RightInspector/Sidebar/SidebarHeader/SidebarRow/SidebarSection/SplitPane/StatusBar/TabBar/ShellTab, EmptyState/ErrorState/LoadingState, ConnectionStatus/StatusBadge/StatusTone, Tabs/WorkspaceTab, Toolbar/ToolbarGroup, TreeView/TreeViewItem, cn.

### `@unfour/command-client` (packages/command-client)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Source layout | Typed command wrappers, shared types, and browser mock fallback |
| Internal deps | None (leaf package) |
| External deps | @tauri-apps/api |

**Exports:** ~40 domain types (Workspace, WorkspaceState, WorkspaceTab, WorkspaceLayout, ApiRequestInput, ApiResponse, ApiHistoryItem, DatabaseConnection, SshConnection, CredentialMetadata, etc.) and ~30 command functions (getSystemHealth, getWorkspaceState, createWorkspace, sendApiRequest, listDatabaseConnections, executeDatabaseQuery, connectSshSession, createCredential, etc.). Includes full mock fallback for browser development.

### `@unfour/app-shell` (packages/app-shell)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Source layout | Thin shell wrapper and package export |
| Internal deps | @unfour/ui |
| External deps | react |

**Exports:** `AppShell` -- a single layout component wrapping `AppShellFrame` from `@unfour/ui`. Accepts slots: `globalToolbar`, `sidebar`, `main`, `rightInspector`, `bottomPanel`, `statusBar`.

### `@unfour/workspace-core` (packages/workspace-core)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Source layout | Workspace Zustand store and type re-exports |
| Internal deps | @unfour/command-client |
| External deps | zustand |

**Exports:** `useWorkspaceStore` plus the existing shared workspace types (`Workspace`, `WorkspaceState`, `WorkspaceEnvironment`, `WorkspaceLayout`, and `WorkspaceTab`) re-exported from `@unfour/command-client`.

### `@unfour/workspace-local` (packages/workspace-local)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Source layout | Compatibility re-export only |
| Internal deps | @unfour/workspace-core |
| External deps | None |

**Exports:** Transitional compatibility re-export of `@unfour/workspace-core`. Future local persistence, import/export, recent workspace, and migration implementations belong here.

### `@unfour/api-client` (packages/api-client)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Source layout | Feature page plus `components/`, `hooks/`, and `model/` |
| Internal deps | @unfour/command-client, @unfour/ui |
| External deps | @monaco-editor/react, @radix-ui/react-dropdown-menu, @tanstack/react-query, @tanstack/react-table, lucide-react, react |

**Exports:** Full barrel: ApiDebuggerPage, ApiClientSidebar, ApiClientToolbar,
ApiCollectionTree, ApiRequestEditor, ApiRequestTabs, ApiRequestBar,
ApiResponseViewer, RequestActionsMenu, RequestParamsTabs, ResponseTabs,
useApiHistory, useApiLayout, useApiRequestTabs,
api-request-state, request-tabs, types, request-utils.

### `@unfour/database` (packages/database)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Source layout | Feature page plus `components/`, `hooks/`, and `model/` |
| Internal deps | @unfour/command-client, @unfour/ui, @unfour/workspace-core |
| External deps | @monaco-editor/react, @tanstack/react-query, lucide-react, react |

**Exports:** `{ DatabasePage, DatabaseConnectionTree, DatabaseSidebarToolbar (as DatabaseModuleToolbar export), TableInspector, databaseKnownGaps }` plus `result-utils`.

### `@unfour/ssh-terminal` (packages/ssh-terminal)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Source layout | Feature page plus `components/`, `hooks/`, and `model/` |
| Internal deps | @unfour/command-client, @unfour/ui, @unfour/workspace-core |
| External deps | @tanstack/react-query, @xterm/addon-fit, @xterm/xterm, lucide-react, react, zustand |

**Exports:** `{ TerminalPage, SshConnectionTree, TerminalLogPanel, TerminalStatusBar }` plus `session-utils` and `model/types`.

### `@unfour/desktop` (apps/desktop)

| Field | Value |
|---|---|
| Entry | `src/main.tsx` (Vite HTML entry) |
| Source layout | Desktop composition root, app components, assets, and Tauri adapter |
| Internal deps | Runtime frontend packages above except `@unfour/workspace-local`, which is a compatibility package |
| External deps | @monaco-editor/react, @radix-ui/*, @tailwindcss/vite, @tanstack/react-query, @tanstack/react-table, @tauri-apps/api, @tauri-apps/plugin-opener, react/react-dom, tailwindcss, zustand |

**Role:** Composition root. Wires together AppShell, all feature pages, sidebar trees, status bars, workspace management, and window controls.

## Cross-Package Dependency Graph

```
@unfour/ui                    (leaf -- no @unfour deps)
@unfour/command-client         (leaf -- no @unfour deps)

@unfour/app-shell             --> ui
@unfour/workspace-core             --> command-client

@unfour/api-client          --> command-client, ui
@unfour/workspace-local       --> workspace-core

@unfour/database              --> command-client, ui, workspace-core
@unfour/ssh-terminal          --> command-client, ui, workspace-core

@unfour/desktop               --> app-shell, api-client, database, ssh-terminal, workspace-core, command-client, ui
```

No circular dependencies detected. The graph is a clean DAG.

## Rust Crates

### `unfour-core` (crates/unfour-core)

Foundation crate. No workspace dependencies. Provides `AppError`/`AppResult<T>`, ~35 shared data models (camelCase serde), redaction utilities, AI/sync reserved stubs.

### `unfour-local-storage` (crates/local-storage)

Depends on: `unfour-core`. SQLite persistence via `LocalDb` (connection pool, 9 migration statements, `ActivityLogService`).

### `unfour-http-engine` (crates/http-engine)

Depends on: `unfour-core`, `unfour-local-storage`. `ApiClientService`: HTTP execution, environment variable resolution, history recording, saved requests CRUD, header redaction. 8 tests.

### `unfour-database-engine` (crates/database-engine)

Depends on: `unfour-core`, `unfour-local-storage`, and related database
driver dependencies. `DatabaseService`: connection CRUD, SQLite/PostgreSQL/MySQL
test/schema/query/browse behavior, and mutation confirmation policy. See
`docs/project/PACKAGE_STATUS.md` for current verification status.

### `unfour-ssh-engine` (crates/ssh-engine)

Depends on: `unfour-core`, `unfour-local-storage`, and `unfour-secret-store`.
`SshService`: connection CRUD, native SSH session lifecycle behind the
`ssh-native` feature, host-key handling, terminal history, reconnect behavior,
and log export with redaction. Live SSH verification remains a release gate.

### `unfour-workspace-engine` (crates/workspace-engine)

Depends on: `unfour-core`, `unfour-local-storage`. `WorkspaceService`: workspace CRUD, environment variables, layout JSON persistence. 8 tests.

### `unfour-secret-store` (crates/secret-store)

Depends on: `unfour-core` only (intentionally decoupled from `local-storage`). OS keychain backend via `keyring` crate. Credential CRUD with workspace-scoped references. 4 tests.

### `unfour-command-bus` (crates/unfour-command-bus)

Depends on: `unfour-core`, `unfour-local-storage`, `unfour-secret-store`, and
the HTTP, Database, SSH, and Workspace engine crates. Provides the reusable
Rust command entry point for Tauri, MCP, and future AI/CLI adapters.

### `unfour-mcp` (crates/unfour-mcp)

Depends on: `unfour-command-bus` and `unfour-core`. Provides the local stdio
MCP server and routes real tool behavior through the command bus.

### `unfour` (apps/desktop/src-tauri)

Depends on the command bus plus the core engine crates. It is the Tauri adapter
and composition layer for desktop commands, app setup, and event wiring.

### Rust Inter-Crate Dependency Graph

```
unfour-core  (foundation)
    |
    +---unfour-local-storage
    |       |
    |       +---unfour-http-engine
    |       +---unfour-database-engine
    |       +---unfour-ssh-engine
    |       +---unfour-workspace-engine
    |
    +---unfour-secret-store  (core only, NOT local-storage)

unfour-command-bus --> unfour-core, local-storage, secret-store, http-engine, database-engine, ssh-engine, workspace-engine
unfour-mcp         --> unfour-command-bus, unfour-core
unfour (desktop)  --> unfour-command-bus and core engine crates
```

## Frontend-to-Rust Call Chain

```
React component
  --> hook (useApiRequestTabs, useSqlExecution, etc.)
    --> @unfour/command-client function (sendApiRequest, executeDatabaseQuery, etc.)
      --> Tauri invoke("command_name", args)   [in Tauri runtime]
      --> mockInvoke("command_name", args)      [in browser fallback]
        --> Rust #[tauri::command] function    [commands.rs]
          --> CommandBus method                [command_bus.rs]
            --> Service (ApiClientService, DatabaseService, etc.)
              --> Driver (reqwest, sqlx, russh, keyring)
```

## State Management and Storage

| Layer | Technology | Scope |
|---|---|---|
| Server state (frontend) | TanStack Query (`useQuery`, `useMutation`) | API requests, DB connections, SSH connections, workspace state, system health |
| Client layout state (frontend) | Zustand (`useWorkspaceStore`) | Active tab, selected resource IDs, sidebar collapsed, tabs list |
| Feature-local state (frontend) | React `useState` / `useReducer` | Form drafts, panel visibility, UI toggles |
| Persistent storage (backend) | SQLite via sqlx | Workspaces, settings, API requests/history, connections, activity events |
| Credential storage (backend) | OS keychain via `keyring` crate | SSH passwords, DB passwords, API tokens (stored as credential references only) |
| In-memory (backend) | Runtime service state | SSH session lifecycle and event buffering |

## app-shell Current Responsibilities

`packages/app-shell` (2 files: `index.ts` + `AppShell.tsx`) is a thin wrapper that:
- Accepts 6 named slots (globalToolbar, sidebar, main, rightInspector, bottomPanel, statusBar)
- Delegates rendering to `AppShellFrame` from `@unfour/ui`

This package is correctly minimal and contains no feature logic.

## app-shell Residual Concerns (in apps/desktop/src/App.tsx)

`apps/desktop/src/App.tsx` is the composition root. It should remain an adapter
and composition layer rather than absorbing API, SSH, Database, or Workspace
business logic. Historical line counts and extraction progress should not be
maintained here; use `docs/project/PACKAGE_STATUS.md` for current status.

## Tauri Configuration

| Field | Value |
|---|---|
| Product name | Unfour |
| Identifier | dev.unfour |
| Version | 0.1.0 |
| Window | 1280x820, min 1040x680, frameless (`decorations: false`) |
| CSP | null (disabled) |
| Bundle targets | all |
| Capabilities | core:default, window close/minimize/drag/maximize, opener:default |
