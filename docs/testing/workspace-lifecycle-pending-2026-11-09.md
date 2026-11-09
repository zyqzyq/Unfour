# Workspace lifecycle pending protection — 2026-11-09

Scope: create/delete dialogs in `WorkspaceDialogs.tsx`, on the existing `main`
branch. No dependencies, navigation/draft-management changes, backend changes,
or C2 styling changes. All commands used isolated component mocks or browser
mock state; no real workspace or production data was accessed.

## Reproduction and change

Before the fix, the first deferred component regression run failed 9 of 14
cases: pending dialogs could close, creation inputs remained enabled, and
three submissions before the pending render issued three commands. Both
Chromium create/delete success cases also failed because clicking Cancel
closed the dialog while the mock command was still unresolved.

Create/delete now follow the existing rename dismissal protection: pending
blocks Cancel, Escape, outside clicks, and `onOpenChange(false)`. Creation
also disables the name/environment fields. Synchronous refs guard both submit
handlers and deferred leave callbacks before React renders `isPending`; refs
are acquired only after the leave gate permits execution and released on
settlement. Existing success switching/closing and error feedback are retained.

## Verification

- **PASS** — 30 component/navigation tests:
  `pnpm exec vitest run packages/app-shell/src/components/WorkspaceDialogs.lifecycle.test.tsx packages/app-shell/src/components/WorkspaceDialogs.test.tsx packages/app-shell/src/components/useDesktopNavigation.test.tsx`
  Includes 18 new lifecycle cases, existing rename protection, and leave gates.
- **PASS** — 6 new Chromium cases:
  `pnpm exec playwright test apps/desktop/tests/smoke/workspace-lifecycle-pending.spec.ts --project=chromium --workers=2`
  Each create/delete scenario manually resolves or rejects its pending command.
  Checks physical Cancel/overlay clicks, Escape, focus containment, keyboard
  submissions, command counts/arguments, success switching, failed-operation
  retries/cancellation, and preservation of the original dirty Flow. Cancelling
  the initial leave confirmation issues no lifecycle command; Enter defaults
  to Cancel. Pending screenshots are generated under ignored `test-results/`.
- **PASS** — 8 existing Chromium cases:
  `pnpm exec playwright test apps/desktop/tests/smoke/workspace-leave-environment-race.spec.ts --project=chromium --workers=2`
  Covers hidden/invalid-JSON Flow drafts, environment editor leave protection,
  activation ordering, and workspace isolation.
- **PASS** — desktop production-source type check:
  `pnpm --filter @unfour/desktop exec tsc --noEmit`
- **PASS** — new test files and their source dependencies type check:
  `pnpm exec tsc --noEmit --target ES2020 --lib "ES2020,DOM,DOM.Iterable" --module ESNext --moduleResolution bundler --skipLibCheck --jsx react-jsx --strict --noUnusedLocals --noUnusedParameters --types "@testing-library/jest-dom/vitest,vite/client" packages/app-shell/src/components/WorkspaceDialogs.lifecycle.test.tsx apps/desktop/tests/smoke/workspace-lifecycle-pending.spec.ts`
  The first invocation failed in pnpm (`unable to open database file`); retry
  completed successfully.
- **PASS** — changed TypeScript lint, zero warnings:
  `pnpm exec eslint packages/app-shell/src/components/WorkspaceDialogs.tsx packages/app-shell/src/components/WorkspaceDialogs.lifecycle.test.tsx apps/desktop/tests/smoke/workspace-lifecycle-pending.spec.ts --max-warnings=0`
- **PASS** — `git diff --check`.
- **FAIL (existing test configuration/source issues)** —
  `pnpm exec tsc --noEmit -p packages/app-shell/tsconfig.json` does not load
  jest-dom/Vite types. Adding `--types "@testing-library/jest-dom/vitest,vite/client"`
  leaves the existing `WorkspaceDialogs.test.tsx:58` unsupported `exact` option
  in Testing Library's `ByRoleOptions`. Confirmed present in HEAD; left outside
  this fix. New test files pass the focused check above.

## Limits

Native Tauri/WebView behavior and real backend lifecycle execution were not
verified. Browser results establish the frontend interaction with controlled
mock latency/failure, not native execution or recovery after process shutdown.
