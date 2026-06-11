# Open Issues

## P0 — Blocks core usage

None.

## P1 — High priority

### SSH Transport

- **Encrypted key passphrase:** The ssh-key crate 0.7.0-rc.10 has limited support for decrypting encrypted OpenSSH keys. Keys without passphrases work. Encrypted key loading returns a clear error guiding users to save a passphrase credential. Passphrase is read from SecretStore, never stored in SQLite.

## P2 — Medium priority

- **Host-key UI:** View/reset fingerprint implemented. Trust confirmation dialog (first trust + mismatch) implemented. known_hosts import/export implemented. Fingerprint change confirmation without full reset is a future enhancement.

## P3 — Low priority / Future

- **Terminal multiplexing:** Support tmux/screen-like session management within the app.
- **SCP/SFTP file transfer:** Leverage the existing russh connection for file operations.
- **Additional database drivers:** PostgreSQL and MySQL support.

## Environment / Tooling

- **OS keychain:** The `keyring` crate is used for production but has not been verified on all target platforms (macOS, Windows, Linux).
- **Windows workspace tests:** `cargo test -p unfour-workspace` fails with `STATUS_ENTRYPOINT_NOT_FOUND` on this Windows environment. Likely a native DLL dependency issue, not a code defect.
- **Real SSH connection verification:** Native keepalive, bounded reconnect, cancellation, retry exhaustion, and recovery after server return have automated coverage but remain `NOT VERIFIED` against a live SSH server in this environment.

## Summary

- P0: 0
- P1: 1 (encrypted key format limitation)
- P2: 1 (host-key UI enhancement)
- P3: 3
- Environment: 3
