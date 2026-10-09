# Workspace bundles and encrypted local backups

A Workspace can be exported from its switcher menu and imported as a new local
Workspace. This is a portable business definition package, not a database backup.
The desktop exports `format: "unfour-workspace", version: 2`; V1 remains readable.
V2 adds optional `localPaths`, `credentialRequirements`, and `apiTemplates` arrays.
The V1 base record contract below is unchanged. Cloud Sync does not consume these
local supplements or encrypted backups.
The schema is defined in `crates/unfour-command-bus/src/workspace_bundle/schema.rs`.

## Envelope and content

All arrays below are required, even when empty. Optional record fields may be
null. Top-level records and connection configurations reject unknown fields.

```json
{
  "format": "unfour-workspace",
  "version": 1,
  "workspace": { "id": "source-id", "name": "Example", "environmentType": "dev" },
  "variables": [],
  "environments": [],
  "environmentVariables": [],
  "collections": [],
  "folders": [],
  "requests": [],
  "connections": [],
  "sshTasks": [],
  "sshSteps": [],
  "savedSql": [],
  "flows": []
}
```

| Records | Portable fields |
| --- | --- |
| Workspace | ID used only as a bundle key, name, environment type |
| Variables / environment variables | Key, non-secret value or `secretRedacted` marker, secret/enabled flags, description, order, environment reference |
| Environments | Name and order |
| API collections / folders | Name, description where supported, order and parent/collection references |
| API requests | Name, method, redacted URL/auth/headers/query/body, body kind, timeout, scripts and script schema version, hierarchy references |
| Connections | SSH/DB kind, name, host/port, username, authentication method or database driver/catalog/SSL mode/read-only flag |
| SSH tasks / steps | Names, description, order, type, enabled flag, configuration version and portable configuration |
| Saved SQL | Name, SQL text, optional connection reference, catalog/schema |
| Flows | Name, input declarations, action/control-flow steps and references |

No timestamps, revision, remote IDs, sync status, tombstones, Cloud Sync bindings,
outbox, histories, activity, open tabs, layouts,
selected environment, active workspace, or host trust records enter the bundle.
Default local companion state is freshly initialized on import. MCP permissions
are not copied: the new Workspace starts with MCP disabled.

## Ownership and transactions

The adapter (`crates/unfour-app`) selects/saves files and calls CommandBus. It
contains no remapping, reference validation, or feature-table reads. Frontend
business state and preview live in `packages/workspace-local`. Its exchange
provider owns one shared dialog for the Workspace menu and command palette;
app-shell only wires the provider, entry points, and activation callback.
The switcher groups creation/import separately from current-Workspace actions
(export, rename, safety tier, and extensions such as Cloud Sync), with deletion
last. Export confirmation identifies the selected Workspace and retains that
target while the dialog is open. Import always creates and activates a new copy.
The save dialog defaults to `<Workspace-name>.unfour-workspace.json`, using the
name from the exported snapshot. Whitespace and unsafe filename characters become
hyphens; Unicode names are preserved within a bounded filename length. Empty or
Windows-reserved stems fall back to `workspace`.

CommandBus coordinates owning Workspace/API/SSH/Database/Flow services. Export
reads all live records through their services on one SQLite read transaction.
V2 local paths, templates, raw secret bindings and the export filename use that
same snapshot. Credential capture reserves the SQLite writer lock until all
referenced Keychain values are read; encryption/KDF runs after releasing it.
Application credential rotation, deletion and reclamation use the same lock.
Existing domain snapshots provide the Workspace/API/connection secret boundary.
Saved SQL and Flow have explicit caller-transaction service methods; neither is
added to the Cloud Sync domain registry.

Import follows:

1. Read bounded UTF-8 JSON (48 MiB envelope / 32 MiB plaintext maximum), check format/version and typed fields.
2. Sanitize untrusted content with the same export security rules.
3. Validate unique IDs, reference kinds, folder ownership/cycles, and Flow graph
   and expression constraints. At most 50,000 top-level records and 128 folder
   ancestors are accepted; existing domain/Flow limits also apply.
4. Preview counts, resolved new name, exclusions, and named reconfiguration items.
   Every materializer runs inside a transaction that is explicitly rolled back;
   previews emit no transactional hooks, execute no resource actions, and persist
   no new Workspace or activity.
5. Confirming re-parses/revalidates the submitted content. A fresh ID map and one
   CommandBus transaction write every record. Existing safe domain materializers
   insert the Workspace/API/SSH/connection records; owning service methods insert
   Saved SQL and Flow. Domain mutations are local when passed to transactional
   hooks. Hook failure or any domain failure rolls back everything.

Names are resolved against current local Workspaces in the import transaction;
conflicts receive ` (Copy N)` within the existing name limit. No original record
is updated and no resource is executed. Repeated imports create independent copies.
No merge mode or caller-specified destination Workspace exists.

## Reference mapping

Entity IDs are bundle keys, never destination IDs. The importer allocates new IDs
for the Workspace, variables, environments, collections, folders, requests,
connections, tasks, task steps, Saved SQL, Flows, and multipart parts. It rewrites
only schema-defined references, not strings that happen to resemble an ID.

- Environment variable → environment.
- Folder/request → collection and parent folder. Folder ancestors must belong to
  the same collection; parents are materialized before children.
- SSH step → task. SSH task definitions do not store a selected connection.
- Saved SQL → optional Database connection.
- Flow API action/probe → API request.
- Flow SSH action → SSH task **and** SSH connection.
- Flow Database action/probe → Database connection (`resourceId`). In the current
  model SQL is in action arguments, not a Saved SQL reference.
- Flow step IDs are regenerated independently per Flow. `next`, `ifTrue`,
  `ifFalse`, `$ref: "/steps/<id>/..."`, and `${/steps/<id>/...}` interpolation are
  rewritten. `/inputs`, `/probe`, and `$end` retain their meaning. Condition,
  poll and wait-until nodes use the same dependency checks. Flow validation runs
  before and after remapping.

Missing or wrong-kind references, duplicate IDs, folder cycles, unsafe Flow
references, unknown versions and malformed records reject import atomically.
Existing broken dependencies are not silently rebound to similarly named local
resources. A bundle exported from an already broken Workspace may need its source
references repaired before it can be imported.

## Secrets and device settings

Sharing mode removes credentials and excludes device paths by default. Paths can
be included explicitly. Encrypted backup mode includes paths by default and offers
an independent switch for saved credentials. Connection DTOs remain allowlists;
local paths and credential requirement metadata travel in separate V2 fields.
SQLite endpoint/catalog fields and opaque connection configuration remain omitted.
Preview identifies specific missing fields instead of treating every connection
as requiring credentials. Retained paths begin as unchecked, with an optional
read-only check and per-path editing/prefix replacement before import.

Sharing keeps secret variable metadata with empty values. Bundle sanitization,
domain snapshots and Cloud Sync use one variable rule: an explicit Secret flag,
a recognized sensitive key (including password/passphrase/token/API-key names),
or a local credential handle makes the value sensitive. A false Secret flag
cannot make those values shareable. Cloud Sync omits their values using its
existing protocol. Unsafe immutable retries from an older client are parked as
dead letters; the existing current-local repair creates a fresh operation with
a safe payload, preserving the original operation's identity and payload.
Sensitive API fields
reuse domain redaction; explicit variable templates in auth/headers/query are
restored through validated local supplements. Request settings retain only the
supported timeout field. Multipart file bindings are currently runtime-only and
must be selected again. Files, including private keys, are never embedded. Retained
paths cover SSH private-key files, SQLite files, and SSH upload/download steps.
An omitted transfer path becomes `{{local_path}}`; the original enabled flag is
preserved, so missing inputs fail rather than silently skipping a transfer.
Native SSH tasks check all resolved transfer paths before starting any remote
command, and drivers recheck at use. SQLite still refuses to create a missing DB.

Flow secret input defaults are removed. Structured sensitive fields and recognized
sensitive lines in scripts/SQL/SSH configuration are redacted. Definition text is
never executed by import. As with existing collection export, arbitrary literals
in free-form code cannot be classified perfectly: the UI explicitly asks users
to inspect scripts, SQL and free text for hardcoded credentials before sharing.
Redacted definitions may need editing before execution. Prefer variables and
runtime inputs for secret values.

## Encrypted envelope and local credentials

The outer format is `unfour-workspace-encrypted`, version 1. It encrypts a V2
bundle and credential bindings using AES-256-GCM. A random 256-bit data key is
wrapped with a key derived from the user password using PBKDF2-HMAC-SHA256,
600,000 iterations and a random 128-bit salt. Both operations use independent
random 96-bit nonces and version-bound AAD. Fixed versioned parameters, strict
field/length validation and authenticated decryption reject unsupported, damaged
or wrong-password input before any persistence. Passwords require at least 12
characters. There is no recovery key or server-side decryption.

Optional secrets include SSH/database passwords, SSH key passphrases, Workspace
and environment secret variables, and saved API auth/header/query/URL secrets.
Body/script/SQL/Flow literal secrets remain subject to redaction; the backup is
not a byte-for-byte database image. Source credential handles never become target
handles. Import allocates fresh workspace-scoped references in the local credential
store. Variables and API values persist only those references; runtime resolves
them with workspace checks. Auth JSON resolves string leaves before serialization,
so quotes/backslashes in credentials cannot change authentication structure.
Preview returns counts, paths and field statuses, never decrypted secret values.

Before Keychain writes, import validates all domain rows in a rollback-only
transaction. The metadata journal tracks `staged`, `attached` and `garbage`
references. Attachment and loss of the last live reference are recorded in the
business transaction. Keychain deletion follows commit, with a fresh reference
check under the SQLite writer lock; failed cleanup stays durable for retry.
The check includes variables, API values and connection fields, so shared handles
survive until their last use disappears. Failed transactions retain the old
credential and reclaim only unpublished stages. Imported variable replacements
and saved SSH credential edits allocate fresh handles instead of overwriting
shared credentials. Clearing a Secret retains its flag, and refilling it writes
a new Keychain credential.

Primary startup reclaims interrupted stages and garbage and adopts live handles
saved by the original V2 implementation without ownership metadata. Satellite/MCP
construction does not run startup recovery. Historically orphaned handles with
neither a live reference nor journal metadata cannot be recovered or enumerated
portably. An external OS credential change cannot participate in the SQLite lock.
Slow Keychain reads may briefly delay local edits during credential capture.

The variable editor shows a localized saved-credential placeholder for an imported
handle. Leaving it untouched preserves the reference; typing replaces it, and an
explicit clear action removes its value. It never reveals the handle as a password.
Save writes a temporary sibling file and persists it only after
the complete write succeeds. Cancelling or failing leaves the dialog editable.

New dependencies: `ring` provides maintained AEAD/KDF primitives, `base64` encodes
the envelope, `zeroize` clears sensitive buffers, `tempfile` protects existing
backups during replacement, and `tracing` reports pending cleanup without secrets.
These dependencies are already present in the workspace lockfile dependency graph.

## Non-goals

Selective export/import, dependency repair/rebinding, merge/conflict policies,
bundle migration between versions, file attachments with explicit portability
rules, signatures and large-bundle progress/cancellation can follow separately.
This change adds no multi-device encrypted credential sync, Flow Cloud Sync,
separate Flow import/export, remote execution, or database file migration.
