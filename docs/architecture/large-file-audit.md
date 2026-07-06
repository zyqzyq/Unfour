# Large File Audit

Generated on 2026-07-05.

This audit is an architectural review aid, not a mandate to split files by line
count alone. Splits should preserve public TypeScript exports, Rust command
names, command-bus method names, database schema, and MCP tool names.

## Scope

The audit uses `scripts/check-large-files.mjs` and scans only:

- `apps`
- `packages`
- `crates`
- `docs`

The script excludes dependency, build, coverage, generated, lock, bundled, and
minified outputs. It intentionally does not use a 150-line limit. Categories are:

| Category | Rule |
| --- | --- |
| Critical | More than 1200 lines |
| P0 | More than 800 lines |
| P1 | More than 500 lines |

Current summary: 23 files above P1; 6 Critical, 3 P0, 14 P1; 0 contain test
code; 0 look like generated/build artifacts.

## Test File Organization

Rust:

- Small focused tests may stay inline in `#[cfg(test)] mod tests`.
- If inline tests exceed roughly 80-120 lines, move them into
  `src/<module>_tests/`.
- Tests that need crate-private implementation access should live in
  `src/<module>_tests/` and be declared from the owning module.
- Integration tests that exercise only public APIs should live in
  `crates/<crate>/tests/`.
- Shared fixtures, mocks, and helpers should live in `test_support` or
  `tests/common`.

TypeScript / React:

- Tests should live in `src/__tests__/` or adjacent
  `*.test.ts` / `*.test.tsx` files.
- Large mocks and fixtures should live in `__fixtures__/`.
- Do not pile large tests or mock data at the bottom of component files.

## Current Large Files

| # | Path | Lines | Category | Test code | Suspected artifact |
| --- | --- | ---: | --- | --- | --- |
| 1 | `crates/database-engine/src/database.rs` | 3371 | Critical | No | No |
| 2 | `crates/ssh-engine/src/ssh.rs` | 2131 | Critical | No | No |
| 3 | `crates/http-engine/src/api_client.rs` | 1723 | Critical | No | No |
| 4 | `crates/unfour-command-bus/src/lib.rs` | 1679 | Critical | No | No |
| 5 | `packages/database/src/DatabasePage.tsx` | 1649 | Critical | No | No |
| 6 | `crates/unfour-mcp/src/tools/api.rs` | 1428 | Critical | No | No |
| 7 | `crates/unfour-mcp/src/tools/database.rs` | 1170 | P0 | No | No |
| 8 | `crates/unfour-mcp/src/tools/ssh.rs` | 886 | P0 | No | No |
| 9 | `packages/database/src/components/DatabaseConnectionTree.tsx` | 831 | P0 | No | No |
| 10 | `packages/ui/src/tree-view.tsx` | 793 | P1 | No | No |
| 11 | `crates/unfour-core/src/models.rs` | 790 | P1 | No | No |
| 12 | `packages/api-client/src/components/ApiCollectionTree.tsx` | 789 | P1 | No | No |
| 13 | `packages/ssh-terminal/src/TerminalPage.tsx` | 740 | P1 | No | No |
| 14 | `packages/api-client/src/model/request-tabs.ts` | 605 | P1 | No | No |
| 15 | `crates/workspace-engine/src/workspace.rs` | 584 | P1 | No | No |
| 16 | `crates/unfour-mcp/src/command_bus_adapter.rs` | 582 | P1 | No | No |
| 17 | `packages/api-client/src/components/ResponseTabs.tsx` | 572 | P1 | No | No |
| 18 | `packages/api-client/src/request-utils.ts` | 561 | P1 | No | No |
| 19 | `packages/database/src/components/SqlEditorTab.tsx` | 560 | P1 | No | No |
| 20 | `packages/command-client/src/types.ts` | 558 | P1 | No | No |
| 21 | `packages/api-client/src/ApiClientPage.tsx` | 542 | P1 | No | No |
| 22 | `crates/unfour-mcp/src/tools/policy.rs` | 517 | P1 | No | No |
| 23 | `packages/ui/src/shell.tsx` | 507 | P1 | No | No |

## Completed Test-Only Splits

This batch moved large inline tests out of business files without changing
business behavior, public APIs, Tauri command signatures, database schema, or
MCP tool names/schemas.

| Original file | New test organization |
| --- | --- |
| `crates/local-storage/src/local_db.rs` | `crates/local-storage/src/local_db_tests/` split into scenario modules |
| `crates/local-storage/src/terminal_history.rs` | `crates/local-storage/src/terminal_history_tests/` |
| `crates/secret-store/src/secret_store.rs` | `crates/secret-store/src/secret_store_tests/` |
| `crates/ssh-engine/src/host_key.rs` | `crates/ssh-engine/src/host_key_tests/` |
| `crates/workspace-engine/src/workspace.rs` | `crates/workspace-engine/src/workspace_tests/` |
| `crates/unfour-mcp/src/tools/mod.rs` | `crates/unfour-mcp/src/tools/tools_tests/` |
| `crates/unfour-mcp/src/command_bus_adapter.rs` | `crates/unfour-mcp/src/command_bus_adapter_tests/` |
| `crates/unfour-mcp/src/tools/policy.rs` | `crates/unfour-mcp/src/tools/policy_tests/` |
| `crates/unfour-mcp/src/server.rs` | `crates/unfour-mcp/src/server_tests/` |
| `crates/unfour-mcp/src/tools/database_tests.rs` | `crates/unfour-mcp/src/tools/database_tests/` scenario modules |
| `crates/unfour-mcp/src/tools/api_tests.rs` | `crates/unfour-mcp/src/tools/api_tests/` scenario modules |
| `crates/ssh-engine/src/ssh_tests/session.rs` | `crates/ssh-engine/src/ssh_tests/session/` scenario modules |
| `crates/unfour-diag/src/lib.rs` | `crates/unfour-diag/src/lib_tests.rs` sibling module |

## Completed Business Responsibility Splits

These batches split adapter boundaries by command domain without changing
public export names, caller import paths, Rust Tauri command names, command
argument shapes, return shapes, database schema, or MCP tool names,
descriptions, and input schemas.

| Original file | New organization |
| --- | --- |
| `packages/command-client/src/tauri.ts` | Public facade re-exporting `src/tauri/{api,database,diagnostics,secret-store,ssh,workspace}.ts`; shared invoke/runtime code in `src/tauri/invoke.ts`; browser-dev mocks in `src/tauri/browser-mocks/` with per-domain handlers |
| `crates/unfour-app/src/commands.rs` | Rust facade re-exporting `src/commands/{api,database,diagnostics,secret_store,ssh,workspace}.rs`; shared command tracing remains in `commands.rs`; existing `invoke_handler` paths remain valid through the facade |

## Remaining Large Test Files

No files containing test code currently exceed the P1 threshold.

## Remaining Business Responsibility Splits

These files no longer have large inline test blocks from this batch. They still
need future splits by responsibility, not by line count alone.

| Path | Recommended split direction | Risk |
| --- | --- | --- |
| `crates/database-engine/src/database.rs` | Keep `DatabaseService` as facade; extract driver modules, query safety, browse, row mutation, and storage conversion helpers. | High |
| `crates/ssh-engine/src/ssh.rs` | Keep `SshService` facade; extract native transport, session registry, persistence conversion, diagnostics, and redaction. | High |
| `crates/http-engine/src/api_client.rs` | Keep `ApiClientService` facade; extract send/history, environments, saved requests, collections, and folder ordering. | High |
| `crates/unfour-command-bus/src/lib.rs` | Move read models and domain forwarding methods into modules; re-export through `lib.rs`. | High |
| `packages/database/src/DatabasePage.tsx` | Extract connection dialog, page controller hooks, query-history orchestration, and shell slot builders. | Medium |
| `crates/unfour-mcp/src/tools/api.rs` | Extract API argument parsers, redaction/truncation, and serializers; keep tool registry names unchanged. | High |
| `crates/unfour-mcp/src/tools/database.rs` | Extract SQL validators, risk classifier, result truncation, and connection summary helpers. | High |
| `crates/unfour-mcp/src/tools/ssh.rs` | Extract command builders, file operation helpers, workspace resolution, and parsers. | High |
| `packages/database/src/components/DatabaseConnectionTree.tsx` | Extract tree model builders, context menus, status labels, and generated SQL snippet helpers. | Medium |
| `packages/ui/src/tree-view.tsx` | Extract flatten/search/typeahead helpers and drag/drop target math; keep `TreeView` public API stable. | Medium |
| `crates/unfour-core/src/models.rs` | Split models by domain only with a compatibility re-export layer; do not rename fields. | High |
| `packages/api-client/src/components/ApiCollectionTree.tsx` | Extract dialog state, tree item builders, drag/drop movement helpers, and export helpers. | Medium |
| `packages/ssh-terminal/src/TerminalPage.tsx` | Extract connection form hook, session action handlers, host-key trust flow, and shell sidebar builder. | Medium |
| `packages/api-client/src/model/request-tabs.ts` | Extract method metadata, response-state derivation, tab close helpers, and history grouping. | Medium |
| `crates/workspace-engine/src/workspace.rs` | Extract layout parsing/validation and environment/policy normalization helpers if the service grows further. | Medium |
| `crates/unfour-mcp/src/command_bus_adapter.rs` | Split per-domain adapter mapping helpers without changing adapter method names. | High |
| `packages/api-client/src/components/ResponseTabs.tsx` | Extract body viewer, snapshot readout, cookies parser, timing metrics, and state panels. | Medium |
| `packages/api-client/src/request-utils.ts` | Extract import/export, auth metadata, key-value utilities, and redaction helpers. | Medium |
| `packages/database/src/components/SqlEditorTab.tsx` | Extract Monaco setup, saved SQL dialogs, SQL formatting helpers, and action toolbar. | Medium |
| `packages/command-client/src/types.ts` | Split frontend command types by domain with re-exports; keep names and shape stable. | High |
| `packages/api-client/src/ApiClientPage.tsx` | Extract close/save workflow hook, environment tab controller, and sidebar intent handler. | Medium |
| `crates/unfour-mcp/src/tools/policy.rs` | Extract confirmation text and policy classification helpers if the module grows further. | Medium |
| `packages/ui/src/shell.tsx` | Extract resize behavior into hooks and split shell surface components if shell helpers continue growing. | Medium |

## Baseline Status

`scripts/large-files-baseline.json` continues to cover the reviewed
grandfathered files that are still above the higher thresholds:

- `crates/database-engine/src/database.rs`
- `crates/ssh-engine/src/ssh.rs`
- `crates/http-engine/src/api_client.rs`
- `crates/unfour-command-bus/src/lib.rs`
- `crates/unfour-mcp/src/tools/database.rs`
- `crates/unfour-mcp/src/tools/api.rs`
- `packages/database/src/DatabasePage.tsx`

No split in this batch required adding a new baseline exception. The previous
`packages/command-client/src/tauri.ts` baseline entry was removed because the
facade is no longer oversized.

## Priority Notes

Top remaining business split candidates:

1. `crates/unfour-command-bus/src/lib.rs`
2. `packages/database/src/DatabasePage.tsx`
3. `packages/database/src/components/DatabaseConnectionTree.tsx`
4. `crates/unfour-mcp/src/tools/api.rs`
5. `crates/unfour-mcp/src/tools/database.rs`
6. `crates/unfour-mcp/src/tools/ssh.rs`
7. `crates/database-engine/src/database.rs`
8. `crates/ssh-engine/src/ssh.rs`
9. `crates/http-engine/src/api_client.rs`

Recommended next test-only cleanup:

1. None currently identified above the P1 threshold.
