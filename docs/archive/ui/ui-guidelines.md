# UI Guidelines

## Product Style

- Lightweight IDE / desktop developer tool.
- High information density, efficiency, and clear state communication.
- MUST NOT use marketing-style dashboard layouts.
- MUST NOT use large border radii, heavy shadows, excessive whitespace, or meaningless card stacks.

## Shared Layout

- `AppShell` (via `packages/ui` shell components) provides the unified `GlobalToolbar`, `Sidebar`, `MainWorkspace`, `RightInspector`, `BottomPanel`, and `StatusBar`.
- Complex module regions SHOULD use `SplitPane` from `packages/ui`.
- Module-level actions MUST live in a module `Toolbar`, not in the global toolbar.
- Each panel SHOULD expose only one primary action.
- Secondary actions MUST go into `DropdownMenu`, context menus, or secondary action groups.
- Wide and narrow viewports MUST have reasonable layout behavior.
- Long content MUST support scroll, truncation, or expand patterns.

## Shared Components

Reuse components exported by `packages/ui` before creating local variants.

Current shared components (observed in `packages/ui/src/index.ts`):

- `Badge`
- `Button`
- `DataTable`
- `Dialog` (with `DialogBody`, `DialogContent`, `DialogFooter`, `DialogHeader`, `DialogTitle`, etc.)
- `IconButton`
- `Input`
- `Menus` (`ContextMenu`, `DropdownMenu`)
- `Select`
- `Shell` (`AppShellFrame`, `BottomPanel`, `CommandPalette`, `GlobalToolbar`, `MainWorkspace`, `RightInspector`, `Sidebar`, `SplitPane`, `StatusBar`, `TabBar`)
- `States` (`EmptyState`, `ErrorState`, `LoadingState`)
- `Status` (`ConnectionStatus`, `StatusBadge`)
- `Tabs`
- `Toolbar` (with `ToolbarGroup`)
- `TreeView`
- `cn` utility

Planned additions (marked in `docs/ui/ui-components.md`):

- `Textarea`, `Checkbox`, `Switch`, `PropertyGrid`, `Popover`, `Tooltip` wrappers still need to move into `packages/ui`.

## Required States

Each feature MUST evaluate the following states and implement the states that are applicable to its workflow.

Common state checklist:

- Initial state
- Nothing selected
- Empty state
- Loading
- Success
- Failure
- Network error
- Timeout
- No data
- Long content
- Unsupported format
- Disabled state

## Module-Specific State Expectations

| State | API Debugger | Database | Terminal | Workspace |
| --- | --- | --- | --- | --- |
| Initial / nothing selected | MUST (no request selected) | MUST (no connection selected) | MUST (no connection selected) | MUST (first load) |
| Empty | MUST (no collections) | MUST (no connections) | MUST (no SSH connections) | MUST (no workspaces) |
| Loading | MUST (sending) | MUST (connecting / executing) | MUST (connecting) | SHOULD |
| Success | MUST (response received) | MUST (query results) | MUST (connected) | SHOULD |
| Failure | MUST (4xx/5xx) | MUST (connection / SQL error) | MUST (connection failed) | SHOULD |
| Network error | MUST | SHOULD (live connections) | MUST | — |
| Timeout | MUST | SHOULD | — | — |
| No data | MUST (empty body) | MUST (empty result set) | SHOULD (no output) | — |
| Long content | MUST (large response) | SHOULD (large result) | SHOULD (log overflow) | — |
| Unsupported format | SHOULD | — | — | — |
| Disabled | SHOULD | SHOULD (untestable connection) | — | — |
| Confirmation required | — | MUST (mutation SQL) | — | — |
| Disconnected / closed | — | — | MUST (session closed) | — |
| Reconnecting | — | — | SHOULD | — |

## API Debugger Pattern

The current API Debugger (`packages/api-client`) implements the following patterns. Use it as a reference for consistent module design.

Verified patterns:

- Module-level toolbar (`ApiRequestToolbar`).
- `Send` is the primary action.
- Collection tree rendered in the unified sidebar via `ApiCollectionTree`.
- Request area and response area use `SplitPane`.
- Request area has tabbed sections: Query, Headers, Body, Auth (`RequestParamsTabs`).
- Response area has a Response / History toggle and tabbed sections: Body, Headers, Cookies, Timing (`ResponseTabs`).
- Status badges show `sending`, status code, duration, and size.
- Empty states use `EmptyState` from `packages/ui` (note: `ResponseTabs.tsx` currently defines a local `EmptyState` — this is a known inconsistency).

## Scope Control

- UI optimization MUST NOT change existing business behavior.
- UI optimization MUST NOT modify backend protocols.
- A component MUST have at least two clear consumers before it is extracted into `packages/ui`.
- Visual uniformity MUST NOT force every module into an identical layout.
- Terminal, Database, and API Debugger MAY share design language but MUST retain their own workflow characteristics.
