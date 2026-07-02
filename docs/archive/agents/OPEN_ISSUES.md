# Open Issues

> Last scanned: 2026-06-29 (SSH vi/xterm request-mode verification pass).

## P0 — Blocks core usage

None.

## P1 — High priority

### SSH Transport

- **Encrypted key passphrase** (Observed): The ssh-key crate 0.7.0-rc.10 has limited support for decrypting encrypted OpenSSH keys. Keys without passphrases work. Encrypted key loading returns a clear error guiding users to save a passphrase credential. Passphrase is read from SecretStore, never stored in SQLite.
- **Real SSH release gate** (Observed, **release blocker**): Password login, private-key login, PTY I/O, resize, search, history restore, TOFU first trust, mismatch rejection, fingerprint reset, keepalive, and reconnect remain `NOT VERIFIED` against a reachable live SSH server. This is the **sole blocker for early basic release**. Automated coverage passes.

## P2 — Medium priority

- **Host-key UI enhancement** (Observed): View/reset fingerprint implemented. Trust confirmation dialog (first trust + mismatch) implemented. known_hosts import/export implemented. Fingerprint change confirmation without full reset is a future enhancement.
- **Lint warning cleanup** (Observed): 53 warnings across `packages/api-client`, `packages/database`, `packages/ssh-terminal`, `packages/ui`, and `apps/desktop`. Predominantly `react-hooks/refs` false positives from TanStack Query destructuring. Reduced from 64 in commit `3649a2d` via pure-function extraction and `useEffect` sync-pattern refactoring. No errors; none block builds.
- **xterm 6 request-mode workaround** (Observed): `@xterm/xterm@6.0.0` throws `ReferenceError: i is not defined` in its `requestMode` parser when vi emits `CSI ... $p` / `CSI ? ... $p` mode-query sequences during full-screen startup. The SSH transport and vi process remain live, but xterm rendering can stop. `packages/ssh-terminal` filters those request-mode sequences before `terminal.write`; live vi verification on 2026-06-29 confirmed `filtered xterm request-mode sequence` is observed, the xterm error is not reproduced, and vi insert/edit/exit interaction works. Version evaluation on 2026-06-29 via `npm view` found `@xterm/xterm` current stable remains `6.0.0`; `6.1.0` is beta-only, while `@xterm/addon-fit@0.11.0` and `@xterm/addon-search@0.16.0` are current stable. Do not upgrade/downgrade in this batch; revisit when xterm 6.1 has a stable release or in a dedicated beta/downgrade compatibility test.

## P3 — Low priority / Future

- **Terminal multiplexing** (Inferred): Support tmux/screen-like session management within the app.
- **SCP/SFTP file transfer** (Inferred): Leverage the existing russh connection for file operations.

## Environment / Tooling

- **macOS/Linux startup and keychain** (Observed): `keyring` platform features are configured in Cargo.toml (`apple-native` for macOS, `crypto-rust` + `sync-secret-service` for Linux). Windows Credential Manager is runtime-verified for SSH passwords, SSH key passphrases, PostgreSQL passwords, and MySQL passwords. Apple Keychain and Linux Secret Service runtime create/read/delete remain `NOT VERIFIED`.
- **Native Windows visual capture** (Observed): The app launches and remains responsive, but WebView content could not be captured or inspected through the available automation surface. Browser-mode UI smoke passes.

## Summary

- P0: 0
- P1: 2 (encrypted key format limitation, real SSH release gate)
- P2: 3 (host-key UI enhancement, lint warning cleanup, xterm 6 request-mode workaround)
- P3: 2 (terminal multiplexing, SCP/SFTP)
- Environment: 2 (macOS/Linux startup and keychain, native Windows visual capture)
