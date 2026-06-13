# Next Steps

> Last scanned: 2026-06-13 (database live verification and hardening).

## Recommended: Polish & Integration

Priority order:

1. **Lint warning cleanup**
   - Goal: Reduce `react-hooks/set-state-in-effect`, `react-hooks/exhaustive-deps`, `react-hooks/refs`, and `react-refresh/only-export-components` warnings across the codebase.
   - Scope: `packages/api-debugger`, `packages/database`, `packages/terminal`, `apps/desktop`.
   - Forbidden: Do not change component behavior or refactor hooks beyond what is needed to resolve the warnings.
   - Risk: Low — targeted fixes per warning.
   - Prerequisites: None.
   - Acceptance criteria: `pnpm run lint` produces fewer warnings than the current baseline (64); no new errors introduced; component rendering unchanged.
   - Independent commit: Yes.
   - Evidence: `pnpm run lint` output — 64 warnings confirmed at commit `c6927c6`. Primary sources: `ApiDebuggerPage.tsx` (react-hooks/refs), `WorkspaceDialogs.tsx` (set-state-in-effect), `useLayoutPersistence.ts` (exhaustive-deps), `utils.tsx` (only-export-components). See PROJECT_STATE.md Lint warnings, OPEN_ISSUES.md P2.
   - Recommended model: weaker cheaper model is sufficient

## Completed

- **Database live verification and hardening:** PostgreSQL 18 and MariaDB 12.3.2 verified against isolated localhost servers with seeded `users`, `orders`, empty tables, and Unicode data. SecretStore-backed passwords, connection tests, schema/database/table/column browsing, reads, pagination, mutation confirmation and confirmed writes, syntax errors, invalid credentials, and unavailable ports all passed. Fixed PostgreSQL non-`public` schema qualification and MariaDB signed `COUNT(*)` decoding. SQLite and browser mocks remain green.
- **MySQL live driver phase 1:** SecretStore-backed password loading, `sqlx::MySqlPool` lifecycle, live `test_connection`, multi-database/schema browsing, columns, read-only query execution, existing mutation confirmation, schema-qualified paginated table browsing, sanitized errors, browser mock compatibility, and focused Rust/TypeScript tests. Public contracts gained optional `schema` metadata for tables and browse requests; SQLite and PostgreSQL behavior remain unchanged. Later live-verified in the database hardening batch.
- **Semantic token replacement in App.tsx:** All hardcoded Tailwind color classes (`slate-*`, `white`, `rose-*`, `teal-*`) replaced with semantic `--u-color-*` CSS custom properties across `apps/desktop/src/App.tsx` and all desktop component files. Zero hardcoded color utility classes remain in any desktop source file. Verified by grep and production build. — Commit `fbc7330` (refactor(desktop): extract app shell components) + `002e54f` (refactor(ui): replace hardcoded colors with semantic tokens)
- **Workspace dialog extraction from App.tsx:** All workspace CRUD dialogs (`WorkspaceMenu`, `WorkspaceDialogs`), window controls (`WindowControls`, `TitlebarWindowButton`), and the title bar (`AppTitleBar`) extracted from `apps/desktop/src/App.tsx` into dedicated component files within `apps/desktop/src/components/`. App.tsx reduced to 199 lines, containing only high-level component composition. All workspace CRUD and window control functionality preserved. Production build succeeds. — Commit `fbc7330` (refactor(desktop): extract app shell components)
- **PostgreSQL live driver phase 1:** Full PostgreSQL connection support in `unfour-database-engine`. Credential loading from SecretStore, `sqlx::PgPool` lifecycle, live `test_connection`, schema browsing, read-only query execution with mutation confirmation policy, table browsing with pagination, error sanitization, browser mocks, and frontend enablement. Later live-verified and schema-qualified in the database hardening batch. — Commit `9c99e0f`
- **SSH authentication UX completion:** Full verification pass confirming private-key authentication (unencrypted + passphrase-encrypted via SecretStore), host-key fingerprint management (view/reset/mismatch), connection form auth-type selection, browser mock compatibility, and error sanitization. 25 native-feature tests pass. All 71 Rust tests across 6 crates pass. 59 frontend tests pass. Production build succeeds. — Commit `acba247`
- **API body redaction:** JSON body redaction via `redact_json_body` in `crates/unfour-core/src/redaction.rs`. Applied in `crates/http-engine/src/api_client.rs` for both `save_request` and `insert_history` persistence paths. Applied in browser mock (`packages/command-client/src/tauri.ts`) for `api_request_save` and `api_send_request`. Sensitive keys (authorization, cookie, proxy-authorization, x-api-key, x-auth-token) recursively replaced with `<redacted>` while preserving JSON structure. 7 Rust unit tests + 2 Rust integration tests + 3 browser mock tests. — Commit `53a2974`
- **Host-key trust confirmation dialog:** `HostKeyTrustDialog` component in `packages/terminal/src/components/HostKeyTrustDialog.tsx`. Pre-connect fingerprint check via `getSshHostFingerprint`, confirmation required for first trust, clear mismatch display. Integrated into `TerminalPage.tsx` connect flow. — Commit `13c4b28`
- **known_hosts import/export:** OpenSSH known_hosts file parsing and generation in `crates/ssh-engine/src/host_key.rs`. SHA-256 fingerprint computation from raw public key bytes. Import parses entries, computes fingerprints, stores via HostKeyStore. Export generates known_hosts format with comments for entries missing key data. Tauri commands `ssh_known_hosts_import` and `ssh_known_hosts_export`. 10 Rust tests covering parsing, import, export, deduplication, comments, bracketed ports, and base64 roundtrip. — Commit `53a2974`
- **Terminal session persistence:** SQLite-backed output history with per-session buffering (16 KB / 500 ms flush interval), secret redaction, UTF-8-safe truncation (256 KB retention limit), hydration on app reopen, browser mock mode compatible. 5 Rust tests in `terminal_history.rs`, 2 integration tests in `ssh.rs`, 1 frontend store test, 1 browser mock test. — Commit `09da7d8`
- **Connection health monitoring:** Native russh keepalive runs every 3 seconds and detects an unresponsive peer in about 9 seconds. — Commit `5351897`
- **Bounded reconnection:** Unexpected disconnects expose degraded/reconnecting/failed states, retry at 1/2/4 seconds, stop after 3 attempts, and support cancellation. — Commit `5351897`
- **Reconnect cleanup:** Explicit close suppresses reconnect, one supervisor owns each session lifecycle, event listeners are centralized, and failed/cancelled sessions release native handles and cancellation senders. — Commit `5351897`
- **Private-key authentication:** Implemented. Unencrypted keys load from disk via `ssh-key::PrivateKey::read_openssh_file`. Encrypted keys attempt passphrase from SecretStore. Host-key TOFU works for both auth methods. — Commit `13c4b28`
- **Host-key fingerprint UI:** View trusted fingerprint and reset fingerprint implemented in `SshConnectionDialog`. Mismatch errors surface via the backend. — Commit `13c4b28`
- **Host-key management Tauri commands:** `ssh_host_key_get` and `ssh_host_key_reset` added. — Commit `13c4b28`
