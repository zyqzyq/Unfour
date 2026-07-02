# Testing Layout

This document defines where tests live in the Unfour monorepo, what each
location is for, and how to decide where a new test should go. The goal is to
keep small unit/component tests close to the source they exercise while giving
cross-module, business-flow, database, command-bus, and desktop smoke/e2e
tests a separate, discoverable home.

For release-candidate verification, use `docs/testing/release-verification.md`
and `docs/testing/manual-test-cases.md`. This file explains test placement; it
does not define release readiness.

The layout is intentionally asymmetric: most tests stay near source, only the
ones that cross module boundaries move out.

## TL;DR

| Test kind | Location | Runner | File pattern |
| --- | --- | --- | --- |
| TS/TSX unit & component | `packages/*/src/...` next to source | vitest | `*.test.{ts,tsx}` |
| TS/TSX cross-module integration | `packages/*/tests/integration/` | vitest | `*.test.{ts,tsx}` |
| TS/TSX test data & mocks | `packages/*/tests/fixtures/` | — | any |
| Desktop smoke (browser) | `apps/desktop/tests/smoke/` | Playwright | `*.spec.{ts,tsx}` |
| Desktop E2E (future) | `apps/desktop/e2e/` | Playwright | `*.spec.{ts,tsx}` |
| Repo-level scripts unit | `scripts/` next to script | `node:test` | `*.test.mjs` |
| Rust unit (`#[cfg(test)]`) | `crates/*/src/...` (inline or `*_tests/`) | cargo | `*.rs` |
| Rust integration | `crates/*/tests/` | cargo | `*.rs` |

## Principles

1. **Do not move all tests into one top-level `tests/` tree.** Small unit and
   component tests stay next to the source they cover. This keeps the diff
   context small and lets AI tools and humans read a feature and its tests in
   the same directory.
2. **Only tests that cross boundaries move out.** Cross-module flows,
   command-bus roundtrips, real database engines, desktop smoke, and E2E
   belong in dedicated `tests/` directories so they cannot be mistaken for
   unit tests and so they can carry their own fixtures and setup.
3. **One test, one home.** Do not mirror a test in two locations. If a test
   grows beyond its location, move it; do not duplicate it.
4. **No parallel test frameworks per layer.** Frontend unit/component tests
   use vitest, browser tests use Playwright, script tests use `node:test`,
   Rust tests use `cargo test`. Do not introduce a new framework to bypass
   this layout.
5. **Fixtures stay with the tests that use them.** Do not move a fixture into
   a package it does not belong to. Shared fixtures live under
   `packages/*/tests/fixtures/` of the owning package.
6. **Tests must not reverse package boundaries.** A test in
   `packages/database/tests/integration/` may not import internal
   implementation details from `packages/api-client`. It should go through the
   public exports (`@unfour/api-client`) or, when only internals need
   coverage, become a unit test inside `packages/api-client/src/`.

## Frontend (TypeScript / TSX)

### `packages/*/src/**/*.test.{ts,tsx}` — unit & component tests

Default location for tests that exercise a single hook, component, model
function, or utility in isolation. Dependencies on other packages are mocked
with `vi.mock`. These tests:

- Live next to the file under test (e.g. `useSqlExecution.ts` and
  `useSqlExecution.test.tsx` in the same `hooks/` directory).
- Use `// @vitest-environment jsdom` at the top of the file when they need a
  DOM.
- Are picked up by vitest's default include pattern
  `packages/*/src/**/*.test.{ts,tsx}`.

When a single source directory accumulates many test files, prefer collapsing
them into `src/**/__tests__/*.test.ts` next to the source rather than moving
them out of `src/`. The `__tests__/` form is allowed inside `src/` so the
tests still travel with the source.

### `packages/*/tests/integration/**/*.test.{ts,tsx}` — cross-module integration

Use for tests that compose multiple modules of the same package, or that
verify a feature flow end-to-end inside one package without a browser. These
tests:

- May mount several components together.
- May use real (non-mocked) package-internal wiring.
- Must still mock the Rust command-bus boundary (`@unfour/command-client`)
  unless the test explicitly targets the bus adapter; the bus itself is a
  Rust integration concern (see below).
- Are picked up by vitest's include pattern
  `packages/*/tests/integration/**/*.test.{ts,tsx}`.

Examples:

- `packages/api-client/tests/integration/collection-flow.test.tsx` — exercises
  sidebar + tabs + save dialog together.
- `packages/database/tests/integration/query-roundtrip.test.tsx` — exercises
  SQL editor + result panel + history hook together with a mocked command
  layer.

### `packages/*/tests/fixtures/` — test data and mocks

Static JSON, factory helpers, and mock payloads that are too large to inline.
A fixture must only be referenced by tests in the same package. If two
packages need the same fixture, duplicate it; do not create a shared fixtures
package unless a task explicitly asks for one.

### `apps/desktop/tests/smoke/*.spec.{ts,tsx}` — desktop smoke (Playwright)

Browser-driven smoke tests that boot the desktop Vite dev server and verify
the app shell, module switching, and top-level layouts do not crash. These
tests:

- Use `@playwright/test`.
- Are discovered via `playwright.config.ts` `testDir:
  "./apps/desktop/tests/smoke"`.
- Should stay small and stable; treat them as a deployment gate, not a
  feature regression suite.

Current files:

- `apps/desktop/tests/smoke/ui-smoke.spec.ts` — app shell renders and module
  switching stays stable.
- `apps/desktop/tests/smoke/api-client-layout.spec.ts` — API Client keeps a
  Postman-like vertical workbench layout.

### `apps/desktop/e2e/*.spec.{ts,tsx}` — desktop E2E (future)

Reserved for end-to-end tests that drive the packaged Tauri app or full user
journeys across the desktop build. No files live here yet; do not create the
directory until the first E2E test is added. When it is, add it to
`playwright.config.ts` as a separate project or `testDir`.

### `scripts/*.test.mjs` — script unit tests

Tests for repo tooling scripts (`scripts/*.mjs`) using Node's built-in
`node:test` runner. Stay next to the script they cover. These are not part of
vitest's include patterns and are not run by `pnpm run test`; run them
explicitly with `node --test scripts/*.test.mjs` when modifying the script.

## Backend (Rust)

### `crates/*/src/` — inline `#[cfg(test)]` unit tests

Small unit tests for a single function or module live inline under
`#[cfg(test)] mod tests { ... }` at the bottom of the source file, or in a
sibling `*_tests/` module directory included via `mod *_tests;` in
`lib.rs` / the parent module. This is the pattern already used by
`crates/http-engine/src/api_client_tests/`,
`crates/ssh-engine/src/ssh_tests/`, and
`crates/database-engine/src/database_tests/`.

Keep these tests focused on pure logic and small in-memory fixtures. They
must not require a live database, network, or OS keychain.

### `crates/*/tests/*.rs` — Rust integration tests

Cargo's conventional integration test directory. Use for tests that exercise
the crate's public API as an external consumer would, or that require
external resources (live DB, OS keychain, real SSH server). Each file is a
separate crate that can only see the target crate's public API.

Current files:

- `crates/secret-store/tests/os_keychain_release_smoke.rs` — release smoke
  test for the platform credential store. Marked `#[ignore]` because it
  requires OS keychain access.

When adding integration tests that need a shared helper, put the helper in
`crates/*/tests/common/mod.rs` and `mod common;` from each test file. Do not
introduce a `dev-dependency` on another crate's tests directory.

## Naming conventions

- vitest unit/component: `*.test.ts` or `*.test.tsx` next to the file under
  test. The base name should match the file under test
  (`useSqlExecution.ts` → `useSqlExecution.test.tsx`).
- vitest integration: `*.test.ts` / `*.test.tsx` named after the flow or
  feature under test (`collection-flow.test.tsx`).
- Playwright: `*.spec.ts` / `*.spec.tsx`. The `.spec` extension is what
  keeps Playwright files out of vitest's include patterns; do not use
  `.test.ts` for Playwright files and do not use `.spec.ts` for vitest files.
- `node:test` scripts: `*.test.mjs`.
- Rust: `*_tests/` module directories for inline unit tests; arbitrary
  snake_case file names under `tests/` for integration tests.

## Test discovery configuration

| Runner | Config file | Include / testDir |
| --- | --- | --- |
| vitest | `vitest.config.ts` | `packages/*/src/**/*.test.{ts,tsx}`, `apps/*/src/**/*.test.{ts,tsx}`, `packages/*/tests/integration/**/*.test.{ts,tsx}`, `apps/*/tests/**/*.test.{ts,tsx}` |
| Playwright | `playwright.config.ts` | `testDir: ./apps/desktop/tests/smoke` |
| cargo | `Cargo.toml` workspace | `crates/*/src` (inline) + `crates/*/tests` (integration), discovered automatically |
| `node:test` | none | run explicitly: `node --test scripts/*.test.mjs` |

The vitest include patterns use `*.test.{ts,tsx}` only, so Playwright's
`*.spec.{ts,tsx}` files under `apps/desktop/tests/smoke/` are never picked up
by vitest. Keep this separation when adding new tests.

## Migration principles

When deciding whether to move an existing test:

1. **Default to leaving it in place.** The cost of moving a test is rarely
   worth the small organizational gain. Move only when the test's current
   location is actively misleading (e.g. a cross-module flow test sitting in
   `src/` next to one of the modules it touches).
2. **Use `git mv` to preserve history.** A test move should be a rename, not
   a delete + add.
3. **Update config, not test logic.** If a move breaks discovery, fix the
   include patterns or `testDir`. Do not rewrite the test's imports or
   assertions unless they point at a path that no longer exists.
4. **Do not move fixtures across packages.** If a fixture is only used by one
   package's tests, it stays in that package even if the tests move out of
   `src/`.
5. **One PR per layer.** Do not bundle frontend, Rust, and Playwright
   migrations into one change. Each layer has its own verification command
   and review audience.

## What does NOT move

- `packages/*/src/**/*.test.{ts,tsx}` — unit/component tests stay next to
  source even when there are many of them. Collapse into `__tests__/`
  directories first; only consider `tests/integration/` when the test
  genuinely crosses module boundaries.
- `scripts/*.test.mjs` — script tests stay next to their script.
- `crates/*/src/*_tests/` and inline `#[cfg(test)]` blocks — Rust unit tests
  stay next to source.
- `crates/*/tests/*.rs` — already in the conventional Rust integration test
  location; do not relocate.

## Verification commands

Run from the repository root:

```bash
# Frontend unit & component tests
pnpm run test

# Frontend unit & component tests with coverage
pnpm run test:coverage

# Desktop smoke (Playwright) — boots the Vite dev server
pnpm run test:e2e

# Rust unit + integration tests
pnpm run test:rust
# equivalent: cargo test --workspace

# Script unit tests (run on demand when touching scripts/)
node --test scripts/*.test.mjs
```
