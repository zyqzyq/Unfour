# app-shell

## Purpose

`@unfour/app-shell` provides the frontend desktop workbench composition root.

## Boundaries

- Can own shell slot composition, workspace switcher wiring, module
  navigation, command palette and diagnostics actions, and module mount
  surfaces.
- Can mount API Client, SSH Terminal, and Database modules.
- Should reuse `@unfour/ui` layout primitives.
- Should not own API, SSH, Database, or Workspace feature internals.

## Key Files

- `src/AppShell.tsx` - slot-based shell wrapper around `AppShellFrame`.
- `src/DesktopApp.tsx` - desktop workbench composition root that wires shell,
  workspace state, command palette, diagnostics actions, and feature module
  mounts.
- `src/index.ts` - package exports.

## Current Capabilities

- Composes global toolbar, sidebar, main workspace, right inspector, bottom
  panel, and status bar slots.
- Mounts API Client, SSH Terminal, and Database into the desktop workbench.
- Delegates visual layout to `@unfour/ui`.

## Known Gaps

- Release readiness belongs in `docs/release/*` and `docs/testing/*`.
- Shell layout primitives still live in `packages/ui` during the UI split.

## Test / Verify

- `pnpm run build`
- For visual shell changes, run the app and inspect the first viewport.
