# Flow Definition local sync foundation — 2026-10-20

This phase prepares local deletion and concurrency contracts only. Protocol 5,
the Sync Entity Registry and all upload payloads remain unchanged. Flow runs,
runtime inputs, outputs, status and secret material are never sync entities.

## Storage and compatibility

Migration `20261020000000_core_flow_definition_tombstones.sql` adds nullable
`flow_definitions.deleted_at` and an index for live workspace definitions.
Existing live JSON, IDs, revisions, timestamps and run history are preserved.
Legacy deleted workspaces receive definition tombstones and their local Flow
history is purged. The upgrade keeps SQL and JSON revisions equal and can be
reopened through the normal migration runner without repeating the backfill.
Previously physically deleted definitions cannot be reconstructed.

Single-definition deletion retains local history, increments revision and
updates both timestamps. Lists, direct reads, saves, runs and exports exclude
tombstones. Every public Flow storage operation requires a live workspace.
Workspace local and external soft-deletion share one transaction and timestamp
for new definition tombstones. Existing tombstones remain unchanged. Workspace
deletion physically removes its `flow_runs`, including definition/resource
snapshots, runtime inputs and outputs, for privacy; history has no cloud
tombstone. Background runners lose their lease and cannot recreate history.
Effects already sent to remote systems still follow existing cancellation limits.

Tauri, command-client, CommandBus and MCP deletion require `expectedRevision`.
MCP callers must obtain this positive integer from list/get; older callers
omitting it receive invalid arguments. A stale revision returns
`FLOW_REVISION_CONFLICT`, including in full-access workspaces. Guarded MCP
confirmation is bound to that revision, which SQL checks again at mutation time.
The desktop retains the draft and offers its existing conflict recovery.

Workspace bundle versions 1 and 2 remain supported. Only live definitions are
exported; no tombstones or history enter the bundle. Imports remap IDs, reset
revisions to 1 and emit local definition mutations in the enclosing transaction.
The FlowDefinition JSON contract, including legacy input decoding, is unchanged.

## Transactions and ownership

Flow persistence stays in flow-engine. CommandBus save/delete use the existing
business/activity/transaction-hook path. Workspace-engine now depends on
flow-engine to invoke its owned cascade from both local and external workspace
lifecycle paths; this internal dependency avoids duplicated Flow persistence SQL
and adds no third-party package.

`DomainEntityType::FlowDefinition` describes local mutations. Conversion into
`SyncEntityType` is now fallible: FlowDefinition is explicitly unsupported. The
outbox boundary excludes unregistered types before owner resolution or worker
triggers, including in bound workspaces. Existing registered entities retain
their durable, owner-scoped outbox behavior. No Flow snapshot adapter, canonical
payload, wire type, registry descriptor or initial-upload entry is added.

## Automated coverage

- Revision conflicts on save/delete, wrong workspace, repeated delete, monotonic
  SQL/JSON revision, immutable tombstone, hidden reads/lists/exports/runs, and
  retained single-Flow history.
- Local and external workspace cascade, existing tombstone preservation,
  privacy purge, blocked reads/edits/execution, and deletion during async run
  preparation without history recreation or executor invocation.
- Hook rejection rolls back creates, updates, deletes, workspace tombstones,
  local history purge, activity and hook SQL together; existing bundle rollback
  tests cover imported definitions as well.
- Full historical database upgrade, preserved live legacy JSON/history,
  deleted-workspace repair and repeat migration.
- V1/V2 bundle round trips, new IDs/revision 1, and tombstone/history omission.
- Desktop revision forwarding/conflict retention; MCP schema, malformed/missing
  revision, stale editor, stale confirmation and retained history.
- Bound Flow mutations produce no outbox/trigger; initial upload excludes both
  live definitions and tombstones; Protocol 5 rejects Flow wire types.

## Before Registry onboarding

1. Agree a definition-only canonical payload, schema/reader revisions and remote
   compatibility behavior; allowlist portable authoring fields and credential
   references. Keep every runtime/history/output/status/secret field excluded.
2. Define resource-reference topology, workspace ownership validation and behavior
   for missing or concurrently deleted API/SSH/database resources across devices.
3. Implement snapshot/materialization adapters, conflict handling and deletion
   restoration rules that preserve revision checks and workspace tombstones.
4. Register only definitions once both ends support them; verify initial upload,
   repair, tombstone delivery and durable owner-scoped capture while signed out,
   paused, offline or lacking entitlement. Never route to historical bindings.
5. Verify pull/no-echo, restart/retry, mixed-version readers, keep-local/use-remote,
   secret redaction and rollback end to end before enabling any upload.

## Verification results

- PASS: final `cargo test -p unfour-command-bus -p unfour-mcp -p
  unfour-cloud-sync --lib --no-fail-fast`: 156 + 253 + 19 tests. This rerun
  includes the final run-start fence and corrected fixtures.
- PASS: other targets from the five-crate run (CommandBus, MCP, local-storage,
  workspace-engine, cloud-sync): 287 tests, including storage (49), workspace
  engine (21), sync contracts (10), outbox (17) and worker (122).
- PASS: final `cargo test -p unfour-command-bus --test workspace_domain`:
  20 tests, including both definition/workspace deletion during preparation.
  The total distinct Rust coverage across the five crates is 715 tests;
  focused reruns are not counted again.
- PASS: frontend FlowPage, FlowEnvironment and command-client tests (88 distinct
  tests); desktop TypeScript `tsc --noEmit`; affected ESLint checks (zero errors,
  two pre-existing useFlowEditor warnings).
- PASS: `cargo check -p unfour-app` and CommandBus/MCP/cloud-sync compile checks;
  formatting for all eight touched Rust crates; migration validation (34 files),
  large-file gate (zero blocking files) and `git diff --check`.
- FAIL, resolved: sandbox execution blocked pnpm/cache writes, Vitest temporary
  renames and loopback HTTP/SSH connections. Verification was rerun outside the
  sandbox against disposable test databases and local test services.
- FAIL, resolved: the first full normal-environment run found three fixture
  assertions needing the new contract: nonexistent workspace history now returns
  NotFound, MCP delete must use revision 2 after an edit, and the new bundle test
  must use an executable authoring fixture rather than an SSH placeholder input.
  Fixtures were corrected; all affected library targets passed the final rerun.
- NOT RUN: packaged desktop/manual E2E. No live cloud service or upload was used.
