# API script diagnostic redaction

Base: `main` / `5818b0c9`, fetched from `origin/main` before editing.

## Confirmed leakage paths

- The JS bootstrap scrubbed Console and assertion text using only the initial
  redaction list. Rust returned collected Console, test names, and assertion
  messages without a second pass. Output validation failures returned those
  same unsanitized collections.
- Runtime exceptions used only the initial value list. Snapshot/history
  mutation provenance was assembled later in Command Bus and did not protect
  script diagnostics or carry arbitrary environment values into post scripts.
- Result export could throw script-controlled errors. JSON decoding errors
  could quote invalid field/enum contents. Response JSON parser errors could
  echo otherwise unclassified response fragments.
- Eager replacement of an old credential could leave a new overlapping
  credential's suffix. Console truncation could retain a credential fragment
  that no longer matched the complete value.

## Fix

- Add an execution-local, bounded host collector for `pm.variables.set`,
  `pm.environment.set`, and header `upsert`. Values remain known after overwrite,
  unset, or remove. The callback is captured privately and removed from globals.
- Combine initial provenance with final variable/environment/request differences,
  regardless of field naming or enabled state. Include structured payload leaves,
  request URL components, and wrapped credential components. Opaque changed
  bodies, invalid changed URLs, or collector overflow hide diagnostic text
  conservatively. No collector data is serialized or persisted.
- Reuse history/snapshot `scrub_values` for raw, JSON-escaped, and URL-encoded
  replacements, preserving its longest-first and short-value behavior. Reuse
  core structural, sensitive-line, and credential-name helpers. Remove the old
  Rust-only replacement/marker list and defer known-value replacement until the
  host has complete provenance.
- Scrub Console messages, test names, assertion errors, and runtime errors before
  returning collected output, including validation and timeout paths. Failed
  collection/decoding and worker failures use fixed diagnostics. Clipped Console
  entries and response JSON parser failures never expose their raw fragments.
- Command Bus seeds script redactions from existing runtime request provenance
  (Auth, URL, query, body, headers) and carries pre-request provenance into post
  execution. Request and variable outputs retain real values. Existing commit,
  rollback, cancellation, HTTP, and history behavior remains intact.
- Request templates, history schema, Cloud Sync, MCP permissions, and Flow
  protocols are unchanged. Flow still rejects scripted API resources with its
  existing `FLOW_API_SCRIPT_OR_MULTIPART_UNSUPPORTED` policy. Historical
  `environmentId` remains deferred to P2.

## Verification

| Check | Result |
| --- | --- |
| `cargo test -p unfour-http-engine -p unfour-command-bus -p unfour-mcp --quiet -- --test-threads=2` | PASS, 541 tests total |
| HTTP Engine | PASS, 112 unit tests |
| Command Bus | PASS, 132 unit + 55 integration tests, including existing Flow/native SSH coverage |
| MCP | PASS, 235 library + 4 binary + 3 protocol/registry tests |
| `cargo check --workspace` | PASS, including desktop and MCP composition |
| `cargo clippy -p unfour-http-engine -p unfour-command-bus -p unfour-mcp --lib --no-deps` | PASS, existing warnings; no diagnostics in changed code |
| Same Clippy invocation with `--all-targets` | FAIL, two existing test errors described below |
| `cargo fmt -p unfour-http-engine -p unfour-command-bus -p unfour-mcp --check` | PASS |
| `node scripts/check-large-files.mjs` | PASS, zero blocking findings |
| `git diff --check` | PASS |

The all-target Clippy failures are `clippy::unused_io_amount` at
`crates/unfour-command-bus/src/lib_tests/flow_native.rs:246` and
`crates/unfour-command-bus/src/lib_tests/flow_authoring.rs:444`. Both exact
unhandled reads exist in the base commit and neither file was modified. No lint
was suppressed or weakened to obtain the production-code result.

Regression coverage includes both phases, ordinary destination names, transient
overwritten/deleted values, Console before writes, test names/assertions, runtime
exceptions, output validation, failed export/decoding, timeout, output/provenance
limits, truncation, overlapping/escaped/encoded existing values, and opaque
request mutation. A real loopback POST verifies exact body/header/query/Auth
values while diagnostics, snapshot, stored history, and activity remain safe.
Environment commit/rollback and the actual MCP registry failure response are
also exercised. The full existing Flow suites remain unchanged and pass.

Initial sandboxed loopback runs failed or waited for unavailable network I/O;
they were stopped and rerun outside the sandbox with isolated test data. A stale
Windows test executable caused one `LNK1104` retry. An intermediate response-JSON
diagnostic regression was fixed before the final passing run. The pnpm wrapper
could not open its sandbox database; the same large-file checker passed through
Node directly. Existing release-channel/default and lint warnings remain.

## Limits

- This is provenance-based diagnostic redaction, not complete information-flow
  tracking for arbitrary JavaScript. A value confined to script locals, never
  provided as known sensitive input or written through an observed boundary,
  cannot reliably be classified under an ordinary name.
- Arbitrary transformations, custom encodings, partial values, and complex
  credential wrappers are not universally reversible by the shared matcher.
  Direct request property/array mutations that are reverted before export can
  also lose provenance; the supported variable/environment/header setters retain
  transient writes. Deliberately hostile sandbox/prototype manipulation is not
  claimed to provide a general confidentiality guarantee.
- Conservative hiding can remove harmless diagnostic text. HTTP request values,
  environment update values, and live HTTP response bodies retain their existing
  execution/display contracts; this change protects script diagnostics and does
  not turn arbitrary HTTP response content into a fully classified secret store.
- No native Tauri visual test or external-service test is claimed. Integration
  tests use ephemeral databases and isolated loopback services.
