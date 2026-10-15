# API execution snapshots and safe history reuse

Base: `main` / `8f7b340c` (fetched from origin before editing).

## Confirmed problems

- The response Request panel copied the frontend draft before Workspace
  resolution, Auth materialization, and pre-request execution.
- HTTP history used the engine input rather than the built HTTP request. URL
  query, disabled headers, generated content-type, and omitted GET/HEAD bodies
  could therefore disagree with execution.
- History reopened as an ordinary sendable draft despite lacking Auth, scripts,
  and variable references. Redaction markers could be sent as credentials.
- Arbitrarily named script-generated signatures and temporary values needed
  execution-local provenance beyond sensitive-key matching.

## Implementation and contracts

- `ApiResponse.requestSnapshot` is optional for backward IPC compatibility. Its
  distinct `ApiExecutedRequestSnapshot` type contains only sanitized request
  fields. It is generated from the built reqwest request before network I/O and
  never reconstructed from a frontend draft. Failed/old-adapter responses without
  a snapshot show no request snapshot.
- Existing history columns now receive normalized enabled request fields, a
  query-free URL plus all decoded executed query rows, and only the body actually
  sent. The UI combines old embedded URL query with separately stored query rows.
  No SQLite migration, template version storage, credential copy, runtime
  variable storage, or Cloud Sync payload change was introduced.
- Shared history redaction handles credentials, encoded variants, sensitive
  variable provenance, custom API-key slots, signature names, and script-derived
  fields. Temporary values and changed script environment values are scrubbed.
  Opaque request bodies are hidden, including on legacy history reads. Multipart
  history retains its portable definition; the live snapshot hides streaming
  bytes rather than presenting the definition as the wire body.
- History explicitly identifies itself as a lossy snapshot. Sending a new
  request or saving from it requires review of Auth/scripts/variables. The saved
  template path continues to rerun the current saved definition with its selected
  environment. No guessed association with an original saved request is made.
- Frontend send/save and the common HTTP execution boundary reject redaction
  markers, including encoded markers and MCP `[mask ...]` values. Explicit but
  incomplete Auth fails instead of silently becoming unauthenticated.
- OpenAPI history association accepts both legacy and normalized execution
  projections, retaining request structure/auth comparisons. MCP history tool
  guidance identifies snapshots as non-replayable. Adapter/Flow permission and
  confirmation policies are unchanged.

## Verification

| Check | Result |
| --- | --- |
| Vitest: API Client, command-client API browser mocks, shared i18n | 26 files, 197 tests passed |
| `cargo test -p unfour-http-engine --lib` | 101 passed |
| `cargo test -p unfour-command-bus --lib -- --test-threads=2` | 128 passed |
| `cargo test -p unfour-mcp --lib` | 234 passed |
| TypeScript: desktop `tsc --noEmit` | Passed |
| ESLint: changed TypeScript/TSX files | No errors; existing large-function warnings in ApiClientPage and useApiRequestTabs |
| `cargo check --workspace` | Passed |
| `cargo fmt --check` for the four affected crates | Passed |
| `pnpm run build` | Passed; existing bundle-size advisory |
| Migration checker | Passed, 33 files; no migration changes |
| Large-file checker | No blocking findings |
| `git diff --check` | Passed |

Coverage includes selected environment overrides, actual loopback wire values,
script-mutated requests and script environment values, temporary variables,
Basic/API-key credentials, opaque bodies, legacy history and IPC, encoded
redaction markers, history review/send/save gates, and OpenAPI association.
Command Bus and MCP suites also exercise existing Flow/adapter policies.

The initial highly parallel Command Bus run hit a timeout in the existing Flow
history-limit test; that test passed in isolation and all 128 tests passed with
two test threads. Windows sandbox filesystem/loopback restrictions required
rerunning frontend builds/tests and Rust loopback tests outside the sandbox.
Final suites above have no failures. Rust emitted existing cache hard-link,
default release-channel, and unused SSH helper warnings.

## Deliberate limits

- This is the initial built request, not a packet capture: transport-added
  headers and subsequent redirect requests are not represented. The UI says so.
- Script-mutated fields without trustworthy sensitivity metadata are hidden
  conservatively, so some public script results are also hidden.
- Legacy history cannot recover lost Auth, scripts, variable references, or the
  historical environment. Review/create is explicit; exact historical replay is
  not claimed.
- No native Tauri visual/manual live-service test is claimed. Automated frontend
  interaction tests and Rust loopback integration tests verify the changed paths.
