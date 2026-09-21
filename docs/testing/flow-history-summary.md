# Flow History summaries — 2026-09-21

History now reads only id, flow_id, status, started_at and finished_at, with the
existing workspace/Flow scope, descending time order and LIMIT 100. Full Run
snapshots are loaded by ID after selection. Definitions and history remain
independent; revision checks and recorded-node inspection still use full Runs.

The original schema has no finished_at column. updated_at is a heartbeat/write
time and cannot reproduce exact historical duration, particularly interrupted
Runs. A minimal additive migration backfills finished_at once from run_json;
it does not rewrite snapshots. Insert (including validation failure), progress,
completion and stale-run recovery maintain the column alongside the snapshot.
The ordinary summary SELECT never reads or parses run_json. Existing stale-run
recovery still updates abandoned running snapshots with json_set.

Contract: flow_runs_list/listFlowRuns now returns FlowRunSummary[] with only
id, flowId, status, startedAt and finishedAt. Command names/arguments and full
get/run/cancel response contracts are unchanged. No new dependencies.

Frontend coverage: opening History enables the summary query; closing disables
polling; opening again refreshes. Running summaries refresh while open without
fetching details. Selection waits for getFlowRun before projection. Completion
invalidates the summary query without fetching closed History. Tests also cover
status/time/duration, cancellation, revision mismatch and deleted recorded nodes.
Reference operands reject extra keys in both predicate JSON and ValueEditor.

Backend coverage: invalid snapshot JSON does not prevent listing; responses have
only five keys; workspace/Flow filtering and the 100-row cap remain; completion,
validation failure and interrupted recovery retain exact finish times; deleting
a definition retains history and detail access. The migration backfill preserves
snapshot bytes and null running finish times.

Verification:
- PASS: Flow frontend suite, 81 tests.
- PASS: desktop TypeScript check and production build (`pnpm run build`).
- PASS: scoped ESLint, zero errors; existing size/complexity and JsonField effect warnings.
- PASS: migration checker (29 migrations), Rust formatting, diff whitespace check.
- PASS: large-file checker, zero blocking files.
- Initial frontend test expected synchronous details from history; updated to await
  the intentional detail fetch. Initial migration check found CRLF; corrected to LF.
- Rust rerun during concurrent desktop compilation hit the existing 10-second
  large-history test timeout. PASS on the final serial rerun: 22 tests
  (`cargo test -p unfour-command-bus --lib flow_ -- --test-threads=1`).
- PASS: `cargo check -p unfour-app`, including the Tauri summary response contract.

NOT VERIFIED: packaged desktop interaction or a full Flow V1 release gate.
Existing release limitations remain documented in flow-v1.md; this scoped change
is not a certification of SSH remote process cancellation or production services.
