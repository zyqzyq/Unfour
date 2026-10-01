# Workspace bundle verification

Date: 2026-10-01. Scope: `unfour-workspace` V1 create-new import/export.
See [format and boundaries](../architecture/workspace-bundle.md).

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
