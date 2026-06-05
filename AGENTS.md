# AGENTS.md

Unfour Workspace is a Tauri 2 desktop app for operations and development work. Treat the product as one unified workspace, not three separate tools.

## Working Rules

- Keep frontend interaction in React/TypeScript and execution/security boundaries in Rust.
- Treat `apps/desktop` as the only desktop application entry. It should compose internal packages, not own feature logic directly.
- Treat `packages/*` as internal module boundaries. Import through package names like `@unfour/api-debugger`, never through another package's `src` internals.
- Keep `packages/ui` free of business logic, `packages/command-client` free of React screens, and feature packages free of direct dependencies on each other.
- Treat `apps/desktop/src-tauri` as the Tauri adapter and composition layer only. Keep backend capability logic in `crates/*`.
- Use the root Cargo workspace for Rust work. New Rust services should land in an existing crate boundary or a focused new crate, then be wired through the Command Bus.
- Route business actions through the Rust Command Bus. Tauri commands are adapters, not the place for domain logic.
- Every persisted business record must carry `workspace_id` unless it is truly global app configuration.
- Never store passwords, private-key passphrases, API tokens, or database passwords in SQLite plaintext. Persist only a credential reference.
- Redact `authorization`, `cookie`, `proxy-authorization`, `x-api-key`, and `x-auth-token` in logs, history, and local activity details.
- Prefer small, verifiable tasks. Each task should define scope, non-scope, acceptance criteria, and tests.

## Current Implementation Slice

- Tauri 2 + React + TypeScript project is initialized.
- The repository uses a pnpm workspace with `apps/*` and `packages/*`. The Community desktop app lives in `apps/desktop`.
- Frontend package boundaries exist for `@unfour/ui`, `@unfour/command-client`, `@unfour/workspace`, `@unfour/app-shell`, `@unfour/api-debugger`, `@unfour/database`, and `@unfour/terminal`.
- Frontend workspace shell, API client panel, terminal preview, database editor preview, and shadcn-style UI primitives exist.
- Rust uses a Cargo workspace with `apps/desktop/src-tauri` plus `crates/*`. Shared core types, storage, workspace, HTTP, database, SSH, and secret boundaries live in crates.
- Rust has the Command Bus boundary, Workspace service, SQLite migrations, local activity log, API request execution/history/save support, database execution, and reserved SSH/sync/AI modules.
- Workspace environments are implemented and API requests can resolve `{{variable}}` placeholders from the active workspace.
- Saved API requests can be created and loaded in the frontend.
- Live SSH sessions are intentionally reserved for a later task batch. The optional `ssh-native` Cargo feature compiles with `russh` using the `ring` backend.

## Task Format

Use task files or issue text like:

```md
# TASK-API-001: Support request environments

## Background
Workspace-scoped API requests need environment variables.

## Scope
- Add environment CRUD under WorkspaceService.
- Resolve `{{variable}}` in URL, headers, query, and body.
- Keep resolved values out of saved request templates.

## Non-Scope
- Cloud sync.
- Team sharing.

## Acceptance
- Variables are scoped by workspace.
- Missing variables return structured validation errors.
- Frontend can select and edit an environment.
```

## Verification

- Run `pnpm run build` for frontend changes.
- Run `cargo check --workspace` from the repository root for Rust changes.
- Run `cargo check -p unfour-workspace --features ssh-native` from the repository root when changing SSH dependency or session code.
- Run `cargo test --workspace` from the repository root for Rust tests.
- For UI changes, run the local app and inspect the first viewport in the in-app browser.
