# command-client

## Purpose

`@unfour/command-client` provides typed frontend command wrappers, shared
frontend-facing types, and browser-development mock behavior for Tauri commands.

## Boundaries

- Can own TypeScript types that mirror frontend command inputs/outputs.
- Can own `invoke` wrappers and browser fallback mocks.
- Should not contain React components, feature UI, feature-local state, or
  package-specific business logic.
- Should stay aligned with Rust command-bus and Tauri command contracts.

## Key Files

- `src/tauri.ts` - Tauri invoke wrappers and browser mock implementation.
- `src/types.ts` - shared command input/output and frontend domain types.
- `src/index.ts` - package exports.

## Current Capabilities

- Workspace, layout, environment, API, credential, Database, SSH, host-key, and
  system-health command wrappers.
- Browser fallback mocks for local frontend development.

## Known Gaps

- Current package status is centralized in `docs/project/PACKAGE_STATUS.md`.
- Mock behavior can drift from Rust command behavior if command contracts change
  without updating this package.

## Test / Verify

- `pnpm test -- packages/command-client/src/tauri.test.ts`
- `pnpm run build`
- For command-contract changes, also run the relevant Rust command-bus/Tauri
  checks.
