# ui

## Purpose

`@unfour/ui` provides shared UI primitives and stateless layout helpers used by
Unfour frontend packages.

## Boundaries

- Can own reusable controls, states, menus, tabs, tables, tree views, toolbars,
  and stateless layout helpers.
- Should not contain API, SSH, Database, or Workspace business logic.
- Should not depend on feature packages or `packages/app-shell`.
- Feature-specific components should stay in their owning feature package.

## Key Files

- `src/index.ts` - shared exports.
- `src/button.tsx`, `src/input.tsx`, `src/select.tsx` - form/control
  primitives.
- `src/dialog.tsx`, `src/menus.tsx` - overlay/menu primitives.
- `src/states.tsx` - empty/loading/error states.
- `src/shell.tsx` - shell layout primitives and split pane.
- `src/tree-view.tsx`, `src/data-table.tsx`, `src/tabs.tsx` - structured data
  display primitives.

## Current Capabilities

- Shared button, input, select, badge, dialog, menu, tab, tree, table, toolbar,
  status, and state components.
- App shell frame, sidebar, panel, status bar, command palette, and split-pane
  layout helpers.

## Known Gaps

- Release readiness belongs in `docs/release/*` and `docs/testing/*`.
- Shell layout helpers and low-level primitives currently coexist during the UI
  split.

## Test / Verify

- `pnpm test -- packages/ui/src/shell.test.ts`
- `pnpm run build`
- For UI changes, run the app and inspect the first viewport.
