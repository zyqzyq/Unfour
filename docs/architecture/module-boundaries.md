# Deprecated

This document is no longer maintained.

Use [`package-boundaries.md`](./package-boundaries.md) as the canonical source of truth.

---

# Module Boundaries

## packages/app-shell

Allowed responsibilities:

- global shell
- layout regions
- tab workspace container
- resizable panel infrastructure
- layout state
- shared application chrome

Forbidden responsibilities:

- terminal business UI
- database business UI
- API debugger business UI
- feature-specific state
- feature-specific mock data
- feature-specific API calls
- feature-specific event handlers

## Feature packages

Each feature package owns:

- feature page
- feature components
- feature hooks
- feature model
- feature state
- feature-specific actions
- feature-specific layout inside the workspace region

## Dependency direction

Allowed:

```text
apps/desktop -> packages/app-shell
apps/desktop -> packages/api-debugger
apps/desktop -> packages/database
apps/desktop -> packages/terminal

packages/app-shell -> packages/ui
packages/api-debugger -> packages/ui
packages/database -> packages/ui
packages/terminal -> packages/ui