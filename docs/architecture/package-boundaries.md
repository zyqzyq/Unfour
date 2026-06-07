# Package Boundaries

## packages/app-shell

### Current State

`packages/app-shell` is a thin wrapper. `src/AppShell.tsx` is the only source file; it receives props and delegates rendering to `AppShellFrame` from `packages/ui`.

### Target Responsibility

- Own application shell composition.
- Provide global layout, sidebar mounting surface, top-level navigation wiring, route assembly, cross-module container slots, and module mount points.
- Be the single source of truth for shell-level behavior and shell-specific state (for example, layout persistence logic).

### Transitional Exceptions

- Shell layout primitives (`AppShellFrame`, `GlobalToolbar`, `Sidebar`, `TabBar`, `MainWorkspace`, `BottomPanel`, `RightInspector`, `StatusBar`, `CommandPalette`) currently live in `packages/ui` and are consumed by both `packages/app-shell` and `apps/desktop`.
- This overlap is temporarily allowed while the UI module split is in progress.

### Follow-up Work

- Decide whether shell layout components should migrate from `packages/ui` into `packages/app-shell`, or remain in `packages/ui` as reusable stateless layout primitives.
- If they remain in `packages/ui`, `packages/app-shell` should still own shell-level orchestration, composition, and any shell-specific state.

---

## packages/ui

### Current State

`packages/ui` contains both low-level primitives (`Button`, `Input`, `Badge`, `IconButton`) and high-level shell layout components (`AppShellFrame`, `GlobalToolbar`, `Sidebar`, `SplitPane`, `CommandPalette`).

### Target Responsibility

- Own shared, reusable UI primitives that can be consumed by any module without pulling in business logic.
- Stateless layout helpers that are cross-module MAY remain here if they have no shell-specific state or orchestration.

### Transitional Exceptions

- Shell layout components currently coexist with primitives. This is temporarily allowed during the UI refactor.
- `packages/ui` exports `EmptyState`, `ErrorState`, `LoadingState`, which are used by feature packages.

### Follow-up Work

- Evaluate the primitive / shell split.
- Add missing primitives (`Textarea`, `Checkbox`, `Switch`, `Popover`, `Tooltip`) when a clear cross-module consumer exists.

---

## packages/workspace

### Current State

Exports only `useWorkspaceStore` (a Zustand store). It is consumed by:

- `apps/desktop` for layout and workspace mutations.
- `packages/database` for `selectedDatabaseConnectionId`.
- `packages/terminal` for `selectedSshConnectionId`.

### Target Responsibility

- Own shared workspace state: active workspace, tab list, sidebar collapse, and selected resource IDs that must be globally accessible.

### Transitional Exceptions

- Direct imports of `useWorkspaceStore` from `packages/database` and `packages/terminal` are allowed during the UI refactor to avoid prop-drilling while module boundaries are still stabilizing.

### Follow-up Work

- After the UI refactor stabilizes, evaluate whether selected connection state should be passed via props from `apps/desktop` instead of being read directly from `packages/workspace` inside feature packages.
- New feature-specific dependencies on `packages/workspace` MUST NOT be added without review.

---

## packages/database

### Current State

- Owns connection tree, schema tree, SQL editor, query results, and table inspector.
- Depends on `packages/workspace` for `useWorkspaceStore`.
- Depends on `@monaco-editor/react` and `@tanstack/react-query`.

### Target Responsibility

- Own all database-specific frontend logic, components, and state.
- MUST NOT contain API request logic or SSH session logic.

### Transitional Exceptions

- Dependency on `packages/workspace` is temporarily allowed (see `packages/workspace` section).

### Follow-up Work

- Remove hardcoded colors and local select/tab styles once shared primitives are available.
- Re-evaluate `packages/workspace` dependency after the refactor.

---

## packages/terminal

### Current State

- Owns SSH connection tree, session tabs, terminal pane, and connection status.
- Maintains a local Zustand store for terminal events and session state.
- Depends on `packages/workspace` for `useWorkspaceStore`.
- Depends on `@xterm/xterm` and `@xterm/addon-fit`.

### Target Responsibility

- Own all terminal/SSH-specific frontend logic, components, and state.
- MUST NOT contain SQL editors or API response viewers.

### Transitional Exceptions

- Dependency on `packages/workspace` is temporarily allowed (see `packages/workspace` section).

### Follow-up Work

- Evaluate whether the local Zustand store should remain in `packages/terminal` or move to a more general state boundary.
- Re-evaluate `packages/workspace` dependency after the refactor.

---

## Other Packages

| Package | Status | Responsibility | Forbidden |
| --- | --- | --- | --- |
| `packages/api-debugger` | Existing | Collection tree, request editor, response viewer, request state, history, environment variables | Database logic, SSH logic, top-level sidebar |
| `packages/command-client` | Existing | Typed Tauri command wrappers, shared frontend types, browser-dev mocks | React components, feature business logic, feature state |
| `packages/command-bus` | **Does not exist** | — | — |
| `packages/extension-contracts` | **Does not exist** | — | — |

### packages/command-client

- `src/tauri.ts` exports typed `invoke` wrappers for every Tauri command (workspace, API, database, SSH, credentials, layout).
- `src/types.ts` exports shared TypeScript interfaces used by the frontend and aligned with Rust models.
- When Tauri is not available (for example, browser development), `tauri.ts` falls back to a comprehensive mock implementation that covers all commands.
- MUST remain free of React components and feature-specific business logic.

---

## Dependency Direction

Allowed:

```text
apps/*
  -> packages/app-shell
  -> feature packages (api-debugger, database, terminal)
  -> packages/workspace
  -> packages/ui
  -> packages/command-client
```

Feature packages MAY depend on:

- `packages/ui`
- `packages/command-client`
- `packages/workspace` (transitionally, during the UI refactor)

Forbidden:

- Feature package -> `packages/app-shell`
- `packages/ui` -> feature package
- `packages/command-client` -> feature package
- `packages/app-shell` -> feature package (except through composition slots in `apps/desktop`)

---

## Known Boundary Issues

1. **`apps/desktop/src/App.tsx` hosts shell-level logic.**
   `App.tsx` contains `AppTitleBar`, `WorkspaceMenu`, `ModuleSidebar`, window controls, command palette wiring, and workspace mutation hooks. This is acceptable because `apps/desktop` is the composition layer, but the file is large and mixes shell concerns with application bootstrapping.

2. **`packages/ui` contains shell layout components.**
   `AppShellFrame`, `GlobalToolbar`, `Sidebar`, `TabBar`, `MainWorkspace`, `BottomPanel`, `RightInspector`, and `CommandPalette` live in `packages/ui` rather than `packages/app-shell`. See the `packages/app-shell` and `packages/ui` sections above for the transitional plan.

3. **Feature packages depend on `packages/workspace`.**
   `packages/database` and `packages/terminal` import `useWorkspaceStore`. This is a transitional exception documented above.

4. **`packages/api-debugger` defines a local `EmptyState`.**
   `ResponseTabs.tsx` contains a page-local `EmptyState` component even though `packages/ui` exports one.
