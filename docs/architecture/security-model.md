# Security Model

Unfour handles credentials, HTTP tokens, remote command execution, and database
access. The default posture is local-first, least privilege, explicit
confirmation for high-risk operations, and redaction before persistence or AI
exposure.

## Sensitive Data

Do not store these in SQLite plaintext:

- SSH passwords.
- SSH private-key passphrases.
- Database passwords.
- API tokens.
- Proxy credentials.

Persist only credential references. Raw secret reads belong behind
`crates/secret-store` and should happen only where required for execution.

SSH command history is not a credential store. It persists only interactive
lines that were echoed by the remote PTY, then applies conservative marker
redaction. Password-prompt input is not stored in SQLite.

## Redaction

Logs, history, activity details, and MCP results must redact or mask sensitive
data.

The shared redaction policy includes HTTP-style keys and common credential
field names:

- `authorization`
- `cookie`
- `proxy-authorization`
- `x-api-key`
- `x-auth-token`
- `password`
- `passwd`
- `token`
- `access_token`
- `refresh_token`
- `secret`
- `private_key`
- `api_key`
- `license_key`

JSON request body redaction applies recursively to sensitive keys before data is
persisted. The actual HTTP request payload sent to a server is not modified by
persistence redaction.

Diagnostics also redact sensitive URL query parameters and semicolon-style
connection-string secrets before writing structured log fields. Diagnostic logs
must record metadata needed for troubleshooting, such as method, host, status,
driver, duration, command id, request id, and stable error code, without raw
payloads or reusable secrets.

MCP results have an additional LLM-facing masking layer. See
`docs/mcp/tools.md` for the MCP-specific masking and tool safety policy.

## Command-Bus Safety

Security-sensitive business actions must route through:

```text
adapter -> CommandBus -> service -> driver
```

Tauri commands and MCP tools are adapters. They must not duplicate domain logic
or bypass command-bus safety, workspace scoping, credential references, SQL
confirmation policy, SSH host-key policy, or redaction.

## Tauri Capabilities

Keep `apps/desktop/src-tauri/capabilities/default.json` narrow. Add permissions
only when a command or plugin requires them, and document the reason when the
change affects security posture.

## Dangerous Actions

Future AI/workflow actions should distinguish local reads, writes, and data
egress. Routine metadata reads can proceed without a confirmation dialog, but
sensitive reads may need workspace-level authorization.

These actions require explicit confirmation and redacted activity records:

- Writing to production databases.
- Running destructive SSH commands.
- Exporting workspaces with sensitive metadata.
- Sending secrets to third-party services.
- Writing local workspace state, saved requests, connection metadata,
  credentials, or files.

Database mutation SQL must continue to use backend safety classification and
confirmation metadata. Frontend confirmation UI is additive; it must not replace
backend checks.

## SSH Host-Key Policy

Unfour uses trust on first use (TOFU) for SSH host-key verification.

- On first connection to a host and port, the server public-key fingerprint is
  recorded locally.
- Later connections must match the stored fingerprint.
- Fingerprint mismatches are rejected and must not be accepted silently.
- Users can view and reset trusted fingerprints.
- Known-hosts import/export should preserve the same fingerprint policy.

## Current Verification Limits

Release readiness depends on platform and live-service checks. Do not report
these as passing unless they have been run for the target release:

- live SSH password and private-key workflows against a reachable SSH server;
- macOS Apple Keychain runtime behavior;
- Linux Secret Service runtime behavior;
- release package signing and installer launch behavior;
- database live checks for any driver path changed since the last verified
  release candidate.

Use `docs/testing/release-verification.md` and
`docs/testing/manual-test-cases.md` for the active verification matrix.
