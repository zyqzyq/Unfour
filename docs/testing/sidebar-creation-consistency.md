# Sidebar creation and interaction consistency

Date: 2026-10-12.

Reviewed resource creation and sidebar lists in Flow, API collections/request
tabs, SSH connections/tasks, Database connections, and variable environments.
The existing design and interaction documents remain the design authority.

## Findings and changes

| Finding | Change |
| --- | --- |
| Flow creation occupied a separate text-button row, unlike connection/task creation | Move creation into the section header using the shared compact `IconButton` and Lucide `Plus`; retain the localized name, tooltip, and existing draft creation handler |
| Flow used standalone rows with no tree keyboard navigation or long-name handling | Reuse `TreeView` for focus, arrow/Home/End navigation, Enter/Space selection, truncation, full-name tooltip, and selected state |
| Flow creation/selection silently did nothing during a pending mutation | Expose the existing busy gate through disabled buttons/rows and section busy state; retain the dirty-draft discard confirmation |
| Flow's empty list had no explanation and loading/error presentation differed from shared states | Use shared empty/loading/error components, keep retry near the error, and retain cached rows when a list refresh fails |
| API collection creation/import used bespoke 24px controls without shared focus styling | Use shared compact 28px icon buttons with the existing handlers and pending gates |
| New API request tabs used a literal text plus | Use the same Lucide `Plus` icon within the existing tab-bar control |
| Variable environment creation used a text button in the resource header | Use the shared compact plus control with the localized full accessible name |
| SSH connection creation scrolled out of view with a long list | Keep the header fixed and scroll the connection tree independently, matching Flow, Database, and Task lists |

The sidebar creation convention is recorded in
[`interaction-guidelines.md`](../ui/interaction-guidelines.md#trees).
Changes are limited to frontend presentation and selection feedback; no
dependencies, package ownership, command protocols, persistence, or execution
policies changed. Empty-state creation calls to action remain available where
they already exist.

## Verification

- PASS: affected Vitest suites — 5 files, 96 tests. Includes Flow keyboard
  selection, dirty-draft Cancel/Discard, disabled creation/selection during Save,
  API collection creation/import, request tabs, environment creation, and i18n.
- PASS: `sidebar-consistency.spec.ts` — 3 Chromium scenarios. English/light and
  Chinese/dark at 960×600 check compact creation controls across modules and
  creation of an unsaved Flow. A 50-row fixture checks keyboard navigation,
  long-name truncation, and fixed creation headers for Flow and SSH.
- PASS: `flow-authoring.spec.ts` — 1 Chromium scenario for new Flow, API/DB/SSH
  authoring and Save. Browser fixtures perform no native or external execution.
- PASS: desktop TypeScript check and Vite production build. Vite retains its
  existing large-chunk advisory.
- PASS: changed TS/TSX ESLint — no errors; the existing SSH connection-tree
  function-size warning remains.
- PASS: large-file check — no blocking files; existing size advisories remain.
- PASS: `git diff --check`.
- Visual inspection: English/light and Chinese/dark Flow, variable-environment
  sidebar, and scrolled Flow/SSH list screenshots under `test-results/`.
- NOT VERIFIED: native Tauri rendering and live SSH/database/API execution.
  This change concerns frontend controls and uses disposable browser fixtures.

The pnpm launcher initially failed to open its local database (FAIL before
tests ran). Direct local Node entry points were used. Sandbox runs of Vitest
and TypeScript initially failed to resolve local/dependency paths (FAIL);
approved execution outside the sandbox passed. The first Chromium run found
an ambiguous test selector matching both the SSH sidebar and empty-state
creation buttons (FAIL); scoping the selector to the sidebar passed.
