# Flow Canvas authoring

Updated 2026-09-21. The former opt-in PoC is now the default authoring surface.

## UI and compatibility contract

- Canvas is the main editor; Start configures typed inputs, and selecting an
  action, Condition, Wait Until, legacy Poll or Wait opens the shared StepEditor
  in the right Inspector. End explains path termination. Inspector and the
  bottom run-input/history panel can be collapsed.
- API URL/body and key/value header/query rows, DB parameters, SSH task inputs,
  and predicate operands use structured controls. Advanced JSON remains available
  for existing/custom values. Switching nodes retains incomplete editor drafts.
- Variable Picker offers declared inputs, earlier step outputs and predicate-only
  probe results. It emits the existing JSON Pointer $ref object, escapes pointer
  segments, and distinguishes legacy Poll output from Wait Until result wrappers.
- Start/End remain virtual. Array order and forward-only connections remain the
  execution contract. Visual positions never change step order or dirty a Flow.
  Layout is transient component state, separate from execution definitions.
- Click the lightweight + on a connection and choose API / DB / SSH / Condition /
  Wait Until / Wait. Start-to-first-step supports insertion too. Condition ports
  have separate controls near their source, including when both target End.
  There is no toolbar Add step; the connection selector plus step type remain in
  Advanced as a fallback. Only that output is redirected through
  the inserted node. All other ports retain their old destinations, including
  implicit fallthrough. A new Condition sends both branches to the original
  destination until configured otherwise.
- Deletion shows affected incoming paths. Incoming paths become End; deleting
  the first step promotes the next entry. Deletion is blocked while another step
  references its outputs, including nested $ref and text interpolation references.
  At least one step is retained. Graph edits enforce the engine's 100-step limit.
- Graph round trips preserve definition metadata, explicit/implicit/absent next,
  Condition's unused next, legacy Poll shape and opaque extension data. Loading
  and saving does not convert old steps to new kinds or add layout fields.
- Matching saved-run revisions project the existing RunView step statuses onto
  nodes. Editing hides stale statuses. RunView remains the source of detailed
  attempts, output, errors and cancellation.
- Manual secret names live in Advanced and are checked against resolved runtime
  inputs (including defaults). Only manual names are sent by the frontend; Rust
  validates them separately and merges schema secrets before persistence, including
  rejected runs. If runtime input validation fails, raw inputs are omitted entirely:
  an unknown manual name cannot establish which innocuously named value is secret.
  Optional absent schema secrets are not treated as manual errors.
- API multipart preflight uses multipart-form-data; Rust uses MULTIPART_BODY_KIND.
  No models, command contracts, migrations, dependencies or scheduling semantics
  were added.

- New failure conditions start with an unconfigured operand and cannot be saved
  or run until completed (or removed). The in operator accepts a literal JSON
  array or a configured `$ref` (the engine resolves the ref, then requires an
  array). Empty `$ref` values and non-array literals stay invalid in structured
  and Advanced editors. Page-level errors link to invalid editors even while
  their Inspector is hidden.
- Chinese Condition ports read 满足 / 不满足; Inspector branches read 满足时 /
  不满足时. Selected failed nodes retain a selection outline and failure border.

## Verification

Commands run from the repository root unless noted. Direct local Node entry points
were used because this machine's pnpm launcher attempted an inaccessible store DB.

- PASS: Flow/i18n Vitest suite (73 tests at this checkpoint).
- PASS: Chromium flow-canvas smoke: inputs, Inspector, refs, Condition insertion,
  save, deletion impact, referenced-node deletion refusal/cancel, drag isolation,
  and independent ordinary / True / False edge insertion (2 browser tests).
- PASS: desktop TypeScript check and Vite production build.
- PASS: changed frontend ESLint, with FlowPage size/complexity warnings.
- PASS: unfour-command-bus Flow tests (18), including real local HTTP/SQLite tests,
  canonical multipart rejection and manual-secret/runtime-name validation.
- PASS: Flow Engine tests (2).
- PASS: Cargo formatting, git diff whitespace check and large-file gate.
- Browser screenshots inspected at test-results/flow-canvas-inspector.png and
  test-results/flow-canvas-branches.png (generated,
  not committed). Native desktop authoring and live SSH were not verified here.

## Remaining UX limits

Layout is not persisted or automatically routed; large graphs still need zoom/pan.
Picker candidates are earlier steps, not a proof of branch reachability. Dynamic
response schemas are not inferred; nested paths use dot-separated fields, with
Advanced JSON for literal dots/complex values. There is no undo/redo or automatic
reference rewriting when renaming inputs. These do not change engine semantics.
