# Task Breakdown

## P0: Engineering Base

- DONE TASK-CORE-001: Initialize Tauri 2 + React + TypeScript + Vite.
- DONE TASK-CORE-002: Add shadcn-style UI primitives and workspace shell.
- DONE TASK-CORE-003: Add Rust `AppError` and structured command responses.
- DONE TASK-CORE-004: Add Command Bus and route Tauri commands through it.
- DONE TASK-CORE-005: Add SQLite local store and migrations.
- DONE TASK-CORE-006: Reserve secret store boundary.
- DONE TASK-CORE-007: Add audit log and redaction rules.
- TODO TASK-CORE-008: Add automated tests for WorkspaceService and ApiClientService.
- DONE TASK-CORE-009: Add automated tests for DatabaseService SQLite flows.
- TODO TASK-CORE-010: Add CI-friendly build/check scripts and document local permission requirements.

## P0: Workspace

- DONE TASK-WORKSPACE-001: Create default workspace on first launch.
- DONE TASK-WORKSPACE-002: List, create, switch, rename, and soft-delete workspaces.
- DONE TASK-WORKSPACE-003: Persist environment variables and workspace layout JSON.
- DONE TASK-WORKSPACE-004: Restore tabs per workspace.
- PARTIAL TASK-WORKSPACE-005: Persist sidebar state, active tab, and selected resource slots in `layout_json`. Active resource wiring remains for future resource-tree entries.
- TODO TASK-WORKSPACE-006: Add workspace-scoped resource tree entries for API collections and database/SSH connections.

## P0: API MVP

- DONE TASK-API-001: Send HTTP/HTTPS requests through Rust `reqwest`.
- DONE TASK-API-002: Support method, URL, headers, query, JSON body, and timeout.
- DONE TASK-API-003: Store request history by workspace.
- DONE TASK-API-004: Save request templates by workspace.
- DONE TASK-API-005: Add workspace environments and variable resolution.
- DONE TASK-API-006: Add import/export for collections without secrets.
- TODO TASK-API-007: Add collection folders and request duplication.
- DONE TASK-API-008: Add response headers, cookies, timing, and size panels.
- DONE TASK-API-009: Add replay from history into an editable request tab.

## P1: SSH MVP

- DONE TASK-SSH-001: Add `russh` dependency compatible with the selected Rust toolchain.
- TODO TASK-SSH-002: Add SSH connection metadata CRUD with `credential_ref`.
- TODO TASK-SSH-003: Password auth returns `session_id`.
- TODO TASK-SSH-004: Private-key auth using local key path and passphrase ref.
- TODO TASK-SSH-005: PTY allocation, xterm input, resize, and event output.
- TODO TASK-SSH-006: Session close and log export with redaction.
- TODO TASK-SSH-007: Add multi-session tab lifecycle and backend cleanup on tab close.

## P1: Database MVP

- DONE TASK-DB-001: Connection metadata CRUD with `credential_ref`.
- DONE TASK-DB-002: SQLite connection test.
- TODO TASK-DB-003: PostgreSQL/MySQL connection tests.
- PARTIAL TASK-DB-004: Schema tree for tables and columns. SQLite is implemented; PostgreSQL/MySQL remain.
- PARTIAL TASK-DB-005: SQL editor execution and paginated results. SQLite is implemented; PostgreSQL/MySQL remain.
- TODO TASK-DB-006: Read-only table data view, then controlled edit support.
- DONE TASK-DB-007: Add safe table browse action from schema tree.
- PARTIAL TASK-DB-008: Add frontend pagination, copy/export, and large result virtualization. Pagination, column width handling, TSV copy, and CSV export are implemented; virtualization remains.
- DONE TASK-DB-009: Add mutation confirmation policy for destructive SQL and future AI calls.

## P2: Reserved Extensions

- TODO TASK-SECRET-001: Implement OS keychain or Stronghold-backed `SecretStore`.
- TODO TASK-AI-001: Expose Command Bus through an AI adapter.
- TODO TASK-AI-002: Add capability metadata, confirmation policy, and dry-run summaries for risky actions.
- TODO TASK-SYNC-001: Add cloud account model and workspace sync queue.
- TODO TASK-SYNC-002: Conflict UI that keeps both versions.
- TODO TASK-PLUGIN-001: Define extension points for future tools.
