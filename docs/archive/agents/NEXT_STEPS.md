# Next Steps

> Last scanned: 2026-06-14 (release readiness smoke verification pass).

## Recommended: Release Gate and Preparation

Priority order:

1. **Live SSH release gate verification**
   - Goal: Verify the complete SSH user journey against a reachable live SSH server to unblock early basic release.
   - Scope:
     - password login
     - private-key login (unencrypted + passphrase-encrypted via SecretStore)
     - PTY input/output
     - resize
     - search
     - terminal history restore (hydration on app reopen)
     - TOFU first trust (host-key fingerprint recording)
     - mismatch rejection (fingerprint change detection and warning)
     - fingerprint reset
     - keepalive / reconnect (unexpected disconnect recovery)
   - Forbidden: Do not modify SSH engine, terminal package, or SecretStore code unless a confirmed blocker is found during verification.
   - Risk: May reveal timing, race condition, or platform-specific issues not covered by automated tests.
   - Prerequisites: A reachable SSH server with both password and key authentication configured. The server must be accessible from the test machine running the Unfour desktop app.
   - Acceptance criteria: Every item in the scope list above is manually verified against the live server and recorded as PASS or FAIL.
   - Independent commit: No code change expected; fix only confirmed blockers.
   - Evidence: OPEN_ISSUES.md P1 — "Real SSH release gate" (release blocker); PROJECT_STATE.md Known Limitations — "Release gate — live SSH verification"
   - Recommended model: If code fixes are needed, use Codex / stronger coding model (Rust async, russh, cross-layer event streaming). If verification only, human or weaker cheaper model is sufficient.

2. **macOS and Linux release smoke**
   - Goal: Verify app startup and real OS keychain create/read/delete on both remaining desktop targets.
   - Scope: App launch, window rendering, SecretStore credential create/read/delete for SSH passwords, SSH key passphrases, PostgreSQL passwords, and MySQL passwords.
   - Forbidden: Do not change Cargo.toml keyring feature flags or SecretStore crate code unless a confirmed blocker is found.
   - Risk: Platform-specific keyring API differences may surface runtime errors.
   - Prerequisites: Access to macOS and Linux machines or CI runners with keychain/keyring services.
   - Acceptance criteria: No startup crash or blank screen; Apple Keychain and Linux Secret Service pass all four credential categories.
   - Independent commit: No code change expected; fix only confirmed blockers.
   - Evidence: OPEN_ISSUES.md Environment — "macOS/Linux startup and keychain"; PROJECT_STATE.md Known Limitations — macOS/Linux release smoke
   - Recommended model: Codex / stronger coding model (only if code fixes are needed)

3. **Packaging and installer smoke**
   - Goal: Build release bundles and install/launch them on each target platform.
   - Scope: Release bundle generation, clean install, first launch, upgrade over a prior build, uninstall behavior.
   - Forbidden: Do not modify Tauri configuration or build pipeline unless a confirmed blocker is found.
   - Risk: Platform-specific packaging quirks (code signing, notarization, Linux AppImage/deb).
   - Prerequisites: Release build pipeline configured; target platform access.
   - Acceptance criteria: Clean install, first launch, upgrade over a prior basic build, and uninstall behavior are recorded.
   - Independent commit: No code change expected; fix only confirmed blockers.
   - Evidence: Logical release preparation step; no existing issue. PROJECT_STATE.md Verified Capabilities (Build) does not cover packaging or installer verification.
   - Recommended model: Codex / stronger coding model (only if code fixes are needed)

4. **Lint warning deep cleanup**
   - Goal: Investigate and reduce the remaining 53 lint warnings, predominantly `react-hooks/refs` false positives from TanStack Query destructuring patterns in `ApiDebuggerPage.tsx`.
   - Scope: `packages/api-client` (primary source of remaining warnings), plus any residual warnings in `packages/database`, `packages/ssh-terminal`, `packages/ui`, and `apps/desktop`.
   - Forbidden: Do not add `eslint-disable` comments to suppress TanStack Query false positives without evaluating alternative hook patterns first. Do not change business logic or component behavior.
   - Risk: TanStack Query destructuring patterns are structural; refactoring them may require hook API changes or wrapper abstractions that increase complexity.
   - Prerequisites: Understanding of the `react-hooks/refs` rule's interaction with TanStack Query's `useQuery`/`useMutation` return values.
   - Acceptance criteria: Warning count reduced or each remaining warning documented with a rationale for acceptance.
   - Independent commit: Yes.
   - Evidence: OPEN_ISSUES.md P2 — "Lint warning cleanup"; `pnpm run lint` output shows 53 warnings
   - Recommended model: weaker cheaper model is sufficient

## Completed

- **Keyring platform features fix:** `keyring` 3 was configured without platform feature flags, causing production builds to use the non-persistent mock backend. Fixed by enabling `windows-native`, `apple-native`, and `crypto-rust` + `sync-secret-service` per target in `crates/secret-store/Cargo.toml`. Added `os_keychain_release_smoke.rs` integration test. Windows Credential Manager create/read/delete verified for SSH passwords, SSH key passphrases, PostgreSQL passwords, and MySQL passwords. — Commit `1a542cb`
- **Windows workspace test stabilization:** `cargo test -p unfour` was crashing on Windows with `STATUS_ENTRYPOINT_NOT_FOUND` because the Tauri-generated `resource.lib` (Windows Common Controls v6 manifest) was not found by the test binary linker. Fixed by adding a Windows-specific `cargo:rustc-link-search` directive in `build.rs` and an explicit `#[link(name = "resource", kind = "static")]` in `lib.rs`. All 8 workspace engine tests now pass. — Commit `b53e4ac`
- **Lint warning reduction (phase 1):** Reduced warnings from 64 to 53 via pure-function extraction (`module-helpers.ts` from `utils.tsx`, `terminal-session-status.ts` from `TerminalSessionTab.tsx`), `useEffect` sync-pattern replacement with render-time state tracking in 6 components, and reference stabilization with `useMemo`/`useRef`. Zero behavioral changes. — Commit `3649a2d`
- **Release-readiness smoke and SecretStore backend fix:** Full frontend/Rust verification passes; Windows native startup and browser UI smoke pass. Found that `keyring` 3 had no platform features enabled and was using its mock backend in production. Enabled Windows Credential Manager, Apple Keychain, and Linux Secret Service. — Commit `bfedcf8` (database hardening), later superseded by `1a542cb` (keyring fix)
- **Database live verification and hardening:** PostgreSQL 18 and MariaDB 12.3.2 verified against isolated localhost servers. SecretStore-backed passwords, connection tests, schema/database/table/column browsing, reads, pagination, mutation confirmation, confirmed writes, syntax errors, invalid credentials, and unavailable ports all passed.
- **MySQL live driver phase 1:** SecretStore-backed password loading, `sqlx::MySqlPool` lifecycle, live `test_connection`, multi-database/schema browsing, columns, read-only query execution, mutation confirmation, schema-qualified paginated table browsing, sanitized errors, browser mock compatibility, and focused tests.
- **Semantic token replacement in App.tsx:** All hardcoded Tailwind color classes replaced with semantic `--u-color-*` CSS custom properties. — Commits `fbc7330`, `002e54f`
- **Workspace dialog extraction from App.tsx:** All workspace CRUD dialogs, window controls, and the title bar extracted into dedicated component files. — Commit `fbc7330`
- **PostgreSQL live driver phase 1:** Full PostgreSQL connection support with credential loading, schema browsing, read-only queries, mutation confirmation, table browsing with pagination, error sanitization, and browser mocks. — Commit `9c99e0f`
- **SSH authentication UX completion:** Private-key authentication, host-key fingerprint management, connection form auth-type selection, browser mock compatibility, and error sanitization. 25 native-feature tests pass. — Commit `acba247`
- **API body redaction:** JSON body redaction via `redact_json_body` in `crates/unfour-core/src/redaction.rs`. 7 Rust unit tests + 2 Rust integration tests + 3 browser mock tests. — Commit `53a2974`
- **Host-key trust confirmation dialog:** Pre-connect fingerprint check, confirmation for first trust, clear mismatch display. — Commit `13c4b28`
- **known_hosts import/export:** OpenSSH known_hosts parsing/generation, SHA-256 fingerprint computation, import/export Tauri commands. 10 Rust tests. — Commit `53a2974`
- **Terminal session persistence:** SQLite-backed output history with per-session buffering, secret redaction, UTF-8-safe truncation, hydration on app reopen. — Commit `09da7d8`
- **Connection health monitoring:** Native russh keepalive every 3 seconds, unresponsive peer detection in ~9 seconds. — Commit `5351897`
- **Bounded reconnection:** Retry at 1/2/4 seconds, stop after 3 attempts, cancellation support. — Commit `5351897`
- **Reconnect cleanup:** Explicit close suppresses reconnect, one supervisor per session, centralized event listeners. — Commit `5351897`
- **Private-key authentication:** Unencrypted and passphrase-encrypted keys via SecretStore. Host-key TOFU works for both auth methods. — Commit `13c4b28`
- **Host-key fingerprint UI:** View and reset fingerprint in `SshConnectionDialog`. — Commit `13c4b28`
- **Host-key management Tauri commands:** `ssh_host_key_get` and `ssh_host_key_reset`. — Commit `13c4b28`
