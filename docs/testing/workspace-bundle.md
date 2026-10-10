# Workspace bundle verification

Current scope: V2 local bundles and encrypted backups. See the latest
[storage and Secret correction verification](secret-backup-storage.md). Earlier
audit/V1 evidence below is historical; imported API/variable Keychain references
and legacy adoption described there are superseded.
See [format and boundaries](../architecture/workspace-bundle.md).
See [Workspace MCP permissions verification](workspace-mcp-permissions.md) for
the current import permission choices and independent security settings.

## Historical credential lifecycle audit of `176f37b` — 2026-10-09

The review fixes imported-variable and saved-SSH credential replacement with
fresh staged handles, durable attachment/reclamation metadata, rollback
compensation and last-reference cleanup across variables, API and connections.
Primary recovery adopts legacy live handles. The editor preserves an untouched
handle behind a localized saved-credential placeholder and supports replacement
and explicit clearing. Bundle/Cloud Sync variable classification is shared;
unsafe older retries are parked without changing their operation IDs or payloads.
V1/V2 formats, Cloud Sync wire contracts and sharing exclusions remain unchanged.
This audit adds no dependencies.

| Check | Result | Coverage |
| --- | --- | --- |
| `cargo test -p unfour-command-bus -p unfour-cloud-sync -p unfour-workspace-engine -p unfour-core -p unfour-local-storage -p unfour-http-engine -p unfour-ssh-engine -p unfour-database-engine -- --test-threads=4` | PASS, 703 tests | Domain rollback, storage migration, Cloud Sync contracts/outbox/worker, runtime resolution and connection cleanup |
| Final `cargo test -p unfour-command-bus workspace_bundle -- --test-threads=4` | PASS, 26 tests | All staged entries removed after failed import/hook; failed bulk/environment edits; rotation, clear/refill, individual and cascade delete; repeated imports; shared SSH/Database/API handles including whitespace templates; pending GC and legacy adoption; V1/V2 and safe sharing |
| Concurrent snapshot regression within the focused suite | PASS | Isolated file SQLite with a concurrent writer updates Workspace name, ordinary value, fresh Keychain handle, API template/URL and transfer path together while reclaiming the old credential. Four encrypted/decrypted captures match one epoch across base records and supplements |
| `pnpm exec vitest run packages/workspace-environments/src packages/workspace-local/src/WorkspaceBundleExchange.test.tsx packages/ui/src/i18n.test.ts` | PASS, 28 tests across 6 files | Workspace/environment saved handles stay opaque; metadata edits retain handles; replacement and clear send intended values; exchange/i18n regressions |
| Targeted ESLint; `pnpm --filter @unfour/desktop exec tsc --noEmit`; `pnpm --filter @unfour/desktop build` | PASS | Changed React sources/tests, shared UI types and frontend production build |
| `cargo check -p unfour-app --features ssh-native` | PASS | Desktop command composition and native SSH feature compatibility |
| Affected Cargo formatting; `git diff --check`; migrations/public-secret/shared-token checks | PASS | 33 migrations and source checks |
| `node scripts/check-large-files.mjs` | FAIL, existing generated Monaco assets | Same blocking `ts.worker-CMbG-7ft.js` (67,731 lines) and `vs/loader.js` (1,368 lines) recorded below; no changed source reaches the blocking threshold |

The first broad run failed one new test's hardcoded credential count (the fixture
also contains an API binding); it now verifies every binding actually present in
the encrypted payload. That run also produced `FLOW_TIMEOUT` in an existing
history-limit test under unrestricted test concurrency. The controlled-concurrency
rerun passed it unchanged. After the broad run, the focused suite was rerun for
the strengthened Keychain-concurrency and whitespace-reference regressions.

NOT VERIFIED: native Windows credential-store persistence, permissions/provider
failures, native dialogs and live SSH/database services. Tests use disposable
in-memory credentials, isolated SQLite files and mock Cloud Sync transport; no
user Workspace, credential or remote service was used. Historical orphaned keys
whose live references and journal metadata are already lost cannot be enumerated
portably. OS-level changes outside Unfour cannot honor the SQLite snapshot lock;
slow Keychain access may briefly delay local writes during credential capture.

## Encrypted backups and local paths — 2026-10-09

Desktop export now distinguishes sharing from encrypted backup, with explicit
path/credential switches. Import unlocks locally, groups missing/unchecked/ready
fields, supports prefix replacement and per-path edits, and preserves enabled
transfer steps. No multi-device credential Cloud Sync was added.

| Check | Result | Coverage |
| --- | --- | --- |
| `cargo test -p unfour-command-bus -p unfour-workspace-engine -p unfour-secret-store -p unfour-core --lib` | PASS, 166 tests | Existing execution/environment/Flow behavior, bundle import and credential runtime, cryptography and path preflight |
| `cargo test -p unfour-command-bus workspace_bundle --lib` | PASS, 17 tests in final focused run | V1 compatibility, V2 paths/templates, encrypted round trip/re-export, quoted API secrets, ordinary URL normalization, wrong password, hook rollback, startup cleanup, workspace isolation and safe sharing |
| Targeted Vitest (exchange, menu, DesktopApp, startup smoke, i18n) | PASS, 35 tests; exchange rerun PASS with 8 tests after adding path review coverage | Password matching, unlock failure/retry, review invalidation, save cancellation, path mapping, reviewed target and existing shell behavior |
| `pnpm exec playwright test apps/desktop/tests/smoke/workspace-bundle.spec.ts` | PASS, 2 tests | English/Chinese sharing and encrypted export at 960×600, masked passwords, visible footer and enabled/disabled save actions |
| `cargo check -p unfour-app --features ssh-native` | PASS | Tauri command registration, options and native SSH preflight |
| `pnpm run build`, final `tsc --noEmit`, targeted ESLint, affected Cargo formatting and `git diff --check` | PASS | Frontend production build, types and source checks |
| Migration, public-secret and shared-token checks | PASS | 32 migrations; public source audit and shared UI token ownership |
| `pnpm run check:large-files` | BLOCKED by generated Monaco assets | `apps/desktop/public/monaco/vs/assets/ts.worker-CMbG-7ft.js` (67,731 lines) and `vs/loader.js` (1,368 lines) are generated by the existing build preparation script but scanned as source; no changed source file triggers the blocking threshold |

Inspected the English/Chinese encrypted-export screenshots under
`test-results/workspace-bundle-*`; form controls and action footer fit the compact
viewport. A browser-label mismatch found by Playwright was fixed with an explicit
localized accessible name on the export mode selector.

All credentials use the disposable in-memory store in tests. Native OS credential
store behavior, native pick/save dialogs and live SSH/database connections remain
manual acceptance items. No user Workspace, real credential or remote server was
used. API multipart file selections and private-key/file contents remain excluded;
embedded body/script/SQL/Flow secrets stay redacted even in encrypted mode.

The first full-suite run preceded the final URL-normalization regression and
credential grouping refinements; the focused bundle suite and native composition
check were rerun after those changes. Earlier evidence below describes V1 only,
including its former disabled-transfer-step behavior.

## Export filename verification — 2026-10-08

The native save adapter now uses `<Workspace-name>.unfour-workspace.json` from
the same export snapshot as the content. Filename sanitization retains Unicode,
replaces whitespace/unsafe characters, bounds the stem length, and falls back
to `workspace` for empty or Windows-reserved stems.

| Check | Result | Coverage |
| --- | --- | --- |
| `cargo test -p unfour-command-bus workspace_bundle` | PASS, 10 tests | Selected Workspace name, Unicode/path characters, reserved/empty stems and long names, plus existing bundle security/import regressions |
| `cargo check -p unfour-app --features ssh-native` | PASS | Export artifact reaches the native save adapter without changing the frontend command contract |
| `cargo fmt -p unfour-command-bus -p unfour-app --check` / `git diff --check` | PASS | Changed Rust files and diff formatting |

NOT VERIFIED: manual native Windows save dialog. Checks use disposable in-memory
Workspaces; no real Workspace data was changed.

## Grouped UI verification — 2026-10-08

The switcher now groups new/import and global extension actions separately from
current-Workspace export, rename, safety-tier and Cloud Sync actions. Delete is
last. The menu and command palette share one exchange controller and dialog in
`workspace-local`; export confirmation shows the reviewed Workspace name and
safety tier and keeps that target if the active Workspace changes. Import
retains preview/reconfiguration details and labels confirmation "Create and
Switch". Both locales describe included and excluded data separately.

| Check | Result | Coverage |
| --- | --- | --- |
| Targeted Vitest: WorkspaceBundleExchange, WorkspaceMenu, DesktopApp, DesktopApp startup smoke, i18n, Cloud Sync workspace actions | PASS, 38 tests across 6 files | Group ordering with extensions, import confirmation/cancel/retry, reviewed export target, missing-Workspace command availability and desktop regressions |
| `pnpm exec playwright test apps/desktop/tests/smoke/workspace-bundle.spec.ts` | PASS, 2 tests | English/Chinese grouped menu, 960×600 import/export dialogs, visible action footer, and both command-palette entry points using the same dialog |
| Targeted ESLint and `pnpm --filter @unfour/desktop exec tsc --noEmit` | PASS | Changed frontend sources and tests |
| `pnpm run build` | PASS | TypeScript and production frontend bundle; existing chunk-size warning remains |

Inspected the menu, import preview and export confirmation screenshots. The
browser test substitutes native file selection and does not establish native
Windows file-pick/save behavior. No real Workspace mutations or Cloud Sync
operations were performed. The original backend evidence below is from
2026-10-01; this UI change does not alter the Rust contract or security rules.

## Automated evidence

| Check | Result | Coverage |
| --- | --- | --- |
| `cargo test -p unfour-command-bus -p unfour-workspace-engine -p unfour-flow-engine` | PASS | CommandBus and domain integration suites; includes six new bundle regression tests |
| `cargo check -p unfour-app --features ssh-native` | PASS | Tauri registration, adapter types and native SSH feature composition |
| `pnpm exec vitest run packages/workspace-local/src/WorkspaceBundleExchange.test.tsx packages/app-shell/src/components/WorkspaceMenu.test.tsx packages/ui/src/i18n.test.ts` | PASS, 20 tests | Four new preview/confirm/cancel/retry/export tests, existing shell and i18n coverage |
| `pnpm exec playwright test apps/desktop/tests/smoke/workspace-bundle.spec.ts` | PASS, 2 tests | English and Chinese menu entry, compact 960×600 preview/export dialogs, scrollable repair list and visible action footer |
| `pnpm run build` | PASS | TypeScript and production frontend bundle |
| Targeted ESLint / Rust formatting / `git diff --check` | PASS | Changed frontend code and affected Rust packages |
| `pnpm run check:large-files` | PASS | No blocking size violations |

Frontend test/build processes needed execution outside the sandbox because
esbuild's child-process startup was initially rejected with `spawn EPERM`.
Re-running the same local checks succeeded. Existing non-blocking warnings remain
for frontend chunk size, the default release channel, and SSH dead code.

## Regression cases

- Complete mixed Workspace bundle, nested/reversed-order API folders, variables,
  environments, requests, SSH/Database connections, SSH task/steps, Saved SQL and
  a multi-capability Flow survive import/export with correct relationships.
- All business IDs, Flow step IDs and multipart part IDs are regenerated;
  importing the same content twice creates independent Workspaces and name copies.
- Flow API/SSH/DB resources, SSH connection selection, condition edges,
  wait-until/poll probes, `$ref` and interpolated step references are remapped.
  Inputs/probe references and `$end` are preserved.
- Missing/wrong-kind references, duplicate IDs, folder cycles, malformed JSON,
  unsupported versions, unsafe upstream references, forbidden connection config,
  invalid transfer configuration and oversized input fail without partial data.
- Secret variable values, API auth/headers/query/multipart values, credential
  references, source private-key/SQLite paths, opaque request settings and secret
  Flow defaults do not escape the bundle security boundary.
- Source active environment is not imported. Connections have no destination
  credential references. SSH transfer steps require reconfiguration.
- A late Saved SQL validation failure rolls back earlier Workspace/API/SSH/DB
  records. An injected transactional hook failure also rolls back Saved SQL and
  Flow definitions; hook context and mutations are local.
- Preview runs domain validation but leaves the Workspace count unchanged.
  UI confirmation sends the reviewed content and edited name; cancelling does
  not import; failure keeps the preview available for retry.

## Visual inspection and limits

Playwright screenshots under `test-results/workspace-bundle-*` were inspected
with dialog animations finished. The Chinese preview and English export dialog
use shared primitives, retain a visible footer, and scroll overflow content.
The browser smoke tests substitute only native file-selection results; they do
not establish native Windows file-dialog or Keychain behavior.

NOT VERIFIED: manual native file save/pick, reconnecting imported SSH/DB resources
against live servers, and execution of arbitrary imported scripts/SQL. No remote
services, real Keychain entries or user Workspaces were used by these tests.

Suggested native acceptance pass: export a disposable Workspace, inspect the JSON,
import twice, confirm new names and relationships, inspect missing connection
settings, select a new SQLite file/private key, and explicitly re-enable configured
SSH transfer steps. Cancel once at both native dialogs and confirm no mutation.
