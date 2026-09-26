# Flow HTTP failure secret provenance closure

Baseline: `d3c9daec3497a645c5f752870843ff07ef1ebfb2`. Verified on 2026-09-26.

## Actual CI failure

[CI run 36231092937](https://github.com/zyqzyq/Unfour/actions/runs/36231092937)
failed only in Frontend / Unit tests. `FlowPage.test.tsx`, “submits the selected
environment only after effects confirmation”, asserted dialog removal immediately
after observing `runFlow` invocation. The log shows the dialog still open with
its fieldset/buttons disabled. Invocation is earlier than promise completion and
the React update. Rust and Release contracts passed in that baseline run.

The regression now controls the request promise explicitly, asserts that the
pending dialog remains disabled, resolves the promise, and waits for dialog
removal and the run view. No test or CI gate was removed or relaxed. The baseline
test passed in isolation locally; the CI log provides the observed failure.

## Secret provenance

`unfour_http_engine::runtime_request_secret_values` owns extraction from resolved,
auth-materialized request input. It reuses the domain auth allowlist, sensitive
key classifier and URL/form parsing helpers. Runtime camelCase aliases include
`apiKey`; persisted API Client snapshot/history behavior is unchanged.

Coverage includes Bearer/Basic/custom API-key auth, enabled header/query slots,
URL userinfo and query values (encoded and decoded), nested JSON, and both raw
and key-value-list form bodies. Disabled and ordinary fields are excluded.
Longer overlapping values are scrubbed first.

Command-bus consumes that helper before failure diagnostic truncation. Its
duplicated auth/header/query extraction was removed. A capability-neutral
`FlowExecutor::snapshot_secret_values` port passes saved-request provenance to
the run-local service in memory, protecting initial/final resource snapshots and
URL-derived request names as well. Flow storage has no HTTP classification rules
or HTTP-engine dependency, and no persisted Flow field was added.

Loopback 422 regressions inspect the stored `flow_runs.run_json`, initial run
response, latest step output and attempt output. They cover saved JSON token,
URL query/userinfo, and raw form secrets, while retaining ordinary body/query
values, trace headers, status and error context. The new regression initially
exposed URL secrets in resource snapshots even when diagnostics were safe; the
snapshot port fixes that additional persistence path.

HTTP status/error types, Wait Until retry semantics, diagnostic truncation order,
256 KiB output and 4 MiB run limits are unchanged. No dependency was added.

## Verification

- PASS: `pnpm run lint` (0 errors, existing 45 warnings).
- PASS: `pnpm run build` (existing chunk-size advisory).
- PASS: `pnpm run test` (150 files, 884 tests).
- PASS: `pnpm run test:release-env` (73 tests).
- PASS: `pnpm run check:secrets`.
- PASS: `pnpm run check:migrations` (29 migrations).
- PASS: targeted persisted HTTP failure regression.
- PASS: Rust formatting for the three changed crates.
- PASS: `pnpm run check:large-files` (0 blocking violations).
- PASS: `cargo check --workspace`.
- PASS: `cargo check -p unfour --features ssh-native`.
- PASS: `cargo test --workspace` (1005 passed, 1 existing ignored test requiring
  access to the platform credential store).
- PASS: `cargo test -p unfour-command-bus --lib flow_` with default features
  (40 passed), including custom auth echo, HTTP retry and size-limit regressions.
- PASS: `git diff --check`.

Intermediate local verification failures: sandboxed Node subprocess startup
returned EPERM; the same checks passed with approved execution outside the
sandbox. Overlapping Cargo runs caused Windows executable locks (LNK1104 / OS
error 32); a single sequential workspace test run passed after those runs ended.
Neither issue required changes to CI or test assertions.

Local host is Windows x64 with Node 24.16.0; CI uses Ubuntu 24.04 and Node 20.
The exact workflow commands and default Cargo features are retained, including
the separate `ssh-native` check and sidecar preparation. No WSL distribution or
Docker is available locally. Linux execution of these uncommitted changes is
NOT VERIFIED; baseline Linux success does not establish new-change success.
No commit, push, or remote CI rerun was performed.
