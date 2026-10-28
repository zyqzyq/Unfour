# Desktop account login audit — 2026-10-28

## Scope and baseline

- Baseline: current local Unfour `main`, commit `1e5bdf63`.
- Evidence: this repository only; no Unfour-Pro or historical implementation used.
- Reviewed the `unfour://` callback path, pending PKCE, token exchange,
  network failures, system credential persistence, and AccountProvider recovery.
- No authentication wire-contract, Cloud Sync protocol, GCP configuration,
  billing behavior, or dependency changes. The Cloud Sync UI consumer only maps
  the new `api_timeout` code to its existing transport-failure state.

## Findings and implemented fixes

1. **One-time authorization codes were retried after ambiguous failures.**
   `/v1/desktop/token` shared a transient retry helper with read-only account
   queries. A timeout, disconnect, or gateway response does not prove that the
   server did not consume the code. Token exchange now sends exactly one request;
   failure consumes the pending attempt and requires fresh authorization.
   Read-only `/v1/me` retains its existing bounded retry policy.
2. **Failure details were lost across the backend/UI boundary.** Stable error
   codes now retain an explicit failure stage. Timeouts, invalid/expired
   authorization, and local credential failures have distinct diagnostics.
   Authoritative signed-out snapshots can carry the reason for credential
   cleanup or session expiry. Browser-open errors use a fixed safe message.
   UI rendering accepts only known code/stage values and localized guidance;
   it never displays arbitrary server/platform messages, callback URLs, PKCE
   verifiers, or session tokens.
3. **Retry had misleading recovery semantics.** Account views now offer
   explicit actions: start a new sign-in, refresh saved account state, or retry
   sign-out. Local configuration/listener failures instead explain restart and
   escalation. Foreground refresh does not erase an unresolved login failure.
4. **Callback delivery and stale work could interfere with recovery.** Live
   callback registration precedes cold-start lookup; an invalid cold callback
   does not disable the live listener or stop processing subsequent callbacks.
   A bounded digest cache coalesces/rejects duplicate deliveries without keeping
   raw callback URLs. Rust still owns pending-state validation and consumption.
   New authorization attempts invalidate older in-flight exchanges, including
   their failures. The UI ignores superseded/no-pending results when a newer
   attempt, completed login, or more useful failure is already present.
5. **Credential cleanup had a stale-read race.** A state query that read corrupt
   credentials could later delete credentials written by a new sign-in. Cleanup
   now uses the session-transition lock and checks the session generation before
   deleting. A deterministic regression reproduced the race before the fix.
   Save failures preserve the previous session and report `credentialSave`;
   they never trigger an authorization-code replay.

## Recovery contract

| Failure stage / condition | User-visible recovery | Credential/code behavior |
| --- | --- | --- |
| Open browser / prepare sign-in | Fix browser or credential access, then start new sign-in | Fresh state and PKCE |
| Callback without pending PKCE, denial, expired/used code | Start new sign-in | No replay of prior callback |
| Code exchange timeout, disconnect, gateway failure | Check connectivity, then start new sign-in | Result may be ambiguous; never resend code |
| Credential save failure | Restore system credential access, then start new sign-in | Previous stored session retained; consumed code not reused |
| Read credential / refresh account transient failure | Restore credential access or network, then refresh account | Query existing saved session only |
| Corrupt saved session / expired authorization | Start new sign-in | Invalid credentials removed with generation protection |
| Sign-out cleanup failure | Retry sign-out | Existing cleanup semantics retained |
| Callback listener / invalid configuration / corrupt installation identity | Restart app; report safe code and stage if persistent | No automatic installation identity rotation |

## Verification

All automated tests use existing mocks, in-memory credentials, or loopback HTTP;
no live account or production credential was used.

| Check | Result |
| --- | --- |
| `cargo test -p unfour-account` | PASS — 53 tests |
| `cargo test -p unfour --lib account::tests` | PASS — 12 tests |
| `pnpm exec vitest run apps/desktop/src/features/account apps/desktop/src/desktopSettings.test.tsx apps/desktop/src/features/cloud-sync/CloudSyncProvider.test.tsx` | PASS — 84 tests in 10 files |
| `pnpm exec eslint apps/desktop/src/features/account apps/desktop/src/desktopSettings.test.tsx apps/desktop/src/features/cloud-sync/CloudSyncProvider.tsx apps/desktop/src/features/cloud-sync/CloudSyncProvider.test.tsx` | PASS |
| `pnpm run build` (includes TypeScript checking) | PASS; existing bundle-size warning remains |
| `cargo fmt -p unfour-account -p unfour --check` | PASS |
| `node scripts/check-large-files.mjs` | PASS — no blocking violations |
| `git diff --check` | PASS |

Regressions cover timeout, dropped/truncated responses, 502/503/504,
expired/used codes, duplicate/cold callbacks, listener failures, stale exchanges,
credential save/read/cleanup failures, expired sessions, preservation of signed-in
profiles after refresh failure, recovery action dispatch, and safe English/Chinese
diagnostics. Tests assert consumed codes cannot issue a second exchange.

Earlier sandbox-limited loopback/cache checks failed; unrestricted reruns passed.
Test-first regressions intentionally failed before their fixes. Rust test runs
warn that the unspecified release channel defaults to `test`; these are local
test runs, not release-package verification.

## Remaining design risks and recommendations

- **Pending PKCE is memory-only.** Restarting the process loses the pending
  verifier, so a cold callback cannot complete that attempt. Safe recovery is a
  fresh browser login. Persisting PKCE would require a separate lifecycle and
  threat-model decision; this change does not persist it.
- **Pending attempts have no local TTL.** An abandoned browser attempt can stay
  in `signingIn`; the UI now exposes fresh sign-in. Agree an expiry policy with
  the browser/server contract before changing it.
- **Ambiguous exchange completion remains a protocol limitation.** A server
  success followed by response loss or local credential-save failure can leave
  a remote session that the desktop cannot recover. Do not retry the one-time
  code. Any future idempotency/reconciliation mechanism needs an explicit
  protocol design and server agreement.
- **Read-only refresh can take a long time.** Existing limits are five attempts
  with 30-second per-request timeout plus 10 seconds of backoff (roughly 160
  seconds maximum). Consider a separate UX deadline/cancellation design; this
  audit retains those limits.
- **Live desktop end-to-end behavior is NOT VERIFIED.** Installed protocol
  registration (including MSIX/NSIS and competing installations), browser-to-app
  handoff, and actual Windows/macOS/Linux credential-service permissions need a
  disposable test account and installed builds. Automated tests establish the
  code paths, not the cause of a specific production incident. Native visual
  inspection is also NOT VERIFIED; localized component interactions are tested.
- **Operational follow-up:** reproduce with the displayed stable code and stage.
  Never collect callback URLs, authorization codes, verifiers, or tokens. For
  persistent installation-identity corruption, investigate local storage rather
  than silently rotating the identity.
