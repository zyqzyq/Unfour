# Next Steps

> Last scanned: 2026-06-11. No changes since previous checkpoint — task list remains current.

## Recommended: Polish & Integration

Priority order:

1. **Lint warning cleanup**
   - Goal: Reduce `react-hooks/set-state-in-effect`, `react-hooks/exhaustive-deps`, `react-hooks/refs`, and `react-refresh/only-export-components` warnings across the codebase.
   - Scope: `packages/api-debugger`, `packages/database`, `packages/terminal`, `apps/desktop`.
   - Forbidden: Do not change component behavior or refactor hooks beyond what is needed to resolve the warnings.
   - Risk: Low — targeted fixes per warning.
   - Prerequisites: None.
   - Acceptance criteria: `pnpm run lint` produces fewer warnings than the current baseline; no new errors introduced; component rendering unchanged.
   - Independent commit: Yes.
   - Recommended model: weaker cheaper model is sufficient.

2. **Semantic token replacement in App.tsx**
   - Goal: Replace 23 hardcoded Tailwind color classes (`slate-*`, `white`, `rose-*`, `teal-*`) in `apps/desktop/src/App.tsx` with semantic `--u-color-*` CSS custom properties.
   - Scope: `apps/desktop/src/App.tsx` only.
   - Forbidden: Do not restructure components, move code to other packages, or change rendered output.
   - Risk: Low — visual-only change, verified by build and viewport inspection.
   - Prerequisites: None.
   - Acceptance criteria: No hardcoded color utility classes remain in App.tsx; `pnpm run build` succeeds; visual output unchanged.
   - Independent commit: Yes.
   - Recommended model: weaker cheaper model is sufficient.

3. **Workspace dialog extraction from App.tsx**
   - Goal: Extract workspace CRUD dialogs (`WorkspaceMenu`, `WorkspaceDialog`) and window controls (`WindowControls`, `TitlebarWindowButton`, `AppTitleBar`) out of `apps/desktop/src/App.tsx` into dedicated component files within `apps/desktop`.
   - Scope: `apps/desktop/src/App.tsx` and new component files within `apps/desktop/src/`.
   - Forbidden: Do not move logic into feature packages. Do not change business behavior or Tauri command calls.
   - Risk: Medium — touches the composition root; must preserve all existing wiring.
   - Prerequisites: Task 2 (semantic tokens) completed first to avoid merge conflicts.
   - Acceptance criteria: App.tsx is significantly shorter; all workspace CRUD and window control functionality preserved; `pnpm run build` succeeds; no new lint errors.
   - Independent commit: Yes.
   - Recommended model: Codex / stronger coding model.

## Completed

- **SSH authentication UX completion:** Full verification pass confirming private-key authentication (unencrypted + passphrase-encrypted via SecretStore), host-key fingerprint management (view/reset/mismatch), connection form auth-type selection, browser mock compatibility, and error sanitization. 25 native-feature tests pass. All 71 Rust tests across 6 crates pass. 59 frontend tests pass. Production build succeeds.
- **API body redaction:** JSON body redaction via `redact_json_body` in `crates/unfour-core/src/redaction.rs`. Applied in `crates/http-engine/src/api_client.rs` for both `save_request` and `insert_history` persistence paths. Applied in browser mock (`packages/command-client/src/tauri.ts`) for `api_request_save` and `api_send_request`. Sensitive keys (authorization, cookie, proxy-authorization, x-api-key, x-auth-token) recursively replaced with `<redacted>` while preserving JSON structure. 7 Rust unit tests + 2 Rust integration tests + 3 browser mock tests.
- **Host-key trust confirmation dialog:** `HostKeyTrustDialog` component in `packages/terminal/src/components/HostKeyTrustDialog.tsx`. Pre-connect fingerprint check via `getSshHostFingerprint`, confirmation required for first trust, clear mismatch display. Integrated into `TerminalPage.tsx` connect flow.
- **known_hosts import/export:** OpenSSH known_hosts file parsing and generation in `crates/ssh-engine/src/host_key.rs`. SHA-256 fingerprint computation from raw public key bytes. Import parses entries, computes fingerprints, stores via HostKeyStore. Export generates known_hosts format with comments for entries missing key data. Tauri commands `ssh_known_hosts_import` and `ssh_known_hosts_export`. 10 Rust tests covering parsing, import, export, deduplication, comments, bracketed ports, and base64 roundtrip.
- **Terminal session persistence:** SQLite-backed output history with per-session buffering (16 KB / 500 ms flush interval), secret redaction, UTF-8-safe truncation (256 KB retention limit), hydration on app reopen, browser mock mode compatible. 5 Rust tests in `terminal_history.rs`, 2 integration tests in `ssh.rs`, 1 frontend store test, 1 browser mock test.
- **Connection health monitoring:** Native russh keepalive runs every 3 seconds and detects an unresponsive peer in about 9 seconds.
- **Bounded reconnection:** Unexpected disconnects expose degraded/reconnecting/failed states, retry at 1/2/4 seconds, stop after 3 attempts, and support cancellation.
- **Reconnect cleanup:** Explicit close suppresses reconnect, one supervisor owns each session lifecycle, event listeners are centralized, and failed/cancelled sessions release native handles and cancellation senders.
- **Private-key authentication:** Implemented. Unencrypted keys load from disk via `ssh-key::PrivateKey::read_openssh_file`. Encrypted keys attempt passphrase from SecretStore. Host-key TOFU works for both auth methods.
- **Host-key fingerprint UI:** View trusted fingerprint and reset fingerprint implemented in `SshConnectionDialog`. Mismatch errors surface via the backend.
- **Host-key management Tauri commands:** `ssh_host_key_get` and `ssh_host_key_reset` added.
