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


## Phase 2 closeout (2026-09-27, base a0d64da)

Scope is limited to complete reference validation and API environment preflight.
No persisted schema, runtime `{{...}}` resolution, Run History or Run Again changes.

- Save/run validate reference structure before capability execution: `$ref`
  objects contain exactly one string field; pointers start with `/`, use valid
  `~0`/`~1` escapes and have an `inputs`, `steps` or `probe` root. Every
  interpolation is checked, including multiple and unterminated interpolations.
- Malformed references report `FLOW_INVALID_REFERENCE`. Dominance and probe
  scope violations retain `FLOW_UNSAFE_REFERENCE`. Desktop serialized errors
  and MCP errors expose the same codes; loading legacy definitions remains allowed.
- Advanced JSON reports invalid versus unsafe references and blocks Save/Run.
- Run confirmation checks static API templates against enabled workspace keys
  plus the selected environment's enabled keys, preserving exact key case and
  workspace fallback. The authoring picker still uses the union of all environments.
- Checks cover saved URL, headers, query, body and auth, literal replacements,
  header patches and duplicate query occurrences. Replaced/removed templates
  are excluded. Dynamic references, template keys and uncertain patch targets
  are deferred to runtime. Condition/DB and SSH workspaceDefaults are unchanged.
- Missing keys display step + variable + selected environment and disable Run.

Verification for this closeout:

- PASS: Flow + locale frontend tests, 136 tests across 15 files (the final
  FlowPage selector correction was rerun separately: 35/35).
- PASS: Flow engine 8/8; CommandBus Flow tests 46/46; MCP Flow tests 9/9;
  core error tests 4/4. Fixtures cover malformed save/run rejection before run
  creation, Desktop/MCP code parity, escaped pointers, multiple interpolation,
  extra `$ref` fields, legacy safe definitions and existing API resolution.
- PASS: production build (chunk-size warning only), TypeScript, ESLint (three
  size/complexity warnings, zero errors), Rust formatting, `git diff --check`
  and large-file check (zero blocking files).
- Initial test failures were repaired: a legacy test expected malformed refs to
  bypass service save validation; UI tests needed to account for the new error
  links and use the existing canvas node selector.
- Native desktop/live remote services were not exercised in this closeout;
  interaction validation uses React/jsdom and disposable Rust fixtures.
