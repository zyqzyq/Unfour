# app-shell

## Purpose

`@unfour/app-shell` provides the frontend desktop workbench composition root.

## Local constraints

See [AGENTS.md](AGENTS.md) for this package's scope and invariants.

## Key Files

- `src/AppShell.tsx` - slot-based shell wrapper around `AppShellFrame`.
- `src/DesktopApp.tsx` - desktop workbench composition root that wires shell,
  workspace state, command palette, diagnostics actions, and feature module
  mounts.
- `src/index.ts` - package exports.

## Current Capabilities

- Composes global toolbar, sidebar, main workspace, right inspector, bottom
  panel, and status bar slots.
- Mounts API Client, SSH Terminal, Database, and Workspace environment
  management into the desktop workbench.
- Delegates visual layout to `@unfour/ui`.

## Known Gaps

- Release readiness belongs in `docs/release/*` and `docs/testing/*`.
- Shell layout primitives still live in `packages/ui` during the UI split.

## Test / Verify

Choose checks for the changed behavior using the
[verification guide](../../docs/agents/EXECUTION_PROTOCOL.md#choose-verification-by-impact).
The commands below are examples, not a checklist for every edit.

- `pnpm run build`
- For visual shell changes, run the app and inspect the first viewport.
