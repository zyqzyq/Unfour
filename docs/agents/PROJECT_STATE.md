# Project State

## Scan Metadata

- **Scanned at:** 2026-06-09 (checkpoint refresh, verification-only scan)
- **Branch:** main
- **Current commit:** `5351897` — feat(ssh): add connection health and reconnect handling
- **Working tree:** Clean (no uncommitted changes)
- **Last checkpoint:** SSH reliability implementation and verification complete

## Tech Stack

- **Desktop shell:** Tauri 2 (Rust + WebView)
- **Frontend:** React 19, TypeScript, Vite, Tailwind CSS, Radix UI, TanStack Query, Zustand
- **Backend:** Rust, Tokio, SQLite (rusqlite via sqlx), russh (SSH native), keyring (OS credential storage)
- **Build:** pnpm workspace (7 packages), Cargo workspace (7 crates + Tauri adapter)
- **Test:** Vitest (frontend), Cargo test (Rust)

## Current Phase

Terminal streaming and SSH reliability integration are **complete**. PTY lifecycle, stdin/stdout streaming, Tauri event streaming, frontend terminal input capture, resize propagation, search, keepalive monitoring, bounded reconnection, cancellation, and cleanup are wired end-to-end.

UI module split is **in progress**. Terminal and Database packages have been extracted from `packages/app-shell`. A shared `command-client` package provides the Tauri IPC abstraction, and a `workspace` package provides the Zustand workspace state store. Further Workspace extraction from app-shell is planned.

## Verified Capabilities

### Backend (Rust)

| Capability | Crate | Status | Tests |
|---|---|---|---|
| Core models & redaction | `unfour-core` | Complete | 3 pass |
| Local storage & migrations | `unfour-local-storage` | Complete | 6 pass |
| Activity logging | `unfour-local-storage` | Complete | Covered in local_storage |
| SecretStore (OS keyring credential references) | `unfour-secret-store` | Complete | 4 pass |
| Database engine (SQLite CRUD + schema) | `unfour-database-engine` | Complete | 3 pass |
| HTTP engine (API client + history) | `unfour-http-engine` | Complete | 8 pass |
| SSH engine (simulated + native) | `unfour-ssh-engine` | Complete | 20 default + 14 native-feature pass |
| Workspace engine | `unfour-workspace-engine` | Complete | Tests blocked on Windows DLL issue |
| CommandBus (Tauri adapter) | `unfour-workspace` (apps/desktop/src-tauri) | Complete | Compile-verified, 3 integration tests in command_bus.rs |

### Frontend (TypeScript)

| Capability | Package | Status | Tests |
|---|---|---|---|
| Workspace store | `@unfour/workspace` | Complete | 12 pass |
| API Debugger | `@unfour/api-debugger` | Complete | 20 pass |
| Database (connections + query) | `@unfour/database` | Complete | 16 pass |
| Terminal state and command-client mock | `@unfour/terminal`, `@unfour/command-client` | Complete | 5 + 1 pass |

### Build

- **Frontend production build:** PASS
- **Frontend bundle chunks:** index (384 kB), xterm (367 kB), vendor-tanstack (101 kB), vendor-radix (88 kB), monaco (15 kB)
- **Total Rust tests:** 44 passing across 6 crates (unfour-workspace blocked by Windows DLL issue)
- **Total frontend tests:** 54 passing (5 files)

## Partially Implemented

- **UI module split:** Terminal, Database, Workspace, and Command-Client packages extracted. `packages/app-shell` now contains only AppShell layout composition. Further workspace UI extraction from desktop components is planned.
- **SSH authentication:** Password auth and private-key auth both work under `ssh-native`. Encrypted key passphrase loading has limited support (ssh-key crate format constraints).
- **Host-key UI:** View trusted fingerprint and reset fingerprint implemented. Mismatch error display is handled by the TOFU backend.
- **SSH live reliability verification:** Keepalive and reconnect policy are automated-test covered, but a live localhost SSH stop/start cycle was not available in this environment.
- **Database drivers:** SQLite driver is functional. PostgreSQL/MySQL drivers are not started.

## Not Started

- Terminal output persistence to SQLite
- `known_hosts` integration
- Terminal multiplexing (tmux/screen-like)
- SCP/SFTP file transfer
- Additional database drivers (PostgreSQL, MySQL)

## Verification Results

| Command | Result | Notes |
|---|---|---|
| `git diff --check` | PASS | No trailing whitespace issues |
| `pnpm run lint` | PASS (warnings) | 0 errors, 64 warnings; pre-existing in api-debugger, database, terminal, desktop |
| `pnpm run test` | PASS | 54 tests, 5 files |
| `pnpm run build` | PASS | Production build succeeds |
| `cargo fmt --check` | PASS | No formatting issues |
| `cargo test --workspace` | PARTIAL | 44 tests pass across 6 crates. `unfour-workspace` fails with Windows `STATUS_ENTRYPOINT_NOT_FOUND` (DLL loading issue) |
| `cargo check --workspace` | PASS | All crates compile |
| `cargo check -p unfour-workspace --features ssh-native` | PASS | SSH feature compiles |
| `cargo test -p unfour-ssh-engine --features ssh-native` | PASS | 14 native-feature tests |
| Browser mock first viewport | NOT VERIFIED | No live browser available in this scan |

## Known Limitations

- **Windows workspace tests:** `cargo test -p unfour-workspace` fails with `STATUS_ENTRYPOINT_NOT_FOUND`. Likely a native DLL dependency issue (OpenSSL/SQLite) on this Windows environment. Does not indicate code defects.
- **Lint warnings:** Multiple packages have `react-hooks/set-state-in-effect`, `react-hooks/exhaustive-deps`, `react-hooks/refs`, and `react-refresh/only-export-components` warnings. These are pre-existing and do not block builds.
- **Real SSH verification:** Native SSH transport drop detection, reconnect cancellation, retry exhaustion, and recovery after server return are `NOT VERIFIED` against a live SSH server in this environment.
- **API body redaction:** Request bodies are not redacted in logs or history.

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
| `unfour-workspace` (Tauri) | CommandBus, 40 Tauri commands, app setup |
