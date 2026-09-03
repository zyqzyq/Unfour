# Cloud Sync Invariants

Cloud Sync is a local-first overlay. The local SQLite database remains the
source of truth for local work, while the Cloud Sync tables record the durable
intent needed to publish a bound workspace later.

## Durable local intent

For every local mutation in a cloud-bound workspace, the business-row commit
and the corresponding outbox head are one transaction. This remains true when
the user is signed out, the account context is inactive, the entitlement is
not currently usable, sync is paused globally or for the workspace, the device
is offline, or the worker is stopped. Those states may prevent a network
attempt; they must not prevent durable intent capture.

The outbox destination is selected by the binding's ownership tuple:

```text
(account_id, local_workspace_id, cloud_workspace_id)
```

`cloud_sync_runtime_context.active_account_id` is only a runtime eligibility
and wake-up signal. It must never be used to decide whether a binding receives
an outbox row. This keeps an offline or signed-out edit available for the
binding that owns it without transferring ownership to another account.

## Account and pause behavior

Bindings and outbox rows are always read and written with an explicit account
and cloud workspace scope. A mutation made while account A is inactive may
create or update A's outbox head, but it must not create B's row or become
eligible for B's worker. Re-authentication by the owning account resumes
bindings that were paused only because the account context was inactive; a
workspace explicitly paused by the user still requires an explicit resume
operation.

Global and workspace pause affect trigger/network eligibility only. They do
not discard pending, uncertain, conflict, or dead-letter history. Re-enabling
sync wakes the worker, which then processes the existing account-scoped
outbox.

## Repair before normal sync

Before pull/push, the worker runs a generic, idempotent reconciliation pass
after the versioned API/SSH/Connection bootstrap passes. It enumerates live
syncable entities, and for each entity with neither matching entity state nor
matching outbox history for the exact binding, captures a canonical redacted
upsert into the outbox. The pass is generation-fenced, uses one local SQLite
transaction, preserves parent-before-child ordering, and writes no completion
marker. Running it again is safe: known state is skipped and the outbox
upsert is coalesced by its account/cloud/entity key.

The repair covers the current syncable set: Workspace, Connection,
WorkspaceVariable, WorkspaceEnvironment, WorkspaceEnvironmentVariable,
ApiCollection, ApiFolder, ApiRequest, SshTask, and SshTaskStep. Snapshot
materialization must use the same redaction boundary as ordinary outbox
capture. Secret values, credentials, private-key material, and other device
local fields must not enter canonical payloads.

## Command bus and MCP

All production mutable desktop and MCP command paths install
`SyncOutboxHook` through the unified storage-mode constructor. Raw storage
constructors are reserved for read-only, extension-injection, or isolated test
scenarios and must not be used as production mutable entry points. The hook is
transaction-local and performs no network I/O; the worker owns all transport
and account/entitlement eligibility decisions.

## Change review checklist

When changing Cloud Sync or a local mutating command, verify:

1. A bound workspace mutation still commits an account-scoped outbox head
   when no account is active.
2. Account A and B cannot consume one another's outbox rows.
3. Global/workspace pause preserves the outbox and suppresses network work.
4. Re-authentication/resume runs repair before normal sync and does not move
   ownership implicitly.
5. Reconciliation is idempotent, parent ordered, generation fenced, and
   redaction safe.
6. Desktop and MCP production constructors route through the hook.
