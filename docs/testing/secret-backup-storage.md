# Secret storage and encrypted backup verification — 2026-10-09

This supersedes the imported API/variable Keychain-handle behavior described in
the earlier [bundle audit](workspace-bundle.md). Scope is the existing V2 format,
connection credential lifecycle, API authentication/redaction and desktop editors.

## Resulting boundaries

- SSH/database passwords and SSH private-key passphrases use OS Keychain. V2
  import creates fresh connection references with the existing journal. API Auth,
  sensitive Headers/Query and Workspace/environment Secret values restore to
  SQLite, including quoted JSON values, flags, enabled state and templates.
- Prior V2 test-import references have no compatibility resolver or migration.
  Ordinary data and V1 import continue through their existing storage paths.
- A per-database OS file lock spans connection staging through commit/rollback
  and primary recovery. Desktop and MCP writers share it; satellites skip primary
  recovery. Last-reference collection still checks connections under SQLite's
  writer lock. Copies, failed edits, clear, delete and interrupted stages are covered.
- API variables and secret provenance come from one temporary SQLite read
  snapshot. Basic/Bearer/API Key materialization follows resolution; explicit
  enabled header/query values retain precedence. The frontend no longer emits
  generated authentication values before backend resolution.
- Sensitive inputs hide/reveal without emitting edit callbacks. Saved connection
  editors distinguish preserve, replace and clear. Desktop reveal requires the
  live connection, workspace, type and expected reference; no MCP plaintext-read
  command/tool was added.
- Runtime secrets and encoded/escaped variants are scrubbed from history and
  diagnostics. URL/header validation returns fixed messages. Send activity omits
  the URL. Configured custom API Key targets participate in existing domain,
  Workspace, Unfour Collection, Postman and OpenAPI export redaction.

No new dependencies, encryption store, schema migrations, Cloud Sync/E2EE feature
or wire contract were added. The unused workspace-engine Keychain dependency and
previous test-import handle runtime/editor paths were removed.

## Executed checks

| Check | Result | Evidence |
| --- | --- | --- |
| `pnpm run test` | PASS — 168 files, 988 tests | Entire frontend suite; API auth transport, editor persistence/failure, hidden/reveal, preserve/replace/clear and late reveal cancellation |
| `cargo test -p unfour-command-bus -p unfour-http-engine -p unfour-workspace-engine -p unfour-secret-store -p unfour-ssh-engine -p unfour-database-engine -p unfour-core -p unfour-mcp -p unfour-cloud-sync --lib -- --test-threads=4` | PASS — 689 tests | V1/V2 safety, encryption password/tamper/format checks, round trips and SQLite consistency, scripts/Flow/HTTP auth, history/error redaction, Keychain staging/rollback/recovery, MCP adapter and cross-process recovery |
| `cargo test -p unfour-command-bus --test api_domain --test workspace_domain --test connection_domain -- --test-threads=4` | PASS — 35 tests | Domain mutations, local-only credentials/usage state, external apply, transaction hooks and activity safety |
| `cargo test -p unfour-cloud-sync --test contracts --test outbox --test worker --test ownership -- --test-threads=4` | PASS — 149 tests | Existing protocol, secret exclusions, durable outbox intent, owner/account fencing and recovery; mock transport only |
| `cargo test -p unfour-secret-store --test os_keychain_release_smoke -- --ignored` | PASS — 1 test | Actual Windows Keychain save/read/delete with a unique disposable service and synthetic passwords/passphrase |
| `cargo check -p unfour-app --features ssh-native` | PASS | Final Tauri command composition and native SSH feature |
| `pnpm run build` | PASS | Desktop TypeScript checks and production bundle |
| ESLint on changed TS/TSX files | PASS — zero errors | Four non-blocking function-size/complexity warnings; new component export warning removed |
| `pnpm exec playwright test apps/desktop/tests/smoke/workspace-bundle.spec.ts apps/desktop/tests/smoke/api-client-layout.spec.ts --workers=2` | PASS — 3 tests | English/Chinese 960×600 backup dialogs and API layout; screenshots inspected |
| Affected Cargo formatting; `git diff --check`; migration, shared-token and public-secret checks | PASS | 33 migrations; no new migration or shared-token redefinition |
| `node scripts/check-large-files.mjs` | FAIL — existing generated Monaco assets | `ts.worker-CMbG-7ft.js` (67,731 lines) and `vs/loader.js` (1,368 lines), already recorded in prior verification; no changed source hits the blocking threshold |

The original `pnpm run test` baseline failed six title-bar tests due to a missing
`WorkspaceBundleExchangeProvider` and one desktop integration test under
unbounded worker concurrency. The test wrapper now supplies the actual provider;
Vitest caps workers at four without increasing timeouts or skipping tests.
During development, new fixture/assertion failures were corrected and affected
suites rerun to the final passing results above. Restricted-shell loopback/process
and pnpm bootstrap failures were rerun with the normal host environment.

## Coverage details and remaining limits

The cross-process regression runs an actual child test process against the same
isolated SQLite file: primary startup blocks while a satellite holds the staging
lock, then resumes after publication and retains the attached reference. In-memory
credential backends isolate those lifecycle tests. Backup snapshot tests update
connection credentials and SQLite values concurrently; API tests retain resolved
secret provenance after a later variable edit. Database password-only rollback is
injected with a failing SQL trigger because no Cloud mutation hook runs for a
credential-only edit. Real HTTP checks use loopback fixture servers.

NOT VERIFIED: native desktop reveal dialogs, live SSH/database servers, macOS or
Linux Keychain providers, and Windows provider/permission failure recovery. No
real user Workspace, Unfour credential or remote service was accessed. The Windows
smoke used its own random service name and deleted its synthetic credentials.

Very short secret values may redact ordinary substrings in diagnostic/history
previews; outgoing requests and persisted request definitions keep their original
values. Slow Keychain operations may delay other staging writers or startup
recovery while the lock is held. External OS credential edits do not honor Unfour's
SQLite/file locks. Historical orphaned keys without live connections or journal
metadata are not enumerated or migrated.
