# Database SQL execution context audit

Date: 2026-09-28

## Findings and changes

- Query tabs, single-query inputs and script inputs already carried catalog and
  schema. Both execution paths use `script_connection` and `effective_connection`.
  Saved SQL and query history persisted only the connection, losing context on
  reopen. Both now persist/restore optional catalog and schema.
- New Query inherits the current query/table context for the selected connection,
  or uses its explicitly configured database. Database-node New Query passes the
  node catalog; table-generated SQL passes table catalog/schema. Connection
  changes reset the old namespace. An explicit default is pinned in the tab
  independently of schema discovery.
- Tree discovery previously selected the first available schema. It no longer
  rewrites execution context. Unknown explicit catalog/schema values remain
  visible in selectors, and a null schema displays server default search path.
  The editor connection summary uses the tab catalog, not the connection default.
- Saved SQL saves the editor context. Server Saved Queries groups are children
  of catalog nodes. SQLite's file connection is already its database node.
  Unresolved, detached or inaccessible legacy snippets remain available in the
  existing Saved SQL dialog; no server connection-level Saved Queries group is
  retained.
- History captures the execution snapshot, including when the user switches tabs
  while a run is pending. Script results additionally carry each statement's
  default catalog. Successful MySQL USE updates subsequent statement contexts;
  qualified cross-database references do not change the default. No extra SQL
  probes are inserted between statements.

## Compatibility and boundaries

`20260927000000_core_sql_execution_context.sql` adds nullable columns. It backfills
only nonblank database names on the original connection in the same workspace,
for persisted postgres/mysql drivers. openGauss uses the postgres driver/runtime
profile. SQLite paths, unknown defaults and detached records are not guessed.
Existing SQL, history, IDs and timestamps are retained. Older input JSON remains
accepted; the service also resolves configured defaults for older callers.

Catalog is a default execution database, not an SQL access restriction.
PostgreSQL/openGauss connect to that database; MySQL can still reference other
databases in qualified SQL. Optional schema preserves the tab's initial search
path override; history is not a complete replay of arbitrary session settings or
transactions. The Tab context remains the starting context for each fresh run.

No new dependencies, command names or MCP/Flow input semantics were introduced.
The sole execution-result addition is the optional script-statement catalog,
needed to reopen MySQL history after USE correctly. Existing confirmation identity
continues to include workspace, connection, SQL, catalog and schema.

## Verification

- PASS: `pnpm exec vitest run packages/database/src packages/command-client/src/tauri`
  — 35 files, 216 tests (the prefix includes `tauri.test.ts`).
- PASS: `cargo test -p unfour-database-engine --lib` — 80 tests, including saved/history
  round trips, backward-compatible inputs, default inference, quoted USE parsing,
  and MySQL protocol-fixture statement catalog propagation.
- PASS: `cargo test -p unfour-local-storage --lib` — 48 tests, including a populated
  pre-migration database with known/blank/SQLite/unbound defaults and retained SQL.
- PASS: `cargo test -p unfour-mcp database --lib` — 60 database adapter/schema/policy
  tests, rerun after the additive script-result field.
- PASS: `cargo test -p unfour-command-bus --test database_script` — 1 test.
- PASS: focused `sql_context` Rust suite — 5 tests, rerun after normalizing
  blank legacy connection references.
- PASS: desktop `tsc --noEmit`.
- PASS: ESLint for Database and changed command-client files; 15 non-blocking
  file-length/complexity warnings remain.
- PASS: affected Rust `cargo fmt --check`, migration checks and `git diff --check`.
- PASS: large-file check; no blocking findings.

Initial frontend runs failed on outdated schema-autofill expectations and test
fixture setup; these were corrected and the final suites above passed. The
initial migration fixture attempted a forbidden cross-workspace saved-SQL
reference; it was corrected to valid legacy data without weakening constraints.
The migration LF line-ending check was corrected before final verification.

NOT VERIFIED: native desktop visual interaction and live PostgreSQL/MySQL/openGauss
servers. Tests use React DOM, disposable SQLite storage and loopback protocol
fixtures; protocol fixtures do not establish real-server SQL compatibility.
