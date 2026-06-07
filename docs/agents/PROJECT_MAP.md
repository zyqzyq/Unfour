# PROJECT_MAP.md

> Read-only checkpoint scan. This file records the structural facts of the repository.

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
    api-debugger/            -- API client frontend module
    app-shell/               -- Global shell composition layer
    command-client/          -- Typed Tauri command wrappers + shared types
    database/                -- Database frontend module
    terminal/                -- Terminal/SSH frontend module
    ui/                      -- Shared UI primitives (leaf package)
    workspace/               -- Shared workspace Zustand store
  crates/
    unfour-core/             -- Foundation: models, errors, redaction
    local-storage/           -- SQLite persistence (LocalDb, ActivityLog)
    http-engine/             -- HTTP/API client service
    database-engine/         -- Database connection + query service
    ssh-engine/              -- SSH connection + session service
    workspace-engine/        -- Workspace lifecycle + layout service
    secret-store/            -- OS keychain credential service
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
| Files | 16 (flat structure) |
| Internal deps | None (leaf package) |
| External deps | @radix-ui/react-dialog, @radix-ui/react-dropdown-menu, @radix-ui/react-slot, class-variance-authority, clsx, lucide-react, react, tailwind-merge |

**Exports:** Badge, Button/ButtonProps, DataTable/DataTableColumn, Dialog (full set), IconButton, Input, ContextMenu/DropdownMenu (full set), Select/SelectOption, AppShellFrame/BottomPanel/CommandPalette/GlobalToolbar/MainWorkspace/RightInspector/Sidebar/SidebarHeader/SidebarRow/SidebarSection/SplitPane/StatusBar/TabBar/ShellTab, EmptyState/ErrorState/LoadingState, ConnectionStatus/StatusBadge/StatusTone, Tabs/WorkspaceTab, Toolbar/ToolbarGroup, TreeView/TreeViewItem, cn.

### `@unfour/command-client` (packages/command-client)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Files | 3 (flat structure) |
| Internal deps | None (leaf package) |
| External deps | @tauri-apps/api |

**Exports:** ~40 domain types (Workspace, WorkspaceState, WorkspaceTab, WorkspaceLayout, ApiRequestInput, ApiResponse, ApiHistoryItem, DatabaseConnection, SshConnection, CredentialMetadata, etc.) and ~30 command functions (getSystemHealth, getWorkspaceState, createWorkspace, sendApiRequest, listDatabaseConnections, executeDatabaseQuery, connectSshSession, createCredential, etc.). Includes full mock fallback for browser development.

### `@unfour/app-shell` (packages/app-shell)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Files | 2 (flat structure) |
| Internal deps | @unfour/ui |
| External deps | react |

**Exports:** `AppShell` -- a single layout component wrapping `AppShellFrame` from `@unfour/ui`. Accepts slots: `globalToolbar`, `sidebar`, `main`, `rightInspector`, `bottomPanel`, `statusBar`.

### `@unfour/workspace` (packages/workspace)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Files | 2 (flat structure) |
| Internal deps | @unfour/command-client |
| External deps | zustand |

**Exports:** `useWorkspaceStore` -- Zustand store managing workspace layout state: active tab, selected resource IDs (API request, DB connection, SSH connection), sidebar collapsed, tabs list, layout hydration/snapshot.

### `@unfour/api-debugger` (packages/api-debugger)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Files | 15 (3 root + components/ + hooks/ + model/) |
| Internal deps | @unfour/command-client, @unfour/ui |
| External deps | @monaco-editor/react, @radix-ui/react-dropdown-menu, @tanstack/react-query, @tanstack/react-table, lucide-react, react |

**Exports:** Full barrel: ApiDebuggerPage, ApiCollectionTree, ApiRequestEditor, ApiRequestToolbar, ApiResponseViewer, RequestActionsMenu, RequestParamsTabs, ResponseTabs, useApiHistory, useApiLayout, useApiRequest, api-request-state, types, request-utils.

### `@unfour/database` (packages/database)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Files | 22 (3 root + components/ + hooks/ + model/) |
| Internal deps | @unfour/command-client, @unfour/ui, @unfour/workspace |
| External deps | @monaco-editor/react, @tanstack/react-query, lucide-react, react |

**Exports:** `{ DatabasePage, DatabaseConnectionTree, DatabaseSidebarToolbar (as DatabaseModuleToolbar export), TableInspector, databaseKnownGaps }` plus `result-utils`.

### `@unfour/terminal` (packages/terminal)

| Field | Value |
|---|---|
| Entry | `./src/index.ts` |
| Files | 25 (3 root + components/ + hooks/ + model/) |
| Internal deps | @unfour/command-client, @unfour/ui, @unfour/workspace |
| External deps | @tanstack/react-query, @xterm/addon-fit, @xterm/xterm, lucide-react, react, zustand |

**Exports:** `{ TerminalPage, SshConnectionTree, TerminalLogPanel, TerminalStatusBar }` plus `session-utils` and `model/types`.

### `@unfour/desktop` (apps/desktop)

| Field | Value |
|---|---|
| Entry | `src/main.tsx` (Vite HTML entry) |
| Files | 4 root + assets/ + components/ (empty) |
| Internal deps | ALL 7 packages above |
| External deps | @monaco-editor/react, @radix-ui/*, @tailwindcss/vite, @tanstack/react-query, @tanstack/react-table, @tauri-apps/api, @tauri-apps/plugin-opener, react/react-dom, tailwindcss, zustand |

**Role:** Composition root. Wires together AppShell, all feature pages, sidebar trees, status bars, workspace management, and window controls.

## Cross-Package Dependency Graph

```
@unfour/ui                    (leaf -- no @unfour deps)
@unfour/command-client         (leaf -- no @unfour deps)

@unfour/app-shell             --> ui
@unfour/workspace             --> command-client

@unfour/api-debugger          --> command-client, ui
@unfour/database              --> command-client, ui, workspace
@unfour/terminal              --> command-client, ui, workspace

@unfour/desktop               --> ALL 7 packages
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

Depends on: `unfour-core`, `unfour-local-storage`. `DatabaseService`: connection CRUD, SQLite test/schema/query/browse, mutation confirmation policy. PostgreSQL/MySQL reserved. 3 tests.

### `unfour-ssh-engine` (crates/ssh-engine)

Depends on: `unfour-core`, `unfour-local-storage`. `SshService`: connection CRUD, simulated session lifecycle, credential boundary enforcement, log export with redaction. `russh` behind `ssh-native` feature (not yet connected). 4 tests.

### `unfour-workspace-engine` (crates/workspace-engine)

Depends on: `unfour-core`, `unfour-local-storage`. `WorkspaceService`: workspace CRUD, environment variables, layout JSON persistence. 8 tests.

### `unfour-secret-store` (crates/secret-store)

Depends on: `unfour-core` only (intentionally decoupled from `local-storage`). OS keychain backend via `keyring` crate. Credential CRUD with workspace-scoped references. 4 tests.

### `unfour-workspace` (apps/desktop/src-tauri)

Depends on: ALL 7 crates. Tauri adapter and composition layer. `CommandBus` orchestrates all services. 36 `#[tauri::command]` functions. `tracing-subscriber` for structured logging.

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

unfour-workspace (desktop)  --> ALL 7 crates via CommandBus
```

## Frontend-to-Rust Call Chain

```
React component
  --> hook (useApiRequest, useSqlExecution, etc.)
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
| In-memory (backend) | `HashMap` behind `Mutex` | SSH session state (simulated) |

## app-shell Current Responsibilities

`packages/app-shell` (2 files: `index.ts` + `AppShell.tsx`) is a thin wrapper that:
- Accepts 6 named slots (globalToolbar, sidebar, main, rightInspector, bottomPanel, statusBar)
- Delegates rendering to `AppShellFrame` from `@unfour/ui`

This package is correctly minimal and contains no feature logic.

## app-shell Residual Concerns (in apps/desktop/src/App.tsx)

`apps/desktop/src/App.tsx` (954 lines) is the composition root but contains significant inline logic:

- **Workspace CRUD dialogs:** `WorkspaceMenu`, `WorkspaceDialog` (create, rename, delete) -- 200+ lines of workspace management UI with mutations
- **Window controls:** `WindowControls`, `TitlebarWindowButton`, `AppTitleBar` -- ~100 lines of custom title bar logic
- **Module sidebar:** `ModuleSidebar`, `ResourceGroup`, `SidebarAction` -- ~100 lines of sidebar routing and layout
- **Hardcoded Tailwind colors:** 23 instances of `slate-*`, `white`, `rose-*`, `teal-*` classes instead of semantic `--u-color-*` tokens
- **Bottom panel placeholder:** Inline diagnostics placeholder content
- **Right inspector placeholder:** Inline placeholder content
- **Layout persistence:** `layoutMutation` and debounce logic for persisting workspace layout

Per AGENTS.md, `apps/desktop` is allowed to be the composition layer, but the workspace CRUD dialogs, window controls, and sidebar could be extracted into dedicated components or packages for clarity.

## Tauri Configuration

| Field | Value |
|---|---|
| Product name | Unfour Workspace |
| Identifier | com.unfour.workspace |
| Version | 0.1.0 |
| Window | 1280x820, min 1040x680, frameless (`decorations: false`) |
| CSP | null (disabled) |
| Bundle targets | all |
| Capabilities | core:default, window close/minimize/drag/maximize, opener:default |
