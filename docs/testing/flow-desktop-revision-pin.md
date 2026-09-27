# Desktop revision confirmation and conflict cache

Date: 2026-09-27. Base: `036193f`.

Desktop `runFlow(input, expectedRevision)` sends the current editor revision as
the required `expectedRevision` Tauri argument. `flow_run` delegates to
`CommandBus::run_flow_at_revision(input, Some(expected_revision))`; Run and Run
Again share this path. No persisted schema or execution engine changes.

`FLOW_CONFIRMATION_STALE` closes confirmation and asks the user to load the
latest definition and confirm again. It never automatically retries. Accepting
latest via discard updates the workspace Flow list cache; cancelling leaves it
unchanged.

## Automated verification

- PASS: targeted Vitest suite, 45 tests across `FlowPage.test.tsx` and
  `command-client/src/tauri/flow.test.ts`. Covers Run revision forwarding,
  Run Again using draft r4 with historical r3, stale confirmation recovery,
  discard cancellation, and reselecting the sidebar after accepting r4.
- PASS: command-client invokes Tauri with `expectedRevision`; a source contract
  assertion checks the required Rust argument and pinned bus delegation.
  This is not a native IPC integration test.
- PASS: `cargo test -p unfour-command-bus
  flow_revision_pin_rejects_before_remote_execution_or_history`. The r3/r4
  fixture rejects stale revision with empty history and zero test-executor
  effects across API/SSH/database steps, then executes matching r4.
- PASS: `cargo check -p unfour-app`, Desktop `tsc --noEmit`, affected ESLint,
  Rust formatting, large-file check, and `git diff --check`.
- ESLint retains the FlowPage function-size/complexity warnings; large-file
  checking reports no blocking files. No unrelated refactoring was performed.
- Initial Vitest startup was blocked by sandbox `spawn EPERM`; rerun outside
  that restriction worked. An initial test assertion syntax failure was fixed;
  the final suite passes.

## Native manual testing: NOT VERIFIED

Stop code-level UX polishing here. In a disposable workspace in real Tauri:

1. Open r3, update it to r4 from another client, then confirm Desktop Run.
   Expect the stale-confirmation message, no new history, and no external effect.
2. Load latest, confirm discard, then explicitly confirm Run again. Verify r4
   runs. Open older history and use Run Again; verify the editor revision is used.
3. Edit r3, cause a save conflict against r4, load latest, cancel discard, and
   verify the old sidebar entry and edits remain. Repeat and accept discard;
   reselect the same sidebar Flow and verify r4 remains loaded.

These native checks remain the next manual-testing phase; automated fixtures
do not establish live API/SSH/database effects or native IPC behavior.
