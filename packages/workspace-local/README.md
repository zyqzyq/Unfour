# workspace-local

`@unfour/workspace-local` owns the frontend boundary for local workspace
lifecycle, persistence, import/export, recent workspaces, and migration.

`src/WorkspaceBundleExchange.tsx` implements Workspace bundle file selection,
import preview, configuration reminders, create-new import, export confirmation,
and success/error feedback. The app-shell mounts its dialog and wires menu
actions; feature packages do not depend on this package.

Backend calls use `@unfour/command-client`; shared dialogs and translations use
`@unfour/ui`. React and React Query reuse existing workspace dependencies for
local interaction state and workspace-list invalidation. No new third-party
library was introduced. `src/index.ts` retains the transitional workspace-core
re-export for existing consumers.

See [Workspace bundle V1](../../docs/architecture/workspace-bundle.md) for the
format and backend boundaries, and [verification](../../docs/testing/workspace-bundle.md).

Checks:

- `pnpm exec vitest run packages/workspace-local/src/WorkspaceBundleExchange.test.tsx`
- `pnpm exec playwright test apps/desktop/tests/smoke/workspace-bundle.spec.ts`
- `pnpm run build`
