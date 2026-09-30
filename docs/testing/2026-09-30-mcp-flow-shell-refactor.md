# MCP cancellation regression and Flow/shell refactor verification

Date: 2026-09-30 (Asia/Shanghai). Base: `main` at
`1cf6063927fabbf46bf2cb53828600ff608e4351`.

## CI investigation

[CI run 36695625688](https://github.com/zyqzyq/Unfour/actions/runs/36695625688),
job `MCP contracts and tests`, failed with 225 passing library tests and
`flow_mcp_run_reports_success_failure_and_timeout_from_shared_engine` failing.
The `unfour.flow.get_run` output carried `DATABASE_ERROR`; the adapter's safe
error response does not expose the underlying SQLx message.

Before editing the pool configuration, the original test passed 100 isolated
repetitions and the complete MCP library passed with 32 test threads. This is
a scheduling-dependent failure, not evidence that Flow status assertions or
timeout semantics should change.

The timeout case uses a 100 ms deadline, coinciding with the engine's heartbeat
interval. Completing/timing out the step drops the heartbeat future. SQLx's
default acquisition health check awaits a SQLite worker ping; cancellation at
that boundary discards the connection. `CommandBus::ephemeral` has one private
`:memory:` connection, so discarding it destroys the schema and records. The
next acquisition opens a fresh empty database. This cancellation mechanism was
reproduced locally: the new lifecycle regression failed before the fix with
`SqliteError { code: 1, message: "no such table: workspaces" }`.

This reproduces a concrete cause consistent with CI's redacted database error;
the original MCP test itself did not fail in the local pre-fix repetitions.
[SQLx's acquisition documentation](https://docs.rs/sqlx/0.8.6/sqlx/struct.Pool.html#method.acquire)
describes this single-connection in-memory cancellation hazard and the
`test_before_acquire(false)` remedy.

The initial fix in `0c39c226766867fb4ee6d2b960b3a109da183bad` applies that option
to the ephemeral command bus. Command-bus unit fixtures use the same constructor.
The follow-up below also covers the unified mutable MCP constructor. File-backed pools, Flow execution,
heartbeat timing, cancellation, timeout results, and MCP assertions are
unchanged. The lifecycle regression polls acquisition once and drops it at a
pending boundary; it uses connection-idle state and cooperative yielding,
without adding a sleep, retrying errors, or relaxing assertions.

Post-fix stability checks passed:

- Original MCP test: 100/100 repetitions.
- Complete MCP library: 20/20 repetitions at 32 threads (226 tests per run).
- Lifecycle regression: 100/100 repetitions, 64 acquisition boundaries per run.

## Frontend responsibilities and metrics

`flowPageValidation.ts` separates enabled-variable resolution/merging, action
environment requirements, and readiness-gated API/SSH resource validation into
pure functions. Selected-environment SSH override precedence and normalized
SSH keys remain unchanged; API keys and environment suggestions keep their
original spelling. `useFlowEditor` was not changed.

`DesktopApp` remains the composition root and retains shell slot state and
layout controls. All extracted code remains in `packages/app-shell`:

- `useDesktopWorkspace`: queries, layout restoration/persistence, environment
  selection, and refresh operations.
- `useDesktopNavigation`, including its internal `useWorkspaceActivation`:
  optimistic activation/rollback, module preload/navigation, variable-manager
  draft state, and the leave confirmation guard.
- `DesktopWorkspaceContent`: startup states and persistent lazy feature mounts.
- `DesktopStatusBar`: feature status/fallback selection.
- `DesktopOverlays`: leave confirmation, command palette, and extension overlays.

There are no dependency, package-boundary, public-contract, translation-key,
or UI-style changes.

| ESLint metric | Before | After |
| --- | ---: | ---: |
| `environmentValidation` complexity | 19 | 13 |
| `DesktopApp` complexity | 80 | 24 |
| `DesktopApp` function lines, excluding blanks/comments | 482 | 120 |
| Flow package warnings | 3 | 2 |
| App-shell package warnings | 3 | 1 |
| Repository warnings | 46 | 43 |
| Repository lint errors | 0 | 0 |

New regression coverage exercises environment overrides, disabled/deleted
variables, exact API key case, SSH explicit inputs, loading/missing environment
states, API probes, optimistic workspace activation and rollback, duplicate
activation prevention, dirty draft navigation, and preservation of visited
API/Database/SSH/Flow drafts while switching modules/opening variable management.

## Initial refactor verification

Local checks used Windows. [Ubuntu CI run 36711917872](https://github.com/zyqzyq/Unfour/actions/runs/36711917872)
passed for `0c39c226766867fb4ee6d2b960b3a109da183bad`, with all seven jobs
successful: frontend lint/test/build, Release contracts, Rust check/test, and
MCP contracts and tests. Native desktop visual/live-service behavior was not
manually exercised.

| Check | Result |
| --- | --- |
| `pnpm run lint` | PASS, 0 errors / 43 existing warnings |
| `pnpm run test` | PASS, 159 files / 940 tests, including the mount-retention regression |
| `pnpm run build` | PASS, existing >500 kB chunk advisory remains |
| `cargo test -p unfour-mcp` | PASS, 226 library + 4 binary + 3 integration tests |
| `cargo test -p unfour-command-bus -p unfour-flow-engine` | PASS |
| `cargo test --workspace --exclude unfour-mcp` | PASS, matching the separate CI Rust-test gate |
| `cargo check --workspace` | PASS |
| `cargo check -p unfour --features ssh-native` | PASS |
| `cargo fmt -p unfour-command-bus --check` | PASS |
| `pnpm run check:migrations` | PASS, 30 migrations |
| `pnpm run check:large-files` | PASS, 0 blocking; existing P0/P1 files remain |
| `git diff --check` | PASS |

The first sandboxed Vitest launch could not spawn esbuild (`EPERM`); the same
checks passed after approved execution outside that process sandbox. Rust
retains existing release-channel/default and feature-dependent dead-code
warnings.

## Unified ephemeral SQLite follow-up

Base: `main` at `0c39c226766867fb4ee6d2b960b3a109da183bad`. The supported
mutable MCP `StorageMode::Ephemeral` constructor still created a separate
single-connection `:memory:` pool with SQLx's default acquisition health check.
Its new regression passed once before the fix, then failed on repetition 9
with `SqliteError { code: 1, message: "no such table: workspaces" }`. This
reproduces the same cancellation race on the unified runtime, independently
of the deprecated test constructor.

`unfour_local_storage::LocalDb::connect_ephemeral()` now owns the shared pool
configuration: private `:memory:`, one connection, foreign keys enabled,
`create_if_missing(true)`, and `test_before_acquire(false)`. Both
`CommandBus::ephemeral()` and unified `StorageMode::Ephemeral` call it. The
helper does not migrate or seed; each caller retains its existing migration,
seed, and extension installation sequence. The existing MCP execution-guard
fixture also uses this helper. Default/file-backed configuration and Flow
timeout, heartbeat, MCP contracts, and error semantics are unchanged.

Coverage includes both construction paths:

- `ephemeral_database_survives_cancelled_connection_acquisition` exercises
  `CommandBus::ephemeral()` and checks schema/records after dropped acquisition.
- `mutable_mcp_ephemeral_survives_cancelled_workspace_reads` constructs
  `LocalCommandBusAdapter::from_storage_mode(StorageMode::Ephemeral)`, including
  unified migrations and extensions. It drops workspace reads at their first
  pending boundary, checks active workspace and retained records after each
  cancellation, and verifies a later workspace write succeeds. It requires
  at least one pending read to have been cancelled; no sleep, error retry, or
  relaxed assertion is used.

Post-fix stability checks passed 100/100 mutable MCP regression repetitions
(64 first-poll boundaries per run) and 10/10 complete MCP library runs at 32
threads (227 tests per run).

The following checks verify this follow-up locally on Windows. The linked
Ubuntu CI evidence above belongs to the committed base revision.

| Check | Result |
| --- | --- |
| `cargo test -p unfour-mcp` | PASS, 227 library + 4 binary + 3 integration tests |
| `cargo test --workspace --exclude unfour-mcp` | PASS, including command-bus cancellation regression and local-storage tests |
| `cargo check --workspace` | PASS |
| `cargo check -p unfour --features ssh-native` | PASS |
| `cargo fmt -p unfour-local-storage -p unfour-command-bus -p unfour-mcp --check` | PASS |
| `pnpm run test:release-env` | PASS, 73 tests |
| `pnpm run check:secrets` | PASS, 1,302 publishable files scanned |
| `pnpm run check:migrations` | PASS, 30 migrations |
| `git diff --check` | PASS |

The sandboxed secret audit initially could not spawn Git (`EPERM`); the same
audit passed with approved execution outside that process sandbox. Existing
Rust release-channel/default and feature-dependent dead-code warnings remain.

### Other private in-memory SQLite constructors

The repository audit found 20 remaining direct single-connection in-memory
constructors with the default acquisition health check. All are test fixtures;
they can have the same hazard if acquisition is cancelled, but this follow-up
does not establish that every fixture currently flakes. They remain unchanged
to keep this fix scoped.

| Crate | Test fixture paths relative to that crate |
| --- | --- |
| `local-storage` | `src/local_db_tests/mod.rs`, `src/terminal_history_tests/mod.rs`, `src/ssh_command_history_tests.rs`, `src/activity_log.rs` (test module) |
| `workspace-engine` | `src/workspace_tests/mod.rs` |
| `http-engine` | `src/api_client_tests/support.rs` |
| `ssh-engine` | `src/task_tests/support.rs`, `src/ssh_tests/support.rs`, `src/host_key_tests/mod.rs` |
| `database-engine` | `src/database_tests/support.rs`, `src/database_tests/runtime_profile.rs` |
| `unfour-cloud-sync` | `tests/worker/support/mod.rs`, `tests/ownership.rs`, `tests/outbox.rs` |
| `unfour-cloud-sync-storage` | `src/lib.rs` (test module) |
| `unfour-command-bus` | `tests/workspace_domain.rs`, `tests/ssh_task_domain.rs`, `tests/api_domain.rs`, `tests/connection_domain/support.rs`, `tests/database_script.rs` |

No other hard-coded production single-connection private `:memory:` storage
constructor was found. The command-bus Flow history migration fixture uses
`SqlitePool::connect("sqlite::memory:")` with the default connection limit;
the database-engine SQLite driver accepts `:memory:` with four connections.
Those are different configurations and are not covered by this regression.

## Further refactor candidates

High-value frontend follow-ups remain `DatabasePage.tsx` (552 function lines,
complexity 52), `ApiCollectionTree.tsx` (659 function lines, 715 lint-counted file
lines), and `TerminalPage.tsx` (577 function lines). Within Flow,
`model.ts`'s `resourceErrors` callback still has complexity 33, and `JsonField`
retains its state-in-effect warning. On the Rust side,
`crates/unfour-mcp/src/command_bus_adapter.rs` remains a 1,151-line adapter and
is a useful responsibility-based extraction candidate. These are outside this
round's edits.
