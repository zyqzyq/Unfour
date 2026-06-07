# OPEN_ISSUES.md

> Read-only checkpoint scan. Each issue is classified by type and priority.

---

## ISSUE-04: Mock fallback in command-client is 972 lines

| Field | Value |
|---|---|
| Description | `packages/command-client/src/tauri.ts` contains 972 lines of `mockInvoke` implementation simulating all Tauri backend commands. This is intentional for browser-only development but is a significant maintenance burden. Any new Tauri command requires a corresponding mock implementation. The mock state is module-level mutable globals, which could cause issues in test scenarios. |
| File | `packages/command-client/src/tauri.ts` |
| Type | Observed |
| Priority | P2 |
| Blocks development | No |

---

## ISSUE-07: SSH sessions are simulated, not real

| Field | Value |
|---|---|
| Description | `unfour-ssh-engine` provides a complete session lifecycle API (connect, input, resize, close, log export), but all sessions are simulated in-memory using a `HashMap`. The `russh` dependency is gated behind the `ssh-native` feature flag and is not connected to the I/O path. The frontend `TerminalPane` has a command input but no real terminal I/O. |
| File | `crates/ssh-engine/src/ssh.rs`, `packages/terminal/src/components/TerminalPane.tsx` |
| Type | Observed |
| Priority | P1 |
| Blocks development | No, but SSH is not functional for end users |

---

## ISSUE-08: PostgreSQL/MySQL connections are metadata-only

| Field | Value |
|---|---|
| Description | `unfour-database-engine` supports CRUD for PostgreSQL and MySQL connection metadata, but `test_connection`, `schema`, `execute_query`, and `browse_table` only work for SQLite. Non-SQLite drivers return "reserved for next phase" messages. The `sqlx` dependency includes postgres and mysql features, but no live connection pool management exists for them. |
| File | `crates/database-engine/src/database.rs` |
| Type | Observed |
| Priority | P2 |
| Blocks development | No |

---

## ISSUE-09: CSP is disabled

| Field | Value |
|---|---|
| Description | `tauri.conf.json` sets `security.csp: null`, meaning no Content Security Policy is enforced. For a desktop application handling user credentials and making external HTTP requests, this is a security consideration. |
| File | `apps/desktop/src-tauri/tauri.conf.json` |
| Type | Observed |
| Priority | P2 |
| Blocks development | No |

---

## ISSUE-12: Terminal search is a placeholder

| Field | Value |
|---|---|
| Description | `TerminalSearchBar.tsx` renders `<Input placeholder="Search integration pending" />`. The `@xterm/addon-search` dependency is installed but not wired up. |
| File | `packages/terminal/src/components/TerminalSearchBar.tsx` |
| Type | Observed |
| Priority | P3 |
| Blocks development | No |

---

## ISSUE-13: Duplicate ContextMenuItem is disabled, not implemented

| Field | Value |
|---|---|
| Description | In `SshConnectionTree.tsx`, the "Duplicate Connection" context menu item is rendered with `disabled` prop. This is a planned feature with no implementation. |
| File | `packages/terminal/src/components/SshConnectionTree.tsx` |
| Type | Observed |
| Priority | P3 |
| Blocks development | No |

---

## ISSUE-14: Database Duplicate Tab is not implemented

| Field | Value |
|---|---|
| Description | In `DatabaseModuleToolbar.tsx`, the "Duplicate Tab" dropdown menu item has no onClick handler and no implementation. |
| File | `packages/database/src/components/DatabaseModuleToolbar.tsx` |
| Type | Observed |
| Priority | P3 |
| Blocks development | No |

---

## ISSUE-15: `packages/ui` contains shell layout components

| Field | Value |
|---|---|
| Description | `packages/ui/src/shell.tsx` exports AppShellFrame, GlobalToolbar, Sidebar, MainWorkspace, TabBar, StatusBar, SplitPane, BottomPanel, RightInspector, and CommandPalette. Per `docs/architecture/package-boundaries.md`, some of these shell layout components may belong in `packages/app-shell`. The `ui` package rule states it MUST NOT contain feature-specific business logic, and these shell components are borderline. |
| File | `packages/ui/src/shell.tsx` |
| Type | Inferred |
| Priority | P3 |
| Blocks development | No |

---

## ISSUE-16: Feature packages depend on @unfour/workspace (transitional)

| Field | Value |
|---|---|
| Description | `packages/database` and `packages/terminal` import `useWorkspaceStore` from `@unfour/workspace`. Per `docs/architecture/package-boundaries.md`, this is documented as a transitional exception. The dependency direction is allowed but should be revisited when workspace state management is further abstracted. |
| Files | `packages/database/src/DatabasePage.tsx`, `packages/terminal/src/TerminalPage.tsx`, `packages/terminal/src/components/TerminalStatusBar.tsx`, `packages/terminal/src/components/SshConnectionTree.tsx` |
| Type | Inferred |
| Priority | P3 |
| Blocks development | No |

---

## ISSUE-17: `@ts-expect-error` in vite.config.ts

| Field | Value |
|---|---|
| Description | `apps/desktop/vite.config.ts` line 5 contains `// @ts-expect-error process is a nodejs global`. This suppresses a TypeScript error for accessing `process` in a Vite config context. Not harmful but indicates a typing gap. |
| File | `apps/desktop/vite.config.ts` |
| Type | Observed |
| Priority | P3 |
| Blocks development | No |

---

## ISSUE-19: No package-level README files

| Field | Value |
|---|---|
| Description | None of the 7 frontend packages or 7 Rust crates have README.md files. Each package relies on its `package.json` / `Cargo.toml` and source code for documentation. AGENTS.md instructs agents to "read any local README or notes in the target package," but none exist. |
| Files | All `packages/*/` and `crates/*/` |
| Type | Observed |
| Priority | P3 |
| Blocks development | No |

---

## ISSUE-22: Bottom panel and right inspector contain only placeholders

| Field | Value |
|---|---|
| Description | The bottom panel (non-SSH mode) renders "Local activity and module diagnostics will appear here" and the right inspector renders "[module] details and properties will use this space." These are shell-level features that have been structurally created but never populated with real content. Components live in `apps/desktop/src/components/BottomPanelPlaceholder.tsx` and `RightInspectorPlaceholder.tsx`. |
| Files | `apps/desktop/src/components/BottomPanelPlaceholder.tsx`, `apps/desktop/src/components/RightInspectorPlaceholder.tsx` |
| Type | Observed |
| Priority | P3 |
| Blocks development | No |

---

## ISSUE-23: `ai_reserved` and `sync_reserved` are stub-only

| Field | Value |
|---|---|
| Description | `crates/unfour-core/src/ai_reserved.rs` and `crates/unfour-core/src/sync_reserved.rs` contain placeholder types (`AppCommand` enum hierarchy, `SyncStatus`, `SyncPolicy`) with no consumers. These serve as boundary reservations but have no runtime impact. |
| Files | `crates/unfour-core/src/ai_reserved.rs`, `crates/unfour-core/src/sync_reserved.rs` |
| Type | Observed |
| Priority | P3 |
| Blocks development | No |

---

## Resolved Issues

The following issues were resolved during frontend hardening:

- **ISSUE-01** (P1): App.tsx oversized composition root -- extracted into `AppTitleBar`, `ModuleSidebar`, `WindowControls`, `WorkspaceMenu`, `WorkspaceDialogs`, `BottomPanelPlaceholder`, `StatusBarPlaceholder`, `RightInspectorPlaceholder`, and utility modules. App.tsx reduced from 954 to 199 lines.
- **ISSUE-02** (P1): Hardcoded Tailwind colors in App.tsx -- all 23 instances migrated to `--u-color-*` semantic tokens.
- **ISSUE-03** (P2): Hardcoded Tailwind colors in feature packages -- all 7 instances migrated to `--u-color-*` / `--u-badge-*` tokens.
- **ISSUE-10** (P2): Workspace CRUD UI inline in App.tsx -- extracted into `WorkspaceMenu` and `WorkspaceDialogs` components.
- **ISSUE-11** (P3): Window controls inline in App.tsx -- extracted into `WindowControls` component.
- **ISSUE-18** (P3): Deprecated `module-boundaries.md` -- deleted; all references point to `package-boundaries.md`.
- **ISSUE-21** (P2): Vite chunk size warning -- configured `manualChunks` to split monaco, xterm, tanstack, and radix into separate chunks. All chunks under 500 KB.

The following issues were resolved during the quality gate foundation batch:

- **ISSUE-05** (P1): No frontend linter configured -- ESLint 10 flat config added with typescript-eslint, react-hooks v7, react-refresh, no-explicit-any enforcement, and unused-vars checking. `pnpm run lint` passes with 0 errors.
- **ISSUE-06** (P2): No frontend test framework -- Vitest 4 configured with 3 test files: workspace store (12 tests), API request utilities (20 tests), database result utilities (16 tests). `pnpm run test` passes with 48 tests.
- **ISSUE-20** (P2): Rust tests missing for local-storage and desktop app -- Added 6 tests for local-storage (migration idempotency, table creation, column migration, activity log event recording, multiple events, JSON details) and 3 CommandBus integration tests (workspace create/list, API request save/list, workspace rename). `cargo test --workspace` passes with 39 tests total.

---

## Summary

| Priority | Count | Types |
|---|---|---|
| P0 | 0 | -- |
| P1 | 1 | ISSUE-07 |
| P2 | 3 | ISSUE-04, ISSUE-08, ISSUE-09 |
| P3 | 9 | ISSUE-12, ISSUE-13, ISSUE-14, ISSUE-15, ISSUE-16, ISSUE-17, ISSUE-19, ISSUE-22, ISSUE-23 |

No issues are currently blocking continued development. The P1 item (real SSH transport) represents the highest-leverage improvement for feature completeness.
