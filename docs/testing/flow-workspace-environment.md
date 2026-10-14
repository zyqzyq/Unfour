# Flow run environment and Workspace semantics

Date: 2026-10-14. Baseline: latest `origin/main`, `91154913`.
Frontend-only fix; see [Flow V1](../architecture/flow-v1.md).

## Behavior and compatibility

- Each desktop Run / Run again opens with a snapshot of the current Workspace's
  active variable environment from the shared workspace-scoped query cache.
  No active environment means Workspace variables only.
- If the initial query is pending or has failed, submission stays disabled until
  a successful query/retry confirms the environment state. Only the first
  success may fill an untouched pending default. Selecting any value, including
  Workspace-only, replaces the initialization token; late results cannot replace
  it. Reopening, changing Flow and switching Workspace also replace the token.
  Later active-environment changes/refetches never change an initialized or
  manually selected value.
- The existing effects notice, masked preview, revision pin, resource validation
  and explicit Run confirmation remain. The selected ID is sent explicitly;
  Workspace-only sends `environmentId: null`. No automatic execution is added.
- A removed selected environment stays visible as its ID and blocks submission
  until the user chooses a valid environment or Workspace-only.
- Run details show only the saved `context.environmentId`, or Workspace-only
  for absent/null IDs. Current names are never presented as historical names.
  Lightweight history summaries do not contain environment context; the ID is
  shown after selecting a run and loading its existing full snapshot.
- Title-bar tooltips and accessible descriptions distinguish Workspace type
  DEV/TEST/PROD, active variable environment, and independent MCP policy.
  Shared UI receives an optional description prop; feature copy stays in the
  shell's shared i18n resources. English and Chinese are covered.
- No dependencies, Rust, persisted Schema, Cloud Sync protocol or Flow definition
  format changed. MCP's omitted/null `environmentId` still means Workspace-only;
  the MCP adapter and policy model are unchanged.

## Verification

PASS: 24 Vitest files, 230 tests:

```powershell
pnpm exec vitest run packages/flow/src packages/app-shell/src/components/AppTitleBar.test.tsx packages/app-shell/src/components/WorkspaceMenu.test.tsx packages/app-shell/src/components/useDesktopNavigation.test.tsx packages/workspace-core/src/workspace-mcp-policy.test.ts packages/command-client/src/tauri/flow.test.ts packages/ui/src/i18n.test.ts
```

The 16 new Flow tests cover active/no-active/empty defaults, manual environment
and Workspace-only overrides, asynchronous initial loading, subsequent refreshes,
repeated opens and Run again, initialization failure, environment deletion,
Workspace switching with an old query pending, submitted arguments, and recorded
IDs including legacy absent IDs. Two title-bar cases cover both locales with
PROD and an explicit full-access policy to keep type and policy distinct.

PASS: changed TypeScript/React/spec ESLint (zero errors; one function-length
warning in `useFlowEditor`), `pnpm --filter @unfour/desktop exec tsc --noEmit`,
`pnpm run build`, `pnpm run check:large-files` (zero blocking files), and
`git diff --check`. Vite retains its large-chunk advisory.

PASS: Chromium's existing Flow Canvas and Run experience specs (5 tests), plus
the new bilingual environment spec (2 tests):

```powershell
pnpm exec playwright test apps/desktop/tests/smoke/flow-environment.spec.ts apps/desktop/tests/smoke/flow-run-experience.spec.ts apps/desktop/tests/smoke/flow-canvas.spec.ts --project=chromium --workers=2
```

The first browser run passed the existing five cases; the new spec's locator and
Chinese button text were corrected, then both new cases passed twice. Final
screenshots for default selection, manual Workspace-only and historical IDs were
inspected in `test-results/flow-environment-*.png`. Intermediate unit-test copy
and asynchronous assertion failures were corrected before the final passing suite.

PASS: existing isolated Rust tests, without changing Rust source:

```powershell
cargo test -p unfour-mcp flow_confirmation_initiator_redaction_summary_detail_and_cancel_share_desktop_history --lib
cargo test -p unfour-command-bus flow_ssh_defaults_follow_explicit_environment_only_and_are_opt_in --lib
```

The MCP test exercises omitted environment, payload-bound consent and shared
history. The bus test verifies Workspace-only versus explicit environment
defaults without remote SSH execution. Source inspection confirms MCP still
maps an omitted environment to null. Existing release-channel/dead-code compiler
warnings do not affect these passing tests.

## Limits

Browser fixtures verify frontend behavior, not native Tauri execution or real
API/SSH/database side effects; those integrations were NOT VERIFIED here.
Historical environment names are not persisted, so displaying the recorded ID
is intentional. Environment/resource changes concurrent with execution remain
subject to the existing backend validation and race limitations in Flow V1.

## Retry recovery follow-up — 2026-10-14

Baseline: latest `origin/main`, `8a4bd9a3`, clean working tree on `main`.

Root cause: the `openRun` refetch callback cleared `runEnvironment.pending` on
failure as well as success. The dialog's Retry button only refreshed queries,
so a later successful response never initialized the default. The new active
environment regression failed against that baseline (received Workspace-only),
while the two no-active-environment variants passed.

The fix is limited to `useFlowEditor.ts`. An open dialog with an untouched
pending default now observes the existing shared query's first success,
including explicit retries or a shared query refresh. Failures leave the
default pending. The update checks both the open dialog and the same selection
object; manual selection, Flow reset and reopening replace that object, and
Workspace changes already remount the editor. After successful initialization
or an explicit selection, subsequent query changes cannot replace the value.
The existing pending/error execution gates, effects consent, revision pin and
MCP environment contract are unchanged. No environment state model, dialog
layout, dependencies, API, Database, Rust or MCP implementation was changed.

PASS:

- `pnpm exec vitest run packages/flow/src`: 18 files, 206 tests.
- `pnpm test --reporter=dot`: all 179 frontend files, 1209 tests.
- `pnpm exec eslint packages/flow/src/useFlowEditor.ts packages/flow/src/FlowEnvironment.test.tsx`:
  zero errors, two non-blocking warnings (existing hook length and guarded
  query-to-state synchronization in an effect; no rule suppression added).
- `pnpm --filter @unfour/desktop exec tsc --noEmit`.
- `pnpm run build`: retains the existing Vite large-chunk advisory.
- `pnpm exec playwright test apps/desktop/tests/smoke/flow-environment.spec.ts --project=chromium --workers=1`:
  both existing English/Chinese browser interaction tests.
- `git diff --check`.

The environment suite now has 30 cases, including 14 new regressions for
failure then successful retry, pending submission guards, no active environment
and empty environment lists, manual Workspace-only before/during retry, cached
manual environment choices across failed refreshes, shared-query recovery,
close/reopen, changing Flow, changing Workspace, and delayed old retry results.
Assertions also verify explicit run arguments and that loading/recovery never
automatically starts a run. Browser tests cover existing default/override
interactions; retry-specific coverage uses real React Query with deferred
command mocks in the component suite. Native Tauri and real service effects
remain NOT VERIFIED in this frontend-only follow-up.
