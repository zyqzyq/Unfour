# Flow V1 verification

Follow-up: [Wait Until and structured inputs verification](flow-wait-until.md).

Verified on Windows on 2026-09-16, based on the repository's existing `main`.
This is implementation evidence, not a production-service certification.

## Automated checks

| Check | Result |
| --- | --- |
| `cargo test -p unfour-command-bus -p unfour-local-storage -p unfour-workspace-engine --features unfour-command-bus/ssh-native` | PASS: 162 tests, including 9 Flow tests |
| `cargo check --workspace` | PASS |
| `cargo check -p unfour --features ssh-native` | PASS |
| `cargo fmt -p unfour-flow-engine -p unfour-command-bus -p unfour-core -p unfour-app -p unfour-http-engine -p unfour-workspace-engine --check` | PASS |
| `node node_modules/vitest/vitest.mjs run packages/flow/src packages/workspace-core/src packages/app-shell/src packages/command-client/src/tauri.test.ts packages/ui/src/i18n.test.ts` | PASS: 23 files, 102 tests |
| Final `vitest run packages/flow/src` after editor/query fixes | PASS: 3 tests |
| Final `cargo test -p unfour-command-bus --features ssh-native flow_` after redaction hardening | PASS: 9 tests |
| `node node_modules/typescript/bin/tsc --noEmit -p apps/desktop/tsconfig.json` | PASS |
| Vite production build in `apps/desktop` | PASS; existing large-chunk warning remains |
| ESLint on changed frontend files | PASS, zero errors; FlowPage function-length warning and two existing DesktopApp complexity/length warnings |
| Migration checker | PASS: 28 migrations |
| Shared-token and version checkers | PASS |
| Public-repository secret audit | PASS |
| Large-file checker | PASS, zero blocking files |
| `git diff --check` | PASS |

The Cargo tests comprise 94 command-bus unit/integration tests, 47 local-storage
tests, and 21 workspace-engine tests. Direct Node entry points were used because
the shell's pnpm executable/PATH did not consistently resolve project binaries.
No production credential or external service is required by these tests.

## Closed loops and failure cases

`flow_real_api_condition_native_ssh_database` exercises saved API resources,
the actual HTTP engine, Condition, an existing SSH Task through the native
transport, and the SQLite database driver. A loopback russh fixture authenticates
the connection, records the command produced from API output (`echo 42`), and
returns a deterministic transcript and successful SSH exit status. This is a
real protocol exchange; the fixture does not execute an operating-system shell.

`flow_real_http_start_poll_and_database` sends exactly one POST start request,
then three GET status requests with the returned job ID. The first two probes
return `ready: false`, the third `ready: true`. A subsequent SQLite query uses
the latest attempt's output. The test also deletes the start resource and
checks a persisted `validationFailed` Run without altering successful history.

Other Flow coverage verifies:

- Structured and interpolated references, missing-reference failure, true/false
  branch selection, skipped nodes, and rejection of backward jumps.
- Fail fast, revision conflicts, definition editing during execution, history
  after Flow deletion, and workspace isolation.
- Poll attempt exhaustion, total timeout, cancellation during polling, and
  stopping subsequent scheduling.
- Active HTTP cancellation closes the connection before the response arrives.
- Runtime secrets can feed later steps but do not appear in raw SQLite run JSON;
  inline sensitive definition literals are rejected.
- Saved bearer/basic/API-key auth works without UI materialization, with explicit
  enabled headers retaining precedence.
- Stale runs become interrupted without replaying side effects.

React tests verify explicit invocation context, confirmation/cancel calls,
independent invalid-JSON fields blocking Save/Run, and dirty-draft selection.

## UI inspection

PASS: browser preview at 1280 x 720, using the real React shell and browser
authoring adapter. Inspected Flow navigation, New Flow, vertical API/Poll steps,
explicit environment/inputs, Poll bounds/predicate controls, and invalid JSON
showing an error while disabling Save/Run. The dark layout uses existing shared
tokens and controls. Browser execution intentionally returns
`FLOW_DESKTOP_REQUIRED`, never a simulated successful Run.

NOT VERIFIED: an interactive packaged Tauri window driving production HTTP,
SSH, PostgreSQL, or MySQL servers. Desktop composition was compiled; backend
execution was verified through CommandBus with local HTTP/SSH fixtures and
SQLite. Native database query cancellation beyond future cancellation and
remote shell process termination guarantees were not established.

## Failures found and resolved

- Initial Flow tests failed from a single-connection SQLx pool deadlock: work
  held the connection while an inline heartbeat await prevented work from
  progressing. Both futures are now polled concurrently; regression tests pass.
- Initial frontend assertions failed when the Flow module was eagerly mounted
  and persisted layout expectations lacked Flow. Lazy mounting and compatible
  layout hydration fixed those failures.
- A strengthened HTTP method/path assertion initially failed because the
  existing HTTP engine preserves a trailing `?` for an empty query. The test now
  parses method/path and verifies one POST followed by GET probes. No existing
  HTTP behavior was changed to satisfy the test.

## Scoped UI closeout (2026-09-21)

- PASS: `pnpm exec vitest run packages/ui packages/flow packages/ssh-terminal packages/api-client packages/database`
  (103 files, 635 tests). Shared DropdownMenu coverage checks rendering outside
  an overflow-hidden parent, visibility, alignment/classes, selection and focus
  return. Flow coverage keeps summary failures in History, removes the alert
  on close, and retains main-page alerts for selected snapshot failures.
- PASS: `pnpm test:e2e` (10 Chromium smoke tests). The edge-menu regression
  pans the real Canvas until its insertion control is near the right edge,
  opens the menu, checks that it is outside the Canvas DOM subtree and extends
  beyond its bounds, then hit-tests every item to detect clipping/occlusion.
  The captured `test-results/flow-edge-menu-portal.png` was visually inspected.
- PASS: `pnpm run build` (TypeScript check and production build; bundle-size
  warning), `pnpm run lint` (0 errors, 45 warnings), and `git diff --check`.
- Initial sandboxed frontend/smoke launches failed with `spawn EPERM`; reruns
  with permission to launch local test processes passed as recorded above.
- Reviewed SSH task/connection menus, API request actions and DB tree/editor/
  export callers. Existing public props, styles and React event propagation
  remain intact; their frontend suites passed without caller patches.
- Architecture documentation now matches the existing `finished_at` migration
  and column-only History summary query. No engine, execution, graph or History
  model changes were made. Packaged native execution was not reverified by this
  UI-only closeout; the native limitations documented above still apply.

## Dependencies and follow-up

The new engine uses dependencies already present in the workspace. Command-bus
adds `russh` and `rand` as dev dependencies for the native protocol fixture;
these packages already existed in the lockfile. Frontend adds only the local
`@unfour/flow` package. No new third-party runtime implementation is introduced.

See [Flow architecture](../architecture/flow-v1.md) for known limits: literal
SQL/shell interpolation, best-effort cancellation, local-only persistence,
resource edit races, bounded outputs, and history pagination/retention work.

Canvas authoring and regression coverage: [Flow Canvas](flow-canvas.md).
