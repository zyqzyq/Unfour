# Large File Audit

Updated on **2026-11-07 (Asia/Shanghai)** against the v0.13.0 hardening tree.
Entry baseline: `48df06828de6a642ca4b195bf56692fd0835e8d8` on `main`;
previous release: `v0.12.0` (`4883df8dbfaf4d9d69c94ed5bae75c627d146e34`).
This replaces the stale 2026-07-05 inventory; earlier splits remain in Git history.
See [verification and audit decisions](../testing/release-hardening-2026-11-07.md).

## Method and baseline

Run `node scripts/check-large-files.mjs --json --top 200` from the repository root.
The existing checker scans `apps`, `packages`, `crates`, and `docs`, with its
existing generated/dependency/build exclusions. Exact pinned Monaco copies
are excluded by byte comparison, not by trusting a filename alone.

Categories remain **Critical >1200**, **P0 >800**, **P1 >500** lines.
The categories are exclusive. A test-code flag can also mark an implementation
file with inline tests; it is not a count of test-only files. Line counts are
physical lines, distinct from ESLint's configured counts.

| Snapshot | Critical | P0 | P1 | Total >500 | With test code | Blocking |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Entry main | 0 | 24 | 71 | 95 | 57 | 0 |
| After hardening | 0 | 22 | 71 | 93 | 57 | 0 |

`scripts/large-files-baseline.json` has **zero exceptions**. No checker threshold,
exclusion, ESLint rule, suppression or baseline allowance was added or relaxed.
A passing size gate does not imply every long function has been decomposed.

## Responsibility changes

| Existing file | Before | After | Responsibility moved or consolidated |
| --- | ---: | ---: | --- |
| `crates/database-engine/src/database/sql.rs` | 856 | 388 | SQL safety into `sql_safety.rs`; identifier/browse/export builders stay here |
| `crates/unfour-mcp/src/tools/database.rs` | 1005 | 598 | Registration/schema into `database_registry.rs`; handlers and policy stay here |
| `crates/ssh-engine/src/ssh/native_transport.rs` | 598 | 159 | Authentication into `native_auth.rs`, terminal supervision into `native_supervisor.rs`; shared dial stays here |
| `crates/ssh-engine/src/ssh/session.rs` | 789 | 779 | Workspace-scoped teardown shared by close, reconnect cancellation and failed-connect cleanup |
| `crates/ssh-engine/src/ssh/diagnostic_execution.rs` | 178 | 143 | Uses the shared authenticated connection entry point |
| `crates/ssh-engine/src/task/native.rs` | 169 | 157 | Uses the shared dial entry point and existing authentication |

The extracted SQL classifier, SSH authentication and MCP registration definitions
retain their bodies; visibility/import wiring changes only. The existing engine
facades, CommandBus trait and public exports remain. Authentication, host-key
identity, timeout policy and per-consumer error messages are preserved.
See [SSH transport ownership](ssh-transport-boundary.md).

## Remaining priorities

| Area | Current decision | Next useful boundary / risk |
| --- | --- | --- |
| MCP `command_bus_adapter.rs` (1158) | Keep the trait implementation and bounded runtime facade | Forwarders are repetitive but splitting one trait impl would need delegation or macros; do not add indirection just for size. Revisit with an actual adapter capability change. |
| CommandBus `api_commands.rs` (906) | Keep transactional orchestration | Separate execution from authoring only with existing activity, outbox and credential-lock tests; do not bypass the bus. |
| Database export (820), Postgres (780) | Retain driver/stream ownership | Export cancellation, rollback and cursor disposal need protocol fixtures before changes. Pools are per-operation, not a shared cache. |
| SSH SFTP (799), session (current count above) | Retain session/generation ownership | Transfer registration and reconnect cleanup have race-sensitive shared state; next split must preserve lock ordering and stale-generation rejection. |
| HTTP OpenAPI import (823), collection domain (786) | Keep existing service/domain boundary | Parser/collection normalization can be isolated when behavior fixtures establish compatibility; do not remove legacy inputs by assumption. |
| SSH TerminalPage (726), SshTasksPage (657) | Retain current controller and dialog ownership | Form/test/trust flows are candidates, but workspace remount and stale-save behavior require page-level coverage. |
| Database SqlEditorTab (628) | Keep editor/execution context together this round | Extract execution UI only with selection, cancellation, context and multi-statement regressions. |
| API RequestParamsTabs (584), request-tabs model (541) | Preserve draft/send/history contracts | Script/body sub-editors are candidates; avoid a second request-state mechanism. |
| UI TreeView (421) | No size-driven split | Generic primitive; function length warning remains. No feature imports or new state framework introduced. |
| Account / Cloud Sync | Preserve generation, outbox and single-owner invariants | Live OAuth, account switches and multi-device conflict/offline tests still require real-service acceptance. |

The broad audit uses source inventory, owner/call-chain inspection, lint and
existing tests; it is not an exhaustive proof of every interleaving. No production
file is marked safe solely because it falls below 500 lines. Current ESLint
warnings and live-service limits are recorded in the verification report.

## Full current inventory over 500 lines

| Path | Lines | Category | Contains test code |
| --- | ---: | --- | --- |
| `crates/unfour-cloud-sync/tests/outbox.rs` | 1200 | P0 | Yes |
| `crates/unfour-mcp/src/command_bus_adapter.rs` | 1158 | P0 | No |
| `crates/unfour-cloud-sync-storage/src/lib.rs` | 1082 | P0 | Yes |
| `packages/flow/src/FlowPage.test.tsx` | 1070 | P0 | Yes |
| `crates/unfour-cloud-sync/src/canonical.rs` | 1047 | P0 | No |
| `crates/unfour-command-bus/tests/ssh_task_domain.rs` | 1033 | P0 | Yes |
| `crates/unfour-cloud-sync/tests/worker/hierarchy_conflicts.rs` | 995 | P0 | Yes |
| `crates/unfour-command-bus/tests/connection_domain.rs` | 992 | P0 | Yes |
| `crates/unfour-cloud-sync/tests/worker/ssh_tasks.rs` | 985 | P0 | Yes |
| `crates/unfour-cloud-sync/tests/worker/dead_letter_recovery.rs` | 949 | P0 | Yes |
| `crates/unfour-command-bus/src/lib_tests.rs` | 935 | P0 | Yes |
| `crates/workspace-engine/src/workspace_tests/mod.rs` | 913 | P0 | Yes |
| `crates/unfour-command-bus/src/api_commands.rs` | 906 | P0 | No |
| `crates/unfour-mcp/src/command_bus_adapter_tests/flow.rs` | 894 | P0 | Yes |
| `packages/api-client/src/components/ApiCollectionTree.test.tsx` | 879 | P0 | Yes |
| `docs/testing/release-verification.md` | 881 | P0 | No |
| `crates/database-engine/src/database_tests/sqlite.rs` | 838 | P0 | Yes |
| `crates/unfour-app/src/mcp_client_config.rs` | 835 | P0 | Yes |
| `crates/http-engine/src/api_client/openapi_import.rs` | 823 | P0 | Yes |
| `crates/http-engine/src/api_client_tests/collections.rs` | 822 | P0 | Yes |
| `crates/database-engine/src/database/export.rs` | 820 | P0 | Yes |
| `crates/unfour-mcp/src/tools/tools_tests/mod.rs` | 803 | P0 | Yes |
| `crates/ssh-engine/src/ssh/sftp.rs` | 799 | P1 | No |
| `crates/http-engine/src/api_client/domain/collections.rs` | 786 | P1 | No |
| `crates/database-engine/src/database/postgres.rs` | 780 | P1 | Yes |
| `crates/unfour-mcp/src/tools/api_handlers.rs` | 773 | P1 | No |
| `crates/ssh-engine/src/ssh/session.rs` | 779 | P1 | No |
| `crates/unfour-account/src/lib.rs` | 769 | P1 | Yes |
| `crates/unfour-mcp/src/command_bus_adapter/contract.rs` | 752 | P1 | No |
| `crates/unfour-command-bus/src/lib_tests/flow_authoring.rs` | 747 | P1 | Yes |
| `crates/unfour-command-bus/src/ssh_commands.rs` | 745 | P1 | No |
| `crates/unfour-cloud-sync/tests/worker/connections.rs` | 738 | P1 | Yes |
| `crates/unfour-mcp/src/tools/api.rs` | 737 | P1 | No |
| `crates/unfour-command-bus/src/core_commands.rs` | 736 | P1 | No |
| `packages/ssh-terminal/src/hooks/useSshTaskRunSession.test.tsx` | 734 | P1 | Yes |
| `crates/unfour-cloud-sync/tests/worker/api_hierarchy.rs` | 729 | P1 | Yes |
| `packages/ssh-terminal/src/TerminalPage.tsx` | 726 | P1 | No |
| `crates/unfour-command-bus/src/workspace_variable_commands.rs` | 724 | P1 | No |
| `crates/unfour-cloud-sync/tests/worker/ssh_tasks/conflicts.rs` | 717 | P1 | Yes |
| `crates/http-engine/src/api_client/domain/requests.rs` | 694 | P1 | Yes |
| `crates/http-engine/src/api_client/domain/secrets.rs` | 693 | P1 | Yes |
| `crates/unfour-core/src/domain.rs` | 684 | P1 | Yes |
| `crates/workspace-engine/src/workspace/variable_executor.rs` | 683 | P1 | No |
| `crates/unfour-cloud-sync/tests/contracts.rs` | 676 | P1 | Yes |
| `crates/unfour-paths/src/lib.rs` | 666 | P1 | Yes |
| `crates/http-engine/src/script_runtime.rs` | 665 | P1 | Yes |
| `crates/unfour-cloud-sync/src/repository/recovery.rs` | 663 | P1 | No |
| `packages/ssh-terminal/src/components/SshTasksPage.tsx` | 657 | P1 | No |
| `packages/ssh-terminal/src/components/TerminalPane.output.test.tsx` | 651 | P1 | Yes |
| `crates/unfour-command-bus/tests/api_domain/snapshot_apply.rs` | 644 | P1 | Yes |
| `apps/desktop/src-tauri/src/account.rs` | 639 | P1 | Yes |
| `crates/workspace-engine/src/workspace.rs` | 633 | P1 | No |
| `crates/unfour-mcp/src/tools/workspace.rs` | 629 | P1 | Yes |
| `packages/database/src/components/SqlEditorTab.tsx` | 628 | P1 | No |
| `crates/unfour-command-bus/src/lib_tests/activity_native.rs` | 625 | P1 | Yes |
| `crates/unfour-cloud-sync/src/repository/reconciliation.rs` | 613 | P1 | No |
| `crates/unfour-cloud-sync/src/transport.rs` | 612 | P1 | No |
| `crates/unfour-mcp/src/tools/ssh_tests/history.rs` | 611 | P1 | Yes |
| `crates/unfour-command-bus/tests/workspace_domain/external_apply.rs` | 607 | P1 | Yes |
| `crates/unfour-mcp/src/tools/api_tests.rs` | 607 | P1 | Yes |
| `crates/unfour-cloud-sync/src/canonical/tests.rs` | 604 | P1 | Yes |
| `packages/ssh-terminal/src/components/TerminalPane.tsx` | 599 | P1 | No |
| `crates/unfour-mcp/src/tools/database.rs` | 598 | P1 | No |
| `crates/unfour-mcp/src/tools/database_tests.rs` | 595 | P1 | Yes |
| `crates/unfour-account/src/client.rs` | 589 | P1 | Yes |
| `crates/local-storage/src/local_db_tests/scenarios/migrations.rs` | 584 | P1 | Yes |
| `packages/api-client/src/components/RequestParamsTabs.tsx` | 584 | P1 | No |
| `crates/unfour-mcp/src/tools/policy.rs` | 582 | P1 | No |
| `crates/unfour-cloud-sync/tests/worker/scheduling.rs` | 576 | P1 | Yes |
| `crates/unfour-cloud-sync/src/entity_registry.rs` | 575 | P1 | Yes |
| `packages/ssh-terminal/src/components/SftpPanel.tsx` | 574 | P1 | No |
| `crates/unfour-command-bus/src/flow_commands.rs` | 573 | P1 | Yes |
| `crates/database-engine/src/database/connection_domain.rs` | 570 | P1 | No |
| `crates/ssh-engine/src/ssh/connection_domain.rs` | 569 | P1 | No |
| `crates/ssh-engine/src/task/domain/external_apply.rs` | 569 | P1 | No |
| `crates/unfour-cloud-sync/src/repository/bindings.rs` | 568 | P1 | No |
| `crates/ssh-engine/src/ssh/connection_crud.rs` | 565 | P1 | No |
| `crates/unfour-cloud-sync/src/types.rs` | 561 | P1 | No |
| `apps/desktop/src-tauri/src/sync.rs` | 560 | P1 | Yes |
| `crates/unfour-cloud-sync/src/service/worker.rs` | 551 | P1 | Yes |
| `packages/api-client/src/model/request-tabs.test.ts` | 551 | P1 | Yes |
| `crates/database-engine/src/database_tests/safety.rs` | 548 | P1 | Yes |
| `crates/unfour-core/src/models/database.rs` | 547 | P1 | No |
| `packages/api-client/src/model/request-tabs.ts` | 541 | P1 | No |
| `crates/unfour-mcp/src/command_bus_adapter_tests/mod.rs` | 532 | P1 | Yes |
| `packages/app-shell/src/DesktopApp.test.tsx` | 522 | P1 | Yes |
| `crates/unfour-cloud-sync/src/service.rs` | 518 | P1 | No |
| `crates/database-engine/src/database/tables.rs` | 513 | P1 | No |
| `crates/unfour-command-bus/src/workspace_bundle/backup_tests.rs` | 513 | P1 | Yes |
| `crates/unfour-command-bus/src/lib_tests/ssh_task_context.rs` | 512 | P1 | Yes |
| `crates/unfour-app/src/lib.rs` | 505 | P1 | Yes |
| `crates/unfour-cloud-sync/tests/worker/accounts.rs` | 505 | P1 | Yes |
| `crates/unfour-core/src/redaction.rs` | 502 | P1 | Yes |

## Test organization

Keep focused tests adjacent to the implementation or under its existing
`*_tests` module. Integration tests exercise public APIs; private-state lifecycle
regressions remain with the owning crate. Shared fixtures belong to test support,
not production UI modules. Split tests by scenario when it improves ownership,
not merely to move a size warning to another file.
