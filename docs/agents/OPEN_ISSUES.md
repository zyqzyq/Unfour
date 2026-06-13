# Open Issues

> Last scanned: 2026-06-13 (checkpoint refresh, working tree clean at commit `c6927c6`).

## P0 — Blocks core usage

None.

## P1 — High priority

### SSH Transport

- **Encrypted key passphrase** (Observed): The ssh-key crate 0.7.0-rc.10 has limited support for decrypting encrypted OpenSSH keys. Keys without passphrases work. Encrypted key loading returns a clear error guiding users to save a passphrase credential. Passphrase is read from SecretStore, never stored in SQLite.

## P2 — Medium priority

- **Host-key UI enhancement** (Observed): View/reset fingerprint implemented. Trust confirmation dialog (first trust + mismatch) implemented. known_hosts import/export implemented. Fingerprint change confirmation without full reset is a future enhancement.
- **PostgreSQL live verification** (Observed): Code is complete for PostgreSQL connection, schema browsing, query execution, and table browsing. Automated tests cover credential loading, error sanitization, confirmation flow, and metadata CRUD. Live server verification remains `NOT VERIFIED`.
- **Lint warning cleanup** (Observed): 64 pre-existing warnings across `packages/api-debugger` (primarily `react-hooks/refs` in ApiDebuggerPage), `apps/desktop` (`react-hooks/set-state-in-effect` in WorkspaceDialogs, `react-hooks/exhaustive-deps` in useLayoutPersistence, `react-refresh/only-export-components` in utils.tsx). Reduced from 65 to 64 since last checkpoint. No errors; none block builds.

## P3 — Low priority / Future

- **Terminal multiplexing** (Inferred): Support tmux/screen-like session management within the app.
- **SCP/SFTP file transfer** (Inferred): Leverage the existing russh connection for file operations.
- **MySQL database driver** (Inferred): Not started. PostgreSQL phase 1 is complete; MySQL is reserved for a future phase.

## Environment / Tooling

- **OS keychain** (Inferred): The `keyring` crate is used for production but has not been verified on all target platforms (macOS, Windows, Linux).
- **Windows workspace tests** (Observed): `cargo test -p unfour-workspace` fails with `STATUS_ENTRYPOINT_NOT_FOUND` on this Windows environment. Likely a native DLL dependency issue, not a code defect.
- **Real SSH connection verification** (Observed): Native keepalive, bounded reconnect, cancellation, retry exhaustion, and recovery after server return have automated coverage but remain `NOT VERIFIED` against a live SSH server in this environment.
- **Real PostgreSQL verification** (Observed): Connection, schema browsing, query execution, and table browsing are code-complete but `NOT VERIFIED` against a live PostgreSQL server in this environment.

## Summary

- P0: 0
- P1: 1 (encrypted key format limitation)
- P2: 3 (host-key UI enhancement, PostgreSQL live verification, lint warning cleanup)
- P3: 3 (terminal multiplexing, SCP/SFTP, MySQL driver)
- Environment: 4 (OS keychain, Windows workspace tests, SSH live, PostgreSQL live)
