# Flow Definition Cloud Sync client contract (dormant)

Client safety revision based on main `f089a90`. This prepares the client only.
Unfour-api does not yet support `flow`; this is **not enabled cross-device sync**.

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

| Contract | Required final value / behavior |
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

### Final schema 1 safety rules (client and future Unfour-api)

The wire shape, schema version and reader revision stay **1**. These rules
replace the former reference-only string restriction before production activation.
Unfour-api must implement this final acceptance contract, not the old restriction.
Validation accepts the complete definition unchanged or returns an explicit
error; it never redacts, strips fields, truncates, or rewrites executable values.

1. Ordinary string literals, numbers, booleans, null, arrays and objects are
   allowed in executable values and ordinary input defaults. This includes SQL,
   SSH Task `inputs` (and `workspaceDefaults`), API URLs/body text, headers/query
   replacement and patch rows, string predicate operands and mixed templates
   such as `build-${/inputs/version}.zip`. Remote SSH directory arguments such
   as `/home/deploy/app` are portable parameter data; a path-shaped string alone
   does not establish that it is a device-local credential.
2. Inputs marked `secret`, or whose names match the shared sensitive-name rule,
   have **no default at all**, including null or a reference. Defaults are copied
   as literal data by the engine, not evaluated: embedded reference-shaped data
   cannot justify a sensitive default or a sensitive nested default field.
3. Explicit sensitive fields require a whole runtime value: a singleton
   `{"$ref":"/inputs/name"}`, exact `${/inputs/name}`, or exact
   `Bearer ${/inputs/name}`. Arbitrary literal prefixes/suffixes in a sensitive
   value are rejected; supply the fully assembled value at runtime instead.
   The same applies to numeric/boolean/null/container literals in such fields.
   The shared field rule lowercases and removes underscores, hyphens and spaces,
   then matches `authorization`, `cookie`, `password`, `passwd`, `secret`,
   `token`, `apikey`, `privatekey`, `passphrase`, `credential`, or `licensekey`.
   This classifies field names; it is **not a general secret detector**.
4. Inspect nested objects/arrays and actual API `{key,value,enabled}` rows,
   including disabled rows, `headersPatch`, `queryPatch` and repeated query
   occurrences. A sensitive `key` (or named `name`/`value` pair) protects `value`.
   Runtime-selected field names also require runtime-supplied values, since
   their sensitivity cannot be known until execution.
5. Inspect JSON-looking text (starts with `{` or `[` after whitespace), URL
   userinfo passwords and query pairs, and API Body form pairs when non-JSON
   body text contains `=`. Apply the same field rules after JSON/percent decoding.
   JSON Body templates can be quoted or occupy a complete unquoted JSON value.
   Only actual `${...}` sequences in the original executable text count as
   runtime references: escaped/percent-encoded lookalikes and `$ref` objects
   inside JSON text remain literals. Reference masking is validation-only.
   Uninspectable JSON-looking text or ambiguous JSON escape/template boundaries
   are rejected; supply the whole body through a runtime reference instead.
6. References use the engine's JSON Pointer roots `/inputs`, `/steps`, `/probe`,
   valid `~0`/`~1` escaping and no braces/control characters. Malformed singleton
   objects/templates, unavailable/forward/self step outputs and `/probe` outside
   a probe predicate are rejected. `/steps` references must be guaranteed on
   every incoming execution path. Actual input values/output paths are checked
   at run time; no secret or referenced resource is resolved during sync.
7. Never accept device-local material: normalized field names containing
   `credentialref`, `secretref`, `secretstore`, `keychain`, `privatekeypath`,
   `localpath`, `runtimevariables`, or `temporaryvariables`, including named pair
   keys, are forbidden even with reference values. Strings containing reserved
   `@unfour-secret:`, `unfour:<workspace>:<kind>:<record>`, `keychain:`,
   `secret-store:`, or `secretstore:` handles, `file:` URIs, PEM private-key
   headers or PuTTY private-key material are forbidden, including in decoded
   structured data. A runtime reference to
   a secret input is portable; a persisted Secret Store reference is not.

Opaque SQL, shell/task text and ordinary text cannot be proven secret-free by
examining keywords, entropy or field lists. They are accepted as user-authored
definition data; hardcoded secrets hidden in arbitrary text, innocuously named
fields, encoded blobs, custom handle formats or metadata cannot be reliably
identified. Authors must keep those secrets in runtime secret inputs or existing
resource credential storage. Names/descriptions are metadata, not a secret
channel. Structured ambiguous cases above fail explicitly instead of falling
back to opaque acceptance. This is a practical boundary, not a claim of DLP.

Flow Runs/history, output values, consent, selected environment, resolved
variables, credential provenance and execution snapshots are never wire fields.
Future changes must not add them to definition payloads. Existing local Flow
save/execution security validation still applies; sync acceptance grants no
execution permission and does not bypass missing-resource preflight.

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
4. Add shared i18n error presentation for rejected safety/size cases. Deploy and
   verify the final schema 1 safety rules above in Unfour-api before enabling
   any client. No automatic truncation/redaction.

Verification evidence is in
[the client test report](../testing/flow-definition-cloud-sync-client.md).
