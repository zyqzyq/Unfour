# Unfour UI Refactor Plan

## Current UI Inventory

The current app has a working frontend, but most of the UI is concentrated in
`packages/app-shell/src/AppShell.tsx`.

Findings:

- `AppShell.tsx` contains the application shell, workspace menu, module
  switcher, sidebar, panel primitives, API debugger, SSH panel, database panel,
  dialogs, list rows, status rows, and table rendering in one large file.
- `packages/ui` only exposes `Badge`, `Button`, `Input`, and `cn`, so feature
  and shell code must recreate common patterns locally.
- `AppTitleBar`, `ModuleSwitcher`, `ModuleSidebar`, `Panel`, `PanelHeader`,
  `ResourceListItem`, `InlineStatus`, and `EmptyState` are local reusable
  components inside `@unfour/app-shell`.
- Page-local styles exist for selects, tab toggles, panel headers, resource
  rows, empty states, inline status messages, schema tree rows, database table
  controls, and terminal input styling.
- The API, SSH, and Database panels use similar two-column layouts but each
  owns its local panel composition.
- There are many hardcoded Tailwind color utilities such as `bg-slate-*`,
  `bg-teal-*`, `bg-rose-*`, and state colors.
- Existing CSS includes gradients and panel shadows that make the UI feel more
  like a web dashboard than a restrained desktop tool.
- Several icon-only buttons have accessible labels, but not all provide an
  explicit tooltip surface.
- The project currently uses `lucide-react` only; no other icon library was
  found.
- The desktop entry `apps/desktop/src/App.tsx` already delegates to
  `@unfour/app-shell`, which is the correct composition direction.

## Phase 1: Shell Foundation

Scope:

- Add token-backed shell components to `packages/ui`.
- Replace local app title/module/sidebar scaffolding with shared shell
  components.
- Keep API, SSH, and Database business panels intact.
- Add `RightInspector`, `BottomPanel`, `StatusBar`, `SplitPane`, and
  `CommandPalette` extension points.
- Keep layout state in React for now, with clear future persistence shape.

Acceptance:

- Terminal, Database, and API Debugger entries remain accessible.
- All modules share one `GlobalToolbar`, `Sidebar`, `TabBar`, `MainWorkspace`,
  `BottomPanel`, and `StatusBar`.
- Global toolbar contains global actions only.
- No new icon library is introduced.

## Phase 2: Shared Component Cleanup

Recommended next work:

- Move local `Panel`, `PanelHeader`, `ResourceListItem`, `InlineStatus`, and
  `EmptyState` into `packages/ui`.
- Add shared `Select`, `Textarea`, `Toolbar`, `TreeView`, `DataTable`, `Tabs`,
  `Dialog`, `Tooltip`, and `DropdownMenu` wrappers.
- Replace page-local select and tab-toggle styles.
- Replace hardcoded colors in feature panels with semantic tokens.

## Phase 3: Module Layout Refactors

Recommended order:

1. API Debugger, because it has the most visible nested tabs, forms, request
   history, and collection list interactions.
2. Database, because it needs dense table and schema-tree consistency.
3. SSH Terminal, because terminal styling needs a focused dark surface while
   still obeying shell tokens.
4. Workspace and credentials dialogs, because they should become shared modal
   and inspector patterns.

## Non-Scope For Current Round

- Rewriting command-client, storage, SSH, database, or API execution logic.
- Introducing a new UI framework.
- Replacing routing or workspace persistence architecture.
- Removing existing feature entry points.

## Implementation Checklist

When implementing or refactoring UI:

1. Read root `design.md` first for the UI design entry point, reading order,
   source of truth, and conflict resolution.
2. Then read `docs/ui/ui-guidelines.md`, `docs/ui/ui-layouts.md`,
   `docs/ui/ui-tokens.md`, and `docs/ui/ui-components.md` in that order.
3. Inspect `packages/ui` for reusable components.
4. Reuse shared layout components.
5. Use design tokens only; do not hardcode colors unless adding a token.
6. Do not create page-local button or input styles.
7. Add empty, loading, and error states applicable to the workflow.
8. Verify narrow-width behavior.
9. Run `pnpm run build`.
10. Inspect the first viewport in the local app for visual review.
