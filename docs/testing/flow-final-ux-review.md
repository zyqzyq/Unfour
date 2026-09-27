# Flow final code-level UX review

Date: 2026-09-27. Base: `bced151`, plus this uncommitted review patch.

This review covers creation → API/SSH/DB configuration → references →
Condition/Wait Until → environment/inputs/effects → save → run → monitoring →
failure recovery → history. It is preparation for human experience testing,
not certification of native execution or live service behavior.

## Findings and fixes

No new P1 defect was confirmed within the inspected code and tested frontend
paths. The following P2 issues were fixed:

| Issue | User impact | Change |
| --- | --- | --- |
| Resource errors disappeared with a closed Inspector | Run was disabled without a visible cause or route to repair | Errors remain in a bounded, scrollable area with step links; dirty Run has a visible save prerequisite |
| Time fields relied on HTML bounds | Save accepted invalid timing and deferred explanation to backend errors | Validate integer timeout, interval, attempts, and wait/timeout relationship against existing Rust rules; validate UTF-8 Flow name limit; link timing errors to their step |
| Revision conflict exposed only a raw error | Users had no explicit recovery path | Explain conflict, preserve draft, fetch latest through command-client, require the existing discard confirmation before replacing draft |
| Detail failure still displayed Loading; errors lacked retry | Failure looked like a permanent pending operation | Distinct failure state and retries for Flow list, resources, history, run detail, and runtime environment/variable prerequisites; explain a removed selected environment |
| Run monitoring and failure repair required manual Canvas hunting | Hard to locate an offscreen running/failed node and return to its editor | Status-labelled locate action, center selected run node, always offer recorded-node selector with statuses, move step error before payload, add Edit this step and explain removed historical steps |
| Completed run had no direct restart path | Users had to discover the editor round trip | Run again opens existing confirmation for the current saved definition; retains current session inputs and does not restore masked historical values; explains dirty/invalid blocking |
| Fixed Inspector and non-wrapping toolbar squeezed narrow layouts | Canvas/actions could become unusable | Wrap toolbar; below 680px of Flow content width, stack Canvas and Inspector; wrap long diagnostics and locator labels |

Also corrected the database selector's SSH-specific accessible name and the
unselected/unavailable SSH task's misleading Loading text. All new UI copy uses
the shared English/Chinese locale files.

## Reviewed behavior retained

- Canvas insertion, branch connections, removal impact/reference protection,
  Inspector editing, and temporary presentation layout remain unchanged.
- API overrides, SSH detected inputs/defaults, DB single-statement editing,
  Variable Picker grouping/nested paths, Condition predicates and Wait Until
  probe/condition controls retain their existing implementations.
- Branch-safe reference validation, Action Authoring payload compatibility and
  secret provenance were not redesigned. Existing regression suites were run.
- Environment selection remains run-scoped. Required/type validation, masking,
  manual secret names and the existing effects confirmation remain in place.
- Run History retains newest-first status/time/duration summaries. Detailed
  payloads and attempts remain in Run Detail. Dirty/revision mismatches still
  suppress Canvas status projection rather than attributing history to a
  different definition; recorded nodes remain accessible.
- No runtime, command schema, persisted schema, dependency, or package-boundary
  changes. No parallel/loop/subflow/scheduler/agent additions.

## Verification

- PASS: `pnpm exec vitest run packages/flow/src packages/ui/src/i18n.test.ts`
  — 16 files, 144 tests. New regressions cover closed-Inspector errors, timing
  repair, conflict cancellation/confirmation, run-detail retry, failed-step
  repair and Run again without historical input replay.
- PASS: `pnpm exec playwright test apps/desktop/tests/smoke/flow-run-experience.spec.ts apps/desktop/tests/smoke/flow-canvas.spec.ts apps/desktop/tests/smoke/flow-authoring.spec.ts apps/desktop/tests/smoke/flow-control-authoring.spec.ts`
  — 7 Chromium tests. Includes 900×760 English/Chinese layouts, long node names
  and diagnostics, failed-step navigation and confirmation before Run again.
- PASS: `pnpm run build` — TypeScript and production Vite build; large-chunk
  advisory remains.
- PASS: changed TypeScript/TSX ESLint, with size/complexity warnings; large-file
  gate and `git diff --check` (no blocking violations).
- Visual inspection: `test-results/flow-run-inspector.png`,
  `flow-run-narrow-en.png`, and `flow-run-narrow-zh-CN.png` were viewed.
- Initial test startup hit sandbox `spawn EPERM`; approved execution resolved
  it. Intermediate unit-test failures from added error metadata/multiple
  actionable errors were corrected. The old Canvas smoke test also had an
  obsolete variable option label and ambiguous text-based node selector;
  updated it to current option text and exact accessible node identity. All
  listed final runs passed.

## Human validation still required

Use disposable test resources; browser records above are isolated fixtures.

1. Native Tauri: API DNS/TLS/timeout and HTTP error responses; SSH auth/host-key,
   disconnect and nonzero task outcome; DB permissions/query/connection errors.
   Confirm displayed reasons match the actual driver outcome. An HTTP error
   response must continue to follow existing runtime semantics.
2. A multi-step live run: locate running step, pan away and locate again, inspect
   another step without polling stealing selection, cancel during active work,
   and inspect interrupted/cancelled history after restart.
3. Wait Until: success, failure predicate, transient retry, timeout and attempt
   cap; check latest result/error, attempts and next-check timing.
4. Environment changes/removal between authoring and running; SSH workspace
   defaults, API variables, required and secret inputs; confirm effects and
   targets before executing. Historical masked values must never be replayed.
5. Concurrent native/MCP save: keep local edits on conflict, cancel reload,
   then explicitly discard to latest; inspect history from an older revision
   including a removed step, and rerun after saving a repaired definition.
6. Native WebView with Chinese/English, 125–200% Windows scaling, smaller usable
   content areas, keyboard focus, many steps/errors and large result payloads.

Native Tauri/live API/SSH/DB execution and Rust test suites were NOT RUN in this
frontend-only patch. These remain human integration checks, not frontend PASS
claims.
