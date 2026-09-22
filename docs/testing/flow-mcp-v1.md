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
