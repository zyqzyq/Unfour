# workspace-local

## Purpose

`@unfour/workspace-local` is the frontend boundary reserved for local workspace
lifecycle, persistence, import/export, recent-workspace, and migration behavior.

## Boundaries

- Can own local-workspace-specific frontend implementations when they are
  introduced.
- Should depend on `@unfour/workspace-core` for shared workspace contracts.
- Should not absorb API, SSH, Database, or app-shell feature state.

## Key Files

- `src/index.ts` - transitional compatibility re-export of
  `@unfour/workspace-core`.

## Current Capabilities

- Re-exports `@unfour/workspace-core` so callers can migrate gradually without
  behavior changes.

## Known Gaps

- Current package status is centralized in `docs/project/PACKAGE_STATUS.md`.
- Concrete local workspace persistence/import/export behavior is not implemented
  in this package yet.

## Test / Verify

- `pnpm run build`
- Add package-specific tests when concrete local behavior is introduced.
