# Execution Protocol

Default execution workflow for AI coding agents working on implementation tasks in this repository.

## 1. Role and Authority

Document precedence (highest to lowest):

1. `AGENTS.md` — repository-wide rules and architecture constraints.
2. Task-specific prompt — current goal, scope, and acceptance criteria.
3. Domain documentation — `docs/architecture/*`, `docs/ui/*`, `docs/engineering/*`.
4. This file — default workflow and reporting conventions.

When a task prompt conflicts with `AGENTS.md`, follow `AGENTS.md` and report the conflict.

This file does not define architecture, UI rules, or security policy. See [Non-Goals](#10-non-goals).

## 2. Start-of-Task Checklist

Before writing any code:

1. Read `AGENTS.md`.
2. Read `docs/agents/START_HERE.md` and follow its reading order.
3. Read task-specific files listed in the prompt.
4. Read relevant domain docs (`docs/architecture/package-boundaries.md` for package changes, `docs/ui/ui-guidelines.md` for UI changes).
5. Run `git status --short` to inspect the working tree.
6. Run `pnpm run build` (or the relevant build command) to confirm baseline.
7. Confirm the requested scope and identify verification commands.

Begin execution directly after a short plan unless a critical ambiguity makes safe execution impossible. If blocked, state the ambiguity and stop.

## 3. Scope Discipline

- Modify only files necessary for the requested task.
- Do not perform opportunistic refactors, dependency upgrades, or unrelated formatting.
- Do not change public contracts (exported types, store shape, Tauri commands) unless the task requires it.
- Record out-of-scope issues in the final report under **Remaining Risks** instead of fixing them.
- Stop and report when a required change would cross an explicitly forbidden boundary.

## 4. Change Discipline

- Prefer small, reviewable changes over large rewrites.
- Preserve existing behavior unless the task explicitly requests behavior changes.
- Keep architecture boundaries intact (see `docs/architecture/package-boundaries.md`).
- Do not add `any`, `@ts-ignore`, or `@ts-expect-error` unless the task explicitly justifies it.
- Do not introduce silent fallbacks or hide errors to make builds pass.
- Reuse existing project patterns before adding new abstractions.
- Do not create new documentation files unless the task requires them.

## 5. Verification Matrix

Run the default verification for every touched area, plus any task-specific commands.

| Changed area | Required default verification |
|---|---|
| Documentation only | `git diff --check` |
| Frontend TypeScript / React | `git diff --check`, `pnpm run build` |
| Frontend tests or lint config | `pnpm run lint` / `pnpm run test` (when available) |
| Rust crates | `cargo fmt --check`, `cargo test --workspace` |
| Rust compile-sensitive feature flags | relevant `cargo check` (e.g. `cargo check -p unfour-workspace --features ssh-native`) |
| Tauri adapter or cross-layer changes | frontend build plus relevant Rust checks |
| Build configuration | build command and output inspection |

Rules:

- If a command is unavailable, fails for an unrelated baseline reason, or cannot run in the current environment, report it honestly as `NOT RUN` with a reason.
- Never report an unexecuted manual check as `PASS`. Use `NOT VERIFIED` for UI or platform-specific behavior that cannot be tested.
- Do not invent lint or test commands that do not yet exist in `package.json`.

## 6. Git Discipline

- Inspect `git status` before editing.
- Inspect `git diff` and run `git diff --check` before committing.
- Include only task-related files in the commit.
- Create one independent commit per task batch unless instructed otherwise.
- Use a conventional commit-style message (e.g. `refactor(desktop): …`, `fix(api-debugger): …`).
- Never commit generated build output unless explicitly requested.
- Never include unrelated user changes.

If the working tree is dirty before the task starts, preserve unrelated changes and mention them in the final report.

## 7. Final Report Format

Every task must end with a structured report:

```
## Result
- Status: Completed / Partially Completed / Blocked
- Commit: <hash or N/A>
- Scope violations: Yes / No

## Modified Files
- file path — purpose

## Verification
- command — PASS / FAIL / NOT RUN (reason)
- manual check — PASS / FAIL / NOT VERIFIED

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

Additional sections (e.g. **Added Tokens**, **Migration Notes**, **Architecture Decisions**) may be appended when the task warrants them.

## 8. Checkpoint and Handoff

At the end of a batch, the report must contain enough information for an external planning agent to understand the new state:

- resulting commit hash
- modified files and their purposes
- verification results
- unverified behavior
- remaining risks
- discovered out-of-scope issues
- whether checkpoint documentation (e.g. `docs/agents/PROJECT_STATE.md`) should be refreshed

Checkpoint documents should only be updated when the task explicitly requests it, or when the repository workflow already requires it.

## 9. Batch Size Guidance

- Prefer one coherent theme per batch.
- A batch may include 1–4 closely related sub-tasks.
- Low-risk cleanup tasks may be grouped together.
- Do not mix unrelated UI cleanup, transport-layer work, security policy, and architecture restructuring in one batch.
- Split high-risk cross-layer work into independently verifiable phases.

## 10. Non-Goals

This file does **not** define:

- package ownership or dependency directions → `docs/architecture/package-boundaries.md`
- UI design rules or semantic tokens → `docs/ui/ui-guidelines.md`, `docs/ui/ui-components.md`
- security policy details → `docs/engineering/security.md`
- feature roadmap or product requirements → `docs/roadmap.md`
- current project progress → `docs/agents/PROJECT_STATE.md`, `docs/engineering/progress.md`
- task-specific scope → provided by the task prompt
