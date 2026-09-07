# api-client

## Purpose

`@unfour/api-client` owns the API Client frontend experience.

## Boundaries

- Can own request drafts, request tabs, Send behavior, response display,
  history, saved requests, collections, and import/export UI.
- Should call backend behavior through `@unfour/command-client`.
- Should reuse `@unfour/ui` primitives where possible.
- Should not own Database, SSH Terminal, app-shell, or global workspace
  orchestration behavior.

## Key Files

- `src/ApiClientPage.tsx` - top-level API Client page composition.
- `src/hooks/useApiRequestTabs.ts` - request tab, send, save, history, and
  collection state orchestration.
- `src/request-utils.ts` - request conversion, import/export, auth metadata,
  query/header/body utilities.
- `src/components/ApiRequestEditor.tsx` - request editor panels.
- `src/components/RequestScriptEditors.tsx` - pre-request and post-response
  script editors.
- `src/components/ResponseTabs.tsx` - response, script test, and console tabs.
- `src/components/ScriptResults.tsx` - script test, console, timing, and error
  presentation.
- `src/model/request-tabs.ts` - request tab model transitions.

## Current Capabilities

- Multi-tab request editing.
- Send request as the primary action.
- Save, duplicate, delete, import, and export saved requests.
- View response body, headers, cookies, timing, and history.
- Save and run pre-request and post-response scripts with bounded execution,
  request mutation, workspace/environment variables, tests, and console output.
- API Send consumes the shared Workspace variable resolution result.

## Known Gaps

- Release readiness belongs in `docs/release/*` and `docs/testing/*`.
- Browser mock behavior in `@unfour/command-client` must stay aligned with real
  command-bus behavior.

## Test / Verify

- `pnpm test -- packages/api-client/src/request-utils.test.ts packages/api-client/src/model/request-tabs.test.ts`
- `pnpm run build`
- For behavior changes, manually verify opening a request, Send, save, history,
  and response rendering.

## Multipart V1

`form-data` supports ordered Text/File rows with stable IDs, duplicate keys,
empty text values, and disabled rows. Saved `bodyKind` is
`multipart-form-data`; `body` is a JSON array of
`{ id, enabled, key, type: "text", value }` or
`{ id, enabled, key, type: "file", fileName }` definitions.

Only the current draft holds `filePath`. The Desktop send DTO carries
`multipartParts: [{ id, filePath }]` as transient bindings joined to that
safe definition. Save omits these bindings; Rust rejects paths/unknown fields
inside definitions and excludes runtime bindings from serialization and Debug.
Saving preserves bindings in the current tab by ID; saved/history reopen
requires selecting each enabled file again. MCP does not accept bindings and
fails closed for enabled File parts. GET/HEAD send no body.

The picker follows `pickApiRequestFile` -> `api_request_file_pick` -> Tauri
DialogExt and reads no content. Rust uses asynchronous file handles and reqwest
streaming multipart parts. The existing reqwest dependency enables `multipart`
and `stream` (with their transitive lockfile dependencies); Tokio enables
`fs` and `macros` for file opening and cancellation. No filesystem frontend
library or upload manager is added. reqwest owns Content-Type/boundary and
replaces explicit Content-Type during multipart sending.

Environment resolution covers Text key/value and File key, never fileName or
filePath. Pre-request scripts retain existing method/URL/header/environment
APIs; changing multipart body is rejected. History stores its actual body kind.
Cloud Sync keeps Protocol 5, payload versions, and reader revisions unchanged;
unknown future body kinds remain deferred. Sensitive Text values are redacted
by field key in snapshots and restored from local data by stable part ID.
OpenAPI uses the existing extension fallback rather than multipart schema mapping.
