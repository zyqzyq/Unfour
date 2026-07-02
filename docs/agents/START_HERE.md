# AI Agent Start Here

This is the stable repository onboarding entrypoint for AI coding tools working
in Unfour. Use it to choose the smallest useful context set before touching
files.

## Source Priority

Use these sources in order when documents overlap:

1. Root `AGENTS.md` for global rules, package boundaries, command-bus
   expectations, verification defaults, and reporting requirements.
2. The current user task for scope and acceptance criteria.
3. This file for repository reading order.
4. `docs/architecture/*` for durable architecture, storage, package boundary,
   and security model details.
5. `docs/ui/*` for UI design-system and interaction rules.
6. `docs/testing/*` and `docs/release/*` for release-readiness and verification
   expectations.
7. Package or crate `AGENTS.md` / `README.md` for local ownership rules.

Historical checkpoint, progress, task, and audit files live under
`docs/archive/`. They are useful for archaeology only and must not override the
active documents above.

## Recommended Reading Order

Read in this order, stopping when the task has enough context:

1. `AGENTS.md`.
2. `docs/agents/START_HERE.md`.
3. For package boundaries or dependency direction:
   `docs/architecture/package-boundaries.md`.
4. For repository shape or call chains:
   `docs/architecture/project-structure.md`.
5. For persistence, credentials, activity, or workspace scope:
   `docs/architecture/data-storage.md` and
   `docs/architecture/security-model.md`.
6. For UI, layout, component, style, or interaction changes:
   `design.md`, `docs/ui/design-system.md`, and
   `docs/ui/interaction-guidelines.md`.
7. For MCP work:
   `docs/mcp/overview.md`, `docs/mcp/tools.md`, and
   `docs/mcp/codex-setup.md`.
8. For release or verification work:
   `docs/testing/release-verification.md`,
   `docs/testing/manual-test-cases.md`, and
   `docs/release/release-checklist.md`.
9. The relevant package or crate `AGENTS.md` / `README.md`, if present.
10. Source files, only as needed to understand or change the implementation.

## Scoped Context Rules

- Do not default to scanning the whole repository.
- Do not default to modifying unrelated packages.
- For a single-package task, read the root rules, this file, the relevant
  architecture document, and that package's local `AGENTS.md` / `README.md`.
- For a cross-module task, also read the relevant app-shell, command-bus, MCP,
  data-storage, or feature-package context before reading source.
- For UI changes, read the active UI docs before editing.
- For package boundary changes, read `docs/architecture/package-boundaries.md`
  before editing.
- For release claims, read `docs/testing/release-verification.md` and report
  only checks that were actually run or are backed by cited repository evidence.

## During Modification

- Modify only files within the current task scope.
- Do not clean up unrelated code as a side effect.
- Do not move directories without an explicit task requirement.
- Do not modify backend call chains unless the task explicitly requires it.
- Do not add dependencies unless the task explicitly requires it.
- Do not write feature logic into `packages/app-shell`.
- Do not write business logic into `packages/ui`.
- Route new user-visible frontend UI text through the shared i18n helper and
  locale keys; do not create package-local i18n variants.
- Keep MCP tool names, command keys, schemas, and stable error codes in English.
  Localize only UI-facing messages.
- Keep current status, release gates, and verification evidence in the active
  testing and release documents, not in temporary progress files.

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
