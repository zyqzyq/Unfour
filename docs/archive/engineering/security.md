# Security

Unfour handles credentials, remote command execution, database access, and HTTP tokens. The default posture is local-first and least privilege.

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

Body redaction is implemented for JSON request bodies. The same sensitive-key list is applied recursively to all nested keys in the JSON structure. Redaction is applied in the Rust persistence paths (`save_request` and `insert_history` in `crates/http-engine/src/api_client.rs`) and in the browser mock (`packages/command-client/src/tauri.ts`). The actual HTTP request payload sent to servers is never modified — redaction only affects stored history and saved requests. Non-JSON bodies and plain text are passed through unchanged.

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

- OS keychain support is configured through `keyring` platform backends: Windows
  Credential Manager, Apple Keychain, and Linux Secret Service. Windows
  create/read/delete behavior was release-smoke verified on 2026-06-14 for SSH
  passwords, SSH private-key passphrases, PostgreSQL passwords, and MySQL
  passwords. macOS and Linux runtime verification remains pending.
- Database query cancellation remains pending. Query safety classification and
  confirmation guardrails exist, and MCP database tools enforce read-only
  allowlists before reaching the command bus.
- Workspace environment values are not encrypted; do not store long-lived secrets there.
- Encrypted SSH private key passphrase decryption is limited by the ssh-key crate's format support.

## SSH Host-Key Verification (ADR: TOFU)

**Decision:** Trust-on-first-use (TOFU) for SSH host-key verification.

**Status:** Implemented in `crates/ssh-engine/src/host_key.rs`. Active under the `ssh-native` feature flag. Works for both password and private-key authentication.

**Behavior:**

- On first connection to a host:port, the server's public key SHA-256 fingerprint is recorded in the `ssh_host_keys` SQLite table.
- On subsequent connections, the stored fingerprint must match the server's presented key.
- A fingerprint mismatch is rejected with a clear error message. The connection is not established.
- Fingerprint changes are never silently accepted.

**Storage:** Fingerprints are stored in `ssh_host_keys (host, port, fingerprint, key_type, public_key_data, created_at)` with `(host, port)` as the composite primary key. The `key_type` and `public_key_data` columns are added via idempotent ALTER TABLE migration.

**Management:** Users can view the trusted fingerprint and reset (delete) it via the connection settings dialog. After a reset, the next connection re-establishes trust (TOFU). A trust confirmation dialog (`HostKeyTrustDialog`) shows the fingerprint and requires explicit user confirmation before first trust. Mismatch errors are displayed clearly.

**known_hosts interoperability:** Import and export of OpenSSH `known_hosts` format files. Import parses each line, computes SHA-256 fingerprints from raw public key bytes, and stores entries that are valid and not already present. Export generates `known_hosts` format output with comments for entries missing key data. Tauri commands: `ssh_known_hosts_import`, `ssh_known_hosts_export`, `ssh_host_key_list`.

**Frontend:** The `HostKeyFingerprint` component in `packages/ssh-terminal` displays the trusted fingerprint and allows resetting it. The `HostKeyTrustDialog` component provides trust confirmation before first connection and shows clear mismatch warnings.

**Future extension points:**

- User confirmation UI for fingerprint changes (allow explicit trust updates without full reset).
- Per-connection host-key policy overrides.
