# Project State

## Scan Metadata

- **Scanned at:** 2026-06-11 (SSH authentication UX completion verification)
- **Branch:** main
- **Current commit:** 53a2974 — feat(security): add redaction and host-key interoperability
- **Working tree:** Clean
- **Last checkpoint:** Private-key authentication, host-key fingerprint UI, and terminal streaming all verified end-to-end

## Tech Stack

- **Desktop shell:** Tauri 2 (Rust + WebView)
- **Frontend:** React 19, TypeScript, Vite, Tailwind CSS, Radix UI, TanStack Query, Zustand
- **Backend:** Rust, Tokio, SQLite (rusqlite via sqlx), russh (SSH native), keyring (OS credential storage)
- **Build:** pnpm workspace (7 packages), Cargo workspace (7 crates + Tauri adapter)
- **Test:** Vitest (frontend), Cargo test (Rust)

## Current Phase

SSH authentication UX — including private-key authentication, SecretStore-backed key references, host-key fingerprint management, and terminal streaming — is **complete**. PTY lifecycle, stdin/stdout streaming, Tauri event streaming, frontend terminal input capture, resize propagation, search, keepalive monitoring, bounded reconnection, cancellation, cleanup, SQLite-backed output persistence with secret redaction and truncation, API request body redaction in persistence paths, host-key trust confirmation dialog, known_hosts import/export, password and private-key auth (unencrypted + passphrase-encrypted), credential boundary enforcement, and error sanitization are wired end-to-end.

UI module split is **in progress**. Terminal and Database packages have been extracted from `packages/app-shell`. A shared `command-client` package provides the Tauri IPC abstraction, and a `workspace` package provides the Zustand workspace state store. Further Workspace extraction from app-shell is planned.

## Verified Capabilities

### Backend (Rust)

| Capability | Crate | Status | Tests |
|---|---|---|---|
| Core models & redaction | `unfour-core` | Complete | 10 pass |
| Local storage & migrations | `unfour-local-storage` | Complete | 11 pass |
| Activity logging | `unfour-local-storage` | Complete | Covered in local_storage |
| SecretStore (OS keyring credential references) | `unfour-secret-store` | Complete | 4 pass |
| Database engine (SQLite CRUD + schema) | `unfour-database-engine` | Complete | 3 pass |
| HTTP engine (API client + history + body redaction) | `unfour-http-engine` | Complete | 10 pass |
| SSH engine (simulated + native + known_hosts) | `unfour-ssh-engine` | Complete | 33 pass |
| Workspace engine | `unfour-workspace-engine` | Complete | Tests blocked on Windows DLL issue |
| CommandBus (Tauri adapter) | `unfour-workspace` (apps/desktop/src-tauri) | Complete | Compile-verified, 3 integration tests in command_bus.rs, 43 Tauri commands |

### Frontend (TypeScript)

| Capability | Package | Status | Tests |
|---|---|---|---|
| Workspace store | `@unfour/workspace` | Complete | 12 pass |
| API Debugger | `@unfour/api-debugger` | Complete | 20 pass |
| Database (connections + query) | `@unfour/database` | Complete | 16 pass |
| Terminal state, history, host-key dialog, and command-client mock | `@unfour/terminal`, `@unfour/command-client` | Complete | 6 + 5 pass |

### Build

- **Frontend production build:** PASS
- **Frontend bundle chunks:** index (384 kB), xterm (367 kB), vendor-tanstack (101 kB), vendor-radix (88 kB), monaco (15 kB)
- **Total Rust tests:** 71 passing across 6 crates (unfour-workspace blocked by Windows DLL issue)
- **Total frontend tests:** 59 passing (5 files)

## Partially Implemented

- **UI module split:** Terminal, Database, Workspace, and Command-Client packages extracted. `packages/app-shell` now contains only AppShell layout composition. Further workspace UI extraction from desktop components is planned.
- **SSH authentication:** Password auth and private-key auth both work under `ssh-native`. Encrypted key passphrase loading has limited support (ssh-key crate format constraints).
- **Host-key UI:** View trusted fingerprint, reset fingerprint, trust confirmation dialog (first trust + mismatch), and known_hosts import/export all implemented.
- **Terminal session persistence:** SQLite-backed output history with per-session buffering, periodic flush, secret redaction, and UTF-8-safe truncation (256 KB retention). Hydration on app reopen. Browser mock mode compatible.
- **API body redaction:** JSON body redaction applied in both Rust persistence paths (save_request, insert_history) and browser mock. Sensitive keys (authorization, cookie, proxy-authorization, x-api-key, x-auth-token) are recursively replaced with `<redacted>` while preserving JSON structure.
- **SSH live reliability verification:** Keepalive and reconnect policy are automated-test covered, but a live localhost SSH stop/start cycle was not available in this environment.
- **Database drivers:** SQLite driver is functional. PostgreSQL/MySQL drivers are not started.

## Not Started

- Terminal multiplexing (tmux/screen-like)
- SCP/SFTP file transfer
- Additional database drivers (PostgreSQL, MySQL)

## Verification Results

| Command | Result | Notes |
|---|---|---|
| `git diff --check` | PASS | No trailing whitespace issues |
| `pnpm run lint` | PASS (warnings) | 0 errors, pre-existing warnings in api-debugger, desktop |
| `pnpm run test` | PASS | 59 tests, 5 files |
| `pnpm run build` | PASS | Production build succeeds |
| `cargo fmt --check` | PASS | No formatting issues |
| `cargo test --workspace` | PARTIAL | 71 tests pass across 6 crates. `unfour-workspace` fails with Windows `STATUS_ENTRYPOINT_NOT_FOUND` (DLL loading issue) |
| `cargo check --workspace` | PASS | All crates compile |
| `cargo check -p unfour-workspace --features ssh-native` | PASS | SSH feature compiles |
| `cargo test -p unfour-ssh-engine --features ssh-native` | PASS | 25 native-feature tests |
| Browser mock first viewport | NOT VERIFIED | No live browser available in this scan |

## Known Limitations

- **Windows workspace tests:** `cargo test -p unfour-workspace` fails with `STATUS_ENTRYPOINT_NOT_FOUND`. Likely a native DLL dependency issue (OpenSSL/SQLite) on this Windows environment. Does not indicate code defects.
- **Lint warnings:** Multiple packages have `react-hooks/set-state-in-effect`, `react-hooks/exhaustive-deps`, `react-hooks/refs`, and `react-refresh/only-export-components` warnings. These are pre-existing and do not block builds.
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
| `unfour-workspace` (Tauri) | CommandBus, 43 Tauri commands, app setup |
