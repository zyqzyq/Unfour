# START_HERE.md

This is the entry point for AI coding tools working in the Unfour repository.

## Before You Modify Code

1. Read the root `AGENTS.md`.
2. Read `docs/architecture/package-boundaries.md`.
3. For UI changes, read `docs/ui/ui-guidelines.md`.
4. For token or layout details, read `docs/ui/ui-tokens.md` and `docs/ui/ui-layouts.md`.
5. Read any local notes inside the target package, if they exist.
6. Inspect the current implementation in the target files.
7. Review related package `package.json` files to understand dependencies.
8. Check `git diff` to avoid overwriting uncommitted work.

## During Modification

- Modify only files within the current task scope.
- Do not clean up unrelated code as a side effect.
- Do not move directories without an explicit task requirement.
- Do not modify backend call chains unless the task explicitly requires it.
- Do not add dependencies unless the task explicitly requires it.
- Do not write feature logic into `packages/app-shell`.
- Do not write business logic into `packages/ui`.

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
