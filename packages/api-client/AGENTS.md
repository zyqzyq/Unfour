# api-client Agent Rules

## Scope

`packages/api-client` owns the API Debugger frontend: request editing, Send,
response display, history, saved requests, collections, environments, and
import/export UI.

## Boundaries

- Backend calls must go through `@unfour/command-client`.
- Shared controls should come from `@unfour/ui` when practical.
- This package must not own Database, SSH, Workspace shell, or app-level
  navigation behavior.
- Keep Send as the primary request action.

## Rules

- Do not make unrelated cross-package changes.
- Prefer existing request models, hooks, utilities, and UI components.
- Do not introduce new dependencies unless the task explicitly requires them.
- Do not break saved-request, history, environment, or Send behavior.
- Keep sensitive headers and auth metadata aligned with backend redaction
  expectations.

## Required Output

After changes here, report:

- Modified files
- Behavior changed
- Validation commands run
- Manual verification needed
- Follow-up risks
