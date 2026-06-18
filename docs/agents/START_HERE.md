# START_HERE.md

This is the entry point for AI coding tools working in the Unfour repository.
Use it to choose the smallest useful context set before touching files.

## Source Priority

Use these sources in order when documents overlap:

1. Root `AGENTS.md` for global long-term rules.
2. This file for AI/Codex reading strategy.
3. `docs/project/PACKAGE_STATUS.md` for current package and crate status.
4. Package or crate `AGENTS.md` for local long-term rules.
5. Package or crate `README.md` for local responsibilities and usage.
6. `docs/architecture/*` for architecture explanation. These files must not
   override current package status from `docs/project/PACKAGE_STATUS.md`.

## Default Reading Order

Read in this order, stopping when the task has enough context. Do not default
to a full repository scan.

1. Root `AGENTS.md`.
2. `docs/agents/START_HERE.md`.
3. `docs/project/PROJECT_STATE.md`.
   - Current legacy fallback: `docs/agents/PROJECT_STATE.md`.
4. `docs/project/PACKAGE_STATUS.md`.
5. `docs/project/NEXT_STEPS.md`.
   - Current legacy fallback: `docs/agents/NEXT_STEPS.md`.
6. `docs/project/OPEN_ISSUES.md`.
   - Current legacy fallback: `docs/agents/OPEN_ISSUES.md`.
7. The `AGENTS.md` and `README.md` for the package or crate involved in the
   current task.
8. Source files, only when necessary to understand or change the implementation.

In this checkout, `docs/project/PACKAGE_STATUS.md` exists. The
`docs/project/PROJECT_STATE.md`, `docs/project/NEXT_STEPS.md`, and
`docs/project/OPEN_ISSUES.md` files may not exist yet; when absent, use the
listed `docs/agents/*` fallback files only as legacy checkpoint context. They
must not override `docs/project/PACKAGE_STATUS.md`. Do not create missing
project-state or issue files unless the task explicitly asks for them.

## Scoped Context Rules

- Do not default to scanning the whole repository.
- Do not default to modifying unrelated packages.
- For a single-package task, read the root rules, this file, the central
  package status, and that package's local `AGENTS.md` / `README.md`.
- For a cross-module task, also read the relevant app-shell, command-bus, MCP,
  or feature-package context before reading source.
- For UI changes, read `docs/ui/ui-guidelines.md` before editing.
- For package boundary changes, read `docs/architecture/package-boundaries.md`
  before editing.

## During Modification

- Modify only files within the current task scope.
- Do not clean up unrelated code as a side effect.
- Do not move directories without an explicit task requirement.
- Do not modify backend call chains unless the task explicitly requires it.
- Do not add dependencies unless the task explicitly requires it.
- Do not write feature logic into `packages/app-shell`.
- Do not write business logic into `packages/ui`.
- Keep package progress centralized in `docs/project/PACKAGE_STATUS.md`; do not
  create per-package `PROGRESS.md`, `NEXT_STEPS.md`, or `OPEN_ISSUES.md` files.

For the full execution workflow, verification matrix, commit discipline, and
reporting format, see `docs/agents/EXECUTION_PROTOCOL.md`.

## After Modification

Output the following report:

```text
1. Modified file list
2. Primary change in each file
3. Whether business logic was modified (yes/no)
4. Whether new dependencies were added (yes/no)
5. Whether package dependency direction changed (yes/no)
6. Commands executed and results
7. Commands not executed and why
8. Unresolved issues
9. Files recommended for human review
```
