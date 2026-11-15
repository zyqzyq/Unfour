# SSH Terminal structural refactor verification — 2026-11-15

Base: local `main` at `a6ed0d1f`. Scope: SSH Terminal frontend composition only.
Existing untracked DSH/SQLx research, evidence, and PoC files were left untouched.

## Ownership and preserved timing

- `TerminalPage.tsx` composes the existing workspace/tree/dialog components and
  retains workspace activation, active-session reconciliation, shared mutation
  scope, search keyboard listener, and stable Shell Sidebar installation/cleanup.
- `useSshConnectionManagement` owns connection drafts, external-update conflict
  detection, save/test mutations, and host-key trust/mismatch/reset UI flows.
  Clean/dirty form adjustment still happens during render. Save still cancels
  queries, updates the originating cache, invalidates it, then checks scope before
  changing selected connection/dialog state.
- `useTerminalSessionOrchestration` owns session query results, frontend-failure
  merging, stable creation-order tabs, frozen event selection, channel/history
  wiring, and connect/close mutations. Existing `useTerminalSessionActions`
  continues to own confirmation, reconnect, batch close, and SFTP removal.
- Channel/history effects remain before page workspace activation; activation
  remains before active-session reconciliation. Hidden pages stop polling/history
  hydration and freeze their event snapshot, while the channel keeps buffering.
- Reconnect retains the source session until the new connection succeeds; frontend
  failures still have no backend session to close. Existing scope guards and
  mutation callback bodies are preserved.

No public component interfaces, command-client/CommandBus contracts, Rust code,
dependencies, state frameworks, SSH transport/security policy, temporary/parent
session implementation, AccessClient, SSH Tasks, or API/Database tunnels changed.
Log export/redaction remains covered by the existing log panel/store tests.
See the [package structure](../../packages/ssh-terminal/README.md).

## Regression coverage

The initial additions passed against the original page before extraction:

| Test file | Coverage added |
| --- | --- |
| `TerminalPage.connections.test.tsx` | New/edit/save, secret whitespace and stored credential behavior, optimistic snapshot, dedicated test endpoint success/failure, first host trust, late save after workspace switch |
| `TerminalPage.lifecycle.test.tsx` | Workspace isolation/restoration, stale connect/close callbacks, hidden event freeze/resume and polling options, stable tab ordering after refreshed results, Sidebar callback identity/cleanup |
| `hooks/useTerminalSessionActions.test.tsx` | Frontend/backend failed-session distinction, frontend reconnect request, source session/SFTP retention until success, mixed batch targets |

Existing tests additionally cover external clean/dirty/deleted connection conflict
protection, host-key mismatch/reset, activity/history failure feedback, session
history cancellation, terminal/SFTP stores, pane lifecycle, and log export/masking.
Two further page integration tests exercise real mutation callback sequencing:
reconnect failure preserves the source Session/SFTP, success activates the new
session before source cleanup, and a late connection rejection after workspace
switch cannot create a frontend failure in either workspace. The lifecycle file
passes all 8 tests after these additions (20 tests added overall).
Tests use isolated query clients, local stores, and mocked command-client calls;
they do not establish native SSH or live-server behavior.

## Executed checks

Commands ran from the repository root. `--config.manage-package-manager-versions=false`
uses the available pnpm without attempting to install another package-manager
version; no repository configuration or dependencies were changed.

| Result | Command / evidence |
| --- | --- |
| PASS | Initial baseline: activity, session-query, active-history, and log-panel tests; 4 files / 19 tests |
| PASS | Post-extraction targeted page/action regressions; 4 files / 29 tests |
| PASS | Final `pnpm --config.manage-package-manager-versions=false exec vitest run packages/ssh-terminal`; 50 files / 393 tests |
| PASS | `pnpm --config.manage-package-manager-versions=false --filter @unfour/desktop exec tsc --noEmit` |
| PASS | `pnpm --config.manage-package-manager-versions=false --filter @unfour/desktop build`; runs the same prepare-monaco + tsc + Vite script delegated to by root `build` |
| PASS with warnings | Targeted ESLint on the page, both new hooks, and three changed/added test files; 0 errors, 3 warnings (two hook lengths and connection-hook complexity). Responsibility boundaries were retained rather than subdividing solely for metrics. |
| PASS | `pnpm --config.manage-package-manager-versions=false run check:large-files`; 0 blocking; existing repository size findings remain |
| PASS | `git diff --check` |
| FAIL, recovered | Initial plain pnpm invocation and root build's nested pnpm invocation attempted package-manager provisioning and failed with sandbox symlink `EPERM`. Explicit configuration plus direct desktop build completed successfully. |

The package suite emitted a jsdom canvas-context notice; no test failed. Vite
emitted its large-chunk warning; production bundling completed successfully.

## Remaining verification

- NOT VERIFIED: live SSH/SFTP transport, native desktop manual interactions, and
  real JumpServer/Bastion acceptance. This refactor makes no claim to complete
  that acceptance and does not change those implementations.
- No new UI regression was observed in the automated component/hook coverage.
