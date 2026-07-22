# Package Boundaries

This architecture reference defines intended ownership, dependency direction,
and forbidden cross-package behavior. It is a stable boundary document, not a
progress log.

## packages/app-shell

### Responsibility

- Own the frontend desktop workbench composition root.
- Provide global shell composition, workspace switcher wiring, module
  navigation, layout slots, command palette actions, diagnostics actions, and
  module mount points.
- Mount the API Client, SSH Terminal, and Database modules while keeping their
  internal state and business components in the owning feature packages.
- Own shell-level behavior only when it is not feature-specific.

### Forbidden

- API request execution or request state.
- SQL editing/execution or database state.
- SSH session state, terminal buffers, or terminal feature logic.
- Feature mock data.
- Feature-specific large UI components.

`packages/app-shell` may compose feature modules and pass shell props, but
feature behavior belongs in the owning package or crate.

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

- Own shared workspace frontend types, store, and contracts.
- Own current workspace state, active tab, sidebar collapse, selected resource
  IDs that must be globally visible, layout snapshot state, and future adapter
  contract types.
- Re-export shared workspace types from `packages/command-client`.
- Avoid local workspace lifecycle, cloud, or sync implementation details.

### Transitional Dependency

Feature packages may currently read selected connection/request IDs from
`packages/workspace-core`. New dependencies on additional shared workspace
state require review.

## packages/workspace-local

### Responsibility

- Own the OSS default local workspace lifecycle boundary.
- Reserve local workspace lifecycle, recent workspace, import/export,
  persistence lifecycle, and migration behavior.
- Depend on `packages/workspace-core` for shared workspace contracts.

### Current Boundary

`packages/workspace-local` is a compatibility/transitional package in v0.1 and
may re-export `packages/workspace-core` until concrete local workspace behavior
is scoped. It is not a completed cloud/local provider abstraction.

### Forbidden

- API request state.
- Database SQL state.
- SSH terminal state.
- App-shell orchestration.

## Future Pro Sync

Future Pro sync should be modeled as a local-first sync overlay, not as a
cloud-primary workspace provider. `workspace-sync` is the recommended
long-term package name because local SQLite remains the runtime source of truth
and cloud support reconciles local workspace data periodically.

Feature packages must not depend on `packages/workspace-local` or any future
`packages/workspace-sync`. API Client, Database, and SSH Terminal should depend
only on `packages/workspace-core`, `packages/command-client`, and
`packages/ui` for shared frontend contracts. App-shell and edition composition
layers choose whether local-only or Pro sync capabilities are wired in.

## Feature Packages

| Package | Responsibility | Forbidden |
| --- | --- | --- |
| `packages/api-client` | API Client UI: request drafts, request tabs, Send behavior, response display, history, saved requests, and collections. Workspace environment data and resolution are consumed through shared workspace contracts. | Database logic, SSH logic, global shell behavior, workspace variable management, persistence, or resolution. |
| `packages/workspace-environments` | Workspace-level variables and environments management UI, editor state, navigation guards, and frontend CRUD hooks. | API request execution, feature navigation, variable resolution, or app-shell orchestration. |
| `packages/ssh-terminal` | SSH Terminal UI: connections, sessions, terminal panes, split/search/log UI, host-key trust UI, terminal-local state. | API request logic, SQL/database logic, global shell behavior. |
| `packages/database` | Database UI: connection tree, schema tree, SQL editor, query results, table inspector, database-local state. | API request logic, SSH session logic, global shell behavior. |
| `packages/command-client` | Typed Tauri command wrappers, shared frontend command types, and browser-dev mocks. | React components, feature business logic, feature state. |

Feature packages should call backend behavior through `packages/command-client`
and should reuse `packages/ui` primitives where possible.

## Rust Boundaries

| Crate | Responsibility | Forbidden |
| --- | --- | --- |
| `crates/unfour-core` | Shared Rust models, error/result types, redaction helpers, reserved AI/sync contracts. | Tauri adapter logic, UI behavior. |
| `crates/unfour-paths` | Stable runtime path resolution shared by desktop and MCP processes. | Feature execution, UI behavior. |
| `crates/unfour-diag` | Structured diagnostics, file logging, log retention, correlation IDs, and diagnostic bundle export. | Feature business execution, raw secret persistence. |
| `crates/local-storage` | SQLite migrations, local database access, and local activity logging. | Raw secret storage. |
| `crates/secret-store` | Credential reference management backed by OS keychain or test memory store. | SQLite plaintext secret persistence. |
| `crates/http-engine` | API request execution after workspace-variable resolution, saved requests, history, redaction persistence. | Workspace variable persistence/resolution, UI state, database query execution, SSH sessions. |
| `crates/database-engine` | Database connection CRUD, schema browsing, query execution, browse-table behavior, SQL safety classification. | API request execution, SSH sessions. |
| `crates/ssh-engine` | SSH connection/session service, terminal events, host-key handling, reconnect, log export. | API request execution, SQL execution. |
| `crates/workspace-engine` | Workspace CRUD, active workspace state, workspace variables/environments, shared variable resolution, and layout persistence. | Feature-specific execution. |
| `crates/unfour-command-bus` | Reusable Rust command entry point for Tauri, MCP, and future AI/CLI adapters. | UI components, duplicated domain logic. |
| `crates/unfour-mcp` | Local stdio MCP server adapter over the command bus. | Bypassing command-bus safety, redaction, or tool policy. |

`apps/desktop/src-tauri` is the thin Tauri desktop binary and edition adapter.
Shared Tauri composition lives in crates/unfour-app.

## Dependency Direction

Allowed frontend direction:

```text
apps/desktop
  -> packages/app-shell
  -> feature packages (api-client, database, ssh-terminal, workspace-environments)
  -> packages/workspace-core, packages/command-client, packages/ui

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
- Feature package -> `packages/workspace-local`
- Feature package -> future `packages/workspace-sync`
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

- `packages/app-shell/src/DesktopApp.tsx` is broad because it is the frontend
  desktop workbench composition root. It must remain a composition layer and
  must not absorb feature business logic.
- `packages/ui` contains stateless shell layout helpers. Keep them
  feature-neutral.
- Feature packages use `packages/workspace-core` for selected resource state.
  Re-evaluate this after workspace ownership stabilizes.

There is no current frontend `packages/command-bus` package and no current
`extension-contracts` workspace member. Do not create either without an
explicit task and boundary review.
