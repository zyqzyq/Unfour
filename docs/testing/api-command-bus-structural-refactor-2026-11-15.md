# API CommandBus structural refactor verification

Date: **2026-11-15 (Asia/Shanghai)**. Entry: `main` at
`964606d429b20832d58b93049becba7984abe96e`.

## Scope and audit

Read the root and CommandBus instructions, API commands, routes, script execution,
execution registry, workspace variable resolution, API Client Service modules,
domain transactions, completion Activity, and related tests before migration.
See [module ownership and compatibility contracts](../architecture/api-command-bus.md).

| File | Result |
| --- | --- |
| `src/api_commands.rs` | 1023 -> 12 lines; private module assembly facade |
| `src/api_commands/environments.rs` | 121 lines; legacy Environment compatibility and metadata-preserving updates |
| `src/api_commands/collections.rs` | 419 lines; Collection/Folder authoring, ordering, and checked deletion |
| `src/api_commands/requests.rs` | 306 lines; Request authoring, History, and atomic route writes |
| `src/api_commands/execution.rs` | 192 lines; direct sends and saved/snapshot Replay coordination |
| `src/core_commands.rs` | Receives the two unchanged Workspace layout methods |
| `src/lib.rs` | Removes imports now owned by the child modules; keeps the response type used only by tests behind `cfg(test)` |

Source paths above are relative to `crates/unfour-command-bus/`. A mechanical
comparison against entry HEAD confirmed that all **45 method signatures and
bodies**, plus the private `saved_replay_input` helper, remain identical after
newline normalization and occur exactly once. Independent review confirmed the
same result. Existing transaction, Route, Script, cancellation, variable
resolution, and HTTP Engine implementations have no changes.

No dependencies, schema, Cloud Sync protocol, MCP tool protocol, Bastion, or
SQLx implementation were changed. Pre-existing untracked DSH/SQLx documents,
testing evidence, and `scripts/poc/` were preserved and excluded from this change.

## Baseline and incremental verification

- The first baseline build could not finish because E: ran out of space.
  Only generated incremental caches under this repository's
  `target/debug/incremental` were removed after checking their resolved paths.
- The next default parallel CommandBus unit run had **193 PASS, 1 FAIL**:
  `flow_action_history_budget_preserves_known_success_and_failure` exceeded its
  serialized history-size assertion. Its exact isolated rerun passed before
  production migration. No Flow code or assertion was changed.
- API Domain (**8**) and Workspace Domain (**24**) baseline tests passed.
  HTTP Engine baseline passed **125** tests outside the sandbox. Sandbox runs
  rejected localhost requests with `ApiNetwork`; the blocked HTTP suite was
  stopped, and the same isolated fixture then the full suite passed outside it.
- Environment migration: Workspace Domain **24 PASS**, environment filter
  **4 PASS**. The latter required the same localhost sandbox workaround.
- Four new transaction regressions passed before moving the Collection/Request
  implementations. Collection/Folder migration: API Domain **12 PASS**.
- Request migration: API Domain **12 PASS**, API Route **2 PASS**.
- Execution migration: API filter **48 PASS**. The parallel Script subset had
  **19 PASS, 1 FAIL** at the existing secret-projection diagnostics assertion
  `logs.contains("api_request_completed")`; this test had passed in the API
  filter run. Serial Script rerun: **20 PASS**. Neither logging nor the existing
  test was changed; final full-suite verification uses one test thread.

## Additional transaction regressions

[`tests/api_domain/guards.rs`](../../crates/unfour-command-bus/tests/api_domain/guards.rs)
adds four isolated-database tests:

1. Stale Request revision preserves the current definition, Activity, and hooks.
2. A changed Folder subtree snapshot rejects deletion and preserves its contents,
   Activity, and hooks.
3. Invalid routed save rolls back the Request, auto-created default Collection,
   local Route, Activity, and hooks.
4. Routed update changes real business fields before Route CAS/validation fails;
   the entire Request including revision, original Route, Activity, and hooks
   remain unchanged.

## Final checks

`cargo test -p unfour-command-bus -p unfour-http-engine -- --test-threads=1`:
**PASS**, **386 tests**, zero failures or ignored tests in the selected default
feature set:

| Suite | Passed |
| --- | ---: |
| CommandBus unit tests (includes API Route, Script, Secret Projection, Replay, cancellation, and completion warnings) | 194 |
| API Domain (includes the four added transaction regressions) | 12 |
| Workspace Domain | 24 |
| Other CommandBus integration tests | 31 |
| HTTP Engine (includes runtime Script and request revision/history regressions) | 125 |

The native SSH integration target contains no tests under these default
features; live SSH behavior is outside this structural change. Doc tests passed
with zero cases.

`cargo test -p unfour-cloud-sync --test outbox -- --test-threads=1`: **PASS**,
**17 tests**, including API entity outbox capture, cascade deletion, pause/local
ownership behavior, and `later_hook_failure_rolls_back_business_data_and_outbox`.
This brings final runtime coverage to **403 passing tests** across the selected
suites without changing Cloud Sync implementation or protocol.

`cargo check -p unfour-command-bus -p unfour-http-engine -p unfour-mcp -p unfour-app`:
**PASS**, including the existing MCP default native-SSH feature and shared Tauri
composition callers. The first check identified obsolete root imports left by
the move; those were removed, with the one remaining test-only import gated.
The repeated check has no unused-import diagnostic from this change. Existing
vendor SQLx lifetime, default release-channel, and filesystem hard-link fallback
warnings remain outside the refactor.

After the import-only cleanup, `cargo test -p unfour-command-bus --no-run`:
**PASS** for all test targets. The first attempt caught a missing test-only
`ApiResponse` import (**FAIL**, corrected with `cfg(test)`); the successful rerun
has no unused-import diagnostic. The final API runtime rerun,
`cargo test -p unfour-command-bus --lib api_ -- --test-threads=1`: **48 PASS**.
The four-crate Cargo Check was also repeated successfully on this final source.

`cargo fmt -p unfour-command-bus --check` and `git diff --check`: **PASS**.
`pnpm run check:large-files` could not launch its pinned pnpm because sandbox
symlink creation returned `EPERM`. Running the identical package script directly
with `node scripts/check-large-files.mjs`: **PASS**, zero Critical/blocking files;
no size threshold or baseline exception was changed.

## Remaining limits

The observed parallel timing/log-capture failures are retained as verification
evidence rather than hidden or fixed through this structural change. Tests use
isolated local databases and loopback fixtures; this work does not establish
live remote SSH or cloud-service behavior. The public CommandBus contract and
existing execution order are preserved rather than consolidated.
