# api-client

## Purpose

`@unfour/api-client` owns the API Client frontend experience.

## Boundaries

- Can own request drafts, request tabs, Send behavior, response display,
  history, saved requests, collections, environments, and import/export UI.
- Should call backend behavior through `@unfour/command-client`.
- Should reuse `@unfour/ui` primitives where possible.
- Should not own Database, SSH Terminal, app-shell, or global workspace
  orchestration behavior.

## Key Files

- `src/ApiDebuggerPage.tsx` - top-level API Client page composition.
- `src/hooks/useApiRequestTabs.ts` - request tab, send, save, history, and
  collection state orchestration.
- `src/request-utils.ts` - request conversion, import/export, auth metadata,
  query/header/body utilities.
- `src/components/ApiRequestEditor.tsx` - request editor panels.
- `src/components/ApiResponseViewer.tsx` - response/history display.
- `src/model/request-tabs.ts` - request tab model transitions.

## Current Capabilities

- Multi-tab request editing.
- Send request as the primary action.
- Save, duplicate, delete, import, and export saved requests.
- View response body, headers, cookies, timing, and history.
- Manage workspace environment variables for request templates.

## Known Gaps

- Release readiness belongs in `docs/release/*` and `docs/testing/*`.
- Browser mock behavior in `@unfour/command-client` must stay aligned with real
  command-bus behavior.

## Test / Verify

- `pnpm test -- packages/api-client/src/request-utils.test.ts packages/api-client/src/model/request-tabs.test.ts`
- `pnpm run build`
- For behavior changes, manually verify opening a request, Send, save, history,
  and response rendering.
