# AGENTS.md

Unfour is a lightweight IDE-style desktop developer tool built on Tauri 2, React, and Rust.

## Project Direction

- Core modules: Terminal / SSH, Database, API Debugger, Workspace.
- UI module split is in progress. Database, Terminal, and further Workspace extraction are planned.
- During the split, module boundary clarity takes priority over adding new page features.

## Architecture Rules

### Frontend Boundaries

- `packages/app-shell` MUST contain only:
  - Global layout composition
  - Sidebar mounting surface
  - Top-level navigation wiring
  - Route assembly
  - Cross-module container slots
  - Module mount points

- `packages/app-shell` MUST NOT contain:
  - API request execution logic
  - SQL editing or execution logic
  - SSH session state or lifecycle
  - Feature-specific mock data
  - Feature-specific large UI components

- Terminal, Database, and API Debugger business state and business components MUST live in their respective packages.
- Shared UI primitives MUST be reused from or added to `packages/ui`.
- `packages/ui` MUST NOT contain feature-specific business logic.
- Feature packages MUST NOT depend on `packages/app-shell`.
- UI refactoring MUST NOT rewrite backend call logic.
- New large dependencies MUST NOT be added without an explicit requirement.
- Code with unconfirmed purpose MUST NOT be deleted for cosmetic reasons.

### Backend Boundaries

- Frontend interaction lives in React/TypeScript; execution and security boundaries live in Rust.
- `apps/desktop/src-tauri` is the Tauri adapter and composition layer only. Backend capability logic lives in `crates/*`.
- Business actions route through the Rust Command Bus. Tauri commands are adapters, not domain logic.
- Every persisted business record MUST carry `workspace_id` unless it is truly global app configuration.
- Passwords, private-key passphrases, API tokens, and database passwords MUST NOT be stored in SQLite plaintext. Persist only a credential reference.
- `authorization`, `cookie`, `proxy-authorization`, `x-api-key`, and `x-auth-token` MUST be redacted in logs, history, and local activity details.

## AI Execution Rules

Before modifying code:

1. Read this file.
2. Read `docs/ui/ui-guidelines.md` for UI changes.
3. Read `docs/architecture/package-boundaries.md` for package changes.
4. Read any local README or notes in the target package.
5. Inspect the current implementation.
6. Review the current Git diff to avoid overwriting uncommitted work.

During modification:

- Keep the change set as small as possible.
- Do not modify files outside the task scope.
- Do not expand the refactor scope without reporting the issue first.
- Do not add feature logic to `packages/app-shell`.
- Do not add business logic to `packages/ui`.

For implementation-task workflow, verification defaults, commit discipline, and final reporting, follow `docs/agents/EXECUTION_PROTOCOL.md`.

After completion, output:

1. Modified file list
2. Purpose of each change
3. Whether business logic was modified
4. Whether new dependencies were added
5. Whether package boundaries changed
6. Verification commands executed and their results
7. Unresolved issues
8. Files recommended for human review

## Verification

Run these commands from the repository root:

```bash
# Frontend build
pnpm run build

# Rust check
pnpm run check:rust

# Rust SSH feature check
pnpm run check:rust:ssh

# Rust tests
pnpm run test:rust
```

For UI changes, run the local app and inspect the first viewport.
