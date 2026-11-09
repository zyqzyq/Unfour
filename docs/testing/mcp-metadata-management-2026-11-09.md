# MCP metadata management verification — 2026-11-09

## Pre-commit review

Review found and fixed a mismatch between the new confirmation snapshot and the
existing recursive deletion semantics: request deletion traverses tombstoned
intermediate folders, while the initial snapshot only followed visible folders.
The snapshot now follows the same request reachability, and the visible-folder
revision set is scoped to the workspace rather than assuming a collection filter.

The added registry regression constructs a partially tombstoned hierarchy,
verifies the hidden live request is covered, changes its revision, and verifies
both the old confirmation and transaction-time snapshot are rejected. A fresh
confirmation deletes the request successfully. The existing adapter forwarding
was compared with HEAD and is unchanged after extraction.

Post-fix checks: PASS for all 9 metadata registry integration tests and all 8
API domain integration tests; formatting, diff whitespace and large-file checks
also passed. The earlier full MCP run below predates this final regression case.

## Scope

Adds 15 tools (72 → 87): workspace create/rename; API folder list/create/rename/
move/reorder/delete and request move/duplicate; Saved SQL list/get/save/delete;
database catalog discovery. No frontend or dependency changes.

Workspace creation keeps the active workspace unchanged and inherits the
authorizing active workspace's environment and MCP policy. It accepts no policy
override. The source selection/security snapshot is checked inside the business
transaction. Desktop creation still activates by default.

Recursive folder deletion uses a content-free revision snapshot built by the
HTTP domain service, with guarded confirmation and a transaction-time recheck.
Saved SQL update/delete uses an atomic revision predicate; metadata-only edits
retain original SQL, while reads use a masked diagnostic projection.

The MCP adapter was split into runtime lifecycle and typed command forwarding
to keep the enlarged implementation below the repository's blocking file limit.

## Results

All Rust test commands set `CARGO_INCREMENTAL=0`. Fixtures use ephemeral/local
test storage; no user workspace or external database was mutated.

| Check | Result |
| --- | --- |
| `cargo test -p unfour-mcp --lib --no-default-features --quiet` | PASS: 275 tests |
| `cargo test -p unfour-workspace-engine -p unfour-database-engine -p unfour-http-engine --no-default-features --quiet` | PASS: 27 workspace, 104 database, 115 HTTP tests; doc tests passed |
| `cargo test -p unfour-command-bus --no-default-features --test workspace_domain --test api_domain --quiet` | PASS: 24 workspace and 8 API integration tests |
| CommandBus library full run | 158 PASS, 1 FAIL: existing `flow_wait_until_final_output_respects_total_history_limit` hit FLOW_TIMEOUT under concurrent load |
| `cargo test -p unfour-command-bus --no-default-features flow_wait_until_final_output_respects_total_history_limit --quiet` | PASS in isolation (6.11 seconds); no test or timeout was weakened |
| `cargo check -p unfour-mcp --quiet` | PASS with default native SSH feature |
| `cargo fmt` check for MCP, command bus, workspace/database/HTTP engines and core | PASS |
| `node scripts/check-large-files.mjs` | PASS: no blocking files |
| `git diff --check` | PASS |

New registry-path integration coverage includes every added tool and its success
schema, inherited guarded policy, unchanged active selection, concurrent security
and selection changes, policy denials, invalid create overrides, cross-workspace
IDs, folder cycles/order, request copy preservation of body/auth/scripts, root
moves, recursive deletion, changed confirmation snapshots, transaction-time
deletion conflicts, SQL redaction and original-text preservation, stale SQL
revisions, and SQLite catalog discovery. Existing policy-classification, alias,
registry schema, mutation-hook and rollback tests also ran.

## Environment observations and limits

- Initial formatting/build failed when D: filled up. Removed only the verified
  repository `target/debug/incremental` cache. Restored the one new source file
  truncated by the failed formatter, then completed formatting and verification.
- Initial sandboxed MCP run: 272 passed, localhost HTTP cancellation failed with
  Windows socket permission error 10013. Full rerun with localhost permission
  passed all 275 tests, including the subsequently added concurrency cases.
- Initial sandboxed CommandBus suite also hit blocked localhost fixtures and
  hung waiting on an HTTP fixture. Stopped that verified test process after the
  permitted rerun completed; the table records the permitted run's results.
- One `pnpm run check:large-files` invocation reported a package-manager database
  error. The exact underlying Node check ran successfully after the adapter split.
- No live PostgreSQL/MySQL catalog or real SSH server verification was performed.
  Native SSH compilation passed; this change adds no SSH transport behavior.
- No running installed MCP server was replaced or restarted. Clients see the
  new registry after building and launching the updated binary.
