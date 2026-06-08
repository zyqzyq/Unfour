# Security

Unfour Workspace handles credentials, remote command execution, database access, and HTTP tokens. The default posture is local-first and least privilege.

## Sensitive Data

Do not store these in SQLite plaintext:

- SSH passwords
- SSH private-key passphrases
- Database passwords
- API tokens
- Proxy credentials

SQLite records should store only `credential_ref`.

## Redaction

Logs and request history redact these header names:

- `authorization`
- `cookie`
- `proxy-authorization`
- `x-api-key`
- `x-auth-token`

Body redaction is not implemented yet. Until it is, users should avoid saving secrets in API bodies.

## Local Activity

The app is local-first, so activity recording is for user troubleshooting and safety rather than enterprise compliance. Keep `activity_events` as a redacted local activity trail, but avoid routine read and UI noise.

Record these events by default:

- Workspace, environment, saved request, connection, credential, and SSH session lifecycle writes.
- External API sends.
- Database SQL that requires confirmation, such as mutation, schema-change, transaction-control, or unknown statements.
- Future AI/workflow actions that write local state, execute external side effects, or send local data outside the app.

Do not record these events by default:

- Routine list/detail reads.
- Read-only database table browsing and read-classified SQL.
- UI layout persistence, terminal resize, and similar interaction noise.

Activity details must be summaries only. Do not store request bodies, response bodies, SQL result rows, passwords, tokens, private-key passphrases, or raw AI prompts/responses in activity details.

## Tauri Capabilities

Keep `apps/desktop/src-tauri/capabilities/default.json` narrow. Add permissions only when a command or plugin requires them, and record why in this document.

## Dangerous Actions

Future AI/workflow actions should distinguish local reads, writes, and data egress. Routine metadata reads can proceed without a confirmation dialog, but sensitive reads may need workspace-level authorization. The following actions must ask for confirmation before execution and must leave a redacted activity record:

- Writing to production databases
- Running destructive SSH commands
- Exporting workspaces with sensitive metadata
- Sending secrets to third-party services
- Writing local workspace state, saved requests, connection metadata, credentials, or files

## Current Gaps

- OS keychain/Stronghold write/read is reserved but not implemented.
- Database query cancellation and read-only guardrails are not implemented.
- API request body redaction is not implemented.
- Workspace environment values are not encrypted; do not store long-lived secrets there.

## SSH Host-Key Verification (ADR: TOFU)

**Decision:** Trust-on-first-use (TOFU) for SSH host-key verification.

**Status:** Implemented in `crates/ssh-engine/src/host_key.rs`. Active under the `ssh-native` feature flag.

**Behavior:**

- On first connection to a host:port, the server's public key SHA-256 fingerprint is recorded in the `ssh_host_keys` SQLite table.
- On subsequent connections, the stored fingerprint must match the server's presented key.
- A fingerprint mismatch is rejected with a clear error message. The connection is not established.
- Fingerprint changes are never silently accepted.

**Storage:** Fingerprints are stored in `ssh_host_keys (host, port, fingerprint, created_at)` with `(host, port)` as the composite primary key.

**Future extension points:**

- OpenSSH `known_hosts` file integration for interoperability.
- User confirmation UI for fingerprint changes (allow explicit trust updates).
- Per-connection host-key policy overrides.
