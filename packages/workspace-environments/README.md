# workspace-environments

## Purpose

`@unfour/workspace-environments` owns the frontend management surface for
workspace variables and workspace environments.

## Boundaries

- Can own environment and variable CRUD hooks, editor state, navigation guards,
  and the Workspace-level management page.
- Calls persistence through `@unfour/command-client`.
- Reuses stateless controls from `@unfour/ui`.
- Must not own API request execution or API Client navigation.
- Must not implement variable resolution; resolution remains a shared Workspace
  command contract exposed through `@unfour/workspace-core`.

## Test / Verify

- `pnpm test -- packages/workspace-environments/src`
- `pnpm run build`
