# AGENTS.md

Unfour Workspace is a Tauri 2 desktop app for operations and development work. Treat the product as one unified workspace, not three separate tools.

## Working Rules

- Keep frontend interaction in React/TypeScript and execution/security boundaries in Rust.
- Route business actions through the Rust Command Bus. Tauri commands are adapters, not the place for domain logic.
- Every persisted business record must carry `workspace_id` unless it is truly global app configuration.
- Never store passwords, private-key passphrases, API tokens, or database passwords in SQLite plaintext. Persist only a credential reference.
- Redact `authorization`, `cookie`, `proxy-authorization`, `x-api-key`, and `x-auth-token` in logs and history.
- Prefer small, verifiable tasks. Each task should define scope, non-scope, acceptance criteria, and tests.

## Current Implementation Slice

- Tauri 2 + React + TypeScript project is initialized.
- Frontend workspace shell, API client panel, terminal preview, database editor preview, and shadcn-style UI primitives exist.
- Rust has the Command Bus boundary, Workspace service, SQLite migrations, audit log, API request execution/history/save support, and reserved SSH/database/secret/sync/AI modules.
- Workspace environments are implemented and API requests can resolve `{{variable}}` placeholders from the active workspace.
- Saved API requests can be created and loaded in the frontend.
- SSH and database execution are intentionally reserved modules for the next task batches. The optional `ssh-native` Cargo feature compiles with `russh` using the `ring` backend.

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
- Run `cargo check` from `src-tauri` for Rust changes.
- Run `cargo check --features ssh-native` when changing SSH dependency or session code.
- For UI changes, run the local app and inspect the first viewport in the in-app browser.
