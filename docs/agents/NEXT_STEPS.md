# NEXT_STEPS.md

> Read-only checkpoint scan. These are candidate tasks only. None have been executed.

---

## TASK-01: Extract App.tsx into focused components

| Field | Value |
|---|---|
| Goal | Break `apps/desktop/src/App.tsx` (954 lines) into smaller, focused modules: `AppTitleBar`, `WorkspaceMenu`/`WorkspaceDialogs`, `ModuleSidebar`, `WindowControls`, and a slim `App` composition root. |
| Scope | `apps/desktop/src/` only. Create new files under `apps/desktop/src/components/` (currently empty). |
| Forbidden | Do not modify any `packages/*` or `crates/*` files. Do not change backend call chains. Do not add new dependencies. |
| Risk | Medium. Large refactor touching the composition root. Risk of temporarily breaking module wiring. Requires careful extraction of prop drilling and shared state. |
| Prerequisites | None. |
| Acceptance criteria | `App.tsx` is under 200 lines. Each extracted component is in its own file. `pnpm run build` passes. Visual behavior is identical. No new `any` types introduced. |
| Independent commit | Yes |

---

## TASK-02: Replace hardcoded Tailwind colors with semantic tokens

| Field | Value |
|---|---|
| Goal | Replace all 23 hardcoded Tailwind color classes in `App.tsx` (and 7 in feature packages) with semantic `--u-color-*` CSS custom properties. |
| Scope | `apps/desktop/src/App.tsx`, `packages/ui/src/badge.tsx`, `packages/api-debugger/src/components/ApiRequestEditor.tsx`, `packages/api-debugger/src/components/ResponseTabs.tsx`, `packages/api-debugger/src/components/RequestParamsTabs.tsx`. May need to add new tokens to `packages/ui/src/styles.css` if semantic equivalents don't exist. |
| Forbidden | Do not change component behavior, layout, or backend logic. Do not modify `crates/*`. |
| Risk | Low-Medium. Some semantic tokens may not yet exist (e.g., `--u-color-danger`, `--u-color-overlay`). May need to define 2-3 new tokens. Visual regression risk is low since colors should map closely to existing tokens. |
| Prerequisites | TASK-01 is helpful (reduces App.tsx size) but not required. |
| Acceptance criteria | Zero instances of `slate-*`, `rose-*`, `teal-*`, or `bg-white` in `apps/desktop/src/App.tsx`. All colors use `var(--u-color-*)`. `pnpm run build` passes. Visual appearance is consistent with current design. |
| Independent commit | Yes |

---

## TASK-03: Configure ESLint or Biome for frontend linting

| Field | Value |
|---|---|
| Goal | Add a frontend linter (ESLint flat config or Biome) with a `lint` script in `package.json`. Configure rules for: unused imports, consistent style, no `any`, React hook rules, and TypeScript strict checks. |
| Scope | Root `package.json` (add script + devDependency). New config file at root (`.eslintrc.cjs` or `biome.json`). May require fixes across all `packages/*/src/` and `apps/desktop/src/` for existing violations. |
| Forbidden | Do not modify `crates/*` or Rust code. Do not change component behavior. |
| Risk | Low. Linter setup is additive. Initial run may surface many warnings that need triage (warn vs. error). |
| Prerequisites | None. |
| Acceptance criteria | `pnpm run lint` succeeds with zero errors. Warnings are documented or tracked. Config file is committed. CI can run the lint step. |
| Independent commit | Yes |

---

## TASK-04: Add vitest for frontend unit testing

| Field | Value |
|---|---|
| Goal | Set up vitest as the frontend test runner. Write initial tests for `useWorkspaceStore` (Zustand store), `request-utils.ts` (API debugger utilities), and `result-utils.ts` (database result utilities). |
| Scope | Root `package.json` (add vitest devDependency + test script). New `vitest.config.ts` in `apps/desktop` or root. Initial test files in targeted packages. |
| Forbidden | Do not modify production code to accommodate tests. Do not modify `crates/*`. |
| Risk | Low. Test infrastructure is additive. |
| Prerequisites | TASK-03 (linter) is recommended first but not required. |
| Acceptance criteria | `pnpm run test` runs vitest and all initial tests pass. At least 3 test files covering pure utility functions and Zustand store logic. |
| Independent commit | Yes |

---

## TASK-05: Wire real SSH transport via russh

| Field | Value |
|---|---|
| Goal | Connect the `russh` SSH library (behind `ssh-native` feature) to the existing `SshService` session lifecycle. Replace the in-memory simulation with real SSH channel I/O: TCP connection, authentication (password and private key), PTY allocation, bidirectional data streaming, and graceful disconnection. |
| Scope | `crates/ssh-engine/src/ssh.rs` (primary). May need changes in `apps/desktop/src-tauri/src/command_bus.rs` for async event streaming. Frontend `packages/terminal/` may need Tauri event listeners for real-time terminal output. |
| Forbidden | Do not restructure the `SshService` public API (keep existing command signatures). Do not modify `packages/command-client/src/types.ts` (types are already correct). |
| Risk | High. SSH transport involves async I/O, error handling, host-key verification, key format parsing, and cross-platform considerations. The `russh` API may require significant adaptation. |
| Prerequisites | SSH host-key verification strategy decided (ISSUE from security.md). Windows NASM dependency resolved (already handled via `ring` backend). |
| Acceptance criteria | `cargo check -p unfour-workspace --features ssh-native` passes. Connecting to a real SSH server (e.g., localhost with OpenSSH) succeeds. Terminal output appears in xterm. Session close cleans up resources. Existing tests still pass. |
| Independent commit | Yes (can be split into: transport layer, auth, PTY, event streaming) |

---

## TASK-06: Add PostgreSQL/MySQL live connection support

| Field | Value |
|---|---|
| Goal | Implement live `test_connection`, `schema`, `execute_query`, and `browse_table` for PostgreSQL and MySQL drivers in `DatabaseService`. Use connection pooling via `sqlx` with credential references from `SecretStore`. |
| Scope | `crates/database-engine/src/database.rs` (primary). May need changes in `command_bus.rs` to pass `SecretStore` reference to `DatabaseService`. |
| Forbidden | Do not change the SQLite implementation. Do not modify frontend types or components. |
| Risk | High. Requires managing connection pools for multiple database types, handling authentication via credential store, and testing against live PostgreSQL/MySQL instances. |
| Prerequisites | `SecretStore.read_secret()` integration in `CommandBus` for database connections. Test infrastructure for PostgreSQL and MySQL (Docker or CI services). |
| Acceptance criteria | `test_connection` succeeds for a live PostgreSQL instance. Schema browsing returns tables and columns. SQL execution works for read queries. Mutation confirmation policy applies. Existing SQLite tests still pass. |
| Independent commit | Yes (can be split into: PostgreSQL, then MySQL) |

---

## TASK-07: Implement Vite code splitting

| Field | Value |
|---|---|
| Goal | Configure Vite `build.rollupOptions.output.manualChunks` to split the 914 KB bundle into smaller chunks. Target: separate Monaco Editor, xterm, and application code into independent chunks for better caching and initial load. |
| Scope | `apps/desktop/vite.config.ts` only. |
| Forbidden | Do not modify any source code. Do not change build targets or Tauri configuration. |
| Risk | Low. Configuration-only change. Risk is that chunk boundaries may affect runtime module loading order. |
| Prerequisites | None. |
| Acceptance criteria | Production build produces multiple chunks, each under 500 KB. Vite chunk size warning is eliminated. `pnpm run build` passes. App loads correctly in both Tauri and browser mock mode. |
| Independent commit | Yes |

---

## TASK-08: Add Rust tests for local-storage and CommandBus

| Field | Value |
|---|---|
| Goal | Add unit tests for `LocalDb::migrate()` (migration idempotency), `ActivityLogService::record()` (event recording and retrieval), and `CommandBus` orchestration (at least 2-3 end-to-end command flows using in-memory SQLite). |
| Scope | `crates/local-storage/src/` (add test module). `apps/desktop/src-tauri/src/` (add test module or integration test). |
| Forbidden | Do not modify production code unless needed for testability (e.g., making `LocalDb::from_pool` public, which it already is). |
| Risk | Low. Tests are additive. `LocalDb::from_pool` already exists for in-memory SQLite testing. |
| Prerequisites | None. |
| Acceptance criteria | `cargo test --workspace` passes with new tests. `unfour-local-storage` has at least 2 tests. `unfour-workspace` has at least 2 integration tests. |
| Independent commit | Yes |

---

## TASK-09: Remove deprecated module-boundaries.md

| Field | Value |
|---|---|
| Goal | Delete `docs/architecture/module-boundaries.md` since it is marked as DEPRECATED and superseded by `package-boundaries.md`. |
| Scope | `docs/architecture/module-boundaries.md` only. |
| Forbidden | Do not modify any other files. |
| Risk | None. |
| Prerequisites | Verify no other files reference `module-boundaries.md`. |
| Acceptance criteria | File is removed. No broken links in other docs. |
| Independent commit | Yes |

---

## TASK-10: Wire terminal search with xterm search addon

| Field | Value |
|---|---|
| Goal | Connect `TerminalSearchBar` to the `@xterm/addon-search` addon. Implement forward/backward search, highlight current match, and keyboard navigation. |
| Scope | `packages/terminal/src/components/TerminalSearchBar.tsx`, `packages/terminal/src/components/TerminalPane.tsx` (attach addon). |
| Forbidden | Do not modify `crates/*` or other packages. |
| Risk | Low. The addon is already installed; wiring is straightforward. |
| Prerequisites | xterm instance must be accessible from `TerminalSearchBar` (via ref or store). |
| Acceptance criteria | Typing in the search bar highlights matches in the terminal. Enter/Shift+Enter navigates between matches. "Search integration pending" placeholder is removed. |
| Independent commit | Yes |

---

## Recommended Priority Order

1. **TASK-03** (linter) -- establishes quality gate for all subsequent work
2. **TASK-01** (extract App.tsx) -- highest-leverage structural improvement
3. **TASK-02** (semantic tokens) -- visual consistency, follows naturally from TASK-01
4. **TASK-07** (code splitting) -- quick win, config-only
5. **TASK-05** (real SSH) -- highest-value feature gap
6. **TASK-04** (vitest) -- testing infrastructure
7. **TASK-08** (Rust tests) -- backend coverage
8. **TASK-06** (PostgreSQL/MySQL) -- second-highest feature gap
9. **TASK-10** (terminal search) -- small feature completion
10. **TASK-09** (remove deprecated doc) -- trivial cleanup
