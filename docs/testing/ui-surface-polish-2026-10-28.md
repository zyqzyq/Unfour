# Scoped UI surface polish — 2026-10-28

Base: local `main` at `d0a97a60` (clean working tree before changes).
Style: existing Catppuccin Latte / Mocha. No business logic, keyboard bindings,
data structures, dependencies, density settings, or feature ownership changed.

## Changes and intent

| Files / area | Visual change |
| --- | --- |
| `packages/ui/src/styles/{tokens,theme,surfaces}.css`, `shell.tsx`; `packages/app-shell/src/styles/host.css` | Explicit chrome token: Mocha Crust → existing Mantle sidebar → Base content. Remove the host status-bar override that previously made it brighter than the content. Sidebar remains `surface`; align its legacy alias. |
| `packages/ui/src/tabs.tsx`; Database `DatabaseWorkspace.tsx`, `QueryResultPanel.tsx`; SSH `TerminalWorkspace.tsx` | Active tab matches the immediately adjoining toolbar/panel, covers the separating lower border, and shows keyboard focus. Terminal tabs retain readable terminal foregrounds in light mode. API request tabs already match their request toolbar and retain their styling. |
| Database `TableDataGrid.tsx`, `SqlEditorTab.tsx`; API `ResponseTabs.tsx`, `response-tab-views.tsx`, `RequestScriptEditors.tsx` | White light-mode data canvas; Mocha Base dark canvas. Table headers/toolbars keep their existing surfaces. No global table background override. |
| `packages/ui/src/editor-theme.ts`, `index.ts`; Database `sql-editor-theme.ts`; API `RequestParamsTabs.tsx`, `RequestScriptEditors.tsx`, `response-tab-views.tsx` | Share existing Monaco themes across API and SQL. Register API themes before mounting, fixing the initial dark API response appearing white before SQL was opened. Align light canvas and improve comment/line-number contrast. |
| UI `data-table.tsx`, `shell-sidebar.tsx`; API `ApiCollectionTree.tsx`, `ApiHistoryTree.tsx`, `ApiRequestBar.tsx`, `KeyValueEditor.tsx` plus the editor/response files above; Database `DatabaseSidebar.tsx`, `TableDataTab.tsx` plus the grid/editor/result files above; SSH `TerminalCommandSuggestions.tsx` | Selected readable 10/11px metadata now uses `text-muted` and an 11px caption token. Decorative icons and disabled details are not globally replaced. |
| `docs/ui/design-system.md`, `packages/ui/src/editor-theme.test.ts`, `apps/desktop/tests/smoke/visual-surfaces.spec.ts` | Document semantic roles and add theme synchronization/rendered regression coverage. |

Hover, selection and border palettes are retained. The rendered browser check
verifies that cell selection survives hover, the tab matches its next toolbar,
and keyboard focus is visible. No new shadow system is introduced.

## Verification

| Check | Result |
| --- | --- |
| `pnpm run build` | PASS: TypeScript and Vite; existing >500 kB chunk warning. |
| `pnpm run check:tokens` | PASS: 110 shared tokens, no host redefinitions. |
| ESLint on all changed TS/TSX files | PASS: zero errors; existing function-length warnings in `SqlEditorTab` and `TableDataGrid`. |
| Targeted Vitest run | PASS: 13 files / 94 tests. |
| `visual-surfaces.spec.ts` | PASS: light and dark initial rendering; API before Database; API/SQL live theme switching; table canvas; tab/toolbar match; sampled caption and terminal-tab contrast >=4.5:1; hover/selection; keyboard focus. |
| Existing `desktop-ux.spec.ts`, `database-tree.spec.ts`, `sidebar-consistency.spec.ts` | PASS: 8 tests, including 960×600 and 1440×900 layout, English/Chinese sidebar checks, navigation and focus retention. |
| `git diff --check` | PASS. |
| Initial test harness / lint invocation | FAIL, resolved: added the missing HTTP fixture interception; corrected text/Settings locators and used real keyboard navigation for `focus-visible`; corrected PowerShell ESLint argument forwarding. Final reruns pass. |
| Native Tauri, real database/SSH services | NOT RUN: screenshots and checks use isolated browser mocks only. |

The targeted Vitest files are UI `editor-theme`, `tabs`, `shell`, `data-table`;
Database `SqlEditorTab`, `QueryResultPanel`, `TableDataGrid`, `TableDataTab`;
API `ResponseTabs`, `RequestParamsTabs`, `ApiHistoryTree`, `ApiCollectionTree`;
SSH `TerminalWorkspace` (their existing `.test.ts` / `.test.tsx` files).

Reproduce the rendered check from the repository root:

```powershell
pnpm exec playwright test apps/desktop/tests/smoke/visual-surfaces.spec.ts --output=test-results/visual-after --workers=1
```

## Screenshot evidence

Captured and inspected actual Chromium renders at 1440×900, English locale,
in both themes: API response, Database table, SQL editor, SSH session. The
baseline was captured before production edits. Screenshot images are unedited;
clock, request timing and cursor state can differ between captures.

Local generated artifacts (intentionally ignored, not committed):

- `test-results/visual-comparison.html`: standalone before/after gallery with
  all 16 images embedded, so it can be copied independently of the PNG folders.
- `test-results/visual-before/visual-surfaces-{light,dark}-data-canvases-and-workbench-surfaces-chromium/`:
  baseline PNGs from `d0a97a60`.
- Corresponding `test-results/visual-after/` directories: final PNGs, plus
  `database-focus-selected-{light,dark}.png` showing focus/selection.

## Remaining manual review and impact

- Review native WebView2/macOS/Linux rendering, OS scaling (125–200%), and
  font rendering; browser checks are not native desktop validation.
- Review tab overflow/scrollbars, long translated metadata, terminal split/SFTP
  layouts, and less common empty/error states with representative real content.
- Shared shell/status-bar/Tabs changes also affect other consumers (including
  SSH Tasks and workspace/settings surfaces); they are not all screenshot-audited.
- Some captions grow from 10px to 11px, potentially truncating sooner. Row heights
  and layout density are unchanged. Bright white data canvases are intentionally
  more prominent than the surrounding Latte panels.
- This is a scoped contrast improvement, not a full accessibility or syntax-color
  contrast audit. Existing focus/hover/selection styles outside the inspected
  components remain unchanged.
