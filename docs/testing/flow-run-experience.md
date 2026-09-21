# Flow Run and History experience

Date: 2026-09-21. Frontend-only follow-up to [Flow Canvas](flow-canvas.md).

## Interaction contract

- Save / Run / Run history remain in the Flow header. Run opens a shared dialog
  containing environment selection, typed and secret inputs, Advanced JSON,
  manual secret names, masked confirmation context and the existing effects
  notice. Runtime validation disables submission, not access to the input form.
- History is a shared popover, newest first, with status, start time and elapsed
  duration when both timestamps are available. Opening it refreshes history.
- Selecting or starting a run enters an explicit read-only viewing mode with
  time, status, revision, Back to editor and cancellation for active runs.
  Graph mutations and definition editors are unavailable in this mode.
- Canvas projects existing step statuses only for a clean matching revision and
  Flow identity. Dirty and mismatched definitions show an explanation instead.
- The right Inspector reuses RunView for the selected node's input, output,
  status, timing, attempts, latest error and Wait Until next-check data. For an
  incompatible definition, a recorded-node selector also exposes nodes removed
  from the current Canvas. Returning to editing retains the definition draft.
- The permanent bottom Run inputs & history panel, history Select and full
  bottom RunView are removed. No bottom Console is introduced.
- Existing RunInputs, JsonField, model validation/masking, StepEditor,
  InputEditor, Canvas graph conversion and command-client calls are reused.
  Engine semantics, persisted contracts and dependencies are unchanged.

## Verification

- PASS: `pnpm exec vitest run packages/flow/src packages/ui/src/i18n.test.ts`
  (9 files, 80 tests). Covers dialog validation, masked context, environment,
  effects confirmation, cancellation, history selection, revision/dirty guards,
  node Inspector, removed historical nodes and Back to editor.
- PASS: Chromium Flow Canvas and Run experience smoke tests (3 tests); the Run
  test was also rerun after keyboard-focus restoration. Screenshots of the
  dialog, History and Run Inspector were inspected in `test-results/`.
- PASS: `pnpm run build` (TypeScript and production Vite build). Vite reports
  the existing large-chunk advisory.
- PASS: changed frontend ESLint, with FlowPage function-size and complexity
  warnings; `git diff --check`; large-file gate (no blocking violations).
- Earlier checks were blocked by missing local dependencies and sandbox process
  permissions; installing the locked dependencies and approved test execution
  resolved those blockers. Intermediate interaction-test failures were corrected
  for the dialog flow and asynchronous focus restoration.

Browser run records are isolated fixtures, not evidence of Rust execution.
Native Tauri execution and live API/SSH/database effects were NOT VERIFIED in
this frontend-only change. Existing graph layout/undo and variable-picker limits
remain as documented in Flow Canvas; no new blocking issue was found in the
tested Run/History interactions.
