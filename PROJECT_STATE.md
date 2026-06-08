## Result
- Status: Completed
- Commit: pending
- Scope violations: No

## Modified Files
- `crates/ssh-engine/Cargo.toml` — added `unfour-secret-store`, `ssh-key`, `tokio` dependencies; expanded `ssh-native` feature
- `crates/ssh-engine/src/lib.rs` — added `host_key` module export
- `crates/ssh-engine/src/host_key.rs` — new: TOFU host-key verification with `HostKeyStore`
- `crates/ssh-engine/src/ssh.rs` — rewritten: native russh transport, password auth from SecretStore, graceful close, idempotent close
- `crates/local-storage/src/local_db.rs` — added `ssh_host_keys` migration
- `apps/desktop/src-tauri/src/command_bus.rs` — pass SecretStore to SshService; async close_session
- `docs/engineering/security.md` — documented host-key verification ADR

## Verification
- `cargo fmt --check` — PASS
- `cargo test --workspace` — PASS (46 tests, 7 new)
- `cargo check --workspace` — PASS (no warnings)
- `cargo check -p unfour-workspace --features ssh-native` — PASS
- `pnpm run build` — PASS

## SSH Transport
- **Real transport path:** `crates/ssh-engine/src/ssh.rs` under `#[cfg(feature = "ssh-native")]`. Uses `russh::client::connect` with a custom `Handler` for host-key verification.
- **Feature flag behavior:** `ssh-native` enables real russh transport. Without the flag, the simulated/mock path remains fully functional. Both paths compile cleanly.
- **Password auth flow:** Password is read from `SecretStore` via the stored `credential_ref`. The password is passed directly to `russh::client::Handle::authenticate_password` and never written to SQLite, logs, or error messages.
- **Close behavior:** Closing a session sends a `Disconnect::ByApplication` to the SSH server (native path), updates the session status to "closed", and retains the session record for idempotency. Repeated close calls return the stored "closed" summary without error.

## Host Key Verification
- **Storage location:** `ssh_host_keys` table in SQLite (`host TEXT, port INTEGER, fingerprint TEXT, created_at TEXT`), composite PK on `(host, port)`.
- **First-connect behavior:** Records the server's SHA-256 fingerprint. Connection proceeds.
- **Mismatch behavior:** Connection is rejected with a clear error message mentioning the host:port and the possibility of a MITM attack. The connection is never established.
- **Future extension point:** The `HostKeyStore::verify_or_record` method is the single verification entry point. Future work can replace or extend it to support `known_hosts` file integration, user confirmation UI for fingerprint changes, or per-connection policy overrides.

## Tests
- **Added tests:**
  - `host_key_first_connect_records_fingerprint` — TOFU first-connect records fingerprint
  - `host_key_matching_fingerprint_succeeds` — matching fingerprint passes verification
  - `host_key_mismatch_is_rejected` — mismatched fingerprint returns clear error
  - `host_key_different_hosts_are_independent` — different hosts have separate fingerprints
  - `host_key_different_ports_are_independent` — different ports have separate fingerprints
  - `repeated_close_does_not_panic_and_returns_stable_result` — idempotent close
  - `auth_failure_does_not_leak_password_in_error` — error sanitization strips secrets
- **Real localhost SSH verification status:** NOT VERIFIED (no SSH server available in the current environment)
- **Unverified areas:** PTY allocation, stdin/stdout streaming, Tauri event streaming, frontend terminal UI, private-key authentication

## Checkpoint Refresh
- **Resolved issues:** SSH host-key verification was listed as not implemented in security.md — now resolved with TOFU.
- **Remaining issues:** OS keychain not implemented (still using keyring crate). PTY, streaming, events, and private-key auth remain unimplemented.
- **Next recommended batch:** SSH transport phase 2 — PTY allocation, stdin/stdout streaming, Tauri events, xterm integration, resize.

## Scope Confirmation
- Unrelated files changed: No
- Dependencies added: `ssh-key` (0.7.0-rc.10, optional), `tokio` (optional, for timeout/sync), `unfour-secret-store` (direct dep for ssh-engine)
- Public contracts changed: No (SshService API unchanged; `new()` signature updated to accept SecretStore, which is an internal construction change)
- Backend call chain changed: Yes (close_session is now async; CommandBus passes SecretStore to SshService)
