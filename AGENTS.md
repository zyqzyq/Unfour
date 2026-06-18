# AGENTS.md

Unfour is a lightweight IDE-style desktop developer tool built on Tauri 2,
React, TypeScript, and Rust.

## Long-Term Direction

- Core modules are Terminal / SSH, Database, API Debugger, and Workspace.
- Module extraction is in progress. Boundary clarity takes priority over new
  page features during the split.
- Keep project context lightweight and stable. Current status belongs in
  `docs/project/PACKAGE_STATUS.md` or the central project docs, not in package
  agent rules.
- Architecture reference docs explain boundaries, but current package status
  is centralized in `docs/project/PACKAGE_STATUS.md`.

## Package Boundary Rules

- `packages/app-shell` is limited to global layout composition, sidebar and
  module mount surfaces, top-level navigation wiring, route assembly, and
  cross-module container slots.
- Do not add API request execution, SQL editing/execution, SSH session state,
  feature mock data, or feature-specific large UI components to
  `packages/app-shell`.
- Terminal, Database, API Debugger, and Workspace business state and business
  components must live in their owning packages or crates.
- Shared UI primitives must be reused from or added to `packages/ui`.
- `packages/ui` must not contain feature-specific business logic.
- Feature packages must not depend on `packages/app-shell`.
- Do not make unrelated cross-package changes. If a fix requires crossing a
  package boundary, explain why in the final report.
- Do not add new dependencies unless the task explicitly requires them and the
  reason is documented.
- Do not delete code with unconfirmed purpose for cosmetic reasons.

## Backend And Command-Bus Rules

- Frontend interaction lives in React/TypeScript; execution and security
  boundaries live in Rust.
- `apps/desktop/src-tauri` is the Tauri adapter and composition layer.
  Backend capability logic belongs in `crates/*`.
- Business actions for API, SSH, Database, Workspace, and future MCP/AI
  surfaces must route through the Rust command bus boundary.
- Tauri commands and MCP tools are adapters over the command bus, not places
  for duplicated domain logic.
- Every persisted business record must carry `workspace_id` unless it is truly
  global app configuration.
- Passwords, private-key passphrases, API tokens, and database passwords must
  not be stored in SQLite plaintext. Persist only credential references.
- `authorization`, `cookie`, `proxy-authorization`, `x-api-key`, and
  `x-auth-token` must be redacted in logs, history, and local activity details.

## AI Execution Rules

Before changing files:

1. Read this file.
2. Read `docs/agents/START_HERE.md` and follow its scoped reading strategy.
3. Read `docs/architecture/package-boundaries.md` for package or boundary
   changes.
4. Read `docs/ui/ui-guidelines.md` for UI changes.
5. Read the relevant package or crate `AGENTS.md` / `README.md`, if present.
6. Inspect the current implementation only as needed for the task.
7. Review `git status --short` and the current diff before editing.

During modification:

- Keep the change set as small as possible.
- Modify only files within the current task scope.
- Do not clean up, reformat, or refactor unrelated packages as a side effect.
- Do not rewrite backend call chains unless the task explicitly requires it.
- Do not add feature logic to `packages/app-shell`.
- Do not add business logic to `packages/ui`.
- Preserve uncommitted user work.

For implementation-task workflow, verification defaults, commit discipline, and
final reporting, follow `docs/agents/EXECUTION_PROTOCOL.md`.

## Required Reporting

After completion, report:

1. Modified file list.
2. Purpose of each change.
3. Whether business logic was modified.
4. Whether new dependencies were added.
5. Whether package boundaries changed.
6. Verification commands executed and their results.
7. Commands not executed and why.
8. Unresolved issues or follow-up risks.
9. Files recommended for human review.

## Default Verification

Run relevant commands from the repository root. For broad implementation work,
prefer:

```bash
pnpm run build
pnpm run check:rust
pnpm run check:rust:ssh
pnpm run test:rust
```

For documentation-only changes, `git diff --check` is the default minimum.
For UI changes, also run the local app and inspect the first viewport when
practical.
