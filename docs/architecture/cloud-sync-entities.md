# Cloud Sync Entities

Cloud Sync uses a registry-driven entity architecture. Local SQLite remains
the source of truth; the Registry describes which workspace entities use the
common sync engine without coupling entity onboarding to a protocol version.

## Registry

`crates/unfour-cloud-sync/src/entity_registry.rs` is the single complete list
of syncable entity types. Each `SyncEntityDescriptor` records:

- Rust domain and sync types plus the stable wire name;
- workspace scope and parent dependency;
- initial-upload and tombstone support;
- the owning snapshot provider;
- exceptional recovery policy and topology rank;
- local table, workspace column, and parent expression for existence,
  revision, deletion, and live enumeration.

`SyncEntityAdapters` dispatches snapshot reads to the owning Workspace, API
Client, SSH Task, SSH Connection, or Database Connection domain service. The
Registry owns orchestration metadata; payload construction and business rules
stay in the owning domain and canonical mapping layers.

## Current entities

| Entity | Wire name | Parent | Snapshot provider |
| --- | --- | --- | --- |
| Workspace | `workspace` | none; root | Workspace |
| Connection | `connection` | none | SSH or Database connection |
| WorkspaceVariable | `workspaceVariable` | Workspace | Workspace |
| WorkspaceEnvironment | `workspaceEnvironment` | Workspace | Workspace |
| WorkspaceEnvironmentVariable | `workspaceEnvironmentVariable` | WorkspaceEnvironment | Workspace |
| ApiCollection | `apiCollection` | Workspace | API Client |
| ApiFolder | `apiFolder` | ApiCollection or ApiFolder | API Client |
| ApiRequest | `apiRequest` | ApiCollection or ApiFolder | API Client |
| SshTask | `sshTask` | none | SSH Task |
| SshTaskStep | `sshTaskStep` | SshTask | SSH Task |

All current entries participate in initial upload and support tombstones.
Parent-before-child push ordering and child blocking continue to use descriptor
topology plus the hierarchy-specific ordering needed for nested API folders and
aggregate deletes.

## Normal lifecycle

Enabling Cloud Sync for a new local workspace performs one local transaction:

```text
create binding and durable owner
  -> enumerate every Registry entry
  -> read redacted domain snapshots through Registry adapters
  -> produce canonical protocol-v4 intents
  -> enqueue the complete initial outbox
  -> set initial_total from the rows actually captured
```

`initial_confirmed` advances only when those operations are confirmed by Push.
Connection, API, SSH Task, and workspace-owned entities have identical initial
upload semantics. There is no API v2, SSH Task v3, or Connection v4 bootstrap.

After initialization, UI and MCP mutations both route through the Rust Command
Bus. A domain mutation and `SyncOutboxHook` capture commit in the same SQLite
transaction, including while signed out, paused, offline, or entitlement
blocked. Business mutations never depend on a UI Cloud Sync call. External
apply uses an External-origin Command Bus operation and does not echo an outbox
intent.

## Pull and Snapshot tolerant reader

Changes and Snapshot always use the complete remote stream. Ordinary business
entities do not use `supportedEntityTypes`, per-client filtering, capability
fingerprints, or capability-specific snapshot tokens.

The wire reader keeps inbound `entityType` as an opaque string. Before any
business apply, the compatibility boundary classifies each item:

- a Registry-known entity with a supported payload is validated and applied;
- an unknown future entity is skipped with a redacted diagnostic;
- a known entity with a future entity-local schema or unsupported critical
  subtype is also skipped with a redacted diagnostic;
- malformed data that claims to use a supported shape remains invalid data.

Skipped items do not create dead letters, do not put the workspace into error,
and do not produce partial business rows. Changes cursor continuity is checked
against every item in the server response, including skipped items, and the
committed cursor advances to the response's `nextCursor`. Snapshot skips use
the same classification and do not interrupt pagination or the pinned
`atCursor` contract.

Inbound envelopes and canonical payload structs accept unknown object fields,
so adding an optional field does not make an old reader reject an otherwise
valid entity. Domain models remain strict: extensible discriminators such as
API body/auth kind, Connection kind/driver/auth/SSL mode, Workspace mode, and
SSH Task step kind/version are examined before conversion. A future value that
an old client cannot apply without changing meaning causes an entity-local
skip, not a weakly typed or half-populated domain write.

## Reconciliation and recovery

`reconcile_missing_local_sync_state` is an idempotent, generation-fenced repair
for a live local entity with neither entity state nor outbox history. It reuses
Registry enumeration and snapshot adapters. It is not a supported way to
introduce a new entity into normal sync.

Dead-letter retry rematerializes current local snapshots. Descriptor recovery
policy identifies leaf entities and parents that require environment-child,
API-subtree, SSH-task-child, or root safety checks before accepting remote
absence. Entity-specific conflict and remote-apply semantics remain explicit
where payload or aggregate behavior is inherently different.

## Protocol version policy

`protocolVersion = 5` describes the frozen common wire engine, not the entity catalog.
Clients with the same protocol version may recognize different Registry
entity sets and must tolerate the full stream.

Upgrade `protocolVersion` only for a breaking change to shared wire semantics,
such as the operation envelope, cursor/pagination model, base-version/conflict
model, or generic delete/tombstone behavior. Do not upgrade it for:

- a new ordinary entity such as Flow, FlowStep, or TestSuite;
- a new entity-specific payload schema;
- a backward-compatible optional field;
- a Registry, initial-upload, or internal recovery refactor.

Do not introduce `xxx_vN_bootstrap_state` or `bootstrap_xxx_vN` to onboard a
normal entity.

An entity payload may use entity-local schema evolution when its own shape has
a breaking change. A client that encounters a newer unsupported entity-local
schema retains that entity and waits while continuing Protocol 5 stream
processing. Do
not add a version to every entity merely for additive optional fields.

## Round-trip preservation constraint

Protocol 5 Push currently sends a complete canonical payload for an upsert.
The local domain snapshot and outbox intentionally retain only fields this
client version understands. Consequently, `baseVersion` prevents overwriting a
concurrent remote edit, but it cannot preserve a future optional field after an
old client has already pulled that version and later uploads a full replacement.

This is a real old-client destructive-overwrite risk if the server implements
upsert as replacement. A future field must not rely on old-client round-trip
preservation until the server contract provides field-preserving patch/merge
semantics, or that entity adopts another explicitly designed preservation
strategy. The client must not implement an untyped universal JSON merge: it
would bypass canonical validation, redaction, deletion, and conflict rules.
Until a safe contract exists, releases that require preserved new fields need
coordinated compatibility handling rather than assuming tolerant reads alone
make writes lossless.

## Adding an entity

Adding a workspace entity that needs Cloud Sync requires:

1. Add the domain model, stable identity/workspace scope, mutation hooks, and
   redacted snapshot in its owning crate.
2. Add canonical payload mapping and typed remote apply behavior.
3. Add one Registry descriptor with wire name, local storage metadata, parent
   dependency, snapshot provider, tombstone support, and recovery policy.
4. Add initial-upload, incremental create/update/delete, remote apply,
   parent/dead-letter, tolerant-reader, and Registry regression tests.
5. Coordinate the stable wire name, payload, server persistence semantics, and
   any entity-local schema behavior with the API.

The Registry entry automatically adds the entity to local enumeration, initial
upload, generic reconciliation, and local revision/deletion lookup. Flow and
FlowStep therefore do not require a protocol-version change or edits to
initial-upload, outbox, worker-bootstrap, or generic-repair entity lists. A
genuinely new aggregate recovery rule or remote payload shape still requires
its focused recovery or remote-apply implementation.

New entities are additive. They may reference existing entities—for example,
Flow may reference an ApiRequest, SSH Task, or database connection—but an
existing entity must not become semantically dependent on the new entity.
Otherwise an old client could skip Flow yet no longer interpret its known
ApiRequest correctly, defeating tolerant evolution.
