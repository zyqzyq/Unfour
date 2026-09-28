# Flow MCP V1 verification

Date: 2026-09-22. Base: current local main.

## Coverage

`crates/unfour-mcp/src/command_bus_adapter_tests/flow.rs` exercises the real
CommandBus adapter and isolated SQLite storage:

- Seven tools in tools/list, compilable input/output schemas, successful text
  content equal to structuredContent, and all Flow node variants round-tripping.
- Desktop CommandBus definitions readable through MCP; MCP saves readable by
  Desktop; shared validation and stable revision-conflict errors.
- Confirmation before run creation, incorrect/stale confirmation rejection,
  server-owned initiator=mcp and confirmEffects, and workspace policy matrix.
- Nested workspace mismatch and caller-supplied consent/provenance rejected.
- Shared human/MCP run history, detail versus five-field summary, and cancellation.
- A deliberately invalid run_json remains listable through summaries while
  get_run fails, proving the summary path does not decode full snapshots.
- Schema/manual secrets and authorization, cookie, proxy-authorization,
  x-api-key and x-auth-token values remain redacted, including validation-failed
  runs; input schema secret flags retain their boolean wire type.

## Results

- PASS: CommandBus and Flow Engine suites in
  `cargo test -p unfour-mcp -p unfour-flow-engine -p unfour-command-bus`.
  That initial aggregate run failed two MCP tool-count assertions (63 to 70),
  which were updated for the seven additions.
- PASS: final `cargo test -p unfour-mcp --quiet`: 198 library tests, four binary
  tests and two integration tests. An intermediate additional history test
  failed Windows file cleanup; explicitly closing its SQLite pool fixed it.
- PASS: `cargo check -p unfour-mcp` (including Flow/CommandBus dependencies).
- PASS: `cargo fmt -p unfour-mcp -p unfour-flow-engine -p unfour-command-bus --check`.
- PASS: `git diff --check`; large-file checker reports no blocking files.

## Limits

Codex/Cursor UI sessions were not manually exercised. Protocol/schema tests
cover the structuredContent contract implicated in Cursor -32602 errors.
Canvas compatibility is established through shared FlowDefinition round-trips;
no Canvas implementation or persistence changes were made.

A running Flow still depends on its originating process remaining alive.
Referenced-resource concurrent edits retain existing engine semantics.
See [Flow architecture](../architecture/flow-v1.md).

## Revision pinning and lightweight list follow-up

- Regression coverage rejects an old confirmation after a save and injects a
  deterministic save after MCP confirmation passes but before service execution.
  The latter returns FLOW_CONFIRMATION_STALE with a reconfirmation instruction,
  and neither path creates a Run.
- CommandBus/FlowService coverage proves stale revisions create no history and
  invoke no remote executor; a matching revision executes successfully.
- FlowSummary outputSchema permits exactly id, workspaceId, name, revision.
  Malformed typed steps/inputs remain listable through metadata projection while
  Desktop's unchanged definition decoder rejects them.
- No snapshot, Canvas, scheduler, or persistence schema changes.

- PASS: follow-up aggregate MCP / Flow Engine / CommandBus tests, including
  199 MCP library tests, four MCP binary tests, and two MCP integration tests.
- PASS: cargo check for MCP, Flow Engine, and CommandBus; cargo fmt --check
  for those crates plus unfour-core; git diff --check.
- PASS: large-file checker (zero blocking files).
- PASS: `cargo test -p unfour-mcp output_schema --quiet` (10 verifier tests); Flow-specific successes also validate against their outputSchema in the aggregate suite.

## 2026-09-28 audit and terminal-state follow-up

The current MCP registry already exposes `flow.list/get/save/run/cancel_run/list_runs/get_run`.
The run handler pins the confirmed definition revision, sets `initiator=mcp`,
and calls CommandBus; CommandBus delegates execution to the existing FlowService
and API/SSH/Database capability paths. No second runtime, persisted-schema
change, or Desktop Flow behavior change was needed.

Added adapter-level regressions that start Runs through MCP and read the shared
history until success, runtime failure, and step timeout. They validate each
terminal status, step status, stable error, finish timestamp, MCP initiator,
summary/detail agreement, and successful outputSchema. A separate persisted
fixture starts a long Run through MCP, closes stdio through the production EOF
path, reopens the same storage, and verifies that stale-lease recovery reports
`interrupted` without replaying the Run. The test ages the heartbeat explicitly
to avoid waiting 30 seconds.

- PASS: `cargo test -p unfour-mcp -p unfour-flow-engine` (219 MCP library tests,
  four MCP binary tests, two MCP registry integration tests, eight Flow Engine
  tests).
- PASS: `cargo test -p unfour-command-bus` after increasing the shared test
  poller's deadline to 30 seconds. The large-history Wait Until case allows a
  20-second step timeout, but its former test poller stopped after ten seconds;
  it passed in isolation and the full suite passed with the corrected poller.
- PASS: `cargo test -p unfour-command-bus --features ssh-native` (82 library
  tests, including the API/SSH/Database Flow execution fixture, plus integration
  tests).
- PASS: `cargo fmt -p unfour-mcp -p unfour-command-bus -p unfour-flow-engine --check`,
  `git diff --check`, and direct Node invocation of `check-large-files.mjs`
  (zero blocking files). The pnpm wrapper could not open its cache database in
  this sandbox; the direct script invocation passed.

## 2026-09-28 delete and history pagination

MCP now exposes eight Flow tools. `flow.delete` uses CommandBus and a
revision-checked delete; guarded workspaces require confirmation bound to that
revision. Existing run history remains readable after deleting the definition.
`flow.list_runs` accepts a page size up to 100 and a run-ID cursor, returns
`nextCursor`, and orders equal start times by ID. The query continues to read
only summary columns and scopes the cursor to the selected workspace and Flow.

- PASS: Flow MCP tests (12), including delete policy, stale confirmation,
  retained history, pagination, and schema contracts.
- PASS: CommandBus history tests (5), including more than 100 runs, an insert
  between page requests, equal timestamps, and cross-workspace cursor denial.
- PASS: full MCP suite (226 library, four binary, two integration tests),
  CommandBus suite (83 library plus integration tests), and Flow Engine (8).
- PASS: CommandBus Flow tests with `ssh-native` (47), Rust format check,
  `git diff --check`, and large-file check (zero blocking files).
- PASS: a freshly built MCP stdio binary in `ephemeral` mode: tools/list exposes
  eight Flow tools; two completed runs paginate without duplication; deleting
  the definition leaves their history readable.
- PASS: the Windows EOF recovery fixture explicitly closes its reopened SQLite
  pool before removing the temporary directory; the focused case passed three
  consecutive reruns after an intermittent file-lock cleanup failure.

The already-connected MCP process was not replaced during this test. The
new tool and response shape were verified through the freshly built isolated
binary, not through the user's persistent workspace.
