# database

## Purpose

`@unfour/database` owns the Database frontend experience.

## Local constraints

See [AGENTS.md](AGENTS.md) for this package's scope and invariants.

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

## Test / Verify

Choose checks for the changed behavior using the
[verification guide](../../docs/agents/EXECUTION_PROTOCOL.md#choose-verification-by-impact).
The commands below are examples, not a checklist for every edit.

- `pnpm exec vitest run packages/database/src/result-utils.test.ts`
- `pnpm run build`
- For engine-dependent behavior, verify only affected SQLite, PostgreSQL, or
  MySQL/MariaDB paths against disposable/authorized test data, especially SQL
  confirmation. Record unavailable live-engine coverage as not verified.
