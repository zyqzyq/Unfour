# API CommandBus structure

Updated on **2026-11-15 (Asia/Shanghai)** for the API command structural refactor.
The adapter call chain remains `adapter -> CommandBus -> service -> driver`.
No public method signature, database schema, Cloud Sync protocol, or MCP tool
contract changes with this split.

## Command ownership

[`api_commands.rs`](../../crates/unfour-command-bus/src/api_commands.rs) is the
private module assembly facade. Its four private child modules define inherent
`impl CommandBus` methods, so Tauri, MCP, and other callers keep using the same
`CommandBus` API without forwarding wrappers or a second command framework.

| Module under `api_commands/` | Responsibility |
| --- | --- |
| `environments.rs` | Legacy API Environment compatibility methods; preserve variable IDs, secret classification, and descriptions, and pass the existing snapshot to the checked workspace update |
| `collections.rs` | Collection and Folder CRUD, moves and ordering, checked renames, and Folder delete snapshots; orchestrate the existing API Client Service |
| `requests.rs` | Request save/update/duplicate/delete, moves and ordering, saved-request and History queries; keep request route writes inside the same domain transaction |
| `execution.rs` | Direct sends and Saved Request Replay, including replay from an already authorized snapshot; delegate to existing environment, routing, and script execution paths |

`workspace_layout` and `workspace_layout_update` belong to the workspace portion
of [`core_commands.rs`](../../crates/unfour-command-bus/src/core_commands.rs).
They retain their existing Workspace Service calls.

## Existing boundaries retained

| Existing module | Responsibility retained |
| --- | --- |
| [`api_execution.rs`](../../crates/unfour-command-bus/src/api_execution.rs) | Execution registry, cancellation leases, timeout preferences and validation |
| [`api_script_commands.rs`](../../crates/unfour-command-bus/src/api_script_commands.rs) | Script execution orchestration through `send_api_request_via`, including pre/post environment commits and result preservation |
| [`api_routes.rs`](../../crates/unfour-command-bus/src/api_routes.rs) | Route selection and preview checks, SSH revision checks, and routed HTTP transport |
| [`workspace_variable_commands.rs`](../../crates/unfour-command-bus/src/workspace_variable_commands.rs) | Workspace Environment mutation, selection, variable resolution, and auth materialization used by API and other command paths |
| [`ApiClientService`](../../crates/http-engine/src/api_client.rs) | Collection/Request domain operations, revision and snapshot checks, HTTP execution, executed-request snapshots, History, and secret projections in its existing child modules |

The extraction does not move domain logic into adapters or duplicate Service
validation, workspace isolation, permission checks, or execution entry points.

## Transaction and completion contracts

Authoring commands continue through the existing
[`transaction.rs`](../../crates/unfour-command-bus/src/transaction.rs) helpers.
The order remains: begin transaction; execute the Service operation and any
request route write; derive and record `CommandActivity`; reconcile credential
references; run transactional mutation hooks; commit; perform credential garbage
cleanup. The existing no-mutation Activity policy is unchanged. Service,
Activity, credential reconciliation, or hook errors before commit roll back the
transaction. Durable Outbox intent produced by installed mutation hooks stays
atomic with the business mutation.

Routed save/update writes use the same connection as the Request mutation.
Duplicate reads the original route before duplicating and writes the copied
route before finalization. Revision checks stay in the Service; the Folder
delete-snapshot comparison stays in the CommandBus executor, using the Service
snapshot read on the same transaction connection. Activity action names,
targets, details, and mutation order are unchanged.

HTTP completion is separate from authoring transactions. Once a complete HTTP
response is available, History or completion Activity failure adds warnings and
retains the response. Post-script failure or cancellation also retains that
response and already committed environment changes. Once post-script persistence
starts, execution waits for its acknowledgement and cleanup even if cancelled;
an uncertain or already committed local outcome must not be hidden by dropping
the future or treated as a reason to resend the HTTP request.

## Execution paths remain distinct

- Direct send selects the environment, resolves variables and auth, resolves the
  desktop timeout, selects routes, sends HTTP, then records completion Activity.
- Plain Saved Request Replay checks the requested workspace, builds the saved
  input with `mcp_timeout` (60 seconds by default), resolves environment and
  secrets, selects the saved route, then sends HTTP. It does not gain a new
  completion Activity call through this refactor.
- Scripted replay uses the supplied saved definition and delegates to the
  existing script path. An already authorized snapshot is not replaced by a
  fresh request read. The script path registers cancellation before preparation,
  fixes the environment and route, runs and commits a successful pre-script,
  resolves the final request and secrets, sends HTTP, completes the post phase,
  then records Activity. Pre-script failure prevents HTTP execution.

Runtime secrets and derived script values continue to reach the intended wire
request while snapshots, History, diagnostics, and Activity use the existing safe
projections. Completion Activity obtains the method from the sanitized executed
snapshot. Cancellation cleanup, timeout defaults, warning codes, and the order
of these paths are compatibility constraints, not consolidation opportunities.
