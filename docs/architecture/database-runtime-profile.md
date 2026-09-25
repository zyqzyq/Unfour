# Database runtime compatibility

Database connection configuration continues to store only the transport driver:
`sqlite`, `postgres`, or `mysql`. There is no persisted product hint, flavor,
runtime profile, dialect, or capability set. Existing connection rows, advanced
configuration JSON, domain snapshots and Cloud Sync payloads require no migration.

## Runtime model and lifecycle

`database-engine::database::RuntimeDatabaseProfile` contains:

- `detected_server_type`: `Sqlite`, `PostgreSql`, `OpenGauss`, `Mysql`, or
  `UnknownPostgresCompatible`.
- `server_version`: the unmodified server version/banner.
- `dialect`: the SQL grammar (`Sqlite`, `Postgres`, or `Mysql`). `Generic` is
  reserved for existing offline validation fallbacks.
- `capabilities`: catalog discovery, schemas, indexes, foreign keys, DDL,
  generated-column metadata, row mutation, export and explain.

These types intentionally have no serde or storage traits. `RuntimePool<DB>`
couples the existing SQLx pool to its detected profile. Pools are scoped to an
operation, as before; no connection-ID cache is introduced. Each new pool,
including a pool for a different catalog or an edited connection, probes anew.
`ScriptConnection` retains the profile with its checked-out physical connection
through the script, including user transactions. Runtime reads do not update
connection revisions, domain mutations, outbox intent, or sync snapshots.

`DatabaseService::runtime_profile` is an engine-only API for fresh facts about a
target. Connection input, MCP and Flow invocation schemas are unchanged.
`test_connection` reuses the pool's version and adds optional transient `protocol`
and `detectedServer` result fields for UI display; these never enter saved
connections, domain snapshots, or Cloud Sync.

PostgreSQL pools run `SELECT version()`. Detection checks the leading PostgreSQL
product/release, not a substring; known fork markers override an embedded upstream
banner. Unrecognized banners resolve to `UnknownPostgresCompatible`. SQLite and
MySQL use `sqlite_version()` and `VERSION()` respectively.

Connection failure is an error: the pool is not created and no profile is
returned. After connect succeeds, the version probe is a separate, bounded
query (`SERVER_VERSION_PROBE_TIMEOUT` in `pools.rs`). It is not retried.

- Detection SQL error: degraded fallback. Diagnostics record
  `database_server_detection_degraded`.
- Detection timeout: the same degraded fallback. The timeout is not a
  connection failure.
- `server_version = None` means the probe was unavailable or failed. It does
  not mean the connection failed.

PostgreSQL probe failure or timeout resolves to `UnknownPostgresCompatible`
with conservative capabilities. MySQL probe failure or timeout stays `Mysql`
with the MySQL baseline capabilities. Diagnostic fields carry the driver and
a stable reason only; they do not include passwords, DSNs, or secrets.

V1 detection is a banner heuristic, not a proof of product identity. Additional
probes can be added here if a product reports an indistinguishable upstream banner.
Capabilities describe Unfour's supported operation set for the detected family;
they do not replace read-only settings, SQL confirmation, permissions, or server
errors. The existing three families retain their supported feature baseline.

## Dependency boundaries

| Concern | Dependency and owner |
| --- | --- |
| Connection options, pools, SQLx query/execute, transaction transport, result decoding | `driver`; `pools.rs`, `sqlite.rs`, `script_connection.rs`, existing driver helpers |
| Tokenization, splitting, safety classification, identifiers, export placeholders/literals | Typed `DatabaseDialect`; `script_parser.rs`, `sql.rs`, `export.rs` |
| Offline Flow/editor preflight | Driver maps once to a baseline dialect; validation still precedes connecting or executing |
| Connected SQL execution | Safety is checked against the runtime dialect before sending user SQL |
| PostgreSQL column/type metadata | Detected product selects SQL at `postgres_columns_sql` in `postgres.rs` |
| Catalog discovery, optional structure metadata, row mutation, export, explain | Runtime capabilities at engine operation boundaries |
| UI/MCP/Flow | Existing generic engine results and contracts; no compatible-product branches |
| Persistence and Cloud Sync | Existing connection types and snapshots, containing driver only |

Unknown PostgreSQL-compatible servers retain the PostgreSQL grammar and query
transport. Table/column browsing uses standard `information_schema` metadata,
without assuming `pg_catalog` layouts or identity/generated-column fields. The
configured catalog remains in the tree even when server catalog discovery is
unsupported. Indexes/foreign keys are empty and DDL is absent; confirmed row
editing and table export return Unsupported. Arbitrary SQL still uses the
existing safety/confirmation gates. These fallbacks do not certify an unknown
product as supported.

## openGauss support

An `openGauss` marker (case insensitive) wins over an embedded PostgreSQL banner.
GaussDB remains unidentified; no product-family preset or special transport is
introduced. Probe failures/timeouts still use the unknown-compatible fallback.

| Capability | OpenGauss policy |
| --- | --- |
| Dialect / persisted driver | `postgres` / `postgres` |
| Catalogs, schemas, columns | Enabled using the existing catalog-scoped pools |
| Query/execute, row mutation, data export | Existing PostgreSQL transport and safety gates |
| Generated columns | `pg_attrdef.adgencol` metadata |
| Indexes, foreign keys, DDL | Disabled; empty lists / absent DDL in table structure |

`postgres_columns_sql` keeps three strategies at the PostgreSQL metadata boundary:
PostgreSQL's original SQL, openGauss catalog SQL, and the conservative standard
information-schema query for unknown products. The openGauss query uses
`pg_attribute`, `pg_class`, `pg_namespace`, `format_type`, `pg_constraint` primary
keys, and `pg_attrdef.adsrc/adgencol`. It does not reference `attidentity`,
`attgenerated`, information-schema identity/generated fields, `pg_get_expr`,
`pg_index`, or `pg_get_indexdef`. The existing PostgreSQL optional index/FK/DDL
implementations remain unchanged and are not invoked for openGauss.

Data-only export requests basic table metadata (existence, kind and columns),
skipping optional metadata for all drivers. Structure export still requires DDL.
Catalog overrides are honored both by the pool and returned table structure.

The connection editor offers PostgreSQL, openGauss, MySQL, SQLite. openGauss is
local editor state mapped to `driver=postgres`, discarded when the dialog closes.
Editing begins from the persisted driver; no product selection is reconstructed.
Connection results show protocol, backend-detected product and original banner.
MCP/Flow continue resolving connections by connection ID without product branches.

This implementation has protocol-fixture/unit coverage, not real openGauss
integration certification. See [openGauss verification](../testing/database-opengauss.md)
for evidence and real-server work still required.

## Changed files

Paths below are relative to `crates/database-engine/src/` unless qualified.

| Files | Change |
| --- | --- |
| `database.rs`, `database/runtime_profile.rs` | Model, resolution, detection, capability checks, runtime pool |
| `database/pools.rs`, `database/sqlite.rs`, `database/schema.rs` | Probes and runtime lifecycle; version reuse and catalog fallback |
| `database/postgres.rs`, `database/tables.rs` | Native/standard metadata selection and optional structure capabilities |
| `database/script_parser.rs`, `database/sql.rs`, `database/queries.rs`, `database/scripts.rs`, `database/script_connection.rs` | Explicit dialect grammar/safety/quoting and script runtime profile |
| `database/row_mutations.rs`, `database/export.rs` | Capability enforcement and dialect-based export SQL |
| `database_tests/mod.rs`, `database_tests/runtime_profile.rs`, `database_tests/profile_server.rs` | Detection, protocol fixture, runtime lifecycle, standard metadata, storage/snapshot regression coverage |
| `database_tests/scripts.rs`, `database_tests/sqlite.rs` | Existing tests adapted to typed dialect/runtime pools |
| `crates/unfour-cloud-sync/src/canonical/tests.rs` | Runtime field exclusion assertions |
| `docs/architecture/database-runtime-profile.md`, `docs/testing/database-runtime-profile.md` | Design, extension guidance and verification evidence |

See [verification evidence](../testing/database-runtime-profile.md).
