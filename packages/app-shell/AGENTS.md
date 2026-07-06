# app-shell Agent Rules

## Scope

`packages/app-shell` owns the frontend desktop workbench composition root:
global layout composition, top-level slots, workspace switcher wiring,
module navigation, command palette and diagnostics actions, and cross-module
mount surfaces.

## Boundaries

- It may compose shell slots, mount API Client, SSH Terminal, and Database
  modules, and pass children into shared layout primitives.
- It must not own API request execution, SQL editing/execution, SSH session
  state, feature mock data, or large feature-specific UI.
- Feature internals must stay in their owning packages; `app-shell` may only
  wire them into the desktop workbench.

## Rules

- Do not make unrelated cross-package changes.
- Prefer existing `@unfour/ui` shell primitives and shared components.
- Do not introduce new dependencies unless the task explicitly requires them.
- Do not move feature behavior into this package to simplify wiring.
- Keep shell orchestration focused unless a task explicitly expands
  workbench-level behavior.
- Use the shared i18n provider/hook for user-visible shell copy. Feature
  packages must not depend on `packages/app-shell` to translate their own UI.

## Required Output

After changes here, report:

- Modified files
- Behavior changed
- Validation commands run
- Manual verification needed
- Follow-up risks
