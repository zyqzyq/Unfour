# MCP Environment / Workspace Variable secret projection

Date: 2026-11-15. Baseline: `main` at
`e73fd1012055817bd4c3ddd13eb79bdc6527923d`.
Scope: output boundaries only. No database migration, Cloud Sync protocol,
tool name/schema, permission policy, or SecretStore framework changes.

## Root causes and corrected paths

| Path | Finding and output boundary |
| --- | --- |
| SQLite → WorkspaceService → CommandBus workspace variables/environments | Typed records retain `is_secret`, `is_enabled`, and actual values. These are trusted local editing/execution inputs, not safe-to-serialize MCP payloads. |
| Legacy `ApiEnvironment` projection | `legacy_api_environment` reduces variables to `KeyValue`, losing `is_secret`. MCP environment list and whole-environment update previously consumed this projection, so ordinary-name secrets escaped keyword masking. |
| MCP environment list/create/update | Now carry `WorkspaceEnvironment` through the adapter to the handler. The typed update result is the mutation's own result, not a second read joined by key. Legacy Desktop methods continue returning their existing editable values. |
| Whole-environment update | Key matching now follows existing case-insensitive variable identity and trims input keys; casing/whitespace edits preserve classification. The existing transactional snapshot check rejects concurrent changes rather than overwriting newer classification with stale metadata. |
| Workspace and environment variable tool payloads | One MCP variable-value helper masks explicit secrets regardless of enablement, adds the existing shared workspace-variable fallback (`db_password`, local handles), and preserves prior MCP sensitive-key coverage. `isSecret=false` is not a request to bypass defensive name masking. Ordinary public values remain visible. |
| API execution | Workspace snapshot resolution still supplies actual values and secret provenance. Existing priority is request-local override → selected environment → workspace; disabled environment values do not shadow enabled workspace values. No mask is written back into execution input. |
| HTTP response → MCP | The HTTP engine captures sanitized headers/body with the exact execution's secret values in a serde-skipped in-process projection. MCP selects that projection at the handler boundary. Desktop and post-response scripts retain raw responses; `sizeBytes` still counts the original body. No variable re-read/race or raw-secret list is added to the response contract. |
| Errors, snapshots, Activity, History, logs | Resolved multipart field names and script-derived HTTP methods could leak through dynamic validation errors or diagnostics. These outputs are scrubbed using execution provenance; the on-wire request stays unchanged. Activity uses the already sanitized executed method. Canonical uppercase methods, lowercase host/header names, and IDNA/Punycode hostnames retain their secret provenance too. |
| MCP text/structuredContent | Success responses serialize the same safe business value into both channels. Error responses retain the existing text-only contract with no `structuredContent`; adapter errors use fixed messages and safe details. |

The legacy `ReadCommand::ApiListEnvironments` and Desktop compatibility methods
remain trusted internal projections with editable values. The registered MCP
list tool no longer consumes that lossy result. Internal raw records must not be
treated as public diagnostic DTOs.

Mask descriptors previously echoed any alphabetic first word as an authentication
scheme and any short token prefix as public shape. They now expose only recognized
schemes/prefixes. Existing diagnostic length/fingerprint fields remain unchanged.

## Implementation pointers

- [CommandBus typed update and legacy delegation](../../crates/unfour-command-bus/src/api_commands.rs)
  and [adapter forwarding](../../crates/unfour-mcp/src/command_bus_adapter/implementation.rs).
- [MCP API handlers](../../crates/unfour-mcp/src/tools/api_handlers.rs),
  [workspace variables](../../crates/unfour-mcp/src/tools/workspace.rs),
  [environment variable mutations](../../crates/unfour-mcp/src/tools/api_environment_variables.rs),
  and [shared MCP masking](../../crates/unfour-mcp/src/sanitize.rs).
- [In-process response model](../../crates/unfour-core/src/models/api.rs),
  [HTTP execution](../../crates/http-engine/src/api_client/execution.rs),
  [response/history sanitization](../../crates/http-engine/src/api_client/history_redaction.rs),
  [executed snapshot](../../crates/http-engine/src/api_client/execution_snapshot.rs),
  and [multipart diagnostics](../../crates/http-engine/src/api_client/multipart.rs).
- [CommandBus execution regressions](../../crates/unfour-command-bus/src/lib_tests/api_secret_projection.rs)
  and [real MCP registry regressions](../../crates/unfour-mcp/src/command_bus_adapter_tests/environment_secrets.rs).

## Reproduction before the fixes

Tests used isolated temporary SQLite databases and unique synthetic sentinels;
HTTP checks used loopback servers. No existing user workspace/credentials or
external API was used.

- `cargo test -p unfour-mcp environment_secret_projection --lib -- --nocapture`:
  **FAIL as expected**, 4 failures / 1 pass on the old implementation: environment
  listing, whole-environment update, casing-only update, and real credential
  request/response echo each failed the no-sentinel assertion. The single-variable
  tool and Activity case passed. The echo server verified real Bearer credentials
  before the MCP output assertion failed.
- Additional `environment_secret_projection_mask` regression: **FAIL as expected**
  before changing descriptor logic; arbitrary secret fragments appeared in masks.
- Additional compound-key (`db_password`, flag false) regression: **FAIL as
  expected** before unifying variable fallback masking.
- CommandBus multipart-error, invalid scripted-method, and executed-method
  diagnostics regressions: **FAIL as expected** before their respective fixes.
  The original credential/precedence/history test passed, confirming that
  execution injection itself did not need replacement.
- HTTP canonical-host/header regression: **FAIL as expected** before retaining
  canonical secret representations; the lowercase sentinel leaked through
  snapshots/history/response diagnostics. A synthetic `.invalid` host was mapped
  directly to the loopback fixture without external DNS.
- Pre-commit review found a further Unicode hostname case: **FAIL as expected**
  before the follow-up fix, because IDNA/Punycode does not contain the original
  secret or its ASCII lowercase variant. The regression uses a secret substring
  inside a larger hostname label, verifies the correct canonical host on the wire,
  and checks that secrets appearing only in userinfo/query leave a public host
  visible. The fix retains the parsed canonical host when a known secret overlaps
  the original hostname. A full-URL secret with outer whitespace also reproduced
  the leak before preserving source offsets from the original, untrimmed input;
  the regression covers that accepted URL form too.

The first MCP build failed before tests because E: ran out of space. Only
rebuildable incremental caches for the task's crates under `target/debug` were
removed; subsequent builds used `CARGO_INCREMENTAL=0`. Sandbox loopback requests
failed before reaching the fixture; the isolated network tests were rerun with
approved local-network execution. These failures were not counted as security
reproductions.

## Verification

| Check | Result |
| --- | --- |
| `cargo test -p unfour-mcp --lib` | **PASS: 290/290**, including seven new real-registry/descriptor regression tests; rerun after the final checked-update change. |
| `cargo test -p unfour-command-bus api_ --lib` | **PASS: 44/44**, including four new execution-boundary regressions, actual diagnostic log capture, and raw post-script response assertions. |
| `cargo test -p unfour-http-engine --lib` | **PASS: 121/121**, including the canonical host/header and Unicode/IDNA regressions. |
| `cargo test -p unfour-command-bus --test workspace_domain` | **PASS: 24/24**, including legacy adapter coordination, stale-environment snapshot rejection, and secret mutation snapshots; rerun after the final checked-update change. |
| `cargo fmt -p unfour-core -p unfour-http-engine -p unfour-command-bus -p unfour-mcp --check` | **PASS**. |
| `git diff --check` | **PASS**. |
| `node scripts/check-large-files.mjs` | **PASS**, zero blocking files; existing size advisories remain. |
| Local links in both audit documents | **PASS: 31 links**. |

`pnpm run check:large-files` initially **FAIL**ed because the sandbox blocked
pnpm's tool-install symlink. The same repository checker was executed directly
with Node and passed. Existing SQLx lifetime, default release-channel, and
non-native SSH dead-code warnings were not changed as unrelated cleanup.

Coverage includes public variables; ordinary-name explicit secrets; exact and
compound password names with the flag false; disabled secrets; both variable
scopes; environment/request-local overrides and fallback; actual authentication;
raw post-script response access; response body/header echoes; original response
size; complete MCP wrappers and matching text/structured values; History detail,
Activity, emitted diagnostics, and failure error payloads.

## Separate storage risk and limits

The [independent storage audit](2026-11-15-environment-secret-storage-audit.md)
confirms plaintext secret `value` columns, retained legacy copies, migration
needs, Bundle behavior, and Cloud Sync implications. This conflicts with the
credential-reference invariant for credentials stored in variables and remains
unfixed by design in this output-only task.

This change does not retract secrets already returned to external MCP clients or
clean up historical logs/history/database copies. Historical diagnostic rows
without the original execution's secret provenance cannot be exhaustively
reclassified from names alone. Runtime tests verify current output boundaries;
no user data cleanup, migration, live Desktop session, Bundle round-trip, or
Cloud Sync service operation was performed.
