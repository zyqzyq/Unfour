# Flow Cloud Sync refresh verification

Date: 2026-10-22. Scope: keeping an already-open Flow page current after remote
data commits. No live account or real workspace was modified.

## Finding and change

`useFlowWorkspaceData` previously loaded the definition list with React Query
but had no subscription or periodic refresh. Cloud Sync committed remote data
through `apply_external_page_on` and `finish_external_cleanup` without notifying
the frontend. A successful background pull could therefore leave an empty or
outdated Flow sidebar until another query fetch, such as reopening the app.
This establishes a display defect consistent with the reported symptom; it
does not establish the timing of the user's original network request.

The post-commit path now reports affected local workspace IDs to the desktop
adapter. Its `cloud-sync://workspace-changed` event carries `{workspaceId}` only.
Flow subscribes through command-client and invalidates its own workspace
queries: definitions, resources, environments and variables. App-shell and
other feature packages receive no Flow business state or imports.

Listener readiness rereads data to cover commits during asynchronous setup.
Pending reads are canceled so a pre-commit result cannot replace fresh data.
Unmount and workspace changes dispose subscriptions, including late setup.
Subscription failure records a static diagnostic and falls back to local reads
every 15 seconds. Browser previews skip native subscriptions and startup
rereads. The editor draft, expected revision, runtime inputs and run history
remain under their existing selection and concurrency behavior.

Cloud requests retain the Rust Worker's existing schedule (periodic pull every
five minutes). This fix makes committed data visible; it does not shorten the
cloud polling interval or introduce a frontend network scheduler.

## Verification

| Check | Result |
| --- | --- |
| `pnpm exec vitest run packages/flow/src packages/command-client/src/tauri/cloud-sync.test.ts packages/command-client/src/tauri/flow.test.ts` | PASS: 21 files, 222 tests |
| `cargo test -p unfour-cloud-sync --test flow` | PASS: 24 tests |
| `cargo test -p unfour-cloud-sync --test worker` | PASS: 122 tests |
| `cargo check -p unfour` | PASS: desktop event plugin compiles |
| `pnpm run build` | PASS: desktop TypeScript and production bundle; existing chunk-size warning |
| ESLint on changed TypeScript files | PASS |
| `cargo fmt -p unfour-cloud-sync -p unfour --check` | PASS |
| `node scripts/check-large-files.mjs` | PASS: zero blocking findings; existing advisories remain |
| `git diff --check` | PASS |
| Additional standalone package TypeScript checks | FAIL: Flow has 104 diagnostics; command-client has 3. TypeScript compiler-host comparison against HEAD sources gives the same counts and diagnostic messages, with zero introduced diagnostics. Existing test fixture/type configuration errors remain outside this fix. |

Regression coverage includes an initially empty open page receiving Flow data,
remote rename/deletion without losing a dirty editor draft, matching-workspace
resource/variable refresh, stale initial reads, late subscription cleanup and
subscription failure recovery. Backend tests cover committed create/delete,
quiet repeated sync, snapshot download, no notification on transaction
rollback, unresolved conflict and the subsequent use-remote resolution.

The first broader frontend run failed 12 environment tests because the initial
implementation also reread data after a browser-preview no-op subscription.
The adapter now distinguishes that path with `null`; all 222 tests pass after
the fix. The first sandboxed Vitest attempt failed before executing tests due
to denied temporary-cache renames; the successful suite ran outside the
sandbox against mocked local commands. No safety assertions were removed.

NOT VERIFIED: native WebView event delivery and real two-device/cloud timing.
To verify those paths, keep Flow open on a receiving device, create/rename/delete
a definition on another device, let the receiver pull (or use Sync Now), and
check that its sidebar updates without reopening. Repeat with an unsaved editor
draft and with a Flow referencing a resource arriving in a later sync page.
