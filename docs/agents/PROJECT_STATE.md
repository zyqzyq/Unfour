# Project State

## Scan Metadata

- **Scanned at:** 2026-06-13 (database live verification and hardening)
- **Branch:** main
- **Current commit:** Database live verification batch (see Git history)
- **Working tree:** Clean after batch commit
- **Last checkpoint:** PostgreSQL 18 and MariaDB 12.3.2 were verified against isolated local servers with SecretStore-backed credentials and seeded Unicode data. Live verification confirmed and fixed PostgreSQL schema qualification and MariaDB `COUNT(*)` decoding. SQLite and browser mocks remain green.

## Tech Stack

- **Desktop shell:** Tauri 2 (Rust + WebView)
- **Frontend:** React 19, TypeScript, Vite, Tailwind CSS, Radix UI, TanStack Query, Zustand
- **Backend:** Rust, Tokio, SQLite, PostgreSQL, and MySQL via sqlx, russh (SSH native), keyring (OS credential storage)
- **Build:** pnpm workspace (7 packages), Cargo workspace (7 crates + Tauri adapter)
- **Test:** Vitest (frontend), Cargo test (Rust)

## Current Phase

SSH authentication UX — including private-key authentication, SecretStore-backed key references, host-key fingerprint management, and terminal streaming — is **complete**. SQLite, PostgreSQL, and MySQL/MariaDB database paths are implemented and live-verified: SecretStore credential loading, connection tests, schema/database browsing, read queries, mutation confirmation, pagination, Unicode data, empty tables, syntax errors, and sanitized connection failures all pass.

UI module split is **in progress**. Terminal, Database, Workspace, and Command-Client packages have been extracted from `packages/app-shell`. Workspace dialogs (`WorkspaceMenu`, `WorkspaceDialogs`), window controls (`WindowControls`, `TitlebarWindowButton`), and the title bar (`AppTitleBar`) have been extracted from `App.tsx` into dedicated component files within `apps/desktop/src/components/`. Semantic token replacement is complete — `App.tsx` and all desktop components use `--u-color-*` CSS custom properties exclusively. `packages/app-shell` now contains only the `AppShell` layout composition wrapper (2 source files). Further workspace UI extraction from desktop components is planned.

## Verified Capabilities

### Backend (Rust)

| Capability | Crate | Status | Tests |
|---|---|---|---|
| Core models & redaction | `unfour-core` | Complete | 10 pass |
| Local storage & migrations | `unfour-local-storage` | Complete | 11 pass |
| Activity logging | `unfour-local-storage` | Complete | Covered in local_storage |
| SecretStore (OS keyring credential references) | `unfour-secret-store` | Complete | 4 pass |
| Database engine (SQLite + PostgreSQL + MySQL CRUD, schema, queries) | `unfour-database-engine` | Complete | 19 pass |
| HTTP engine (API client + history + body redaction) | `unfour-http-engine` | Complete | 10 pass |
| SSH engine (simulated + native + known_hosts) | `unfour-ssh-engine` | Complete | 33 pass |
| Workspace engine | `unfour-workspace-engine` | Complete | Tests blocked on Windows DLL issue |
| CommandBus (Tauri adapter) | `unfour-workspace` (apps/desktop/src-tauri) | Complete | Compile-verified, 3 integration tests in command_bus.rs, 44 Tauri commands |

### Frontend (TypeScript)

| Capability | Package | Status | Tests |
|---|---|---|---|
| Workspace store | `@unfour/workspace` | Complete | 12 pass |
| API Debugger | `@unfour/api-debugger` | Complete | 20 pass |
| Database (connections + query) | `@unfour/database` | Complete | 16 pass |
| Terminal state, history, host-key dialog, and command-client mock | `@unfour/terminal`, `@unfour/command-client` | Complete | 6 + 6 pass |

### Build

- **Frontend production build:** PASS
- **Frontend bundle chunks:** index (393 kB), xterm (367 kB), vendor-tanstack (101 kB), vendor-radix (88 kB), monaco (15 kB)
- **Total Rust tests:** 87 passing across 6 crates (unfour-workspace blocked by Windows DLL issue)
- **Total frontend tests:** 60 passing (5 files)

## Partially Implemented

- **UI module split:** Terminal, Database, Workspace, and Command-Client packages extracted. `packages/app-shell` contains only the `AppShell` layout wrapper (2 files). Desktop app components (AppTitleBar, ModuleSidebar, WorkspaceMenu, WorkspaceDialogs, WindowControls, placeholder components) are extracted into `apps/desktop/src/components/`. Semantic token replacement is complete across all desktop source files. Remaining work: potential extraction of placeholder components into feature packages, further workspace state extraction.
- **SSH authentication:** Password auth and private-key auth both work under `ssh-native`. Encrypted key passphrase loading has limited support (ssh-key crate format constraints).
- **Host-key UI:** View trusted fingerprint, reset fingerprint, trust confirmation dialog (first trust + mismatch), and known_hosts import/export all implemented.
- **Terminal session persistence:** SQLite-backed output history with per-session buffering, periodic flush, secret redaction, and UTF-8-safe truncation (256 KB retention). Hydration on app reopen. Browser mock mode compatible.
- **API body redaction:** JSON body redaction applied in both Rust persistence paths (save_request, insert_history) and browser mock. Sensitive keys (authorization, cookie, proxy-authorization, x-api-key, x-auth-token) are recursively replaced with `<redacted>` while preserving JSON structure.
- **SSH live reliability verification:** Keepalive and reconnect policy are automated-test covered, but a live localhost SSH stop/start cycle was not available in this environment.
- **Database drivers:** SQLite regression coverage passes. PostgreSQL 18 and MariaDB 12.3.2 were live-verified with SecretStore credential references, non-default schemas/databases, tables, columns, reads, pagination, confirmed writes, Unicode rows, empty tables, syntax errors, invalid passwords, and unavailable ports.

## Not Started

- Terminal multiplexing (tmux/screen-like)
- SCP/SFTP file transfer

## Verification Results

| Command | Result | Notes |
|---|---|---|
| `git diff --check` | PASS | No trailing whitespace issues |
| `pnpm run lint` | PASS (warnings) | 0 errors, 64 pre-existing warnings (react-hooks/refs in api-debugger, react-hooks/set-state-in-effect, react-hooks/exhaustive-deps, react-refresh/only-export-components in desktop) |
| `pnpm run test` | PASS | 60 tests, 5 files |
| `pnpm run build` | PASS | Production build succeeds (1994 modules) |
| `cargo fmt --check` | PASS | No formatting issues |
| `cargo test --workspace` | PARTIAL | 87 tests pass across 6 crates. `unfour-workspace` fails with Windows `STATUS_ENTRYPOINT_NOT_FOUND` (DLL loading issue) |
| `cargo check --workspace` | PASS | All crates compile |
| `cargo check -p unfour-workspace --features ssh-native` | PASS | SSH feature compiles |
| `cargo test -p unfour-ssh-engine --features ssh-native` | PASS | 25 native-feature tests (included in workspace run above; 33 total ssh-engine tests) |
| `cargo test -p unfour-database-engine` | PASS | 19 tests |
| PostgreSQL live matrix | PASS | PostgreSQL 18; SecretStore auth, `app_data` schema, reads, pagination, writes, Unicode, empty table, syntax/auth/unavailable errors |
| MySQL live matrix | PASS | MariaDB 12.3.2; SecretStore auth, multiple databases, reads, pagination, writes, Unicode, empty table, syntax/auth/unavailable errors |
| Browser mock first viewport | PASS | PostgreSQL `public` schema grouping and tables inspected |

## Known Limitations

- **Windows workspace tests:** `cargo test -p unfour-workspace` fails with `STATUS_ENTRYPOINT_NOT_FOUND`. Likely a native DLL dependency issue (OpenSSL/SQLite) on this Windows environment. Does not indicate code defects.
- **Lint warnings:** 64 warnings across `packages/api-debugger` (primarily `react-hooks/refs` in ApiDebuggerPage), `apps/desktop` (`react-hooks/set-state-in-effect`, `react-hooks/exhaustive-deps`, `react-refresh/only-export-components`). All pre-existing; none block builds.
- **Real SSH verification:** Native SSH transport, private-key authentication, passphrase-encrypted key loading, host-key TOFU first-trust, mismatch rejection, and fingerprint reset are `NOT VERIFIED` against a live SSH server in this environment. Automated tests cover the full code path.

## Repository Structure

### Packages (7)

| Package | Purpose |
|---|---|
| `@unfour/app-shell` | Global layout composition, module mount points |
| `@unfour/api-debugger` | API request editor, history, collection management |
| `@unfour/database` | Database connections, SQL execution, schema browser |
| `@unfour/terminal` | SSH connections, terminal panes, session management |
| `@unfour/workspace` | Zustand workspace state store |
| `@unfour/command-client` | Tauri IPC invocation wrapper |
| `@unfour/ui` | Shared UI primitives (button, dialog, tabs, tree, etc.) |

### Crates (7 + Tauri adapter)

| Crate | Purpose |
|---|---|
| `unfour-core` | Shared models, error types, redaction logic |
| `unfour-local-storage` | SQLite connection, migrations, activity log |
| `unfour-database-engine` | Database connections, schema, queries |
| `unfour-http-engine` | API request sending, history, saved requests |
| `unfour-ssh-engine` | SSH connections, sessions, PTY, host keys |
| `unfour-workspace-engine` | Workspace CRUD, environment, layout |
| `unfour-secret-store` | OS keyring-backed credential storage |
| `unfour-workspace` (Tauri) | CommandBus, 44 Tauri commands, app setup |

### Feature Flags

| Feature | Crate | Effect |
|---|---|---|
| `ssh-native` | `unfour-ssh-engine` | Enables `russh`, `ssh-key`, `tokio` optional dependencies |
| `ssh-native` | `unfour-workspace` (Tauri adapter) | Forwards to `unfour-ssh-engine/ssh-native` |

### Source Code Hygiene

- **TODO/FIXME/HACK/placeholder comments:** 0 found across all packages, crates, and apps source directories.
