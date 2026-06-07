# OPEN_ISSUES.md

> Read-only checkpoint scan. Each issue is classified by type and priority.

---

## ISSUE-01: App.tsx is an oversized composition root

| Field | Value |
|---|---|
| Description | `apps/desktop/src/App.tsx` is 954 lines. It mixes shell composition, workspace CRUD dialogs (create/rename/delete with mutations), custom title bar with window controls, module sidebar routing, command palette, layout persistence, and placeholder content. This makes the file hard to navigate and violates the principle of separation of concerns. |
| File | `apps/desktop/src/App.tsx` |
| Type | Observed |
| Priority | P1 |
| Blocks development | No, but slows feature integration and increases merge conflict risk |

---

## ISSUE-02: Hardcoded Tailwind colors in App.tsx

| Field | Value |
|---|---|
| Description | `App.tsx` contains 23 instances of hardcoded Tailwind color classes (`bg-white`, `bg-slate-100`, `text-slate-500`, `border-slate-200`, `bg-rose-700`, `bg-teal-50`, etc.) instead of using semantic `--u-color-*` CSS custom properties defined in the design token system. The UI guidelines (`docs/ui/ui-guidelines.md`) explicitly require semantic tokens only. |
| File | `apps/desktop/src/App.tsx` (lines 547-882) |
| Type | Observed |
| Priority | P1 |
| Blocks development | No, but creates visual inconsistency and prevents theme changes |

---

## ISSUE-03: Hardcoded Tailwind colors in feature packages

| Field | Value |
|---|---|
| Description | 7 instances of hardcoded Tailwind colors found across `packages/ui/src/badge.tsx` (3), `packages/api-debugger/src/components/ApiRequestEditor.tsx` (1), `packages/api-debugger/src/components/ResponseTabs.tsx` (1), and `packages/api-debugger/src/components/RequestParamsTabs.tsx` (2). |
| Files | `packages/ui/src/badge.tsx`, `packages/api-debugger/src/components/ApiRequestEditor.tsx`, `packages/api-debugger/src/components/ResponseTabs.tsx`, `packages/api-debugger/src/components/RequestParamsTabs.tsx` |
| Type | Observed |
| Priority | P2 |
| Blocks development | No |

---

## ISSUE-04: Mock fallback in command-client is ~900 lines

| Field | Value |
|---|---|
| Description | `packages/command-client/src/tauri.ts` contains approximately 900 lines of `mockInvoke` implementation simulating all 36 Tauri backend commands. This is intentional for browser-only development but is a significant maintenance burden. Any new Tauri command requires a corresponding mock implementation. The mock state is module-level mutable globals, which could cause issues in test scenarios. |
| File | `packages/command-client/src/tauri.ts` |
| Type | Observed |
| Priority | P2 |
| Blocks development | No |

---

## ISSUE-05: No frontend linter configured

| Field | Value |
|---|---|
| Description | `package.json` has no lint script. No `.eslintrc`, `biome.json`, or equivalent configuration file exists. The project relies solely on TypeScript's `strict` mode and `tsc` for static analysis. This leaves style inconsistencies, potential unused imports, and code quality issues undetected. |
| File | `package.json` (root) |
| Type | Observed |
| Priority | P1 |
| Blocks development | No, but degrades code quality over time |

---

## ISSUE-06: No frontend test framework

| Field | Value |
|---|---|
| Description | No test runner (vitest, jest) is configured for the TypeScript packages. The Rust backend has 30 passing tests, but the frontend has zero test coverage. Feature packages (`api-debugger`, `database`, `terminal`) contain hooks and state management logic that would benefit from unit tests. |
| File | `package.json` (root), all `packages/*/` |
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

## ISSUE-10: Workspace CRUD UI lives in apps/desktop, not in a package

| Field | Value |
|---|---|
| Description | The `WorkspaceMenu`, `WorkspaceDialog`, and workspace delete confirmation dialog are defined inline in `App.tsx` (lines 486-730). These include `useMutation` calls for create, rename, and delete operations. Per the architecture rules, workspace management state should be accessible from feature packages, but the UI for managing workspaces is currently locked in the composition root. |
| File | `apps/desktop/src/App.tsx` (lines 486-730) |
| Type | Observed |
| Priority | P2 |
| Blocks development | No |

---

## ISSUE-11: Window controls are inline in App.tsx

| Field | Value |
|---|---|
| Description | `WindowControls` and `TitlebarWindowButton` components (lines 832-891) handle Tauri window minimize/maximize/close/drag. These are general shell concerns that could be extracted to `packages/app-shell` or `packages/ui` for reuse. |
| File | `apps/desktop/src/App.tsx` (lines 832-891) |
| Type | Observed |
| Priority | P3 |
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
| File | `packages/terminal/src/components/SshConnectionTree.tsx` (line 167) |
| Type | Observed |
| Priority | P3 |
| Blocks development | No |

---

## ISSUE-14: Database Duplicate Tab is not implemented

| Field | Value |
|---|---|
| Description | In `DatabaseModuleToolbar.tsx`, the "Duplicate Tab" dropdown menu item has no onClick handler and no implementation. |
| File | `packages/database/src/components/DatabaseModuleToolbar.tsx` (line 86) |
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

## ISSUE-18: Deprecated doc still present

| Field | Value |
|---|---|
| Description | `docs/architecture/module-boundaries.md` is marked as DEPRECATED and superseded by `package-boundaries.md`. It remains in the repository and may confuse AI agents or new contributors. |
| File | `docs/architecture/module-boundaries.md` |
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

## ISSUE-20: Rust tests missing for local-storage and desktop app

| Field | Value |
|---|---|
| Description | `unfour-local-storage` (0 tests) and `unfour-workspace` desktop app (0 tests) have no Rust tests. The `LocalDb` migration logic and `ActivityLogService` are untested at the unit level. The desktop app's `CommandBus` orchestration is also untested. |
| Files | `crates/local-storage/src/`, `apps/desktop/src-tauri/src/` |
| Type | Observed |
| Priority | P2 |
| Blocks development | No |

---

## ISSUE-21: Vite chunk size warning (> 500 KB)

| Field | Value |
|---|---|
| Description | The production build produces a single 914 KB JavaScript chunk. Vite warns about chunks over 500 KB. Monaco Editor and xterm are major contributors. No code splitting via dynamic `import()` or `manualChunks` is configured. |
| File | `apps/desktop/vite.config.ts` |
| Type | Observed |
| Priority | P2 |
| Blocks development | No |

---

## ISSUE-22: Bottom panel and right inspector contain only placeholders

| Field | Value |
|---|---|
| Description | In `App.tsx`, the bottom panel (non-SSH mode) renders "Local activity and module diagnostics will appear here" and the right inspector renders "[module] details and properties will use this space." These are shell-level features that have been structurally created but never populated with real content. |
| File | `apps/desktop/src/App.tsx` (lines 226-276) |
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

## Summary

| Priority | Count | Types |
|---|---|---|
| P0 | 0 | -- |
| P1 | 4 | ISSUE-01, ISSUE-02, ISSUE-05, ISSUE-07 |
| P2 | 7 | ISSUE-03, ISSUE-04, ISSUE-06, ISSUE-08, ISSUE-09, ISSUE-10, ISSUE-20, ISSUE-21 |
| P3 | 9 | ISSUE-11 through ISSUE-19, ISSUE-22, ISSUE-23 |

No issues are currently blocking continued development. The P1 items represent the highest-leverage improvements for code quality and developer velocity.
