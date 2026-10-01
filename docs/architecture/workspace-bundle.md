# Workspace bundle V1

A Workspace can be exported from its switcher menu and imported as a new local
Workspace. This is a portable business definition package, not a database backup.
The independently versioned contract is `format: "unfour-workspace", version: 1`.
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
outbox, credentials, Keychain values, histories, activity, open tabs, layouts,
selected environment, active workspace, or host trust records enter the bundle.
Default local companion state is freshly initialized on import. MCP permissions
are not copied: the new Workspace starts with MCP disabled.

## Ownership and transactions

The adapter (`crates/unfour-app`) selects/saves files and calls CommandBus. It
contains no remapping, reference validation, or feature-table reads. Frontend
business state and preview live in `packages/workspace-local`; app-shell only
wires its Workspace menu, dialog, and activation callback.

CommandBus coordinates owning Workspace/API/SSH/Database/Flow services. Export
reads all live records through their services on one SQLite read transaction.
Existing domain snapshots provide the Workspace/API/connection secret boundary.
Saved SQL and Flow have explicit caller-transaction service methods; neither is
added to the Cloud Sync domain registry.

Import follows:

1. Read bounded UTF-8 JSON (32 MiB maximum), check format/version and typed fields.
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

Connection DTOs are allowlists. No credential reference, password, token,
passphrase, private-key path, SQLite file path, tunnel/other opaque device config,
or Keychain content is transported. SQLite endpoint/catalog fields are omitted.
Fresh connections retain the existing engine's missing-credential/device-setting
behavior; the preview lists connections requiring configuration. No secrets are
read from the Keychain during export or written during import.

Secret variables keep their names/metadata but import with empty values. Sensitive
API auth/header/query/body fields reuse domain redaction. Request settings retain
only the supported timeout field. Multipart file bindings/bytes never travel and
must be selected again. SSH upload/download local paths become `{{local_path}}`
and the transfer steps are disabled until explicitly configured/enabled. Remote
paths are business intent and are retained.

Flow secret input defaults are removed. Structured sensitive fields and recognized
sensitive lines in scripts/SQL/SSH configuration are redacted. Definition text is
never executed by import. As with existing collection export, arbitrary literals
in free-form code cannot be classified perfectly: the UI explicitly asks users
to inspect scripts, SQL and free text for hardcoded credentials before sharing.
Redacted definitions may need editing before execution. Prefer variables and
runtime inputs for secret values.

## V2 candidates and non-goals

Selective export/import, dependency repair/rebinding, merge/conflict policies,
bundle migration between versions, file attachments with explicit portability
rules, richer per-field repair reports, signatures/checksums and large-bundle
progress/cancellation can follow separately. V1 adds no Flow Cloud Sync, separate
Flow import/export, remote execution, credential migration, or database schema
migration.
