# Workbench polish verification — 2026-11-10

## Scope

Unified the resource sidebar and editor into one rounded workspace frame for
API, SSH, Database, and Flow. Sidebars share the surface background and retain
independent widths, collapse behavior, and scrollable lists. Document tabs use
a shared flat rail; the API method and URL share one address outline.

No backend, persistence, command contracts, dependencies, or security policies
changed. Visual rules are recorded in [the design system](../ui/design-system.md).

## Review and correction

Review found that an unlayered shared CSS height overrode Database's `h-9`
result-tab density. Added a browser assertion, reproduced the failure in both
themes (42px instead of 36px), and moved the default height into the component's
mergeable utility classes. Both cases pass after the fix.

Also verified horizontal tab overflow without vertical overflow, stable tab
width on hover, keyboard access to close buttons, and real sidebar dragging
followed by collapse/restore without remounting the edited request input.

## Final checks

- PASS: `pnpm run build` (TypeScript and production Vite build).
- PASS: `pnpm run check:tokens`.
- PASS: ESLint on all changed TypeScript files; zero errors. Existing
  `SshConnectionTreeContent` function-length warning remains.
- PASS: `git diff --check`.
- PASS: 46 Vitest tests across shared tabs, tree view, shell, ModuleSidebar,
  ApiClientSidebar, ApiRequestTabs, and ApiRequestBar.
- PASS: 18 Playwright tests across `workbench-theme`, `api-tabbar-layout`,
  `desktop-ux`, `sidebar-consistency`, `redesign-workbench`, and
  `ssh-task-load-error-layout`. Coverage includes both themes, English/Chinese,
  960px and 1100px layouts, long lists, draft retention, and error panels.

Browser tests use disposable adapters and fixtures; they do not establish
native SSH or database-engine behavior. Screenshots of all four module surfaces
were inspected. Final native desktop visual verification was not repeated.
The production build retains its existing large-chunk advisory.
