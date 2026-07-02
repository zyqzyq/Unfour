# database

## Purpose

`@unfour/database` owns the Database frontend experience.

## Boundaries

- Can own database connection UI, schema tree, SQL editor UI, query result UI,
  table preview UI, and database-local view state.
- Should call backend behavior through `@unfour/command-client`.
- May use `@unfour/workspace-core` for selected database connection state as
  the documented transitional boundary.
- Should not own API Client, SSH Terminal, or app-shell behavior.

## Key Files

- `src/DatabasePage.tsx` - top-level database page composition and command
  mutations.
- `src/hooks/useDatabaseConnections.ts` - connection query hook.
- `src/hooks/useSchemaTree.ts` - schema loading hook.
- `src/hooks/useSqlExecution.ts` - SQL execution mutation hook.
- `src/hooks/useTableData.ts` - table preview mutation hook.
- `src/components/DatabaseConnectionTree.tsx` - connection and schema tree.
- `src/components/DatabaseWorkspace.tsx` - SQL/table/results workspace.
- `src/result-utils.ts` - result serialization and error classification helpers.

## Current Capabilities

- Save, delete, connect, disconnect, and test database connections.
- Browse schema tables and columns.
- Edit and execute SQL with confirmation-aware mutation handling.
- Preview table data with pagination.
- View query results, messages, logs, structure, and local history.

## Known Gaps

- Release readiness belongs in `docs/release/*` and `docs/testing/*`.
- Real engine behavior should be manually verified for database behavior
  changes that automated tests cannot cover.

## Test / Verify

- `pnpm test -- packages/database/src/result-utils.test.ts`
- `pnpm run build`
- For behavior changes, manually verify SQLite plus any affected PostgreSQL or
  MySQL/MariaDB path, especially high-risk SQL confirmation behavior.
