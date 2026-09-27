# Flow UX phase 2: variables and control flow

Verification date: 2026-09-27.

## Compatibility contract

- No persisted Flow schema, operator enum, action execution, or runtime expression
  semantics changed. No new dependencies. Run History / Run Again are out of scope.
- `VariablePicker` is shared by typed values and action text insertion. It groups
  Flow inputs, workspace/environment templates, guaranteed upstream outputs, and
  the current Poll / Wait Until probe. Nested paths are entered as field names,
  not JSON Pointers; existing pointer escaping is retained.
- API workspace/environment choices insert existing `{{key}}` templates. They
  never create `/environment` Flow references or copy variable values into a
  definition. Available keys come from enabled workspace/environment metadata;
  the effective environment is still selected for the entire Flow at run time.
  SSH continues using its existing same-name workspace defaults. Conditions and
  database arguments do not gain environment-variable execution capabilities.
- Condition, Poll and Wait Until share the predicate adapter/editor. `contains`
  is array membership: UI `array contains item` encodes as wire
  `{left: item, op: "in", right: array}`. True/false shortcuts encode as `eq`.
  Literal values and variable operands remain editable, including legacy inputs.
- **Semantic limit:** `exists` / `not exists` are disabled and explained. Missing
  references still raise `FLOW_MISSING_REFERENCE`; comparing against null is not
  an existence test. String substring matching is also not added. Supporting
  these operators requires a separately authorized runtime/schema change.
- Wait Until shows probe resource, Until condition, interval, timeout and shared
  environment context. Existing probe action parameters remain in a collapsible
  section. Node summaries use resource names and field paths instead of ref JSON.

## Reference safety

`packages/flow/src/referenceGraph.ts` and
`crates/flow-engine/src/reference_graph.rs` compute guaranteed predecessors from
the engine's actual forward edges. Non-condition nodes follow explicit `next`
or array fallthrough; conditions follow `ifTrue` and `ifFalse` and ignore `next`.
Incoming reachable paths are intersected. Branch predicates are not evaluated
statically. End paths and unreachable predecessors do not pollute joins.

The picker uses this result, including after route edits. Authoring diagnostics
check nested refs and text interpolations in action arguments and predicates;
self, future, missing and branch-only outputs are rejected. Probe references
are accepted only in the owning Poll / Wait Until predicates, never its action
arguments. Output field existence/types still depend on runtime data.

The shared Flow service validates on save and before starting execution, so
Desktop and MCP reject unsafe definitions before effects. The stable error is
`FLOW_UNSAFE_REFERENCE`, explicitly mapped by the MCP adapter. Loading/listing
legacy definitions remains possible for repair; definitions with unsafe paths
must be repaired before saving or running. Safe old definitions retain their
wire shape, including legacy Poll, implicit next and escaped pointers.

## Verification

- PASS: Flow frontend and shared locale tests (129 tests), including branch
  joins/jumps/nesting/unreachable nodes, route edits, predicate round trips,
  variable grouping/insertion, old Poll load/edit/save, confirmation context and
  React interactions.
- PASS: `cargo test -p unfour-flow-engine` (7 tests).
- PASS: `cargo test -p unfour-command-bus flow` (43 tests). Includes an old
  definition inserted into disposable SQLite: load, refuse save/run, repair,
  save and successfully run. Existing missing-input runtime failure is retained.
- PASS: `cargo test -p unfour-mcp flow` (8 tests), including matching unsafe
  reference rejection through Desktop CommandBus and MCP save.
- PASS: Chromium authoring smoke test `flow-control-authoring.spec.ts`, using
  disposable browser fixtures with no external requests. Screenshots generated
  at `test-results/flow-control-wait-until.png` and
  `test-results/flow-control-condition.png`; inspected for layout.
- PASS: frontend production build, TypeScript, ESLint (warnings only), Rust
  formatting, `git diff --check`, and large-file check (no blocking files).

Browser verification covers authoring only; it does not claim live remote
API/SSH/database service verification. Runtime coverage above uses the existing
Flow engine and command-bus regression fixtures.
