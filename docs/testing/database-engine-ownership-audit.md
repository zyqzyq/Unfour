# Database Engine ownership audit ¡ª 2026-09-27

## Scope and findings

Architecture review used package boundaries, data storage, Cloud Sync invariants,
and the execution protocol. The working tree was clean before editing.

`database.rs` was already a facade/module assembly point, so no artificial facade
layer or public API replacement was needed. Actual overlap was subtype persistence
in normal save and external apply, config helpers attached to CRUD, saved SQL
unlinking attached to external-apply support and duplicated in local deletion,
and local record persistence mixed into query execution.

The resulting ownership is documented in
[Database Engine](../../crates/database-engine/README.md). Parent-row SQL and
normal/external validation remain separate intentionally: revision, sync status,
timestamps, resurrection, credential compatibility, and mutation semantics differ.
No migrations, dependencies, CommandBus/MCP/Flow contracts, transport drivers,
or PostgreSQL/MySQL/openGauss compatibility code changed.

## Regression coverage added

Three in-memory SQLite tests in `database_tests/connection_persistence.rs` cover:

- Credential-only normal save preserves revision/timestamps/sync and emits no
  mutation; shared edits increment revision and mark pending. External updates
  retain credentials and complete compatible local config, use remote timestamps,
  mark local, and emit External mutations; identical replay is a no-op.
- A failed subtype insert rolls back its parent; normal update does not recreate
  a missing subtype or change the parent record.
- Saved SQL detachment respects workspace scope and live rows, retains deleted
  records, and rolls back with connection deletion.

Existing CommandBus coverage additionally exercises external insert/replay,
credential incompatibility, pathless SQLite, resurrection, hook rejection,
snapshots, workspace deletion, and Flow SQL validation/execution.

## Verification

All commands ran from the repository root on Windows.

| Check | Result |
| --- | --- |
| `cargo test -p unfour-database-engine -p unfour-command-bus --no-fail-fast` | PASS: Database Engine 75, CommandBus 135; 210 passed, 0 failed, 0 ignored; doc tests also passed |
| `cargo fmt -p unfour-database-engine --check` | PASS |
| `git diff --check` | PASS |
| `pnpm run check:large-files` | PASS: 0 blocking; existing size advisories remain |
| Real PostgreSQL/MySQL/openGauss service integration | NOT RUN: no dedicated live-service environment used; protocol fixtures are not live compatibility evidence |

The test build reports an existing unused `reset` method in SSH command-line
tracking; it is outside this change. No release-readiness claim is made.
