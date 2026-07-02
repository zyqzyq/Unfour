# Progress

> Historical/reference engineering log. Current package and crate status lives
> in `docs/project/PACKAGE_STATUS.md`; use that file as the source of truth
> when status here looks stale.

This document records historical implementation state and work slices. It is a
development progress log, not a user guide.

## Historical State Snapshot

### Foundation

- DONE: Tauri 2 + React + TypeScript + Vite project scaffold.
- DONE: Tailwind CSS and shadcn-style local UI primitives.
- DONE: Workspace shell with left resource navigation, top tabs, and API/SSH/Database work surfaces.
- DONE: Rust `AppError` with structured serialization for Tauri command errors.
- DONE: Thin Tauri commands routed through `CommandBus`.
- DONE: Local SQLite app database with repeatable migrations.
- DONE: Local activity table and redacted action recording for high-value workspace/API/database/SSH operations.
- DONE: Root-level validation scripts cover frontend build, Rust check, `ssh-native` check, and Rust tests.
- DONE: AI and cloud sync extension boundaries reserved.
- DONE: `russh` dependency is available behind `ssh-native` and configured to use the `ring` backend.

### Workspace

- DONE: Default workspace is created on first launch.
- DONE: Workspaces can be listed, created, switched, renamed, and soft-deleted.
- DONE: Active workspace is persisted in `app_settings`.
- DONE: Workspace environment variables are stored in `workspace_settings.env_json`.
- DONE: Workspace layout JSON persists sidebar state, active tab, open tabs, and selected resource slots.
- DONE: The sidebar resource tree lists workspace-scoped API saved requests, database connections, and SSH connections, and selecting them restores the matching work surface.
- DONE: WorkspaceService tests cover lifecycle, active workspace fallback, environment persistence, and layout validation.

### API Client

- DONE: HTTP/HTTPS requests are executed by Rust `reqwest`.
- DONE: Methods, URL, headers, query params, JSON body, and timeout are supported.
- DONE: Workspace variables resolve with `{{variable}}` syntax in URL, headers, query, and body.
- DONE: Workspace environment updates reject duplicate enabled variable names, and sensitive-looking values are masked in the frontend.
- DONE: Request history is stored per workspace.
- DONE: Saved request templates are stored per workspace and can be loaded back into the editor.
- DONE: ApiClientService tests cover saved request scoping, secret header redaction, environment resolution, missing variables, and query URL building.
- DONE: Saved API requests support folder grouping, duplication, and soft-delete.
- DONE: Request history entries can be replayed into the editable API request form.
- DONE: Response viewer shows body, headers, cookies, timing, and payload sizes.
- DONE: Saved API request collections can be exported/imported as secret-redacted JSON.

### Database

- DONE: Database connection metadata CRUD uses the shared `connections` table.
- DONE: Connection records are workspace-scoped and keep `credential_ref` instead of secret material.
- DONE: SQLite connection test is implemented.
- DONE: SQLite schema browsing reads tables, views, and columns.
- DONE: SQLite SQL editor execution supports row results and affected row counts.
- DONE: SQLite table browse action uses backend-generated safe `SELECT * FROM table LIMIT/OFFSET` queries.
- DONE: SQLite read-only table data browsing supports page size, offset pagination, total row counts, refresh, and read-only UI state.
- DONE: DatabaseService has automated tests for connection CRUD, SQLite test, schema browsing, query execution, and table browsing.
- DONE: Mutating, schema-changing, transaction-control, and unknown SQL require explicit confirmation before execution.
- DONE: Database result grids use frontend pagination, stable column widths, large-result row virtualization, TSV copy, and CSV export.
- DONE: PostgreSQL/MySQL/MariaDB connection, schema, query, and table-browse
  paths exist behind credential references.
- EXPERIMENTAL: Live PostgreSQL/MySQL/MariaDB behavior is environment
  dependent and should be re-verified when database behavior changes.
- PLANNED: Controlled table editing is not a confirmed current user workflow.

### Secret Store

- DONE: `SecretStore` uses the OS keychain in production and an in-memory backend for tests.
- DONE: Credential create, inspect, rotate, and delete are exposed through the Command Bus and Tauri adapters.
- DONE: Credential references are workspace-scoped and secrets are never returned by metadata commands.
- DONE: Shared Rust redaction helpers cover sensitive HTTP-style keys and credential-bearing log lines.
- DONE: Frontend SSH and database forms can create, inspect, rotate, delete, and attach credential references without persisting secret material.

### SSH

- DONE: Frontend has an SSH work surface and xterm preview component.
- DONE: Rust SSH service boundary and `ssh-native` dependency path are in
  place.
- DONE: SSH connection metadata CRUD stores host/user/auth metadata with `credential_ref` only.
- DONE: SSH connection metadata is workspace-scoped and covered by Rust service tests.
- DONE: Session lifecycle command surface supports password/private-key connection sessions, PTY sizing, input events, resize events, close, redacted log export, and cleanup when a connection is deleted.
- EXPERIMENTAL: Real SSH server workflows remain a live verification gate
  before release-level confidence.

## Verification Status

- PASS: `pnpm run test:rust`.
- PASS: `pnpm run check:rust`.
- PASS: `pnpm run check:rust:ssh`.
- PASS: `pnpm run build` after workspace resource-tree changes.
- PASS: `node .\node_modules\typescript\bin\tsc --noEmit`.
- PASS: `cargo fmt --all`.
- PASS: `pnpm run build` with local permission elevation for the Vite/esbuild helper process.
- PASS: `cargo check --workspace` with local permission elevation for Rust target artifacts.
- PASS: `cargo test --workspace` with local permission elevation for Rust target artifacts.
- PASS: In-app browser loaded `http://127.0.0.1:1420/` and reported no console errors.
- BLOCKED CURRENT RUN: In-app browser automation tools were not exposed while checking the workspace resource-tree UI; the Vite dev server was already available at `http://127.0.0.1:1420/`.

## Next Work Slices

### Slice 1: Workspace Persistence

Goal: make the workspace shell remember layout and open tabs.

- DONE: Add `workspace_layout_get` and `workspace_layout_update` commands.
- DONE: Store sidebar state, active tab, open tabs, and selected resource slots in `workspace_settings.layout_json`.
- DONE: Hydrate Zustand state after workspace load.
- DONE: Add workspace-scoped resource tree entries for API saved requests, database connections, and SSH connections.
- DONE: Clear stale selected resource IDs when saved requests or connections are removed.
- DONE: Add tests for layout JSON defaults and update validation.
- DONE: Add tests for workspace lifecycle, active fallback, and environment validation.

### Slice 2: Database Hardening

Goal: harden the database tool across supported drivers.

- DONE: Add backend tests around connection CRUD, SQLite test, schema browsing, query execution, and table browsing.
- DONE: Add query safety policy for mutating SQL, including confirmation metadata for future AI/automation.
- DONE: Add table data browse action that generates safe `SELECT * FROM table LIMIT/OFFSET` queries.
- DONE: Add read-only table data mode with backend offset pagination and total row metadata.
- DONE: Add result pagination, column width handling, and large-result virtualization in the frontend.
- Re-run PostgreSQL/MySQL/MariaDB live verification whenever database behavior
  changes.

### Slice 3: Secret Store

Goal: keep credential-bearing workflows at a real OS secret boundary.

- DONE: Implement `SecretStore` using OS keychain backends in production and an
  in-memory backend for tests.
- DONE: Add commands for creating, reading metadata for, rotating, and deleting credentials.
- DONE: Store only `credential_ref` in SQLite.
- DONE: Add redaction helpers shared by API, SSH, database, local activity, and future sync.
- DONE: Add frontend credential management UI for SSH/database credential references.

### Slice 4: SSH MVP

Goal: implement a real multi-session terminal flow.

- DONE: Add SSH connection metadata CRUD under the shared `connections` table.
- DONE: Implement password auth and return `session_id`.
- DONE: Implement private-key auth with passphrase referenced through `SecretStore`.
- DONE: Allocate PTY and stream backend output to the frontend with Tauri events.
- DONE: Wire xterm input, resize, close, and session log export.
- PENDING VERIFICATION: Exercise the complete SSH flow against a reachable live
  SSH server.

### Slice 5: API Client Polish

Goal: move from functional request runner to Postman-like daily tool.

- DONE: Add collections and folders.
- DONE: Add response headers, cookies, timing, and size panels.
- DONE: Add history replay into the editable request form.
- DONE: Add import/export without secrets.
- DONE: Add environment variable duplicate detection and masking for sensitive-looking values.
- DONE: Add service tests for environment template resolution, missing variable errors, saved request scoping, and query URL building.
- DONE: Add collection folders, request duplication, and saved request soft-delete.

### Slice 6: Packaging And Final User Guide

Goal: make the app understandable and installable.

- Re-run full `pnpm run build`, `cargo check --workspace`, `cargo check -p unfour --features ssh-native`, and `pnpm run tauri build` in an unrestricted local shell.
- Add screenshots after the UI is stable.
- Expand `docs/user/USER_GUIDE.md` with real workflows, troubleshooting, and screenshots.
- Keep engineering details in `docs/engineering/*` and keep user instructions in `docs/user/*`.
