# Independent UI redesign — verification record

**Historical record — C2 adopted.** The workbench redesign was adopted with
the later C2 palette; temporary palette URL switches are removed. Branch status,
screenshots, and conclusions below describe their original verification stage.
Use the [canonical design system](../ui/design-system.md) for current visual
rules and the final integration record for adoption checks.

Date: November 8, 2026. Branch: `codex/ui-redesign-experiment`.
Baseline commit: `36a75353`. All changes remain uncommitted.

Design direction and component rules:
[Graphite / Jade](../ui/redesign-experiment.md).

## Branding review update

The user's branding constraint applies to the packaged **U application icon**,
not internal feature/action icons. The U assets and bundle icon configuration
were unchanged throughout the experiment, and have been checked against HEAD.
The additional four-part shell brand mark has been removed to avoid a competing
brand identity. Internal UI icons and the redesigned layouts remain in place.

The comparison viewer's `after/` images show this corrected scope. The original
first-pass screenshots are preserved in `with-experimental-icons/`, selectable
as the left-side reference; `before/` still contains the original product UI.
Screenshots from the superseded broad icon-removal attempt are not delivery
artifacts.

After this clarification, the production build (including TypeScript), 12
affected workbench browser checks, and 4 SSH/SFTP screenshot/interaction checks
passed again. The 24 delivered current screenshots were refreshed from those
runs (`test-results/ui-brand-preserved` and `ui-brand-preserved-ssh`). The
complete 54-test suite below records the earlier full experiment verification.

## Review the result

Open the [interactive before/after comparison](artifacts/ui-redesign-2026-11-08/index.html)
in a browser. Choose API, database tables, SQL, SSH or Flow; drag the divider or
show either version. The original PNGs are linked in the viewer.

| View | Before | After |
| --- | --- | --- |
| API dark | [PNG](artifacts/ui-redesign-2026-11-08/before/api-dark.png) | [PNG](artifacts/ui-redesign-2026-11-08/after/api-dark.png) |
| API light | [PNG](artifacts/ui-redesign-2026-11-08/before/api-light.png) | [PNG](artifacts/ui-redesign-2026-11-08/after/api-light.png) |
| Database table dark | [PNG](artifacts/ui-redesign-2026-11-08/before/database-table-dark.png) | [PNG](artifacts/ui-redesign-2026-11-08/after/database-table-dark.png) |
| Database table light | [PNG](artifacts/ui-redesign-2026-11-08/before/database-table-light.png) | [PNG](artifacts/ui-redesign-2026-11-08/after/database-table-light.png) |
| SQL dark | [PNG](artifacts/ui-redesign-2026-11-08/before/database-sql-dark.png) | [PNG](artifacts/ui-redesign-2026-11-08/after/database-sql-dark.png) |
| SQL light | [PNG](artifacts/ui-redesign-2026-11-08/before/database-sql-light.png) | [PNG](artifacts/ui-redesign-2026-11-08/after/database-sql-light.png) |
| SSH dark | [PNG](artifacts/ui-redesign-2026-11-08/before/ssh-dark.png) | [PNG](artifacts/ui-redesign-2026-11-08/after/ssh-dark.png) |
| SSH light | [PNG](artifacts/ui-redesign-2026-11-08/before/ssh-light.png) | [PNG](artifacts/ui-redesign-2026-11-08/after/ssh-light.png) |
| Flow editor | [PNG](artifacts/ui-redesign-2026-11-08/before/flow-canvas-inspector.png) | [PNG](artifacts/ui-redesign-2026-11-08/after/flow-canvas-inspector.png) |

Additional final screenshots:
[configured Flow dark](artifacts/ui-redesign-2026-11-08/after/flow-canvas-dark.png),
[Flow light](artifacts/ui-redesign-2026-11-08/after/flow-canvas-light.png),
[Chinese settings](artifacts/ui-redesign-2026-11-08/after/settings-chinese-dark.png),
[compact API](artifacts/ui-redesign-2026-11-08/after/api-compact.png),
[compact Flow](artifacts/ui-redesign-2026-11-08/after/flow-compact.png),
[save dialog validation](artifacts/ui-redesign-2026-11-08/after/api-save-validation-compact.png),
[SSH task](artifacts/ui-redesign-2026-11-08/after/ssh-task-editor.png),
[split terminal](artifacts/ui-redesign-2026-11-08/after/ssh-split-dark.png),
[SFTP](artifacts/ui-redesign-2026-11-08/after/sftp-workspace.png).

The API/database/terminal comparison pairs use the same browser fixture and
1440×900 viewport. The Flow comparison uses the same 1280×720 authoring test;
its deliberately incomplete definition retains validation feedback. Supplemental
Flow screenshots show a configured resource and valid boolean branch. Timing,
cursor/hover state and mock PTY resize text can vary between captures.

## Final verification

| Check | Result | Evidence |
| --- | --- | --- |
| Frontend production build, including `tsc` | PASS | `pnpm run build`; Vite 2676 modules, final build 6.55s |
| Complete browser regression suite | PASS | 54/54 Chromium tests, 51.0s; final source after layout repairs |
| Shared UI and app shell unit tests | PASS | 35 files, 147 tests |
| API unit tests | PASS | 28 files, 270 tests; final save-dialog tests rerun 29/29 |
| Affected Database unit tests | PASS | 7 files, 54 tests; final dialog checks rerun |
| Flow and SSH component unit tests | PASS | 39 files, 348 tests, including affected-suite reruns after visual assertion/layout adjustments |
| Final editor-theme token alignment | PASS | 2/2 tests |
| ESLint, changed TypeScript/TSX | PASS | No errors; four existing function-length warnings |
| Shared-token guard | PASS | 110 canonical tokens; no host token redefinitions |
| Large-file guard | PASS | Zero blocking files; existing repository size warnings retained |
| `git diff --check` | PASS | No whitespace errors |
| Native Tauri/real database/SSH/SFTP integration | NOT RUN | This experiment changes frontend presentation; browser fixtures do not verify native execution |

Affected unit suites cover 819 tests without counting reruns twice. They are
not a claim that the entire repository unit or Rust suite was run.

The normal browser command is `pnpm exec playwright test`. This session used
the equivalent installed CLI with an ignored temporary Playwright config:

```text
node node_modules/@playwright/test/cli.js test --config=test-results/ui-redesign.config.ts --project=chromium --workers=3 --output=test-results/ui-redesign-final
```

That config inherits the repository configuration, starts Vite on isolated
`127.0.0.1:1426`, and points to the existing smoke-test directory. Port 1420 was
already occupied and was not stopped. `npm_config_manage_package_manager_versions=false`
was used for build invocations to avoid pnpm's sandbox-incompatible self-bootstrap;
package manifests and lockfiles were not changed. Unit tests used the installed
Vitest CLI; sandbox runs used an ignored workspace-local temporary directory.

## Regression coverage and repairs

- Existing browser tests cover API layout/cURL import, offline Monaco, database
  trees and table browsing, sidebar search, persistent module mounts, command
  palette focus, workspace import, environment draft guards, Flow authoring/run
  views, SFTP preview, SSH task recall and sensitive-input behavior.
- New `redesign-workbench.spec.ts` covers light/dark Flow editing and saved
  revisions, real control actionability at 1100×720 and 960×600, graph/inspector
  non-overlap with Advanced expanded, Chinese locale and dark/system preference
  persistence across reloads.
- New `api-save-layout.spec.ts` reproduces long collection lists plus a duplicate
  name error at 960×600. It checks footer geometry before scrolling can conceal
  a clipping defect, then verifies a corrected name saves successfully.
- Visual-surface tests retain text contrast checks, Monaco theme updates, table
  selection, keyboard focus, visible terminals and page overflow assertions.
  Palette and sidebar/search geometry assertions were updated to the intentionally
  redesigned values; behavior assertions were not removed.
- Review fixed clipped SSH history rows at a 140px panel height, terminal split
  action overlap with the prompt, wrapped Flow controls, low light-theme edge
  contrast, stacked canvas overflow, and the API save footer clipping.

Intermediate failures were resolved before final verification: sandbox launch
and temporary-file permissions, occupied development port, one HMR-interrupted
cURL test, old exact appearance assertions, and a new test's missing punctuation.
The first integrated browser run was 49/53; after correcting the two old height
assertions and two test selectors, the final expanded suite was 54/54.

## Remaining tradeoffs and limits

- Larger labels/toolbars reduce simultaneous data density. Compact API and Flow
  workbenches use more vertical scrolling; controls remain reachable in tested
  sizes. This is a desktop design, not a new mobile layout.
- Vite still reports a chunk above 500kB (largest about 790kB). Dependency or
  bundling refactoring was outside this visual experiment.
- Existing long-function lint warnings remain in SQL/table/SSH components.
- Native window dragging, OS-specific fonts/DPI, real PTY sessions, database
  drivers and remote requests need native acceptance testing. Screenshots use
  isolated browser data and intercepted HTTP responses, with no real credentials.
- No backend, service, data model, command contracts, dependencies, workspace
  persistence or cloud-sync logic was modified. No commit, push or deployment
  was performed.
