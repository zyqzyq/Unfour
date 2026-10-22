# SSH task sensitive source locks

Baseline: `main` at `4802b18a24d6be805e366d567aea71ad59ae2aef`.
Verified on Windows on 2026-10-22 (Asia/Shanghai).

## Root cause and repair

History recall resolved values using the saved input sources, but the dialog's
automatic sensitivity list and toggle guard independently consulted merged
current defaults. A public Environment variable could therefore hide the Secret
attribute of the Workspace variable actually selected by history. The state
toggle could remove its sensitive marker and expose the restored value.

The existing run-input model now carries `automaticSecretNames` separately from
the editable sensitive markers. Defaults initialize it from sensitive names and
the selected variables. Recall calculates it from the actual restored source,
the Secret attribute in that scope, and explicit historical secret provenance.
Both the UI lock and the state-layer toggle guard consume this same list.

- A historical Workspace Secret remains locked under a public Environment
  overlay, even if older history omitted its explicit secret marker.
- Explicit historical secret provenance remains locked if the selected variable
  disappears, is disabled/deleted, or becomes public. Missing source values stay
  empty rather than falling back to an unrelated scope.
- Editing a resolved Secret makes its execution source manual while retaining
  sensitivity provenance for that dialog scope: partial edits can retain secret
  material. Ordinary manual marking remains freely reversible and does not
  enter the automatic list.
- Selecting an environment creates fresh defaults and provenance; it also
  retains the existing preparation-generation remasking. Recall and new dialog
  preparation similarly replace provenance rather than accumulating old locks.
- Value recall, manual value suppression, execution payloads, input-version
  checks, and redaction retain their existing behavior. No UI layout, icons,
  visibility controls, localization, dependencies, Rust, schema, MCP, or Cloud
  Sync changes were made.

## Verification

| Check | Result |
| --- | --- |
| `pnpm exec vitest run packages/ssh-terminal/src packages/ui/src/secret-input.test.tsx` | PASS: 45 files, 289 tests |
| `pnpm exec playwright test ssh-task-sensitive-sources.spec.ts ssh-task-rerun.spec.ts ssh-task-first-recall.spec.ts` | PASS: 6 tests |
| `pnpm --filter @unfour/desktop exec tsc --noEmit` | PASS |
| ESLint on all five changed TypeScript files | PASS: no errors; two existing hook length/complexity warnings |
| `git diff --check` | PASS |

New model and hook regressions cover both actual variable scopes, overlays,
explicit historical manual secrets, source disappearance/disable/delete/public
changes, direct toggle calls without UI guards, manual edits retaining secret
provenance, environment resets, and source/version metadata in execution requests.
Playwright covers the shadowed, removed, and now-public Workspace cases through
real dialog interactions, keyboard lock attempts, manual source edits, independent
visibility, environment changes, and remasking. Existing rerun and first-recall
browser tests also pass. The new dialog screenshot was visually inspected.

The initial regression run failed as expected before the repair, including a
direct toggle removing the submitted sensitive marker. Its recall fixture was
also adjusted to wait for React's preparation render before calling recall.
The initial sandboxed pnpm invocation could not open its cache database; the
checks above passed with approved tool-cache and local browser/server access.

NOT VERIFIED: packaged Tauri against a real SSH host or production credentials.
The browser tests use isolated fixtures and assert that preparation starts no SSH
execution. This repair changes frontend sensitivity state only; Rust execution
and persistence were not modified or rerun.
