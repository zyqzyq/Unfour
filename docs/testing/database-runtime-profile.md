# Database runtime profile verification

Date: 2026-09-24, with probe-timeout coverage rechecked on 2026-09-25.
Windows local workspace; all database fixtures are disposable.
Design and changed-file inventory: [runtime compatibility](../architecture/database-runtime-profile.md).

Probe timeout recheck on 2026-09-25:

| Check | Result |
| --- | --- |
| `cargo test -p unfour-database-engine --lib runtime_profile -- --test-threads=2` | PASS: 10 tests |
| `cargo fmt -p unfour-database-engine` | PASS |

The 2026-09-24 matrix below was not re-run for this timeout change.

| Check | Result |
| --- | --- |
| `cargo test -p unfour-database-engine --lib --no-fail-fast` | PASS: 63 tests |
| `cargo test -p unfour-command-bus --lib flow -- --test-threads=2` | PASS: 29 tests |
| `cargo test -p unfour-command-bus --test database_script --test connection_domain` | PASS: 12 tests |
| `cargo test -p unfour-mcp database -- --test-threads=2` | PASS: 60 tests |
| `cargo test -p unfour-cloud-sync --lib canonical::tests::connection_snapshots_use_a_strict_device_local_safe_allowlist` | PASS: 1 test |
| `pnpm exec vitest run packages/database/src` | PASS: 28 files, 183 tests |
| `cargo fmt -p unfour-database-engine -p unfour-cloud-sync --check` | PASS |
| `pnpm run check:migrations` | PASS: 29 migration files; none changed |
| `pnpm run check:large-files` | PASS: zero blocking findings; existing size advisories remain |
| `git diff --check` | PASS |

The first frontend test attempt failed at startup with sandbox `spawn EPERM` for
esbuild. The same command passed after an approved execution outside the sandbox.
An initial new SQLite test failed on Windows asynchronous file-lock cleanup;
bounded fixture cleanup now waits for SQLx's dropped pools to close, and the
final full engine suite passes.

## New coverage

- PostgreSQL banner recognition, blank/malformed/compatible banners and fork
  markers, including openGauss embedded in an upstream PostgreSQL banner.
- Known family capability baselines and the conservative unknown profile;
  unknown retains the PostgreSQL grammar without native metadata assumptions.
- Real SQLx handshake and `SELECT version()` over a loopback protocol fixture.
  The fixture's banner changes while the saved connection ID is unchanged;
  the next connection/catalog target resolves a fresh profile.
- Unknown catalog discovery avoids `pg_database` while returning the configured
  catalog, preserving the existing UI tree contract.
- Connection failure, including a handshake that never completes, remains an
  error. `runtime_profile` and `test_connection` both return `Err`.
- Detection SQL error is a degraded fallback, not a connection failure.
  PostgreSQL becomes `UnknownPostgresCompatible` with `server_version = None`
  and conservative capabilities. MySQL stays `Mysql` with `server_version = None`
  and MySQL baseline capabilities. `test_connection` stays `ok=true`.
- Detection timeout uses the same degraded fallback. A loopback fixture accepts
  the version query and never answers. `server_version = None` means the probe
  was unavailable, not that connect failed.
- Standard column/primary-key fallback SQL executes against an isolated minimal
  `information_schema` fixture without PostgreSQL catalogs or generated fields.
- Connection serialization, revisions, advanced JSON and domain sync snapshots
  are unchanged after runtime detection. Canonical Cloud Sync payload assertions
  exclude all profile/product/dialect/capability fields.
- Existing SQLite connection rows work with the unchanged schema and version
  result. Existing CRUD, scripts, safety, export and metadata regression suites
  remain green, along with Flow and MCP contracts.

## Verification limits

SQLite uses a real local engine. PostgreSQL detection uses a minimal protocol
fixture; it does not certify metadata SQL against a real PostgreSQL server.
Existing PostgreSQL/MySQL configuration, SQL generation, credential, rejection
and safety tests pass, but real PostgreSQL and MySQL service integration is
**NOT VERIFIED** in this environment. No live service credentials were used.
The unknown metadata fixture verifies the standard SQL and selection path, not
compatibility with any particular vendor. No openGauss support claim is made.
