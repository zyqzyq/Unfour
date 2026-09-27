# Database Engine

`DatabaseService` in `src/database.rs` remains the public facade. Its private
modules implement existing domains through inherent methods; adapters continue
to call the same service through CommandBus.

## Connection ownership

- `connections.rs`: normal CRUD, reads/conversion of stored connection records,
  local change detection, credential reference validation, and local mutations.
- `connection_domain.rs`: snapshots, external apply, workspace deletion, and
  post-commit credential cleanup. External validation stays separate from normal
  input validation (in particular, remote SQLite may have no local file path).
- `connection_config.rs`: connection input normalization, config serialization,
  port decoding, and shared connection validation. Empty local config has one
  constructor; driver acceptance and existing error messages are unchanged.
- `connection_storage.rs`: private subtype insert/update primitives shared by
  normal save and external apply. They use the caller's SQLite connection and
  return the affected-row result; they do not open/commit transactions, validate
  ownership, choose credentials, update the parent, or emit mutations. Insert
  and update remain distinct; no upsert repairs a missing subtype row.

Parent `connections` writes deliberately stay in their respective workflows:

| Policy | Normal save | External apply |
| --- | --- | --- |
| Existing shared fields change | Increment revision, pending sync | Increment revision, local sync |
| Only local fields change | Keep revision/timestamps/sync; no mutation | Preserve compatible local config/credential; incompatible driver clears them |
| Timestamps | Local clock | Remote timestamps |
| Repeated identical metadata | Local fields may still be written | No-op after canonical persisted state matches |
| Deleted row | Not found | May restore through upsert |
| Mutation origin | Supplied context | Requires External context |

These differences are business policy, not duplicate persistence to merge.
CommandBus retains transaction/hook orchestration; cleanup runs after commit.

## Query and record ownership

`queries.rs` owns single-statement validation/execution; `scripts.rs` owns script
execution. `query_history.rs` owns local query-history records. `saved_sql.rs`
owns saved SQL CRUD and its connection-detachment primitive, called by normal,
external, and workspace connection deletion inside the existing transaction.

Runtime pools/profiles, schema/table inspection, row mutations, export, and
PostgreSQL/MySQL/SQLite drivers retain their existing modules. openGauss remains
a PostgreSQL-compatible runtime profile, with no new driver or persisted fields.

## Verification

Run `cargo test -p unfour-database-engine -p unfour-command-bus --no-fail-fast`.
Connection persistence regressions use disposable in-memory SQLite databases;
CommandBus tests cover hook rollback, snapshots, external replay, credential
compatibility, deletion, and Flow integration. Real-server tests require their
existing explicit test environment; protocol fixtures do not establish live
server compatibility.
