# DatabasePage structural refactor verification

Date: 2026-09-30 (Asia/Shanghai).
Baseline: `main`, `31899cc97f099629cdfc4fcd506787347d9a4903`.

## Scope and ownership

The change extracts three responsibilities within `packages/database`:

- `useDatabaseTreeController.ts` owns per-connection catalog/schema caches,
  loading keys, errors, selected schema/catalog queries and synchronization,
  and automatic connected-root loading. Synchronization and root loading have
  separate hook entry points to preserve the original effect order and compose
  with the existing workspace controller's actions.
- `useDatabaseShellIntegration.tsx` owns saved SQL grouping for the tree,
  sidebar/status element creation, stable action handlers using committed refs,
  and slot injection/cleanup.
- `useDatabaseActiveContext.ts` owns selected/query/table connection derivation,
  toolbar status, schema/structure eligibility, selected tree ID, catalog/schema
  options, and binding the existing `createTableEditing` implementation.

`useDatabaseWorkspaceController.ts` is byte-for-byte unchanged. Its schema
loading and refresh actions still receive the same caches and React setters.
Query keys, root/catalog loading guards, connection lifecycle, query context
normalization, SQL execution/batching, table mutations, saved SQL behavior and
tab persistence remain in their existing implementations. Package exports,
dependencies, and the page's rendered component structure are unchanged.

## Measurements

ESLint uses the repository's cyclomatic complexity rule; function line counts
exclude blank lines and comments. Physical file lines include both.

| Measurement | Before | After |
| --- | ---: | ---: |
| `DatabasePage.tsx` physical file lines | 658 | 396 |
| `DatabasePage` physical function lines | 612 | 361 |
| `DatabasePage` ESLint function lines | 552 | 346 |
| `DatabasePage` complexity | 52 | 22 |
| Workspace controller physical file lines | 413 | 413 |
| Workspace controller ESLint function lines | 294 | 294 |
| Workspace controller complexity | 9 | 9 |

The page still exceeds the 300-line component function warning threshold.
The extracted active context hook has 60 ESLint function lines and complexity
28, exceeding the `.ts` threshold of 15. These warnings are retained; JSX and
individual fallback expressions were not split solely to satisfy lint limits.

## Regression coverage

Four new test files add 14 cases, using actual hooks/controllers/stores and
mocked command-client calls:

- Multiple connected roots, including a connecting root; PostgreSQL/MySQL
  catalog enumeration and direct SQLite schema loading.
- Catalog schemas load only on expansion, with in-flight/cache deduplication
  and simultaneous completions retaining other connections' entries.
- Selected schema cache mirroring, MySQL grouping across catalogs, empty SQLite
  schemas, inactive visibility, and error/retry isolation.
- Selected schema success/error updates respect the existing enabled gate.
- Cache writes and new loader identities do not trigger additional root loads;
  activation or connection/state changes use the latest committed loader.
- Sidebar and status injection, independent updates, accessories/name fallback,
  absent callbacks, callback replacement, deactivation and unmount cleanup.
- Stable sidebar handlers invoke the latest actions without reinjecting the
  sidebar when only action closures change.
- Actual page wiring preserves simultaneous tree expansion, lazy PostgreSQL
  catalog loading, query drafts/context, table page/pending changes, selection,
  and caches across connection/tab switches and module activation.
- Refresh invalidates the same keys, reloads only the target connection's
  already loaded catalogs, and preserves other caches and tree expansion.

The three page integration cases also passed against the original baseline
`DatabasePage.tsx`; the refactored source was restored in a `finally` block.
This checks that the assertions describe existing behavior.

## Verification results

| Check | Result |
| --- | --- |
| Baseline page integration tests | PASS: 3 tests |
| `pnpm exec vitest run packages/database` | PASS: 34 files, 209 tests |
| `pnpm run test` | PASS: 163 files, 969 tests |
| `pnpm run lint` | PASS: 0 errors, 41 warnings |
| `pnpm --filter @unfour/desktop exec tsc --noEmit` | PASS |
| Additional TypeScript program including the four new test files and fixtures | PASS |
| `pnpm run build` | PASS; Vite reports chunks larger than 500 kB |
| `pnpm run check:large-files` | PASS: 0 blocking files |
| `git diff --check` | PASS |

Vitest initially could not start esbuild inside the restricted sandbox
(`spawn EPERM`); test/build runs succeeded with reviewed escalation.
Live database engines and native Tauri sessions are NOT VERIFIED by these DOM
tests. Commands are mocked; the page tests retain the actual injected sidebar
and status components while replacing the Monaco-backed workspace surface.

## Remaining assessment

The workspace controller still carries substantial wiring and several
responsibilities: connection form/lifecycle actions, schema refresh, query
workspace actions, and SQL/table controller composition. Its low outer
complexity does not represent all nested handlers. It remains a reasonable
candidate for a separate scoped refactor, but no immediate continuation is
required for this change. Revisit its ownership/interfaces when work on those
responsibilities makes the current coupling materially costly; retain the
regression harness as a boundary check.

An existing wiring issue was observed: the injected sidebar element receives
`onDesignTable`, but `DatabaseSidebar` does not consume/forward that prop to
`DatabaseConnectionTree`. This was already present on the baseline and was
preserved by the structural refactor. The separate wiring repair below fixes
the action without further structural splitting.

## Design Table wiring follow-up

Baseline: `main`, `826382df2a24487b1dbcb95e2925e7e107354ebd`.

`DatabaseSidebar` now declares and destructures the optional
`onDesignTable?: (connectionId: string, table: DatabaseTable) => void` prop and
passes it unchanged to `DatabaseConnectionTree`. These are the only three
production lines added. `DatabasePage`, `useDatabaseShellIntegration`,
`designTable`, the tree/menu implementations and their view/read-only display
rules are unchanged.

The existing shell integration suite adds one parameterized DOM regression
test with three scenarios: ordinary table, read-only table, and read-only view.
It renders the actual injected sidebar, opens the object context menu and
clicks Design Table. The path covered is
`useDatabaseShellIntegration -> DatabaseSidebar -> DatabaseConnectionTree ->
TableContextMenu -> stable shell handler -> sidebarActions.designTable`.
The callback must run exactly once with the same connection ID and original
table object. INSERT generation and table export remain visible for tables
and hidden for views under the existing rules.

Before the repair, all three scenarios failed because Design Table was absent
from the rendered menu. After the repair, the six shell integration cases and
the following checks passed:

| Check | Result |
| --- | --- |
| `pnpm exec vitest run packages/database` | PASS: 34 files, 212 tests |
| `pnpm run test` | PASS: 163 files, 972 tests |
| `pnpm run lint` | PASS: 0 errors, 41 existing warnings |
| `pnpm run build` | PASS; Vite reports chunks larger than 500 kB |
| TypeScript program including the updated shell regression test | PASS |
| `git diff --check` | PASS |

A read-only TypeScript audit checked 76 Database component JSX usages,
including spread callback props, and 29 functions receiving callback props.
There were no other undeclared callback props, missing callback bindings or
unused callback bindings. The main Sidebar/Tree/Menu and workspace forwarding
paths were also inspected; no other obvious forwarding gap was found. This
is a scoped wiring review, not proof of every dynamic callback path.
