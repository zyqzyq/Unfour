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

The fix applies that option only to the ephemeral command bus. Command-bus unit
fixtures now use the same constructor. File-backed pools, Flow execution,
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

## Verification

Local checks use Windows; the GitHub Ubuntu runner has not been rerun with
this change. Native desktop visual/live-service behavior was not
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

## Further refactor candidates

High-value frontend follow-ups remain `DatabasePage.tsx` (552 function lines,
complexity 52), `ApiCollectionTree.tsx` (659 function lines, 715 lint-counted file
lines), and `TerminalPage.tsx` (577 function lines). Within Flow,
`model.ts`'s `resourceErrors` callback still has complexity 33, and `JsonField`
retains its state-in-effect warning. On the Rust side,
`crates/unfour-mcp/src/command_bus_adapter.rs` remains a 1,151-line adapter and
is a useful responsibility-based extraction candidate. These are outside this
round's edits.
