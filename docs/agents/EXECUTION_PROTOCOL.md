# Execution Protocol

Default execution workflow for AI coding agents working on implementation or
documentation tasks in this repository.

## 1. Role And Authority

Document precedence (highest to lowest):

1. `AGENTS.md` for repository-wide rules and architecture constraints.
2. Task-specific prompt for current goal, scope, and acceptance criteria.
3. `docs/agents/START_HERE.md` for context loading strategy.
4. Domain documentation under `docs/architecture/`, `docs/ui/`, `docs/mcp/`,
   `docs/testing/`, and `docs/release/`.
5. Package or crate `AGENTS.md` / `README.md`.
6. This file for default workflow and reporting conventions.

When a task prompt conflicts with `AGENTS.md`, follow `AGENTS.md` and report
the conflict.

## 2. Start-Of-Task Checklist

Before writing code or documentation:

1. Read `AGENTS.md`.
2. Read `docs/agents/START_HERE.md` and follow its scoped reading order.
3. Read task-specific files listed in the prompt.
4. Read relevant domain docs, such as package boundaries for dependency
   changes or UI docs for interaction changes.
5. Run `git status --short` to inspect the working tree.
6. Inspect the current diff before editing.
7. Confirm the requested scope and identify verification commands.

Begin execution directly after a short plan unless a critical ambiguity makes
safe execution impossible. If blocked, state the ambiguity and stop.

## 3. Scope Discipline

- Modify only files necessary for the requested task.
- Do not perform opportunistic refactors, dependency upgrades, or unrelated
  formatting.
- Do not change public contracts, exported types, store shape, or Tauri command
  behavior unless the task requires it.
- Record out-of-scope issues in the final report under remaining risks instead
  of fixing them.
- Stop and report when a required change would cross an explicitly forbidden
  boundary.

## 4. Change Discipline

- Prefer small, reviewable changes over large rewrites.
- Preserve existing behavior unless the task explicitly requests behavior
  changes.
- Keep architecture boundaries intact.
- Do not add `any`, `@ts-ignore`, or `@ts-expect-error` unless the task
  explicitly justifies it.
- Do not introduce silent fallbacks or hide errors to make builds pass.
- Reuse existing project patterns before adding new abstractions.
- Do not create new documentation files unless the task requires them.

## 5. Verification Matrix

Run the default verification for every touched area, plus any task-specific
commands.

| Changed area | Required default verification |
| --- | --- |
| Documentation only | `git diff --check` |
| Frontend TypeScript / React | `git diff --check`, `pnpm run build` |
| Frontend tests or lint config | `pnpm run lint` / `pnpm run test` when available |
| Rust crates | `cargo fmt --check`, `cargo test --workspace` |
| Rust compile-sensitive feature flags | relevant `cargo check`, such as `cargo check -p unfour --features ssh-native` |
| Tauri adapter or cross-layer changes | frontend build plus relevant Rust checks |
| Build configuration | build command and output inspection |
| Release preparation | `docs/testing/release-verification.md` matrix |

Rules:

- If a command is unavailable, fails for an unrelated baseline reason, or cannot
  run in the current environment, report it honestly as `NOT RUN` with a reason.
- Never report an unexecuted manual check as `PASS`. Use `NOT VERIFIED` for UI,
  platform, network, or live-service behavior that cannot be tested.
- Do not invent lint or test commands that do not exist in `package.json`.

## 6. Git Discipline

- Inspect `git status` before editing.
- Inspect `git diff` and run `git diff --check` before committing.
- Include only task-related files in the commit.
- Create one independent commit per task batch unless instructed otherwise.
- Use a conventional commit-style message, such as `docs(release): update
  verification checklist` or `fix(api-client): handle request save error`.
- Never commit generated build output unless explicitly requested.
- Never include unrelated user changes.

If the working tree is dirty before the task starts, preserve unrelated changes
and mention them in the final report.

## 7. Final Report Format

Every task must end with a structured report:

```text
## Result
- Status: Completed / Partially Completed / Blocked
- Commit: <hash or N/A>
- Scope violations: Yes / No

## Modified Files
- file path - purpose

## Verification
- command - PASS / FAIL / NOT RUN (reason)
- manual check - PASS / FAIL / NOT VERIFIED

## Remaining Risks
- out-of-scope issues found during the task
- unverified behavior
- suggested follow-up tasks

## Scope Confirmation
- unrelated files changed: Yes / No
- dependencies added: Yes / No
- public contracts changed: Yes / No
- backend call chain changed: Yes / No
```

Additional sections may be appended when the task warrants them.

## 8. Handoff Notes

At the end of a batch, the report must contain enough information for another
agent or maintainer to understand the new state:

- resulting commit hash, if a commit was requested;
- modified files and their purposes;
- verification results;
- unverified behavior;
- remaining risks;
- discovered out-of-scope issues.

Historical checkpoint files under `docs/archive/` must not be refreshed as part
of normal work. Use the active release and testing documents for current
release readiness.

## 9. Batch Size Guidance

- Prefer one coherent theme per batch.
- A batch may include 1-4 closely related sub-tasks.
- Low-risk cleanup tasks may be grouped together.
- Do not mix unrelated UI cleanup, transport-layer work, security policy, and
  architecture restructuring in one batch.
- Split high-risk cross-layer work into independently verifiable phases.

## 10. Non-Goals

This file does not define:

- package ownership or dependency directions: see
  `docs/architecture/package-boundaries.md`;
- repository structure and call chains: see
  `docs/architecture/project-structure.md`;
- data storage and workspace scope: see `docs/architecture/data-storage.md`;
- security policy details: see `docs/architecture/security-model.md`;
- UI design rules or semantic tokens: see `docs/ui/design-system.md` and
  `docs/ui/interaction-guidelines.md`;
- MCP tool behavior: see `docs/mcp/tools.md`;
- release verification: see `docs/testing/release-verification.md`;
- task-specific scope: use the current task prompt.
