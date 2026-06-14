# Open Issues

> Last scanned: 2026-06-14 (post-fix verification sweep).

## P0 — Blocks core usage

None.

## P1 — High priority

### SSH Transport

- **Encrypted key passphrase** (Observed): The ssh-key crate 0.7.0-rc.10 has limited support for decrypting encrypted OpenSSH keys. Keys without passphrases work. Encrypted key loading returns a clear error guiding users to save a passphrase credential. Passphrase is read from SecretStore, never stored in SQLite.
- **Real SSH release smoke** (Observed): Password login, private-key login, PTY I/O, resize, search, history restore, TOFU first trust, mismatch rejection, fingerprint reset, and reconnect remain `NOT VERIFIED` against a reachable live SSH server in the current environment. Automated coverage passes.

## P2 — Medium priority

- **Host-key UI enhancement** (Observed): View/reset fingerprint implemented. Trust confirmation dialog (first trust + mismatch) implemented. known_hosts import/export implemented. Fingerprint change confirmation without full reset is a future enhancement.
- **Lint warning cleanup** (Observed): 53 warnings across `packages/api-debugger`, `packages/database`, `packages/terminal`, `packages/ui`, and `apps/desktop`. Predominantly `react-hooks/refs` false positives from TanStack Query destructuring. Reduced from 64 in commit `3649a2d` via pure-function extraction and `useEffect` sync-pattern refactoring. No errors; none block builds.

## P3 — Low priority / Future

- **Terminal multiplexing** (Inferred): Support tmux/screen-like session management within the app.
- **SCP/SFTP file transfer** (Inferred): Leverage the existing russh connection for file operations.

## Environment / Tooling

- **macOS/Linux startup and keychain** (Observed): `keyring` platform features are now configured in Cargo.toml (`apple-native` for macOS, `crypto-rust` + `sync-secret-service` for Linux). Windows Credential Manager is runtime-verified for SSH passwords, SSH key passphrases, PostgreSQL passwords, and MySQL passwords. Apple Keychain and Linux Secret Service runtime create/read/delete remain `NOT VERIFIED`.
- **Native Windows visual capture** (Observed): The app launches and remains responsive, but WebView content could not be captured or inspected through the available automation surface. Browser-mode UI smoke passes.

## Summary

- P0: 0
- P1: 2 (encrypted key format limitation, real SSH release smoke)
- P2: 2 (host-key UI enhancement, lint warning cleanup)
- P3: 2 (terminal multiplexing, SCP/SFTP)
- Environment: 2 (macOS/Linux startup and keychain, native Windows visual capture)
