# Environment / Workspace Variable secret storage audit

Date: 2026-11-15. Scope: source inspection on `main` accompanying the MCP secret
projection fix. This is a separately tracked storage finding; this task does not
change storage schemas, Cloud Sync payloads, or migrate existing data. No user
database, credentials, or remote services were accessed for this audit.

## Confirmed current storage behavior

Both `workspace_variables.value` and
`workspace_environment_variables.value` contain the supplied plaintext string,
including when `is_secret = true`. `is_secret` is classification metadata, not an
encryption or SecretStore storage instruction.

Evidence:

- [Normalized schema](../../crates/local-storage/migrations/20260722120000_core_workspace_variables.sql)
  defines `value TEXT NOT NULL DEFAULT ''` beside independent `is_secret` and
  `is_enabled` columns in both tables. There is no variable credential-reference
  column.
- [Variable persistence](../../crates/workspace-engine/src/workspace/variable_persistence.rs)
  binds `input.value` directly in all four insert/update functions; it does not
  call SecretStore or replace a secret value with a credential reference.
- [LocalDb initialization](../../crates/local-storage/src/local_db.rs) uses
  ordinary SQLite connection options without database encryption configuration.
- [Variable reads and resolution](../../crates/workspace-engine/src/workspace/variables.rs)
  read the same value column. Resolution needs the actual value in memory;
  output projection must not alter that execution source.
- [Existing credential tests](../../crates/unfour-command-bus/src/workspace_bundle/credential_tests.rs),
  especially `sqlite_secret_edits_clear_flags_and_rollback_keep_normal_semantics`,
  assert plaintext reads and editable secret flags after backup restore and
  variable edits. These are source-level corroboration, not a new runtime result
  claimed by this audit.
- [The existing Bundle design](../architecture/workspace-bundle.md) explicitly
  documents restoring variable secrets to their original SQLite slots.

For passwords, tokens, private keys, and passphrases stored in these variables,
this conflicts with the root `AGENTS.md` rule to persist credential references
instead of plaintext credentials. Output masking fixes the MCP disclosure
boundary but does not resolve this storage conflict. Disabled and soft-deleted
secret rows are also relevant: disabling does not clear the value, and
`soft_delete` changes lifecycle metadata without removing the value.

## Historical data and migration needs

A separate storage fix needs an explicit, recoverable migration for both tables,
including disabled and soft-deleted rows. Current code has no variable-secret
storage migration. An SQLite schema change alone would not move existing secrets
into the OS credential backend or coordinate its failure/rollback lifecycle.

The normalization migration copies legacy
`api_environments.variables_json` values into the new environment-variable table
and sets `is_secret = 0`, because the old format had no such metadata. It retains
the legacy table for compatibility; subsequent inspected migrations do not purge
those copies. Therefore a migration must inventory that legacy source as well as
current rows. Name heuristics cannot reliably recover the secret classification
of ordinary-named legacy values. Classification, compatibility with older
binaries, and user review require a separately designed policy.

Backups, SQLite journal/WAL files, freelist pages, and historical copies may
retain prior plaintext after a logical update. An independent remediation plan
must define backup retention and database cleanup/credential rotation policy;
this audit has not inspected user files and does not claim that any particular
user's files contain such remnants.

## Workspace Bundle and environment exchange

| Path | Current behavior | Impact of a future storage fix |
| --- | --- | --- |
| Sharing Bundle | [Domain snapshots](../../crates/workspace-engine/src/workspace/snapshot.rs) and [Bundle sanitization](../../crates/unfour-command-bus/src/workspace_bundle/safety.rs) emit `SecretRedacted` for explicit secret flags, sensitive names, or local credential handles; values are omitted from sharing. | Preserve the metadata/template contract; never export a local reference as a portable value. |
| Encrypted backup with secrets | [Backup export](../../crates/unfour-command-bus/src/workspace_bundle/backup.rs) requires a password for `include_secrets`, captures actual variable values via `bundle_variable_values_on`, and encrypts the payload. | Capture actual values from the credential backend consistently with metadata and keep the same encrypted archive semantics. |
| Encrypted backup restore | Connection secrets receive fresh credential references. Variable secrets are passed as plaintext to [restore_bundle_variable_on](../../crates/workspace-engine/src/workspace/local_bundle.rs), which writes `value` and sets `is_secret = 1`. | Must stage fresh workspace-scoped variable credentials and coordinate commit/recovery; never reuse source-device handles. |
| Environment JSON / Postman export | [Environment exchange](../../crates/workspace-engine/src/workspace/environment_exchange.rs) writes an empty value and redacted marker for flagged or sensitive-name variables. Import accepts supplied secrets and uses normal variable persistence. | Keep redacted imports empty and migrate the storage destination for explicitly supplied secret imports. |

The current [Bundle design](../architecture/workspace-bundle.md) says experimental
V2 API/variable imports that used Keychain handles are unshipped and have no
compatibility resolver or migration. Do not infer that an arbitrary value string
is a resolvable handle or silently adopt it during this MCP fix.

## Cloud Sync

Current Cloud Sync does not transmit a variable's secret value:

- [canonical_payload and canonical_intent_on](../../crates/unfour-cloud-sync/src/canonical.rs)
  use the shared secret predicate, preserve metadata, and omit `value` for
  secrets. The durable outbox captures this safe payload in the business
  transaction.
- Incoming secret payloads map to `ExternalVariableValue::PreserveLocal`.
  [External apply](../../crates/workspace-engine/src/workspace/external_apply/mod.rs)
  retains a current local value or leaves it empty, reporting present/missing
  status; supplying a plaintext secret through `Set` is rejected.
- [Outbox regression coverage](../../crates/unfour-cloud-sync/tests/outbox/variable_secrets.rs)
  includes sensitive names and local handles even when the explicit flag is
  false. [Worker regression coverage](../../crates/unfour-cloud-sync/tests/worker/variable_secrets.rs)
  confirms unsafe historical retries are parked instead of transmitted. Their
  immutable old payload remains locally until the existing explicit repair flow
  creates a safe replacement operation; parking is not plaintext cleanup.
- [Conflict status](../../crates/unfour-cloud-sync/src/repository/reconciliation.rs)
  currently checks `is_secret` and nonempty `value` in the business tables to
  report local secret presence.

A storage migration need not change the wire protocol. It must adapt local
secret-preservation/presence and backup behavior to references, retain safe
outbox intent in the business transaction, and keep the single durable workspace
owner invariant. Existing unsafe historical outbox payloads need a separate
retention/repair plan without rewriting immutable operation identities.

## Separate remediation proposal

1. Design variable credential-reference persistence using the existing
   SecretStore and established staging/recovery mechanisms; do not introduce a
   parallel generic secret framework.
2. Define migration classification, legacy-table handling, disabled/deleted-row
   policy, rollback/recovery, and old-client compatibility before writing data.
3. Move execution-only resolution, explicit local reveal/edit, encrypted backup
   restore/capture, and local Cloud Sync presence handling together. Preserve
   variable priority and legitimate API authentication.
4. Verify on disposable databases that business rows/outbox/share artifacts and
   observable diagnostics contain no synthetic secret sentinels, while request
   execution and password-protected backup round trips retain correct values.

Status: storage defect confirmed by schema and write/read path inspection;
storage remediation intentionally not implemented in the MCP projection task.
