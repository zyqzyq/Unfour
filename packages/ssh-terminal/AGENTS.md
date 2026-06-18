# ssh-terminal Agent Rules

## Scope

`packages/ssh-terminal` owns the SSH Terminal frontend: SSH connection forms,
connection tree, terminal sessions, terminal panes, split/search/log UI,
host-key trust UI, and terminal-local state.

## Boundaries

- Backend calls must go through `@unfour/command-client`.
- Shared controls should come from `@unfour/ui` when practical.
- Workspace selection state may use `@unfour/workspace-core` only as the
  documented transitional boundary.
- This package must not own API request, Database SQL, or app-shell behavior.

## Rules

- Do not make unrelated cross-package changes.
- Prefer existing terminal hooks, stores, models, and components.
- Do not introduce new dependencies unless the task explicitly requires them.
- Do not weaken terminal log redaction or host-key trust behavior.
- Real SSH connection behavior requires explicit manual verification against a
  reachable SSH server.

## Required Output

After changes here, report:

- Modified files
- Behavior changed
- Validation commands run
- Manual verification needed
- Follow-up risks
