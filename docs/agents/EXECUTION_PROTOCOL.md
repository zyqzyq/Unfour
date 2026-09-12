# Execution and Completion

Use this reference to choose verification and finish a scoped change. It
defines outcomes and decision boundaries, not a required sequence of tool calls.

## Autonomy and scope

Inspect the working-tree status and relevant staged/unstaged diff before
editing so existing work is preserved. Infer routine implementation choices
from the task and current patterns. Complete safe, reversible local edits,
checks, and fixes caused by the change without asking for approval at each step.
Use disposable fixtures and isolated test data where execution can mutate state;
do not assume every test or local app instance is isolated from real services.

Ask only when missing information prevents a safe or correct result, or an
action needs authorization not already provided: destructive operations on user
data, production or external side effects, publishing, or a material scope
expansion. Continue independent authorized work while blocked on that action.
Product confirmation/capability policies remain in force; coding autonomy does
not permit bypassing them.

Keep changes necessary to the requested outcome, including related fixes needed
to make it work. Preserve public contracts and backend call chains unless the
task requires changing them. Report unrelated defects instead of repairing them
as incidental cleanup. Do not suppress errors, weaken safety checks, or add
unjustified type-check escapes to make verification pass.

## Definition of Done

A task is complete when:

- The requested outcome and acceptance criteria are met, including running or
  inspecting the result when that is part of the task.
- The diff is scoped, respects applicable invariants, and preserves user work.
  Update affected documentation/contracts when the change makes them stale.
- Checks appropriate to the affected behavior have passed; failures caused by
  the change are fixed and affected checks rerun. Any unavailable verification
  is stated with its limitation; a missing required gate means partial or
  blocked completion, not an unconditional success claim.
- The final response explains the result, changed files or areas and their
  purposes, verification evidence, and material remaining risks.

Do not stop at a first implementation when requested validation or repair
remains. Once these conditions are met, stop rather than expanding the task.

## Choose verification by impact

Run commands from the repository root. Use actual scripts in
[package.json](../../package.json) and the relevant Cargo manifest; placeholders
below must be replaced with the affected file, directory, or crate name.
`git diff --check` applies to all edits.

| Change | Smallest useful verification; expand when indicated |
| --- | --- |
| Documentation / repository instructions | Check changed links, paths, references, and conflicting guidance. No application build for prose-only changes. |
| Configuration | Validate syntax and the affected configuration consumer; run its build/test only when behavior depends on the change. |
| Local TypeScript / React behavior | Affected tests, e.g. `pnpm exec vitest run <test-path>`, and `pnpm exec eslint <changed-files>`. For type-sensitive changes use `pnpm --filter @unfour/desktop exec tsc --noEmit`. |
| UI styles or interactions | Inspect the affected view/states when possible; test changed interaction behavior. Use `pnpm run check:tokens` for shared-token changes. |
| Frontend imports, exports, dependencies, or bundling | `pnpm run build` and tests for affected consumers. Broaden for shared contracts. |
| Rust crate behavior | `cargo fmt -p <crate> --check` and `cargo test -p <crate> <optional-test-filter>`; use `cargo check -p <crate>` for compile-only changes. Include affected dependent crates when contracts change. |
| Rust features / native SSH | Check the changed feature combination, e.g. `cargo check -p unfour --features ssh-native`, when that integration path is affected. |
| Tauri / command-client / command-bus contract | Verify both sides and the relevant adapter path; combine affected frontend and Rust checks. |
| SQLite migrations / persisted data compatibility | `pnpm run check:migrations` and affected storage/migration tests against disposable data. |
| Broad implementation or release preparation | Broaden to build, workspace Rust checks/tests, frontend suites, and relevant feature checks for cross-cutting impact. For releases, use [release verification](../testing/release-verification.md). |

A copy/style-only edit does not need new tests that merely repeat the source.
Behavior changes need meaningful coverage of the affected outcome, including
regression cases where useful. A passing mock test does not establish real
SSH, database-engine, native UI, or live-service behavior; verify the affected
path in an authorized test environment or state `NOT VERIFIED`.

Do not default to the aggregate `pnpm run check` or workspace-wide suites for
small changes. Broaden or repeat passing checks only for new edits, failures,
shared impact, or unresolved concerns. Report executed failures as `FAIL`
(including unrelated baseline failures), unavailable commands as `NOT RUN`,
and checks actually passed as `PASS`.

For changes that grow or restructure large source files, consult
[scripts/check-large-files.mjs](../../scripts/check-large-files.mjs) and run
`pnpm run check:large-files` as appropriate. Keep thresholds/exclusions in the
checker, not duplicated here. Extract by responsibility when it improves
maintainability; do not split unrelated files just to satisfy line counts.

## Delivery and Git

Keep the final response proportional to the task. Identify changed files
(group related files when useful), their purpose, checks and results, important
checks skipped and why, and any remaining risks or files needing human review.
Mention changes to business logic, dependencies, or package boundaries; explain
necessary cross-package changes. Do not enumerate unrelated checks as skipped.

Commit only when requested; use a Conventional Commit message and include only
task-related files. Do not commit generated build output unless requested.
Committing, pushing, and publishing are separate actions; authorization for one
does not imply the others. Include the commit hash when a commit was made.
