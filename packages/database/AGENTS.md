# database Agent Rules

## Scope

`packages/database` owns the Database frontend: connection tree, connection
settings, schema browser, SQL editor, query execution UI, query results, table
preview, and local query history UI.

## Boundaries

- Backend calls must go through `@unfour/command-client`.
- Shared controls should come from `@unfour/ui` when practical.
- Workspace selection state may use `@unfour/workspace-core` only as the
  documented transitional boundary.
- This package must not own API request, SSH terminal, or app-shell behavior.

## Rules

- Do not make unrelated cross-package changes.
- Prefer existing database hooks, result utilities, models, and components.
- Do not introduce new dependencies unless the task explicitly requires them.
- High-risk SQL operations must keep clear confirmation and safety behavior.
- Database behavior that touches real engines should be manually verified when
  automated tests cannot cover it.

## Required Output

After changes here, report:

- Modified files
- Behavior changed
- Validation commands run
- Manual verification needed
- Follow-up risks
