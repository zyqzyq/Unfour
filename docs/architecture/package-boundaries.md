# Package Boundaries

This architecture reference defines intended ownership, dependency direction,
and forbidden cross-package behavior. It is a stable boundary document, not a
progress log.

## packages/app-shell

### Responsibility

- Own application shell composition.
- Provide global layout, sidebar mounting surface, top-level navigation wiring,
  route assembly, cross-module container slots, and module mount points.
- Own shell-level behavior only when it is not feature-specific.

### Forbidden

- API request execution or request state.
- SQL editing/execution or database state.
- SSH session state, terminal buffers, or terminal feature logic.
- Feature mock data.
- Feature-specific large UI components.

`packages/app-shell` may compose slots and pass props, but feature behavior
belongs in the owning package or crate.

## packages/ui

### Responsibility

- Own shared, reusable UI primitives and stateless layout helpers.
- Stay free of feature business logic.
- Export components that can be consumed by multiple modules without pulling in
  API, SSH, Database, Workspace, or app-shell behavior.

### Allowed Transitional Shape

Some shell layout primitives live in `packages/ui` while the desktop workbench
continues to stabilize. They are allowed only while they remain stateless and
feature-neutral.

### Forbidden

- Importing feature packages.
- Owning feature request, query, SSH session, or workspace business state.
- Creating feature-specific variants that belong in a feature package.

## packages/workspace-core

### Responsibility

- Own shared frontend workspace state: active workspace, active tab, sidebar
  collapse, workspace tabs, and selected resource IDs that must be globally
  visible.
- Re-export shared workspace types from `packages/command-client`.

### Transitional Dependency

Feature packages may currently read selected connection/request IDs from
`packages/workspace-core`. New dependencies on additional shared workspace
state require review.

## packages/workspace-local

### Responsibility

- Own future local workspace lifecycle, persistence, import/export,
  recent-workspace, and migration behavior.
- Depend on `packages/workspace-core` for shared workspace contracts.

### Current Boundary

`packages/workspace-local` is a compatibility boundary and may re-export
`packages/workspace-core` until concrete local workspace behavior is scoped.

### Forbidden

- API request state.
- Database SQL state.
- SSH terminal state.
- App-shell orchestration.

## Feature Packages

| Package | Responsibility | Forbidden |
| --- | --- | --- |
| `packages/api-client` | API Client UI: request drafts, request tabs, Send behavior, response display, history, saved requests, collections, environments, import/export. | Database logic, SSH logic, global shell behavior. |
| `packages/ssh-terminal` | SSH Terminal UI: connections, sessions, terminal panes, split/search/log UI, host-key trust UI, terminal-local state. | API request logic, SQL/database logic, global shell behavior. |
| `packages/database` | Database UI: connection tree, schema tree, SQL editor, query results, table inspector, database-local state. | API request logic, SSH session logic, global shell behavior. |
| `packages/command-client` | Typed Tauri command wrappers, shared frontend command types, and browser-dev mocks. | React components, feature business logic, feature state. |

Feature packages should call backend behavior through `packages/command-client`
and should reuse `packages/ui` primitives where possible.

## Rust Boundaries

| Crate | Responsibility | Forbidden |
| --- | --- | --- |
| `crates/unfour-core` | Shared Rust models, error/result types, redaction helpers, reserved AI/sync contracts. | Tauri adapter logic, UI behavior. |
| `crates/local-storage` | SQLite migrations, local database access, and local activity logging. | Raw secret storage. |
| `crates/secret-store` | Credential reference management backed by OS keychain or test memory store. | SQLite plaintext secret persistence. |
| `crates/http-engine` | API request execution, environment resolution, saved requests, history, redaction persistence. | UI state, database query execution, SSH sessions. |
| `crates/database-engine` | Database connection CRUD, schema browsing, query execution, browse-table behavior, SQL safety classification. | API request execution, SSH sessions. |
| `crates/ssh-engine` | SSH connection/session service, terminal events, host-key handling, reconnect, log export. | API request execution, SQL execution. |
| `crates/workspace-engine` | Workspace CRUD, active workspace state, environment variables, layout persistence. | Feature-specific execution. |
| `crates/unfour-command-bus` | Reusable Rust command entry point for Tauri, MCP, and future AI/CLI adapters. | UI components, duplicated domain logic. |
| `crates/unfour-mcp` | Local stdio MCP server adapter over the command bus. | Bypassing command-bus safety, redaction, or tool policy. |

`apps/desktop/src-tauri` is the Tauri adapter and composition layer. Backend
capability logic belongs in crates, not in Tauri command wrappers.

## Dependency Direction

Allowed frontend direction:

```text
apps/desktop
  -> packages/app-shell
  -> feature packages (api-client, database, ssh-terminal)
  -> packages/workspace-core
  -> packages/ui
  -> packages/command-client

packages/workspace-local
  -> packages/workspace-core
```

Feature packages may depend on:

- `packages/ui`
- `packages/command-client`
- `packages/workspace-core` as a documented transitional dependency

Forbidden:

- Feature package -> `packages/app-shell`
- `packages/ui` -> feature package
- `packages/command-client` -> feature package
- Feature package -> another feature package
- Tauri command adapter -> duplicated domain logic
- MCP tool handler -> duplicated domain logic

## Command-Bus Rule

Manual UI actions, MCP tools, and future AI/CLI adapters must use the same Rust
command bus path:

```text
adapter -> CommandBus -> service -> driver
```

Tauri commands and MCP tools are adapters. They must not become parallel
implementations of API, SSH, Database, Workspace, credential, or activity
behavior.

## Stable Exceptions To Revisit

- `apps/desktop/src/App.tsx` is broad because it is the desktop composition
  root. It must remain a composition layer and must not absorb feature
  business logic.
- `packages/ui` contains stateless shell layout helpers. Keep them
  feature-neutral.
- Feature packages use `packages/workspace-core` for selected resource state.
  Re-evaluate this after workspace ownership stabilizes.

There is no current frontend `packages/command-bus` package and no current
`extension-contracts` workspace member. Do not create either without an
explicit task and boundary review.
