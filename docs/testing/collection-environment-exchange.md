# Collection and environment exchange

## Architecture

Collection format detection and normalization live beside the existing OpenAPI
parser in `crates/http-engine/src/api_client/openapi_import/exchange.rs`.
OpenAPI, Unfour, and Postman all produce `NormalizedCollection`. Preview runs
the same decode/validation as import, without writing. CommandBus executes
`import_collection_on` inside its existing domain transaction, including
mutation/outbox processing. Source folder IDs only reconstruct relationships;
the transaction generates new entity IDs.

Collection create/rename reject ASCII case-insensitive name conflicts among live
collections in the same workspace with a domain Validation error. Rename excludes
the current ID; an unchanged name remains a no-op even for historical duplicates.
Existing duplicates are not migrated. Folder/request display names may repeat.
No database uniqueness constraint is added; IDs remain the identity.

Collection Preview requires `workspaceId` and returns `conflict` and `targetName`
alongside `name`. Both names are the trimmed collection name: `name` is the
normalized source, and `targetName` is the name import will try to store. The
confirmation dialog displays the copy target on conflict. Import trims with
`normalize_collection_name`, allocates with `import_copy_name`, then runs
`validate_collection_name_on` on that final name inside the import transaction.
A name occupied after Preview advances the Copy suffix. `import_copy_name`
rejects a base longer than its character limit before looking for conflicts.
Collection and Environment imports share it: `Name (Copy 1)`, `Name (Copy 2)`,
etc., truncating the stem by character count to fit the collection's
120-character or environment's 80-character limit.

Environment exchange belongs to workspace-engine. Import calls existing
environment create/update services inside one CommandBus transaction. It always
creates a new environment, choosing `Name (Copy N)` on a name conflict and
shortening the original name when needed so the result stays within 80
characters. It never
merges or replaces existing environments or Workspace Variables. Collection
import never imports environments or collection variables implicitly.

Desktop adapters only choose/read/write files and forward to CommandBus. Import
is now two explicit calls: preview chooses the file and returns its content and
summary; import accepts that content after confirmation and validates it again.
Browser development mocks cancel file dialogs rather than implement a second
parser or persistence path.

## Unfour Collection v1

JSON envelope: `{"format":"unfour.collection","version":1,"collection":...}`.
Unknown envelope/model fields and unknown versions are rejected.

The collection contains `name`, nullable `description`, `folders`, and `requests`.
Each folder contains `sourceId`, nullable `parentSourceId`, `name`, and `sortOrder`.
Each request contains:

- nullable `parentSourceId`, `name`, `method`, `url`, and `sortOrder`;
- `headers` and `query`: arrays of `{key,value,enabled}`;
- nullable `body`, `bodyKind`, `authJson`, and `settingsJson`;
- nullable `preRequestScript` and `postResponseScript`, and `scriptSchemaVersion`.

Auth/settings retain the existing request JSON representation. This format
preserves portable Unfour request definitions, including disabled values and
ordering, subject to intentional credential redaction and omission of runtime
file bindings. It has no history, workspace IDs, sync state, revisions, remote
IDs, deletion timestamps, or Flow definitions. Multipart definitions retain
file descriptors only; users must select local files again.

## Unfour Environment v1

JSON envelope: `{"format":"unfour.environment","version":1,"name":...,"variables":[...]}`.
Variables contain `key`, `value`, `isSecret`, `isEnabled`, nullable `description`,
and `redacted`. Array order determines local sort order. Imported IDs are ignored.
Redacted values become empty strings and retain the secret flag.

Export reads WorkspaceEnvironment directly, never the legacy ApiEnvironment
projection. `isSecret` is authoritative; key-name classification supplements it.
Secret values are empty in Native/Postman environment exports. Existing OpenAPI
environment extensions use their existing redaction marker and now preserve
secret metadata. Non-secret names marked secret cannot populate OpenAPI server
defaults.

## Postman and OpenAPI boundaries

Postman Collection v2.1 supports nested items, methods, raw/structured URLs,
query/header enabled state, raw/urlencoded/formdata bodies, basic/bearer/API-key
auth, and pre-request/test source. Inherited auth/scripts are flattened into
requests. Explicit noauth overrides inheritance. Disabled events are omitted,
never activated. Settings and original URL/body kind use `x-unfour-*` extensions
for Unfour round trips. Collection variables are shown by name in Preview and
are not persisted. Unsupported auth/body modes, metadata and script APIs produce
localized warnings. Scripts use the existing runtime, not a complete Postman
Sandbox; static API warnings are advisory, not a full JavaScript analyzer.

Postman Environment uses `_postman_variable_scope: "environment"`, `name`, and
`values`, mapping `type: "secret"` and `enabled`. Scalar number/boolean values
become strings; structured values are rejected instead of silently erased.

OpenAPI JSON/YAML retains the existing 3.x importer and 3.1 exporter. Preview
warns about projection loss and ignored embedded environments. Use Native for
portable request definitions. Literal secrets in arbitrary script/free-form
source cannot be classified exhaustively; the export dialog calls for review.

## Verification (2026-09-22–23)

### Collection name conflicts (2026-09-24)

- PASS: HTTP engine suite (75 tests), including workspace-scoped create/rename
  conflicts, case-only rename, historical duplicates, soft-deleted name reuse,
  Preview races, consecutive Copy suffixes, multibyte 120-character names,
  and duplicate folder/request display names inside imported content.
- PASS: import trims the collection name before the case-insensitive conflict
  check, then rejects the final name if it is still taken. `import_copy_name`
  rejects a base longer than its limit with no existing names (unfour-core).
- PASS: CommandBus exchange tests (6), including Collection validation/import
  and Environment copy-name/length/rollback regressions; workspace environment
  tests (7).
- PASS: Collection tree component tests (17), desktop TypeScript, affected
  ESLint, production Vite build, Tauri/MCP compile checks, Rust formatting,
  diff whitespace checks, and large-file checker (no blocking findings).
- Initial pnpm verification launch failed on local store permissions; direct
  invocation of the installed tools passed. An initial Vite invocation from the
  repository root could not find `index.html`; rerunning in `apps/desktop` passed.
- NOT VERIFIED: native file-dialog/desktop visual interaction; conflict target
  rendering and confirmation are covered by the component test.

### Previous exchange verification

- PASS: HTTP engine library suite, including native/Postman round trips, existing
  OpenAPI JSON/YAML, folder hierarchy, scripts, auth, disabled headers/query/body,
  credential redaction, unsafe-file omission, and an injected transaction failure.
- PASS: CommandBus library suite, including environment copy conflicts,
  secret metadata with non-sensitive names, Native/Postman environment round trips,
  heuristic supplementation, and failure during variable insertion rolling back
  the newly created environment.
- PASS: Collection tree, Environment Manager, and i18n frontend suites, including
  explicit Preview confirmation and export format selection.
- PASS: desktop TypeScript check, production Vite build, affected ESLint checks,
  `cargo check -p unfour-app`, `git diff --check`, and large-file checker.
- PASS: browser visual walkthrough of the final production build: Collection
  row menu, export format selection, OpenAPI-only JSON/YAML control, Environment
  Manager import entry, environment export menu, and export dialog layout.
- NOT VERIFIED: native OS file dialog interaction. Browser mocks cannot
  establish native dialog behavior; Preview/confirmation use component tests.

pnpm's automatic dependency check failed to access its local cache database in
the sandbox. Frontend verification used installed repository executables
directly (`node node_modules/typescript/bin/tsc`, Vitest, ESLint, and Vite), without
adding or changing dependencies. Existing size/lint/build warnings remain.
