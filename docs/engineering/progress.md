# Progress

This document records the current implementation state and the next work slices. It is a development progress log, not a user guide.

## Current State

### Foundation

- DONE: Tauri 2 + React + TypeScript + Vite project scaffold.
- DONE: Tailwind CSS and shadcn-style local UI primitives.
- DONE: Workspace shell with left resource navigation, top tabs, and API/SSH/Database work surfaces.
- DONE: Rust `AppError` with structured serialization for Tauri command errors.
- DONE: Thin Tauri commands routed through `CommandBus`.
- DONE: Local SQLite app database with repeatable migrations.
- DONE: Audit log table and action recording for workspace/API/database operations.
- DONE: AI and cloud sync extension boundaries reserved.
- DONE: `russh` dependency is available behind `ssh-native` and configured to use the `ring` backend.

### Workspace

- DONE: Default workspace is created on first launch.
- DONE: Workspaces can be listed, created, switched, renamed, and soft-deleted.
- DONE: Active workspace is persisted in `app_settings`.
- DONE: Workspace environment variables are stored in `workspace_settings.env_json`.
- DONE: Workspace layout JSON persists sidebar state, active tab, open tabs, and selected resource slots.

### API Client

- DONE: HTTP/HTTPS requests are executed by Rust `reqwest`.
- DONE: Methods, URL, headers, query params, JSON body, and timeout are supported.
- DONE: Workspace variables resolve with `{{variable}}` syntax in URL, headers, query, and body.
- DONE: Request history is stored per workspace.
- DONE: Saved request templates are stored per workspace and can be loaded back into the editor.
- PARTIAL: Response viewer exists for body/status/duration; headers/cookies/timing panels are still basic.
- TODO: Collection folders and import/export without secrets.

### Database

- DONE: Database connection metadata CRUD uses the shared `connections` table.
- DONE: Connection records are workspace-scoped and keep `credential_ref` instead of secret material.
- DONE: SQLite connection test is implemented.
- DONE: SQLite schema browsing reads tables, views, and columns.
- DONE: SQLite SQL editor execution supports row results and affected row counts.
- DONE: SQLite table browse action uses a backend-generated safe `SELECT * FROM table LIMIT ?` query.
- DONE: DatabaseService has automated tests for connection CRUD, SQLite test, schema browsing, query execution, and table browsing.
- PARTIAL: PostgreSQL/MySQL metadata can be saved, but live credential-backed connections are still reserved.
- TODO: PostgreSQL/MySQL connection tests, schema browsing, paginated result grids, and controlled editing.

### SSH

- DONE: Frontend has an SSH work surface and xterm preview component.
- DONE: Rust service boundary and dependency strategy are reserved.
- TODO: Real session lifecycle, password/private-key auth, PTY allocation, event streaming, resize, close, and redacted logs.

## Verification Status

- PASS: `node .\node_modules\typescript\bin\tsc --noEmit`.
- PASS: `cargo fmt`.
- PASS: `npm run build` with local permission elevation for the Vite/esbuild helper process.
- PASS: `cargo check` with local permission elevation for Rust target artifacts.
- PASS: `cargo test workspace::tests` with local permission elevation for Rust target artifacts.
- PASS: `cargo test database::tests` with local permission elevation for Rust target artifacts.
- PASS: In-app browser loaded `http://127.0.0.1:1420/` and reported no console errors.
- BLOCKED THIS RUN: In-app browser automation did not expose an available `iab` browser while checking the database UI after Slice 2 changes.

## Next Work Slices

### Slice 1: Workspace Persistence

Goal: make the workspace shell remember layout and open tabs.

- DONE: Add `workspace_layout_get` and `workspace_layout_update` commands.
- DONE: Store sidebar state, active tab, open tabs, and selected resource slots in `workspace_settings.layout_json`.
- DONE: Hydrate Zustand state after workspace load.
- DONE: Add tests for layout JSON defaults and update validation.

### Slice 2: Database Hardening

Goal: turn the SQLite database MVP into a dependable local database tool.

- DONE: Add backend tests around connection CRUD, SQLite test, schema browsing, query execution, and table browsing.
- TODO: Add query safety policy for mutating SQL, including confirmation metadata for future AI/automation.
- DONE: Add table data browse action that generates safe `SELECT * FROM table LIMIT ?` queries.
- Add result pagination and column width handling in the frontend.
- Add Postgres/MySQL live connection tests after secret storage is ready.

### Slice 3: Secret Store

Goal: stop all credential-bearing workflows at a real OS secret boundary.

- Implement `SecretStore` using OS keychain or Stronghold.
- Add commands for creating, reading metadata for, rotating, and deleting credentials.
- Store only `credential_ref` in SQLite.
- Add redaction helpers shared by API, SSH, database, audit log, and future sync.

### Slice 4: SSH MVP

Goal: implement a real multi-session terminal flow.

- Add SSH connection metadata CRUD under the shared `connections` table.
- Implement password auth and return `session_id`.
- Implement private-key auth with passphrase referenced through `SecretStore`.
- Allocate PTY and stream backend output to the frontend with Tauri events.
- Wire xterm input, resize, close, and session log export.

### Slice 5: API Client Polish

Goal: move from functional request runner to Postman-like daily tool.

- Add collections and folders.
- Add response headers, cookies, timing, and size panels.
- Add history replay into a new tab.
- Add import/export without secrets.
- Add environment variable duplicate detection and masking for sensitive-looking values.

### Slice 6: Packaging And Final User Guide

Goal: make the app understandable and installable.

- Re-run full `npm run build`, `cargo check`, `cargo check --features ssh-native`, and `npm run tauri build` in an unrestricted local shell.
- Add screenshots after the UI is stable.
- Expand `docs/user/USER_GUIDE.md` with real workflows, troubleshooting, and screenshots.
- Keep engineering details in `docs/engineering/*` and keep user instructions in `docs/user/*`.
