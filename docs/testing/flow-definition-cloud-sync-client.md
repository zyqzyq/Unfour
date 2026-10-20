# Flow Definition Cloud Sync client verification

Date: 2026-10-20. Safety/CI follow-up baseline: main `f089a90`.
Scope: Unfour only. No Unfour-api changes, production registration or real
cross-device synchronization.

## Coverage

The unit build rehearses Flow registration with the existing Protocol 5
Registry/Outbox/Worker. It uses disposable SQLite databases and scripted
transport. The production-library integration build excludes Flow.

| Area | Evidence |
| --- | --- |
| Definition contract | All five node kinds roundtrip; strict top-level/nested fields; graph and resource UUID preservation |
| Literal values | SQL, SSH Task inputs and remote directories, API URLs/JSON/form/plain bodies, patch rows, mixed templates, string predicates and ordinary defaults preserve exact values |
| Inputs and secrets | Secret metadata only; every secret/sensitive-named default rejected, including null/references; nested sensitive defaults cannot use reference-shaped literal data |
| Structured safety | Sensitive object fields, API key/value rows (including disabled/dynamic keys), JSON text, URL passwords/query and form pairs require actual runtime values; encoded lookalikes, local handles and private keys rejected without redaction |
| Reference syntax | Invalid roots/escapes, malformed singleton objects/templates, missing/self step outputs and out-of-scope probe references rejected; existing graph validation retained |
| Size | 64 KiB UTF-8 payload boundary; larger definitions rejected, no truncation; failed initial upload/edits roll back atomically |
| Initial upload | Canonical Flow snapshot with Workspace parent, no local revision, history or runtime state |
| Old bound workspace | Generic reconciliation captures previously local-only Flow |
| Offline mutation | Signed-out/paused owner retains canonical intent; resume publishes through existing Worker |
| External apply | Whole-document update, local revision increment, duplicate content no-op, stale version fence, tombstone and resurrection |
| Conflict | Keep-local/use-remote for local edits and local tombstones against remote updates/deletes; conflict baseVersion retained |
| Workspace | Local deletion captures Flow tombstones offline; remote Workspace deletion conflicts with pending Flow and cascades history cleanup |
| References | API/SSH/Database UUIDs accepted before resources exist; no resource rows synthesized; execution still fails preflight |
| History | Sentinel local run payload remains unchanged by Flow update/delete and never enters serialized definition payloads |
| Transaction rollback | Normal string default enters the durable Outbox unchanged; rejected local-handle edit preserves both prior definition revision and canonical intent |
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
| `cargo check --workspace` | PASS, including desktop `sync.rs` exhaustive error mapping |
| `cargo check -p unfour --features ssh-native` | PASS |
| `cargo test --workspace --exclude unfour-mcp` | PASS: 978 tests; existing OS Keychain smoke ignored (requires platform credential store) |
| `cargo test -p unfour-mcp` | PASS: 260 tests, including binary contracts |
| `cargo fmt -p unfour-cloud-sync -p unfour --check` | PASS |
| `pnpm run check:secrets` | PASS; no audit-rule/exemption changes |
| `pnpm run lint` | PASS: zero errors; 34 existing warnings |
| `pnpm run build` | PASS; existing bundle-size warning |
| `pnpm run test` | PASS: 186 files, 1,244 tests |
| `pnpm run test:release-env` | PASS: 81 tests |
| `pnpm run check:migrations` | PASS: 35 migration files |
| `pnpm run check:large-files` | PASS: zero blocking files; existing advisory findings remain |
| `git diff --check` | PASS |

The first targeted run passed 23/24 cases: the old rollback fixture needed to
reach Sync validation after ordinary literals became valid. A nested sensitive
default is rejected earlier by the owning Flow engine. The corrected fixture
uses a locally accepted Secret Store handle; all 24 Flow cases passed on rerun.
The final suite additionally verifies an ordinary default's canonical intent
survives rejection unchanged.

The sandboxed pnpm launcher initially reported `unable to open database file`.
The required pnpm commands were rerun successfully outside that sandbox. Rust
loopback HTTP tests also run outside the network sandbox against disposable
fixtures; no deployed sync service or real user workspace is used. Initial-phase
verification history remains available in Git.

## Limits and remaining release work

- NOT VERIFIED: deployed server acceptance, real two-device synchronization,
  native UI and real API/SSH/database resource execution. Tests establish client
  adaptation and local safety, not service deployment.
- CI-equivalent commands run locally on Windows. No push or GitHub Actions run
  is part of this commit-only task; hosted Linux CI status is not asserted.
- Production entry points deliberately remain disabled. The test-only
  registration is not a product toggle.
- Arbitrary SQL, shell/text, innocuously named fields, custom encodings/handles
  and metadata can hide secrets that structural checks cannot detect. These
  are user-authored definition data, not certified secret-free data. Malformed
  JSON-looking text and ambiguous sensitive templates fail explicitly; authors
  can supply a whole runtime value. No automatic redaction/rewrite occurs.
- Unfour-api must implement the final schema 1 acceptance rules before release
  activation. Shared i18n error presentation is still a future activation task.
- Resources such as saved SQL must exist under the referenced UUID before a run
  can succeed; Flow synchronization does not itself sync or synthesize them.
- No dependency was added in this follow-up. Cloud Sync already uses the owning
  `unfour-flow-engine` snapshot/external-apply adapter.

See [the final server contract and activation checklist](../architecture/flow-definition-cloud-sync.md).
