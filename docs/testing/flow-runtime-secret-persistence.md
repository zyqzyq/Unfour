# Flow runtime secret persistence closure

Baseline: `99481de7fd44a04f8e84ae523fcf3e2e3f9bdceb`. Verified on 2026-09-26.

## Scope and behavior

The API executor registers `runtime_request_secret_values()` from the final
resolved, auth-materialized request before sending it. The capability-neutral
`FlowPersistenceContext` carries these values only in memory. Snapshot provenance
and runtime provenance feed the same `safe_run()` serialization path.

Successful responses and expression context retain their original values in
memory. Only the persisted view is scrubbed, including step output, attempt
input/output, and later writes. Existing HTTP failure diagnostic scrubbing stays
in place. The context is freshly allocated for every run, even through cloned
services; no global registry, persisted schema field, dependency, or HTTP-specific
rule was added to Flow engine. Its mutex is never held across an await.

## Regression coverage

- Real loopback HTTP 200: selected Environment secret `API_TOKEN=real-secret`,
  saved JSON body `{"token":"{{API_TOKEN}}"}`, response
  `{"echo":"real-secret","safe":"visible"}`.
- The second step sends the first step's real `echo` back on the wire after the
  first output was persisted, proving execution context remains unredacted.
- Runtime body override and previous-step `echo` in a sensitive request field
  take the same final materialized request path.
- The final SQLite `flow_runs.run_json` contains no `real-secret`; both step and
  attempt outputs contain `<redacted>` and preserve `safe=visible`. A later Wait
  step exercises additional persistence after the API actions.
- Concurrent runs through cloned services use the same executor and ordinary
  value, but only the run registering provenance redacts that value. Both runs
  still pass the real value to their later step.

Existing suites cover Wait Until/Poll retry, 256 KiB output and 4 MiB history
limits, snapshot protection, HTTP failure diagnostics, Query occurrences, SSH
provenance, and legacy persisted Flow compatibility.

## Verification

- PASS: `pnpm run lint` (0 errors, 45 existing warnings).
- PASS: `pnpm run build` (existing chunk-size advisory).
- PASS: `pnpm run test` (150 files, 884 tests).
- PASS: `pnpm run test:release-env` (73 tests).
- PASS: `pnpm run check:secrets`.
- PASS: `pnpm run check:migrations` (29 migrations).
- PASS: `node scripts/prepare-tauri-sidecars.mjs`.
- PASS: `cargo check --workspace`.
- PASS: `cargo check -p unfour --features ssh-native`.
- PASS: `cargo test --workspace` (1007 passed, 1 existing ignored test requiring
  access to the platform credential store).
- PASS: `cargo test -p unfour-command-bus --lib flow_` (42 tests, default features).
- PASS: `cargo test -p unfour-http-engine` (80 tests, default features).
- PASS: `cargo fmt -p unfour-flow-engine -p unfour-command-bus --check`.
- PASS: `pnpm run check:large-files` (0 blocking violations).
- PASS: `git diff --check`.

Initial sandboxed build, release-contract tests, and secret audit failed with
Node subprocess `EPERM`; approved execution outside the sandbox passed without
code or assertion changes. Cargo commands ran sequentially.

Local host is Windows x64 with Node 24.16.0; CI uses Ubuntu and Node 20. The CI
verification commands were run locally using the existing installed dependencies.
Linux execution of these changes is NOT VERIFIED. No commit, push, or remote CI
rerun was performed.
