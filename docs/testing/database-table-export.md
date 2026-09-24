# Single-table export verification

The `database-engine` export path reads a table through a SQLx row stream and
writes directly to a buffered staging file beside the chosen destination,
then replaces the destination only after a successful export. SQL inserts are
capped at 100 rows per statement. Desktop and MCP call the same command-bus method. MCP creates files
under the active Unfour product data directory's `exports` subdirectory and
does not accept a destination path.

## Automated coverage

- `cargo test -p unfour-database-engine --offline --lib`: PostgreSQL column DDL
  formatting; CSV quoting; JSON empty output; SQL values (NULL, boolean,
  numeric, JSON, binary, quotes, backslashes); bounded INSERT batches; SQLite
  full-table export across 252 rows and a separate 20,002-row CSV export;
  empty CSV; JSON row count; SQLite SQL restore round trip.
- `cargo test -p unfour-mcp --offline --lib tools::database`: complete describe
  table output and output schema validation; export schema, managed path, and
  rejection of caller-supplied destination paths. Column selection, `eq`/`in`
  filters, and `limit` are optional and omitted calls still export the whole table.
  Filter values are strings, numbers, or null. `eq` null stays `IS NULL`; `in`
  rejects null; boolean values are rejected.
- Vitest for `TableExportDialog`, `TableDataTab`, and `DatabaseConnectionTree`:
  whole-table request parameters and existing table UI behavior.

## Live-database follow-up

The test environment has no PostgreSQL or MySQL server. Catalog queries for
PostgreSQL DDL and live data decoding for these two drivers were compiled but
not integration-tested against a server. PostgreSQL DDL is intended for common
development tables and views; specialized objects such as foreign tables and
partitioning clauses are outside this single-table export scope. PostgreSQL
array and custom types may return an unsupported-type error during data export.
CSV and JSON encode binary values as lowercase hexadecimal strings; CSV has no
distinct representation for `NULL` versus an empty string. The MCP adapter's
existing 120-second execution deadline also applies to exports; a timed-out
export removes its staging file.
