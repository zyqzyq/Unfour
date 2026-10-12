# Workspace MCP permissions — 2026-10-10

The imported Workspace was correctly persisted with MCP disabled, but the
desktop safety-tier dialog only exposed environment type. The existing
`updateWorkspaceMcpPolicy` client and Rust command already support policy edits.

## Result and implementation scope

- Workspace security settings expose DEV/TEST/PROD and all five MCP policies
  with English/Chinese descriptions. Separate save actions send only the
  selected field. Successful responses update the Workspace cache; failed saves
  keep the draft and show an inline retry message. Closing/reopening discards
  unsaved selections and reloads the current Workspace values.
- Saved policy is displayed separately from the selection, including Auto's
  effective policy even while another policy is being drafted. Auto display
  follows the saved environment rather than an unsaved environment selection.
- The import preview defaults to `disabled` on each new file selection. Its
  explicit `mcpPolicy` option reaches the existing import transaction for V1,
  V2 and encrypted bundles. Omitted options remain disabled. Existing strict
  bundle schemas continue to reject embedded permission fields.
- Policy validation remains in Workspace Engine, before credential staging.
  Domain validation, ID remapping, credential cleanup, transaction hooks and
  business-record rollback keep their original paths.
- New Workspace creation still defaults to Auto. DEV → `full_access`, TEST →
  `guarded`, PROD → `read_only` is unchanged. MCP permission evaluation is
  unchanged, and no MCP tool can edit policy or environment type.
- No dependencies, storage migrations or new backend commands were added.
  The security component lives in workspace-local; app-shell only mounts it.

## Changed files

| Area | Files |
| --- | --- |
| Local security UI | `packages/workspace-local/src/WorkspaceSecurityDialog.tsx`, `WorkspaceMcpPolicyField.tsx`, `index.ts` |
| Import UI | `packages/workspace-local/src/WorkspaceBundleExchange.tsx` |
| Shared display policy | `packages/workspace-core/src/workspace-mcp-policy.ts`, `index.ts` |
| Shell wiring and summaries | `packages/app-shell/src/components/WorkspaceDialogs.tsx`, `WorkspaceMenu.tsx` |
| Existing import option contract | `packages/command-client/src/tauri/workspace.ts`; `crates/unfour-command-bus/src/workspace_bundle/local.rs`, `remap.rs`, `mod.rs`, `backup.rs` |
| Shared i18n | `packages/ui/src/i18n/locales/en.json`, `zh-CN.json` |
| Frontend regression tests | `WorkspaceSecurityDialog.test.tsx`, `WorkspaceBundleExchange.test.tsx`, `workspace-mcp-policy.test.ts`, `WorkspaceMenu.test.tsx` in their owning source directories |
| Rust regression tests | `crates/unfour-command-bus/src/workspace_bundle/mcp_policy_tests.rs`, `backup_tests.rs`; `crates/unfour-mcp/src/tools/tools_tests/mod.rs` |
| Browser smoke | `apps/desktop/tests/smoke/workspace-bundle.spec.ts` |
| Documentation | `docs/architecture/workspace-bundle.md`, this record, and the index link in `docs/testing/workspace-bundle.md` |

## Verification

| Check | Result | Evidence |
| --- | --- | --- |
| `pnpm exec vitest run packages/workspace-local/src packages/workspace-core/src/workspace-mcp-policy.test.ts packages/app-shell/src/components/WorkspaceMenu.test.tsx packages/command-client/src/tauri.test.ts packages/ui/src/i18n.test.ts` | PASS, 56 tests / 6 files | Default disabled, four explicit grants, reset on next import, import failure draft preservation, imported permission edits, independent saves, cache refresh, pending saves, failed save retry, Auto rules and client/i18n regressions |
| Final saved-Auto summary regression: `pnpm exec vitest run packages/workspace-local/src/WorkspaceSecurityDialog.test.tsx` | PASS, 8 tests | Saved effective permission remains visible while a different policy is drafted |
| `cargo test -p unfour-command-bus workspace_bundle --lib` | PASS, 33 tests | Existing bundle, secret, credential, compatibility and transaction regressions plus six new permission tests |
| `cargo test -p unfour-command-bus encrypted_import_uses_the_explicit_local_mcp_policy --lib` | PASS, 1 additional test | Encrypted source policy is not inherited; local selection is persisted |
| Compiled Command Bus test binary, filter `workspace_bundle::backup_tests::encrypted_` | PASS, 2 tests | Final default-disabled and explicitly authorized encrypted imports |
| `cargo test -p unfour-mcp policy --lib` | PASS, 20 tests | Auto mapping, explicit overrides, disabled/read-only restrictions, risk/confirmation behavior and registry classification |
| Compiled MCP test binary, filter `agents_cannot_modify_workspace_mcp_permissions` | PASS, 1 test | Policy-editing command names and aliases are not exposed as tools |
| `pnpm exec playwright test apps/desktop/tests/smoke/workspace-bundle.spec.ts` | PASS, 4 tests | English/Chinese at 960×600: preview default/reset, imported permission display, independent edits, Auto and visible footer |
| Targeted ESLint; `pnpm --filter @unfour/desktop exec tsc --noEmit`; `pnpm run build` | PASS | Changed frontend sources, cross-package exports and desktop build |
| `cargo check -p unfour-app`; `cargo fmt -p unfour-command-bus -p unfour-mcp --check`; `git diff --check` | PASS | Existing Tauri adapter option compatibility, Rust format and whitespace |

Initial sandbox runs could not access pnpm's database, Vite realpaths or Cargo
paths. Approved retries passed. An outdated safety-tier test expectation was
updated, and exact browser label lookup was fixed with explicit localized
`aria-label` values; the affected tests passed on rerun.

## Limits and remaining risks

Browser smoke mocks native file selection/import and seeds the browser store;
it verifies React, navigation, query-cache and browser command behavior. Backend
tests use isolated SQLite and in-memory credentials. A packaged Tauri application
with native file dialogs and the real OS credential store was **NOT VERIFIED**.

Builds retain existing warnings for the default test release channel, unused SSH
tracker `reset`, and large frontend chunks. These did not fail the affected
checks. This record makes no platform packaging or live-service readiness claim.

## PROD risk notice regressions — 2026-10-12

The two independent security saves now preview their actual combinations:
environment draft + saved policy, and saved environment + policy draft.
The saved environment is displayed separately, and each notice explains which
saved counterpart the action preserves. This also fixes the reverse bypass:
drafting disabled/read-only permissions cannot hide the risk of saving PROD
while guarded/full access remains persisted. The small workspace-local
`WorkspaceMcpRiskNotice` component supplies the same PROD rules and localized
message to security settings and bundle import. Import still starts disabled,
ignores permission fields in the file, and keeps its existing review flow.
Auto resolution, backend authorization and persistent schemas are unchanged.

Changed files: `WorkspaceSecurityDialog.tsx`, `WorkspaceBundleExchange.tsx`,
the new `WorkspaceMcpRiskNotice.tsx` and the two adjacent component test files;
`packages/ui/src/i18n/locales/{en,zh-CN}.json`;
`apps/desktop/tests/smoke/workspace-bundle.spec.ts`; this verification record.

| Check | Result | Evidence |
| --- | --- | --- |
| Targeted Vitest: both workspace-local component suites, workspace-core MCP policy, UI i18n helpers/provider, WorkspaceMenu and DesktopApp smoke | PASS, 86 tests / 7 files | English/Chinese independent saves, unsaved counterpart drafts, dynamic warnings before/after success, both save failure/retry paths, PROD import defaults/overrides/reset/retry, refreshed encrypted preview, unchanged Auto rules and shell integration |
| `pnpm exec playwright test apps/desktop/tests/smoke/workspace-bundle.spec.ts` | PASS, 4 tests | English/Chinese at 960×600: PROD import Auto/guarded/full access notices, read-only dismissal, saved PROD with an unsaved DEV environment and permission-only save, reopening and independent environment save; screenshots inspected |
| ESLint on all six changed TypeScript/TSX source and test files, with `--max-warnings 0` | PASS, zero warnings | No thresholds or exclusions changed |
| `pnpm --filter @unfour/desktop exec tsc --noEmit` | PASS | Desktop and imported workspace components |
| `pnpm run build` | PASS | Production frontend build; only the existing >500 kB chunk advisory remains. A comparison build loaded the original four production files from HEAD without replacing working-tree files and reproduced that advisory (747.32 kB baseline / 748.12 kB after fix) |
| `git diff --check` | PASS | Scoped diff and whitespace |

One initial Chinese browser-button lookup in a component test used the wrong
label; it was corrected and the final suite passed. Added browser assertions
initially exceeded the test complexity warning threshold; shared expected
localized strings removed that duplication, and final Lint passed with zero
warnings. The first browser run also reported conflicting terminal color
environment variables; the final run removed `NO_COLOR` while retaining
Playwright's `FORCE_COLOR` and emitted no warning.

Native Tauri file dialogs/credential-store behavior and live Agent operations
were **NOT VERIFIED** in this UI-only follow-up. Component and browser tests
mock native import/file selection; no Rust or backend semantics were modified.
