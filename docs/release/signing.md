# Signing and release secrets

The updater public verification key and public certificates may be published.
Private signing material must live only in the relevant CI or operator secret
store.

## Standard updater

`apps/desktop/src-tauri/updater_secret.key.pub` is tracked and must exactly
match the updater public key in the base `tauri.conf.json`. The base config
provides updater runtime configuration for Standard dev and local builds, but
does not generate updater artifacts. `tauri.release.conf.json` is the release
override that enables updater artifact generation only. The GitHub Environment
named `production` stores `TAURI_SIGNING_PRIVATE_KEY` and its optional
password. The reusable Standard build's actual `build` job binds that
Environment and reads the secrets directly. The private key must never be
generated, decoded, or written inside the repository. A release is blocked if
the signing secret is absent.

Both **Standard Release Candidate** and **Standard Release** call the same
reusable Standard build workflow. Its `build` job binds the `production`
Environment, so both callers receive the same
`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` values
without manually forwarding signing secrets. The shared build checks that the
private key is non-empty before invoking Tauri; Release Candidate never falls
back to an unsigned updater build. Run RC only for trusted refs because build
scripts from the selected commit execute with access to the production signing
secrets.

Tauri updater signatures authenticate the downloaded update artifact; they do
not replace Windows Authenticode or Apple code signing/notarization. Record
those OS trust states separately for each published candidate.

## Microsoft Store

Partner Center can sign an uploaded correctly identified Store candidate.
Local installed testing may use a disposable self-signed certificate. Keep
PFX/P12 files, certificate passwords, base64 certificate values, and private
keys outside Git. Trust only the public test certificate on a disposable
machine, then remove it.

## Secret inventory

The following are secret-manager values and are forbidden in tracked files:

- Creem API and webhook secrets;
- Supabase service-role keys;
- OAuth client secrets (public client IDs are not secrets);
- Tauri updater private signing key;
- certificate private keys and PFX/P12 contents/passwords;
- Cloudflare/R2 access credentials;
- Google service-account/private credentials;
- Partner Center application secrets; and
- real test access/refresh tokens.

Run `pnpm run check:secrets` before publication. It reports only filenames and
rule categories, never matched values. Repository visibility must not change
until findings are removed and any previously exposed value is revoked and
rotated.
