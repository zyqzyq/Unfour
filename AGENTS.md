# Unfour Repository Instructions

Unfour is a lightweight IDE-style desktop developer tool built on Tauri 2,
React, TypeScript, and Rust. Its core modules are API Client, SSH Terminal,
Database, and Workspace.

## Architecture and security invariants

- Feature business state and components belong in their owning packages or
  crates. `packages/app-shell` owns global layout, navigation, route assembly,
  and module slots only; no request execution, SQL editing/execution, SSH
  session state, feature mock data, or large feature components.
- `packages/ui` owns reusable, feature-neutral primitives; no business logic
  or feature imports. Feature packages must not depend on app-shell, other
  feature packages, workspace-local, or a future workspace-sync package.
- Shared frontend contracts come from `packages/command-client`,
  `packages/workspace-core`, and `packages/ui`. workspace-core owns shared
  workspace state; workspace-local reserves local lifecycle, persistence,
  import/export, recent-workspace, and migration behavior.
- Frontend interaction lives in React/TypeScript; execution and security
  boundaries live in Rust. Frontend backend calls use command-client.
  Business actions follow `adapter -> CommandBus -> service -> driver`.
  Tauri and MCP adapters must not duplicate domain logic.
- `apps/desktop/src-tauri` is the desktop adapter/composition layer; shared
  Tauri composition lives in `crates/unfour-app`, capabilities in owning crates.
- Every persisted business record carries `workspace_id`, except truly global
  app configuration.
- Persist credential references, never plaintext passwords, private keys,
  passphrases, or tokens in SQLite. Redact `authorization`, `cookie`,
  `proxy-authorization`, `x-api-key`, and `x-auth-token` in logs, history,
  and local activity details.
- Cloud-bound local mutations retain durable outbox intent in the business
  transaction regardless of login, active account, entitlement, pause, offline,
  or worker state. Each local workspace has at most one durable Cloud Sync
  owner `(account_id, cloud_workspace_id)`; mutations and repair resolve that
  owner and never fan out across historical bindings.
- New user-visible frontend copy uses the shared i18n provider/hook and locale
  keys. Do not create package-local translation systems. MCP tool names,
  schemas, command names/keys, event names, request metadata keys, and stable
  error codes stay English; localize only UI-facing messages.
- Unfour's UI style is fixed. Its only design authorities are
  [design.md](design.md), [design-system.md](docs/ui/design-system.md), and
  [interaction-guidelines.md](docs/ui/interaction-guidelines.md).
  Use relevant sections for UI work; do not generate a competing design system.
- Add dependencies only when the task requires them and document the reason.
  Keep changes scoped and preserve uncommitted user work.
- Current release readiness and verification evidence belong in
  `docs/release/` and `docs/testing/`; `docs/archive/` is historical context.
  Internal engineering and agent docs do not need translations unless requested.

## Context entry points

Apply local `AGENTS.md` instructions in directories you touch; they add scoped
constraints without repeating these invariants. Use
[START_HERE](docs/agents/START_HERE.md) when you need an ownership or context
pointer, and [EXECUTION_PROTOCOL](docs/agents/EXECUTION_PROTOCOL.md) for completion,
autonomy, or verification guidance. These are references, not a mandatory
reading sequence; reuse context already available.

Repository instructions are defaults under higher-priority system/developer
instructions and the user's explicit task. Local docs and skills do not expand
the authorized scope or silently override global architecture/security rules.
