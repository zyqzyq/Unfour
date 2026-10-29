# Database and SSH sidebar local search

## Design and scope

Based on local `main` at `c693bf82`. API Collection Tree uses trimmed, case-insensitive
substring matching over loaded records. Its structure-based remount is unsuitable
for Database: manual expansion must survive filtering and auto-expansion can
trigger schema loads.

- [x] Add a compact shared search input using Input/IconButton and shared i18n.
- [x] Give TreeView optional controlled expansion, preserving its uncontrolled
  expansion and lazy-load behavior when the override is removed.
- [x] Filter Database's rendered, already-loaded tree by node names. Keep ancestor
  paths; a matching parent retains its loaded subtree. Ignore status placeholders
  as search targets. Temporarily expand results without invoking loaders; allow
  search-only collapse. Keep the tree mounted for no results and clear.
- [x] Filter SSH connections by name, host, username and port. Tasks use a separate
  sidebar mode, so connection filtering does not filter tasks or terminal sessions.
- [x] Scope transient search to Workspace and retain existing selection/data.
- [x] Test filtering, clearing, selection/activation/menus, lazy-load isolation,
  Workspace switches and English/Chinese copy before implementation; run affected
  suites, lint, TypeScript, build and diff checks.

No backend changes, persistence, dependencies, fuzzy matching, or global search.
Disconnected Database roots retain their existing behavior (no schema subtree).

## Verification (2026-10-29)

- PASS: 89 Vitest tests across the 10 affected/regression suites below. Initial
  red runs confirmed missing search controls and missing controlled expansion.
  After the final SSH display-only filtering adjustment, its 8 tests passed again.
- PASS: `pnpm --filter @unfour/desktop exec tsc --noEmit`.
- PASS: `pnpm run build` (includes TypeScript); existing chunk-size warning remains.
- PASS: ESLint over changed TypeScript files, including the browser test, with
  zero errors. Existing large functions in SshConnectionTree and TreeView still
  produce max-lines-per-function warnings. An intermediate purity error was
  resolved by keeping filtered data separate from mutation callback inputs.
- PASS: Playwright existing Database tree regression (connect, catalog loading,
  preview, saved-query delete confirmation) and both new sidebar-search cases.
  English/light and Chinese/dark at 960x600 verify 28px input height, ancestor
  visibility, no results, clear/focus restoration, manual collapse restoration,
  and a live browser-mock xterm DOM node remaining mounted while filtering.
  The first Chinese run failed because the test assumed translated disclosure
  labels; corrected the test to use the existing English Expand/Collapse labels.
- PASS: screenshot inspection of filtered Database (English/light) and no-result
  SSH with terminal visible (Chinese/dark). Artifacts are generated under ignored
  `test-results/` and are not committed.
- PASS: independent read-only code review found no actionable regressions.
- PASS: `pnpm run check:large-files` (zero blocking files) and `git diff --check`.

Vitest suites:

```text
packages/database/src/components/DatabaseConnectionTree.test.tsx
packages/database/src/components/DatabaseSidebar.search.test.tsx
packages/database/src/hooks/useDatabaseShellIntegration.test.tsx
packages/database/src/hooks/useDatabaseTreeController.test.tsx
packages/ssh-terminal/src/components/SshConnectionTree.search.test.tsx
packages/ssh-terminal/src/SshPage.test.tsx
packages/ssh-terminal/src/components/SshTasksPage.test.tsx
packages/ui/src/tree-view.test.tsx
packages/ui/src/i18n.test.ts
packages/api-client/src/components/ApiCollectionTree.test.tsx
```

Browser command:

```sh
pnpm exec playwright test apps/desktop/tests/smoke/sidebar-search.spec.ts apps/desktop/tests/smoke/database-tree.spec.ts --workers=1
```

## Limits

- NOT VERIFIED: native Tauri/WebView runtime and real database/SSH servers.
  Browser checks use disposable mocks; component tests assert that searching
  invokes no connection/schema loaders, mutations, or extra list requests.
- NOT VERIFIED: very large loaded trees (performance/stress testing).
- Workspace changes are covered by React tests, including stale connection
  records and pending next-workspace loading; browser Workspace switching is not
  exercised by these new smoke tests.
- Search covers loaded nodes already exposed by the existing Database tree.
  Disconnected roots, unloaded schemas, and unresolved legacy saved queries keep
  their existing browsing semantics; search does not fetch or reveal new data.
