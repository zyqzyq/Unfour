# openGauss runtime support verification

## 2026-09-28 closeout for openGauss 6.0.3

The earlier 2026-09-25 section below records the initial implementation and
its evidence at that time. This closeout enables openGauss indexes and DDL;
foreign-key metadata remains disabled. No runtime architecture or persisted
connection type changed.

- `describe_table` reads openGauss index flags and ordered index expressions
  from `pg_index` plus the documented three-argument `pg_get_indexdef`.
  Table DDL comes from the server's `pg_get_tabledef(oid)`, which reconstructs
  table structure, indexes and comments. Structure-only and structure-and-data
  exports use the same table-structure path as `describe_table`.
- MCP `list_tables` accepts an optional schema. PostgreSQL/openGauss run one
  count query and one bounded listing query. The latter applies schema and
  limit before an aggregate column count, avoiding per-table column queries.
  The default order places `current_schema()` then `public` first. Unscoped
  openGauss listings exclude `dbe_perf`, `snapshot` and other documented
  internal schemas; an explicit schema can still request them. PostgreSQL
  retains its ordinary schema set.
- Internal PostgreSQL `"char"` values decode at the shared row boundary for
  query and export results. The connection-test success message uses the
  runtime detected server name while protocol remains PostgreSQL.
- MCP database adapter errors expose a sanitized `details.reason` only for
  Unfour `Unsupported`, `Validation`, and `ReadOnly` errors. Database driver
  errors retain the SQLSTATE/databaseMessage sanitizer.

OpenGauss catalog references: [PG_INDEX](https://docs.opengauss.org/zh/docs/6.0.0/docs/DatabaseReference/PG_INDEX.html),
[PG_INDEXES](https://docs.opengauss.org/zh/docs/6.0.0/docs/DatabaseReference/PG_INDEXES.html),
and [6.0.0 system information functions](https://docs.opengauss.org/en/docs/6.0.0/docs/SQLReference/system-information-functions.html).
The default internal-schema set follows the [openGauss schema reference](https://docs.opengauss.org/en/docs/2.1.0/docs/Developerguide/schema.html).
These document relevant metadata interfaces and schemas; they are not live-server test evidence.

Automated verification: `cargo test -p unfour-database-engine -p unfour-mcp --lib`
passed (78 and 222 tests, with the final database listing SQL change retested).
Core and command-bus suites passed (38 and 80 tests).
The loopback fixture covers index names/flags/order,
native DDL return, both structure export modes, bound list SQL, explicit
`schema=dbe_perf`, PostgreSQL listing behavior, `"char"` decoding, and the
connection message. It does not execute the new catalog SQL inside openGauss.
The request reports real openGauss 6.0.3 validation of connection test,
read-only query, EXPLAIN, catalog switching, SQL/JSON/CSV data export,
columns/filters/limit and column/PK metadata. Indexes, native DDL and the two
structure export modes still need live 6.0.3 validation after this code change.

Date: 2026-09-25. Scope: existing PostgreSQL transport and runtime profile;
no new persisted driver, product hint, migration, Cloud Sync field, dependency,
MCP/Flow product branch, or provider framework.

## Implementation and compatibility policy

- Detect `openGauss` before the PostgreSQL leading banner. PostgreSQL and unknown
  detection keep their prior behavior, including probe error/timeout fallback.
- Enable catalogs, schemas, column metadata, generated-column detection, row
  mutation and data export. Keep indexes, FK metadata and DDL disabled. Disabled
  optional metadata is not requested by `table_structure` / MCP `describe_table`.
- Use the openGauss `pg_attrdef.adgencol` generated marker and `adsrc` expression,
  basic `pg_attribute` fields, `format_type`, and `pg_constraint` primary keys.
  Do not use PostgreSQL `attidentity`, `attgenerated`, information-schema
  identity/generated fields, `pg_get_expr`, `pg_index`, or `pg_get_indexdef`.
- Data-only export skips all optional structure metadata for every driver.
  Structure export still fails explicitly if DDL is unavailable.
- Catalog-scoped pools and returned table structure both use the requested
  catalog. Tests exercise `qingqi_config.public.config_info` with selected
  columns, parameterized filters, and a limit.
- The UI preset maps to `driver=postgres`. Closing/reopening discards the preset;
  existing connections initialize from the saved driver. Optional result-only
  `protocol` / `detectedServer` fields convey backend runtime facts to the UI.

Catalog references consulted:
[openGauss PG_ATTRIBUTE](https://docs.opengauss.org/en/docs/7.0.0-RC3/database_reference/pg_attribute.html),
[openGauss PG_ATTRDEF](https://docs.opengauss.org/en/docs/6.0.0/docs/DatabaseReference/pg_attrdef.html),
[earlier PG_ATTRDEF definition](https://docs.opengauss.org/en/docs/2.0.1/docs/Developerguide/pg_attrdef.html).
These references inform the SQL strategy; they are not live-server test evidence.

## Automated evidence

The loopback PostgreSQL protocol fixture returns canned metadata and data. It
records requested SQL and startup catalogs, rejects unhandled metadata requests,
and exercises actual SQLx pools, service dispatch, decoding, export file writing,
query execution and confirmed row mutation. It does **not** execute metadata SQL
inside openGauss or simulate its SQL planner, authentication, or storage engine.

Coverage:

- Case-insensitive openGauss detection and mixed PostgreSQL/openGauss banners.
- Unchanged PostgreSQL detection, unknown/GaussDB behavior, probe error/timeout.
- OpenGauss profile and three-way metadata dispatch.
- Catalog discovery, schema/table/column mapping, primary keys, openGauss
  `adsrc = 'AUTO_INCREMENT'` auto-increment columns, generated columns, and
  table structure with optional metadata unavailable. PostgreSQL `nextval` /
  identity recognition is unchanged. The MySQL driver label stays
  `MySQL / MariaDB`.
- Catalog override across metadata, query, execute, row insert/update/delete and
  data-only export; connection domain snapshot remains unchanged.
- CSV export selecting `content`, filtering `id`, and `LIMIT 1`; PostgreSQL
  data-only export also skips optional metadata despite enabled capabilities.
- UI preset save payload, reopening from driver, and protocol/detected server
  display. Existing MCP database tests cover generic describe/export contracts.

| Verification | Result |
| --- | --- |
| `cargo test -p unfour-database-engine --lib` | PASS: 71 tests |
| `cargo test -p unfour-mcp database --lib` | PASS: 60 tests |
| `pnpm exec vitest run packages/database/src/components/DatabaseConnectionDialog.test.tsx packages/database/src/hooks/useDatabaseConnectionForm.test.tsx packages/ui/src/i18n.test.ts` | PASS: 8 tests |
| ESLint on changed TS/TSX files | PASS |
| `pnpm --filter @unfour/desktop exec tsc --noEmit` | PASS |
| `pnpm run build` | PASS; Vite reports its bundle-size warning |
| `cargo fmt -p unfour-database-engine -p unfour-core -p unfour-mcp --check` | PASS |
| `git diff --check` | PASS |
| `pnpm run check:large-files` | PASS: zero blocking findings |

Initial test failures were corrected: a fixture assertion assumed one physical
connection per pool, and a UI assertion found the old MySQL/MariaDB label. Final
runs pass. The first sandboxed Vitest launch failed with esbuild `spawn EPERM`;
the authorized unsandboxed retry passed.

## Not verified against a real service

No real openGauss endpoint was configured or used. Real authentication/TLS,
catalog SQL and permissions, generated/serial columns, query/transaction behavior,
row mutations, exports and SQLx type decoding remain **NOT VERIFIED** against
openGauss (including 5.x). The desktop view was verified by rendered component
tests and production build, not a running native desktop session.

Before certifying a server version, use a disposable database and cover multiple
catalogs/schemas, quoted identifiers, composite primary keys, serial/generated
columns, read-only/confirmation gates, NULL and numeric/text/date/binary values,
CSV/JSON/SQL data exports and parameterized filtering. Index/FK/DDL capabilities
must remain off until dedicated implementations are validated for that version.
