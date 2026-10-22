# Cloud Sync UI refresh verification

Date: 2026-10-22. Baseline: `c3cac8b` (the separately committed Flow refresh fix).
Scope: receiving committed cloud data in already-open desktop modules.

## Implementation

The desktop Cloud Sync layer owns one command-client event subscription. Query
owners register local reads using `meta: {cloudSyncWorkspaceId: workspaceId}`;
the global workspace list uses `null`. A shared command-client contract/matcher
keeps this independent of feature query names. No dependency was added and no
feature imports another feature, app-shell or a sync implementation package.

| Owner | Registered reads |
| --- | --- |
| App shell | Workspace list for changes in any workspace; current environments and sidebar Database connections |
| API Client | Saved requests (both editor and tree observers), collections, folders, environments |
| SSH | Connections including preload; task list and detail/steps, including imperative detail fetches |
| Database | Connection definitions |
| Workspace variables | Workspace variables and environments including their variables |
| Flow | Definitions, resource references, environments and workspace variables |

The desktop refresher invalidates matching workspace caches, refreshes active
observers and leaves disabled/inactive queries stale for their next activation.
Pending reads finish for existing callers and trigger another invalidation on
settlement, covering pre-commit results without canceling imperative task reads.
It rereads after asynchronous listener
setup, cleans up late subscriptions, and falls back to registered local reads
every 15 seconds after subscription failure. Browser previews skip native
startup rereads. Cloud HTTP polling remains exclusively in the Rust Worker.

Editor drafts and execution snapshots retain their existing selection and
concurrency rules. SQL results/history, terminal sessions/output, task runs,
credentials, layout and database schema/catalog requests are not registered
for this refresh. No query execution, SSH command or API Send is initiated.

The initial Flow-only hook was removed after moving its regression assertions
to desktop integration tests. The post-commit Rust notification path is
unchanged from `c3cac8b`.

## Checks

| Check | Result |
| --- | --- |
| Vitest: API Client, Database, SSH, Workspace environments, Flow, app-shell, desktop source, command-client cloud-sync adapter, desktop integrations | PASS: 166 files, 1,121 tests |
| `pnpm run build` | PASS: desktop TypeScript and production bundle; existing bundle-size warning |
| TypeScript compiler API check of all four new desktop integration test files using desktop compiler options and `vite/client` types | PASS: zero diagnostics |
| ESLint on changed TypeScript files | PASS: zero errors; existing function-size/complexity warnings remain |
| `node scripts/check-large-files.mjs --top 4` | PASS: zero blocking files; existing advisories remain |
| `git diff --check` | PASS |

Regression tests exercise real query owners and editor state with mocked local
commands: initially empty API/Flow/SSH lists receive data, renamed/deleted items
update, SSH step detail caches refresh, and API bodies, SQL/connection edits,
SSH task drafts, Flow drafts and dirty variable values are retained. A separate
workspace-switcher case receives another workspace's addition/deletion while
keeping current-workspace resources isolated. Generic lifecycle tests cover
initial-read races, repeated commits during refetch, global query scope,
unrelated/runtime exclusion, disabled-query activation, preview behavior,
subscription fallback and cleanup. The desktop composition test verifies one
subscription is mounted even when account commands are unavailable.

Intermediate runs: FAIL on a new hidden-query fixture that seeded an imperative
read before startup refresh finished. Final review identified the same
cancellation risk for user-initiated reads; refresh now queues another read
after settlement instead of canceling pending reads. A dedicated imperative
fetch regression verifies completion and subsequent cache invalidation.
An unstable inline navigation callback in the new SSH test shell
caused render churn; those runs were interrupted, the fixture now passes a
stable callback, and its focused and broader reruns passed. Product assertions
and existing tests were not weakened.

NOT VERIFIED: native WebView event transport and real two-device/cloud timing.
No live account, real workspace or remote resource was changed, and no build
was published. Verify on a receiving device by leaving each module open while
another device creates, renames and deletes its synced records, then allowing
the receiver to pull or using Sync Now. Repeat with dirty editors and while a
different workspace/module is visible.
