# Flow Definition Cloud Sync client verification

Date: 2026-10-20. Baseline: latest fetched `origin/main`, `bbfc8d1f`.
Scope: Unfour only. No Unfour-api changes, production registration or real
cross-device synchronization.

## Coverage

The unit build rehearses Flow registration with the existing Protocol 5
Registry/Outbox/Worker. It uses disposable SQLite databases and scripted
transport. The production-library integration build excludes Flow.

| Area | Evidence |
| --- | --- |
| Definition contract | All five node kinds roundtrip; strict top-level/nested fields; graph and resource UUID preservation |
| Inputs and secrets | Secret metadata only; secret defaults, opaque executable strings, local handles/paths and sensitive fields rejected without redaction |
| Size | 64 KiB UTF-8 payload boundary; larger definitions rejected, no truncation; failed initial upload/edits roll back atomically |
| Initial upload | Canonical Flow snapshot with Workspace parent, no local revision, history or runtime state |
| Old bound workspace | Generic reconciliation captures previously local-only Flow |
| Offline mutation | Signed-out/paused owner retains canonical intent; resume publishes through existing Worker |
| External apply | Whole-document update, local revision increment, duplicate content no-op, stale version fence, tombstone and resurrection |
| Conflict | Keep-local/use-remote for local edits and local tombstones against remote updates/deletes; conflict baseVersion retained |
| Workspace | Local deletion captures Flow tombstones offline; remote Workspace deletion conflicts with pending Flow and cascades history cleanup |
| References | API/SSH/Database UUIDs accepted before resources exist; no resource rows synthesized; execution still fails preflight |
| History | Sentinel local run payload remains unchanged by Flow update/delete and never enters serialized definition payloads |
| Compatibility | Legacy local inputs normalized; unknown local step fields rejected; future schema/subtypes retained rather than partially applied |
| Production exclusion | Wire enum/Registry reject Flow; initial upload and mutations never capture it; Changes/Snapshot retain it without business apply |
| Local behavior | Bound production workspace still accepts 70 KiB Flow with literal default; local limit and definition remain unchanged |
| Storage upgrade | Existing canonical intents, uncertain leases/retries and conflict/reader fences preserved; foreign-key validation and CHECK constraints retained |

Tests live under `crates/unfour-cloud-sync/src/flow_definition/tests/`,
`crates/unfour-cloud-sync/tests/flow_disabled.rs`, the previous phase's
`tests/outbox/flow_definitions.rs`, and
`crates/unfour-cloud-sync-storage/src/tests/flow.rs`.

## Commands and results

| Check | Result |
| --- | --- |
| `cargo test -p unfour-cloud-sync --lib` | PASS: 37 tests, including 18 Flow adapter/lifecycle cases |
| Cloud Sync `--test contracts` | PASS: 10 |
| Cloud Sync `--test flow_disabled` | PASS: 4 production-library gate cases |
| Cloud Sync `--test outbox` | PASS: 17 |
| Cloud Sync `--test ownership` | PASS: 2 |
| Cloud Sync `--test worker` | PASS: 122 existing worker regressions |
| `cargo test -p unfour-cloud-sync-storage -p unfour-flow-engine` | PASS: 15 storage + 8 Flow engine tests |
| `cargo test -p unfour-command-bus --lib flow` | PASS: 60 |
| `cargo test -p unfour-command-bus --lib workspace_bundle` | PASS: 36 |
| `cargo test -p unfour-command-bus --test workspace_domain` | PASS: 20 |
| `cargo test -p unfour-core` | PASS: 41 |
| `cargo fmt ... --check` for all five changed Rust crates | PASS |
| `node scripts/check-migrations.mjs` | PASS: 35 migration files |
| `node scripts/check-large-files.mjs` | PASS: zero blocking files; existing advisory findings remain |
| `git diff --check` | PASS |

The full three-crate test command initially caught an index-name regression in
the new storage migration (`idx_pro_sync_outbox_due`). The migration now retains
both historical index names. All 15 storage tests passed on rerun, including
the historical-state preservation test. The adapter and production
gate/Outbox suites were rerun against that corrected migration.

The first sandboxed full Cloud Sync run failed five existing loopback HTTP
contract tests with `connection_failed`/timeout. Those same ten protocol
contract tests passed outside the network sandbox. No server endpoint was used.
An initial migration-format check found CRLF in the new SQL file; this was
corrected to LF and the 35-file migration check passed.

`pnpm run check:migrations` could not start because the pnpm launcher reported
`unable to open database file`; its exact underlying Node script was run
directly instead.

## Limits and remaining release work

- NOT VERIFIED: deployed server acceptance, real two-device synchronization,
  native UI and real API/SSH/database resource execution. Tests establish client
  adaptation and local safety, not service deployment.
- Production entry points deliberately remain disabled. The test-only
  registration is not a product toggle.
- The conservative v1 safety contract rejects harmless opaque string literals
  as well as unsafe ones. Release activation requires accepting that limitation
  or reviewing a broader safety contract, plus shared i18n error presentation.
- Resources such as saved SQL must exist under the referenced UUID before a run
  can succeed; Flow synchronization does not itself sync or synthesize them.
- New dependency: the existing workspace crate `unfour-flow-engine` is used
  directly by Cloud Sync for its owning snapshot/external-apply adapter. No
  third-party dependency was introduced.

See [the proposed server contract and activation checklist](../architecture/flow-definition-cloud-sync.md).
