# Flow run environment and Workspace semantics

Date: 2026-10-14. Baseline: latest `origin/main`, `91154913`.
Frontend-only fix; see [Flow V1](../architecture/flow-v1.md).

## Behavior and compatibility

- Each desktop Run / Run again opens with a snapshot of the current Workspace's
  active variable environment from the shared workspace-scoped query cache.
  No active environment means Workspace variables only.
- If the initial query is pending, submission stays disabled until it resolves.
  Only that initial load may fill the default. Selecting any value, including
  Workspace-only, replaces the initialization token; late results cannot replace
  it. Reopening, changing Flow and switching Workspace also replace the token.
  Later active-environment changes/refetches never change an open selection.
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
