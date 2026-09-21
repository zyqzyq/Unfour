# Flow Canvas PoC

Implemented on the local `main` checkout, 2026-09-21. This is an opt-in authoring
surface above the existing Flow editor, not a replacement editor or engine.
Open Flow, select/create a definition, and toggle **Canvas PoC**.

## Choice and scope

`@xyflow/react` 12.11.6 (MIT) supplies React nodes, named handles, connections,
dragging, selection, zoom and pan. Its controlled graph and
[`isValidConnection`](https://reactflow.dev/examples/interaction/validation)
fit the existing TypeScript/React frontend. No new workflow engine, Rust
dependency, command, migration or backend model is introduced. A lower-level
drawing library would require implementing the graph interactions ourselves;
there is no observed reason to take that route for this PoC.

The dependency and its transitive packages are recorded in the workspace
lockfile. UI code belongs entirely to `packages/flow`; the only shared UI
change adds English/Chinese messages to the existing i18n resources.

## Mapping contract

- Virtual `$start` points to the first array entry; virtual `$end` is terminal.
  Neither is serialized as an engine step. Start cannot be rewired.
- API / Database / SSH actions, Condition, Wait Until, Wait and legacy Poll
  retain their original step payloads. Canvas nodes show only names/types;
  resource arguments and predicates remain in the existing editor.
- An ordinary output maps to `next`. Null means fallthrough to the following
  array entry, or End at the end of the array, and is drawn dashed. An unchanged
  round trip preserves null versus explicit destinations exactly.
- Condition has separate `true`/`false` handles mapped to `ifTrue`/`ifFalse`,
  even when both destinations are the same. Its unused `next` is preserved.
- Connecting an already connected output replaces that output. Multiple
  incoming edges are allowed; multiple outgoing edges on one port are not.
- Targets must be later in the existing `steps` array or `$end`. This is
  stricter than merely forbidding cycles and matches Rust's validator.
  Screen positions never determine execution order.
- Reverse conversion validates ports/targets and preserves definition metadata,
  inputs and step configuration from the authoritative definition. Node additions
  use the existing `newStep` factory and append to the definition before projection.
- Removing an edge means explicit termination, not automatic fallthrough.
  Deleting a node redirects incoming links to End; deleting the first node
  makes the next remaining step the entry. The UI retains at least one step
  and limits additions to the engine's 100-step limit.
- Adding after a final implicit `next: null` extends that sequence, as in the
  current editor; an explicit `$end` continues to terminate.

There is no structural conflict for existing definitions. This engine models
ordered, single-path execution with conditional jumps, not arbitrary parallel
DAG scheduling. A visually free-form graph must continue to enforce that contract.

## Layout and execution isolation

Positions, measured node sizes, selection and viewport are component state only.
Layout resets when the panel closes or another Flow is selected. Moving a node
does not dirty the definition or create a revision. A future persisted layout
should be a separately versioned UI document keyed by `(workspace_id, flow_id)`
and stable step IDs, with its own stale-node cleanup; it must not be added to the
execution definition or resource snapshot.

Save/run still use the existing FlowPage handlers and command-client. Typed
inputs, revision/resource snapshots, run history, secret/redaction policies and
cancel/timeout behavior have not been changed. The PoC does not execute requests
directly. Existing engine validation remains authoritative at save/run time.

## Verification

- PASS: Flow tests plus shared i18n test: 7 files / 43 tests. The new adapter
  suite covers all supported kinds, round trip, true/false destinations, null
  fallthrough, `$end`, metadata preservation, illegal/backward/self connections,
  duplicate outputs, deletion and order independence from visual positions.
- PASS: production frontend build (TypeScript and Vite).
- PASS: changed TypeScript ESLint, with the existing FlowPage large-function
  warning; Vite also reports its large-chunk advisory.
- PASS: local browser smoke test using an unsaved draft: open panel; add
  Condition and Wait; drag both; connect true to Wait and observe the original
  editor's true destination update; reject a backward false connection; select
  and delete Wait and observe true return to End; pan, fit view and zoom controls.
  Browser inspection found and fixed controlled node measurement handling and
  End overlapping newly appended nodes.
- Native Tauri WebView, real API/DB/SSH execution and native cancel/timeout were
  not exercised in this frontend PoC verification.

## Before replacing the editor

Recommended to continue with XYFlow, but retain the current editor until:

1. A selected-node inspector reuses the current field editors and validity
   tracking, so users need not find the matching form below the Canvas.
2. Step-order editing has an explicit policy. Arbitrary reordering must check
   jump targets and expression dependencies rather than silently topologically
   sorting definitions or interpreting node positions as execution order.
3. Node deletion reports references such as `/steps/deletedId/...`; this PoC
   repairs control-flow destinations only and never rewrites expressions.
4. Layout persistence, narrow-window sizing, full accessibility/localized
   library announcements and native WebView interaction coverage are finalized.
5. Save/reload/run integration and revision handling receive dedicated Canvas
   end-to-end coverage with disposable native resources.

Undo/redo, automatic layout, complex shortcuts and execution overlays are outside
this PoC. No commit or publish operation is included.
