# CI / Build warning cleanup — 2026-10-22

Scope: local `main` at `ff2fab5e508b595504c3c61c84c99beaa0358714`;
working tree initially clean. Reviewed ordinary CI and the reusable Standard
build configuration. No release, deployment or dependency update.

## Evidence and classification

The latest five main CI runs all passed. The latest is
[CI #317](https://github.com/zyqzyq/Unfour/actions/runs/39784024124)
at `9138223d` (one commit behind local HEAD). Inspected all seven job logs.
Its frontend build passed with a largest JS chunk of 763.68 kB
(226.90 kB gzip). Its lint result matches the locally reproduced baseline:
**0 errors / 36 warnings**.

| Warning | Classification | Decision |
| --- | --- | --- |
| Unused `waitFor` import in `ApiClientSidebar.test.tsx` | Low risk | Removed; no test behavior change. |
| `UNFOUR_RELEASE_CHANNEL` omitted in ordinary Rust/MCP CI | Low risk | Explicitly set `test` in `ci.yml`, preserving the previous fallback. Standard verify/build retain `stable`; the build-script diagnostic remains available for genuinely missing configuration. |
| SSH `ingest_terminal_output` unused in native-feature test builds | Low risk | Compile this private helper only for non-native unit tests, its only callers. Removed the previous dead-code allowance. Native output handling is unchanged. |
| 33 ESLint size/complexity warnings: 13 complexity, 19 function length, 1 file length | Requires refactoring | Defer responsibility/lifecycle work in feature controllers, views, browser mocks and shared TreeView. Do not split files just to reduce counts. |
| 2 Flow `set-state-in-effect` warnings | Requires lifecycle review | Defer; external JSON draft synchronization and environment selection need behavior-specific validation before changing scheduling. |
| Large-file guard: 22 P0 / 73 P1, 0 blocking | Requires refactoring | Existing advisories from CI; no broad file splitting in this pass. |
| Vite chunk above 500 kB | Temporarily acceptable | Build succeeds; assess loading/performance before changing module boundaries or chunk configuration. |
| 6 `AtomicUsize::fetch_update` call sites in Cloud Sync test transport, compiled by multiple test targets | Temporarily acceptable; compatibility follow-up | CI uses Rust 1.99.1, local Rust is 1.96.0, and the workspace declares no MSRV. Confirm the supported compiler baseline before adopting the suggested `try_update` API; no toolchain change in this pass. |
| Action bootstrap/cache `url.parse()` / `punycode` deprecations | Temporarily acceptable | Located in action setup/cache steps, not application calls. Defer upstream action/runtime maintenance. |
| pnpm ignored dependency build scripts: esbuild, lefthook | Temporarily acceptable | Latest CI install/build/tests succeed. Do not broaden install-script execution permissions merely to silence this notice. |
| Local default-feature SSH test: unused `session` in `session/lifecycle.rs:96` | Low risk; deferred | Existing binding in an unchanged test, absent from the native-feature CI test log. Keep this pass to the three selected issues. |
| Local Rust incremental-cache hard-link fallback | Temporarily acceptable | Sandbox/filesystem limitation; Cargo copies files and completes. No product change needed. |

## Changes

Three implementation files, 5 added / 2 removed lines:
`.github/workflows/ci.yml`,
`packages/api-client/src/components/ApiClientSidebar.test.tsx`, and
`crates/ssh-engine/src/ssh/command_history.rs`.
No public contract, feature behavior, ESLint threshold/ignore, Vite setting,
dependency, or release workflow changed.

## Verification

Commands ran at repository root. Rust commands used
`UNFOUR_RELEASE_CHANNEL=test`.

| Check | Result |
| --- | --- |
| Full ESLint, before/after, using installed `node_modules/eslint/bin/eslint.js . --format json` | PASS: 0 errors; warnings 36 → 35. Remaining rules/counts unchanged. |
| ESLint on `ApiClientSidebar.test.tsx`, `--max-warnings 0` | PASS: no warnings. |
| Vitest `ApiClientSidebar.test.tsx` | PASS: 6 tests. |
| `cargo fmt -p unfour-ssh-engine --check` | PASS. |
| `cargo test -p unfour-ssh-engine session::operations:: --offline -- --test-threads=1` | PASS: 6 simulated-session/history tests, including echo deduplication, secret input and reconnect reset. Existing unused-binding/cache warnings remain. |
| `cargo check -p unfour-ssh-engine --tests --features ssh-native --offline` | PASS: no target dead-code or missing-channel warning. Compile verification only; no native connection exercised. |
| `node --test scripts/release-environment.test.mjs` | PASS: 7 tests. |
| Workflow diff review and Node assertions | PASS: ordinary CI declares Test once; reusable Standard verify/build retain Stable. Full YAML parser was unavailable locally. |
| `git diff --check` | PASS. |
| New frontend production build / remote CI run | NOT RUN: no production frontend changes; used the latest successful CI build as bundle-warning evidence. No remote workflow triggered. |

Initial environment attempts: pnpm's package-manager bootstrap failed with a
sandbox symlink `EPERM`; installed tools were invoked directly. The first Vitest
attempt failed before executing tests because its temporary cache rename was
denied; the same test passed with normal local permissions. An initial Rust
filter matched zero tests; corrected to the filter recorded above and ran all
six relevant tests. No checks or compiler diagnostics were suppressed.
