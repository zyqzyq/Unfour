# workspace-core

## Purpose

`@unfour/workspace-core` owns shared frontend workspace state that must be
available across modules.

## Boundaries

- Can own active workspace, active tab, sidebar collapse, workspace tabs, and
  selected resource IDs that are intentionally shared.
- Can re-export workspace types from `@unfour/command-client`.
- Should not own API request internals, Database SQL state, SSH terminal
  session state, or local persistence implementations.
- Feature package imports are a documented transitional boundary and should not
  expand without review.

## Key Files

- `src/workspace-store.ts` - Zustand workspace store.
- `src/index.ts` - store export and workspace type re-exports.

## Current Capabilities

- Tracks active workspace/tab state.
- Tracks selected API request, Database connection, and SSH connection IDs.
- Hydrates and snapshots layout state.
- Opens workspace tabs and toggles sidebar collapse.

## Known Gaps

- Release readiness belongs in `docs/release/*` and `docs/testing/*`.
- Selected resource state may need ownership review after the module split
  stabilizes.

## Test / Verify

- `pnpm test -- packages/workspace-core/src/workspace-store.test.ts`
- `pnpm run build`
