# Flow Definition Cloud Sync client contract (dormant)

Based on main `bbfc8d1f`. This change prepares the client only. Unfour-api does
not yet support `flow`; this is **not enabled cross-device sync**.

## Ownership and activation boundary

The Flow engine owns definition snapshots and transaction-local external
whole-document writes. Cloud Sync owns canonical validation and wire mapping.
The existing Protocol 5 Registry, initial upload, Outbox, Worker, retained remote
envelopes, generation fences and conflict resolver remain the sync engine.

Production `SyncEntityType`, Registry, mutation conversion, first upload, repair,
push and pull do not recognize `flow`. Core's generic external page command
explicitly rejects a nonempty Flow page with `FLOW_SYNC_NOT_ENABLED`. Independent
adapter functions are available for verification. `#[cfg(test)]` arms rehearse
registration with the real sync engine in unit tests only; integration tests
compile the production library and assert the opposite boundary. There is no
runtime preference, environment flag or Cargo feature enabling Flow sync.

The new storage migration extends only the entity-name CHECKs in Outbox and
entity state. It preserves canonical intents, leases, retry metadata, conflict
schemas and applied reader revisions. It creates no Flow intent or binding.
Community databases continue to use the local migration chain independently.

## Server alignment required

| Contract | Proposed value / behavior |
| --- | --- |
| EntityType | `flow` (one complete definition, never one entity per step) |
| Protocol | 5, unchanged |
| payloadSchemaVersion / reader revision | 1 / 1 |
| Identity | Envelope `entityId` is the existing Flow UUID |
| Parent | Envelope `parentEntityId` is the Workspace root ID |
| Payload | Exactly `name`, `inputs`, `steps`; no identity, local revision or timestamps |
| Update | Replace the entire document using existing baseVersion CAS |
| Tombstone | `operation=delete`, absent/null payload, RFC3339 `deletedAt`; positive schema version |
| Limit | At most 65,536 bytes of compact UTF-8 payload JSON; never truncate |
| Dependencies | Workspace required. Referenced resource UUIDs need not exist at apply time |
| Conflict | Existing keep-local/use-remote; no step merge |

The server must add `flow` to its entity validation, persistence, parent checks,
Push/Changes/Snapshot contracts, tombstone handling and any entity-name schema
constraints. Do not require referenced resources to arrive before Flow. Saved
SQL/resource synchronization remains governed by their existing contracts; this
change does not onboard another entity. Missing resources must still fail local
run preflight until the referenced UUID exists in that workspace.

The server must retain immutable operation/idempotency semantics, canonical
payloads, baseVersion conflict envelopes, complete Changes/Snapshot streams and
the existing ownership/authorization checks. No client capability filtering,
protocol negotiation extension or bootstrap marker is needed.

### Payload schema 1

Top level is a strict object:

- `name`: nonempty string after trim, at most 200 UTF-8 bytes.
- `inputs`: array of strict input objects. Each contains `name`, `type`
  (`string`, `number`, `boolean`, `json`), `required`, `secret`, optional
  `description`, and optional `default`. Absent default differs from JSON null.
  Canonical output includes boolean defaults; `description` is omitted when
  absent. Legacy local string input names become required JSON-typed inputs
  before canonicalization; string shorthand is not a wire shape.
- `steps`: ordered array of 1–100 steps. Each has `id`, `name`, `timeoutMs`,
  nullable `next`, and the fields selected by `kind` below. Step IDs are local
  graph labels, not resource UUIDs. Ordering and all graph targets are retained.

| kind | Additional fields |
| --- | --- |
| `action` | `action` |
| `condition` | `predicate`, `ifTrue`, `ifFalse` |
| `poll` | `probe`, `predicate`, `intervalMs`, `maxAttempts` |
| `waitUntil` | `probe`, `successWhen`, nullable `failureWhen`, `intervalMs`, nullable `maxAttempts`, `probeErrorPolicy`, `intervalStrategy` |
| `wait` | `durationMs` |

Actions/probes are strict objects with `capability` (`api`, `ssh`, `database`),
`resourceId` (UUID), nullable `connectionId` (UUID), and `arguments` (JSON).
Predicates are strict objects with `left`, `op` (`eq`, `ne`, `gt`, `ge`, `lt`,
`le`, `in`) and `right`. Input types, forward-only branches, reference graph,
timeouts and polling bounds use the existing Flow validator. Canonical output
materializes existing model defaults (for example `next: null`,
`probeErrorPolicy: failImmediately`, `intervalStrategy: fixed`). It does not
change execution semantics.

Unknown fields are rejected, including fields serde would otherwise discard in
flattened steps. Unknown schema versions or critical enum variants are deferred
by the tolerant reader. Future schema changes must be coordinated with this
strict entity-local contract; do not silently drop executable fields. A
schema-independent tombstone can still be applied by a reader supporting Flow.
Clients that do not support Flow retain its envelopes, advance the cursor and
enter compatibility waiting without materializing definitions.

### Sensitive data and conservative literal policy

Secret inputs carry name/type/required/secret/description only. A secret default
(including null) is invalid. No credential references, Keychain handles,
private-key material, device paths, runtime variables, Flow Runs, outcomes,
consent, environment selection or execution snapshots enter the payload.

Schema 1's client safety validator intentionally supports a conservative subset:
string values in executable slots (`arguments`, input defaults, predicate
operands) must be exact runtime references, either `{"$ref":"/inputs/name"}`
or `${/inputs/name}` (also `/steps/...` and predicate `/probe/...`; an exact
`Bearer ${/...}` is allowed).
Numeric, boolean and null literals and structured containers are supported;
sensitive-named values require references. Local handles/path fields are
forbidden even when their value is a reference. No sanitization is performed.
Opaque literal SQL, shell text, bodies, URLs, string defaults and string
predicate constants are rejected with `cloud_sync_flow_unsafe_definition`.
This deliberately rejects some harmless definitions rather than pretending to
infer the secrecy of arbitrary strings. Expanding that subset requires a
separate reviewed safety contract, not silently removing or rewriting values.
Names/descriptions are declarative metadata, not a channel for secret values.

Oversized definitions produce `cloud_sync_flow_payload_too_large`. The existing
local 1 MiB limit is unchanged. In the rehearsed enabled path, unsafe/oversized
first upload rolls back the binding transaction; an unsafe/oversized edit to a
bound workspace rolls back both the definition and its Outbox capture. The
production-disabled path continues accepting locally valid definitions normally.
No new UI copy is introduced while the feature is dormant; user-facing localized
error presentation must be wired before release activation.

## Revision, history and conflicts

Local revision is never a serverVersion. New external definitions start at 1;
changed external documents, resurrection and live-row deletion increment the
existing local revision. Equivalent content and duplicate tombstones do not.
An editor holding an older revision continues to receive its normal optimistic
concurrency error. The Worker retains the remote serverVersion fence and rejects
stale replay before domain apply. Direct domain apply requires External origin;
its caller must own that remote version/conflict fence and transaction.

External apply returns External-origin mutations and performs no Outbox writes.
It never resolves/remaps/creates referenced resources or touches run rows.
Individual Flow tombstones preserve local history. Workspace deletion reuses
the previous phase's cascade and purges local run history. Keep-local captures
the current entire document/tombstone against the conflict serverVersion;
use-remote abandons the scoped intent and applies the remote document/tombstone
atomically. Workspace deletion conflicts include pending Flow intent.

## Final activation steps (future change)

1. Align and deploy the server contract above, with its own migration and
   compatibility tests. This task makes no Unfour-api changes.
2. Promote only the Flow rehearsal arms in `types.rs`, `entity_registry.rs`,
   `canonical.rs`, `remote_compatibility.rs` and `service/external_apply.rs` from
   `#[cfg(test)]` to production. Remove the disabled domain-to-sync conversion;
   retain the schema migration. Update the generic Core external-page adapter
   if that entry point is to support Flow, including materialization reporting
   and External-origin hooks. Do not remove all test cfgs indiscriminately.
3. Replace production-exclusion assertions with registration assertions. Run
   the same lifecycle cases through a deployed test server and two real clients;
   verify first upload/repair for pre-existing bound workspaces, old-client
   compatibility waiting, offline retry, conflicts, workspace cascade and
   reference resources that arrive later or remain unavailable.
4. Add shared i18n error presentation for rejected safety/size cases. Decide
   whether the conservative literal policy is acceptable for release, or
   implement a separately reviewed expansion. No automatic truncation/redaction.

Verification evidence is in
[the client test report](../testing/flow-definition-cloud-sync-client.md).
