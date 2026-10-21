# Flow Definition Cloud Sync client verification

Date: 2026-10-21. Baseline: latest main `d5cb4c00` (origin/main fetched and
confirmed equal before editing). Scope: Unfour client production activation.
No Unfour-api inspection/changes, GCP verification, actual two-device test,
native packaging, publication, or push.

## Implementation and coverage

Flow is registered in the shipping Protocol 5 Registry, wire enum, canonical
mapping, snapshot adapters, external apply and mutation conversion. There is no
new feature flag, schema version, dependency, or synchronization engine.

Lifecycle tests moved from unit-only wiring into
`crates/unfour-cloud-sync/tests/flow.rs` and `tests/flow/`; they compile the
production library. Adapter schema/safety tests remain in
`src/flow_definition/tests/`. Production-exclusion tests were replaced with
registration, upload and materialization coverage.

| Area | Evidence |
| --- | --- |
| Registry | `flow`, Workspace parent, initial upload, tombstones, schema/reader 1 and unchanged Protocol 5; complete initial-upload entity set includes Flow |
| Initial upload | Whole canonical document, existing UUID, no revision/timestamps/runtime fields; 64 KiB rejection rolls back binding and Outbox |
| Existing bindings | Generation-fenced, owner-scoped missing-state capture runs for Flow before Pull/replay; protects legacy documents and tombstones against remote replacement/resurrection and Workspace deletion |
| Incremental/offline | Bound mutations capture canonical durable intent while paused/signed out; resume sends updates and deletion tombstones through the existing Worker |
| Restart/idempotency | Uncertain delivery retries the identical operation and payload after restart; repeated sync emits no new upload |
| Remote apply | Whole-document replacement; local revision independent of serverVersion; equivalent newer content and duplicate tombstones are no-ops; stale replay cannot resurrect a newer tombstone |
| Page ordering | One Changes page with update/delete/restore materializes the last Flow document only |
| Conflicts | Keep-local/use-remote for edits and local tombstones against remote update/delete; CAS uses remote serverVersion; whole-document UI identifies Flow |
| Workspace | Offline local delete captures Flow tombstones; remote deletion conflicts with pending/untracked Flow and atomically purges local run history after resolution |
| Core adapter | Generic external page supports Flow, reports equivalent materialization, emits External-origin hooks, and rolls the page back on hook failure; SyncOutboxHook creates no echo |
| History/credentials | Canonical payload is exactly name/inputs/steps; run/history sentinel stays local through Flow update/delete, never enters push data |
| Resource references | API/SSH/Database UUIDs remain unchanged; remote apply synthesizes no resources; missing-resource execution remains blocked |
| Compatibility | Old-reader retained Flow envelopes replay on upgrade without echo; future schema/critical subtype waits with retained envelope/cursor; future-schema tombstones still apply |
| Payload safety | Existing schema 1 tests preserve SQL, SSH/API arguments, ordinary defaults and templates; strict fields, runtime-reference rules, sensitive content rejection and compact UTF-8 64 KiB boundary remain unchanged |
| Local behavior | Community/unbound local limit remains 1 MiB; a definition above 64 KiB with a literal default remains locally valid; only binding/upload applies the cloud limit |
| UI | Stable Flow size/safety codes survive CommandBus errors; English/Chinese editor feedback retains draft edits and hides raw diagnostics; Flow conflict actions use entity type flow |
| Other modules | Full workspace and frontend suites cover existing sync entities, MCP, local Flow execution and Workspace bundle import/export |

## Commands and results

| Check | Result |
| --- | --- |
| `cargo test --workspace --no-fail-fast` | FAIL: 1,242 passed, 1 existing history-capacity test timed out, 1 OS Keychain smoke ignored |
| `cargo test -p unfour-command-bus --features ssh-native --lib flow_wait_until_final_output_respects_total_history_limit -- --test-threads=1` | PASS: unchanged test passes in 5.52 seconds in isolation |
| `cargo test --workspace --no-fail-fast -- --test-threads=1` | PASS: 1,243 tests, zero failures; 1 existing OS Keychain smoke ignored (platform credential store required) |
| `cargo test -p unfour-cloud-sync --test flow` | PASS; shipping-library lifecycle cases, subsequently expanded to 21 passing cases in workspace run |
| `cargo test -p unfour-cloud-sync --test worker initial_upload_entity_types_match_registry_exactly` | PASS; updated Registry fixture includes Flow |
| `cargo fmt --all -- --check` | PASS |
| `pnpm run test` | PASS: 186 files, 1,250 tests |
| `pnpm exec vitest run apps/desktop/src/features/cloud-sync` | PASS: 4 files, 51 tests after final scope-copy correction |
| `pnpm run build` | PASS; existing bundle-size warning |
| `pnpm run lint` | PASS: zero errors; 34 existing warnings |
| `pnpm run check:migrations` | PASS: 35 migration files |
| `pnpm run check:secrets` | PASS; no audit-rule changes |
| `pnpm run check:large-files` | PASS: zero blocking files; advisory findings remain |
| `git diff --check` | PASS |

The new upgrade-preservation regression first failed: Pull/replay overwrote an
old client's local-only Flow before the generic missing-state repair. Flow now
uses the same repair transaction before Pull for previously untracked documents
and tombstones; existing entity readers retain replay-before-repair behavior.
The regression passes for both a remote Flow replacement and Workspace deletion.

Intermediate verification also caught and fixed the old Registry-completeness
fixture, which did not seed the newly registered Flow. The first Rust unit
compile after moving lifecycle tests needed the remaining safety-test support
module restored; that was corrected. Sandboxed HTTP contract tests could not
reach their loopback fixtures, and the sandboxed pnpm launcher could not open its
local database. Relevant commands were rerun outside the sandbox against local,
disposable fixtures. No deployed service or real user workspace was used.

The complete parallel workspace run reached every suite, including 202 Cloud
Sync tests and all 260 MCP tests, which passed. One unchanged CommandBus test,
`flow_wait_until_final_output_respects_total_history_limit`, hit its existing
20-second step timeout under concurrent workload and returned `FLOW_TIMEOUT`
instead of the expected `FLOW_HISTORY_LIMIT`. Its isolated rerun passed in 5.52
seconds. No execution code, timeout or assertion was changed to suppress it;
the serial full-workspace result is reported separately above.

## Limits and manual packaging checks

- NOT VERIFIED: deployed server acceptance, native UI, actual cross-device
  convergence and real API/SSH/database resource execution. These are outside
  this client's local automated evidence; packaging/release is user-managed.
- Verify first upload and upgrade of an already-bound workspace, including a
  local-only edit/deletion concurrent with a retained remote update/deletion.
- Verify offline edits/deletions across pause, sign-out and process restart,
  then resume without duplicate operations or workspace-owner fan-out.
- Verify both conflict choices, stale open editors, remote resurrection and
  Workspace deletion; individual Flow deletion retains local runs, whereas
  Workspace deletion purges them.
- Verify unchanged resource UUIDs, absent resources, and resources arriving
  later. Existing run preflight must still block missing resources.
- Verify English/Chinese safety and size feedback retains drafts, normal SQL,
  SSH/API parameters and templates remain unchanged, and runs/runtime values
  and credentials never appear in cloud payloads.
- Old clients remain unable to materialize Flow. They must retain its envelopes
  and enter compatibility waiting; upgrading replays them. No old-client binary
  or actual two-device exercise was performed here.
- Schema 1 is structural protection, not general DLP. Arbitrary SQL/shell/text,
  innocuously named values, encoded blobs and metadata may hide secrets that
  cannot be identified reliably. Explicit sensitive/device-local material is
  rejected; values are never silently redacted or truncated.

See [the client/server contract and manual checklist](../architecture/flow-definition-cloud-sync.md).
