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

## Workspace ownership invariant

A local workspace has at most one Cloud Sync owner at a time:

```text
local_workspace_id -> (account_id, cloud_workspace_id)
```

Sign-out does not remove ownership, and signing into another account does not
transfer it. Rebinding is an explicit future ownership transition; it is not
an implicit side effect of Enable. Historical duplicate bindings are retained
for data safety but are unresolved until an explicit repair/rebind flow exists.

`cloud_sync_workspace_ownership` is the authoritative runtime ownership source.
When a binding exists without its matching ownership row, runtime resolution
fails closed with an ownership invariant error; a binding is never treated as
an implicit owner. A workspace is unbound only when it has neither an ownership
row nor a Cloud Sync binding.

## Mutation routing invariant

Every local mutation resolves exactly one owner before it is captured. An
unbound workspace produces no outbox row; an ambiguous workspace fails the
mutation transaction so the business row cannot commit without durable intent.
Pause, entitlement, and active-account state only affect network eligibility
and wake-up, never the outbox destination.

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

Repair is allowed only when the supplied binding exactly matches the resolved
workspace owner. A historical non-owner binding is skipped, while an
ambiguous workspace fails closed with a stable ownership diagnostic; neither
case may fan out local data.

## Legacy paused bindings

Before account pause reasons were persisted, `sync_enabled = 0` and
`state = 'paused'` could mean either account sign-out or an explicit workspace
pause. The upgrade preserves those rows as paused, records
`cloud_sync_legacy_paused_binding_ambiguous`, and requires an explicit Enable.
This conservative behavior avoids incorrectly resuming a workspace the user
had deliberately paused. New account-context pauses carry a pause-reason row
and can resume on re-authentication.

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
2. A local workspace has at most one owner; Account A and B cannot create,
   consume, or repair one another's binding.
3. Global/workspace pause preserves the outbox and suppresses network work.
4. Re-authentication/resume runs repair before normal sync and does not move
   ownership implicitly.
5. Reconciliation is idempotent, parent ordered, generation fenced, and
   redaction safe.
6. Desktop and MCP production constructors route through the hook.
