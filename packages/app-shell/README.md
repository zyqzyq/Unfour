# app-shell

## Purpose

`@unfour/app-shell` provides the thin frontend shell wrapper used by the desktop
composition layer.

## Boundaries

- Can own shell slot composition and module mount surfaces.
- Should reuse `@unfour/ui` layout primitives.
- Should not own API, SSH, Database, or Workspace feature behavior.
- Should not depend on feature packages.

## Key Files

- `src/AppShell.tsx` - slot-based shell wrapper around `AppShellFrame`.
- `src/index.ts` - package exports.

## Current Capabilities

- Accepts global toolbar, sidebar, main workspace, right inspector, bottom
  panel, and status bar slots.
- Delegates visual layout to `@unfour/ui`.

## Known Gaps

- Current package status is centralized in `docs/project/PACKAGE_STATUS.md`.
- Shell layout primitives still live in `packages/ui` during the UI split.

## Test / Verify

- `pnpm run build`
- For visual shell changes, run the app and inspect the first viewport.
