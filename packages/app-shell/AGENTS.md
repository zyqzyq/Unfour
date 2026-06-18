# app-shell Agent Rules

## Scope

`packages/app-shell` owns the frontend shell mount surface: global layout
composition, top-level slots, navigation/mount wiring, and cross-module
container surfaces.

## Boundaries

- It may compose shell slots and pass children into shared layout primitives.
- It must not own API request execution, SQL editing/execution, SSH session
  state, feature mock data, or large feature-specific UI.
- It must not depend on feature packages. Feature modules are composed by the
  desktop app or another explicit composition layer.

## Rules

- Do not make unrelated cross-package changes.
- Prefer existing `@unfour/ui` shell primitives and shared components.
- Do not introduce new dependencies unless the task explicitly requires them.
- Do not move feature behavior into this package to simplify wiring.
- Keep this package thin unless a task explicitly expands shell-level
  orchestration.

## Required Output

After changes here, report:

- Modified files
- Behavior changed
- Validation commands run
- Manual verification needed
- Follow-up risks
