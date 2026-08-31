# v0.9.1 release checklist

## v0.9.1 preparation status

- Source version: `0.9.1`; the latest published release remains `v0.9.0`.
- Version synchronization: `PASS` (`node scripts/sync-version.mjs --check`).
- Cargo workspace lock/check: `PASS` (`cargo check --workspace --locked`).
- Release contract suite: `PASS` (72 tests via the direct Node test command).
- Candidate artifact, platform, live-service, and manual checks: `NOT VERIFIED`;
  they require fresh evidence for the v0.9.1 candidate.

## Previous v0.9.0 recorded release outcomes

This is the v0.9.0 outcome record, not an inference from CI or release assets.
The Linux baseline follow-up is explicitly marked as new-artifact work:

| Area | Recorded status |
| --- | --- |
| Windows NSIS install, launch, and uninstall | VERIFIED |
| Windows previous-Stable-to-new-Stable updater journey | VERIFIED |
| GitHub browser OAuth, Desktop login/callback, and basic account state | VERIFIED |
| Creem Test checkout, webhook, entitlement, and billing portal | VERIFIED |
| PostgreSQL and MySQL | VERIFIED |
| SSH Terminal, SFTP, and SSH Tasks | VERIFIED |
| macOS arm64 and x64 install/run | VERIFIED |
| Real Codex and Cursor MCP client start, initialization, discovery, tool call, and real Unfour data/tool access | VERIFIED |
| v0.9.0 unified-client Cloud Sync multi-device regression, with single-device coverage recorded in the same run | NOT VERIFIED; historical live multi-device verification exists |
| Creem Production first real end-to-end transaction and entitlement flow | NOT VERIFIED; Test is VERIFIED and Production is not failed |
| MCP prod read-only, blocked-write, confirmation binding, and confirmed-retry behavior | NOT VERIFIED |
| v0.9.0 Linux x64 AppImage artifact build / Ubuntu 20.04 runtime | PASS / FAIL; Ubuntu 24.04-era GLIBC/GLIBCXX requirements; Ubuntu 20.04 is outside the current baseline |
| Linux x86_64 AppImage, Ubuntu 22.04+ runtime baseline | Experimental; new-artifact Ubuntu 22.04/24.04 regression, desktop integration, and updater remain NOT VERIFIED |
| Real MSIX install, Store servicing, Partner Center, callback, and alias journey | NOT VERIFIED; static contract/build-policy tests exist |
| macOS Gatekeeper warning/trust behavior | NOT VERIFIED; signing/notarization is not enabled, and arm64/x64 install/run stays VERIFIED |

The completed real Codex and Cursor checks supersede a separate basic MCP
manual-smoke release gate. Keep the protocol smoke procedure for diagnostics
and future regression use.

The sections below preserve the reusable release procedure for v0.9.1. Their
imperative steps are not additional v0.9.1 `PASS` claims; the v0.9.0 table above
and `docs/testing/release-verification.md` are historical outcome records until
new v0.9.1 evidence is recorded.

## Shared gate

- Working tree and intended release commit are reviewed.
- `pnpm run check:version`, `pnpm run check:secrets`, migration checks,
  frontend/Rust tests, and release contract tests pass.
- The release version is unused, exactly `X.Y.Z`, and the tag is `vX.Y.Z`.
- Historical `pro_*` SQL migration files pass their immutable checksum guard.
- Historical Community DB, historical Pro DB, and clean DB migrations have
  current test evidence.
- API, SSH, Database, MCP, Account, Cloud, and multi-device manual results are
  recorded; unavailable live services remain `NOT VERIFIED`.

## Release Candidate

- In GitHub Actions, manually run **Standard Release Candidate** with `ref=main`
  or another reviewed branch/commit; record the resolved commit SHA.
- Confirm the GitHub Environment `production` contains
  `TAURI_SIGNING_PRIVATE_KEY` and, when the key is encrypted,
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The reusable workflow's actual
  `build` job binds that Environment and a missing private key must fail the
  shared build rather than produce unsigned updater artifacts.
- Confirm the reusable workflow completes the full verify job and exactly four
  native builds: Windows x64, macOS arm64, macOS x64, and Linux x64.
- For the next candidate, confirm the actual Linux build uses `ubuntu-22.04`
  with a runner-specific Rust cache key. The independent verify job may use
  `ubuntu-latest`; it must not supply packaged native artifacts.
- Download all four `release-candidate-*` Actions artifacts. Verify canonical
  filenames, non-empty updater signatures, and that each file belongs to the
  expected architecture.
- Confirm Windows contains one NSIS installer plus `.sig`; each macOS artifact
  contains its DMG, `.app.tar.gz`, and `.app.tar.gz.sig`; Linux contains only
  the public x64 AppImage and `.sig` even if Tauri also built `.deb`/`.rpm`.
- Record install, launch, upgrade, updater, signature rejection, OS trust, and
  uninstall results from the downloaded candidate artifacts.
- For Linux Experimental (x86_64 AppImage only), record Ubuntu 22.04 executable
  permission, launch, first-window rendering, API/SSH/Database module opening,
  and quit/relaunch, plus an Ubuntu 24.04 launch smoke with the same artifact.
  Retain Linux updater smoke and signature-rejection checks; unrun items remain
  `NOT VERIFIED`. Follow `docs/testing/manual-test-cases.md`. Do not rebuild or
  overwrite v0.9.0 artifacts to exercise the new baseline.
- Confirm the RC run created no tag or GitHub Release, accessed no R2 path,
  changed neither `stable/latest.json` nor `stable/downloads.json`, and built
  or published no MSIX.

## Standard

- Proceed only from the reviewed commit represented by the Release Candidate;
  create the immutable `vX.Y.Z` tag according to the release procedure.
- CI exports `UNFOUR_DISTRIBUTION=standard` and `stable`.
- The updater private signing key exists only in the GitHub Environment
  `production`; the tracked public key exactly matches the updater
  configuration.
- The formal workflow calls the same reusable verify/build/staging workflow as
  RC; one matrix build produces each installer and updater signature.
- The reusable Standard `build` job binds the `production` Environment for
  Tauri signing, and the formal `publish` job also binds `production` for
  `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_ACCOUNT_ID`, and `R2_BUCKET`.
- Linux Standard staging contains only the x64 AppImage and its `.sig`;
  `.deb`, `.rpm`, and Linux ARM64 are not canonical public release assets.
- The aggregation job creates `SHA256SUMS.txt`, `latest.json`, and
  `downloads.json` only after validating actual canonical installer files and
  all four signed updater artifacts. Missing installers fail the release.
- `downloads.json` contains exactly `windows-x64` (`.exe`), `macos-arm64`
  (`.dmg`), `macos-x64` (`.dmg`), and `linux-x64` (`.AppImage`). Website and
  Download Worker consumers read its URLs instead of guessing filenames.
- Tauri `latest.json` retains `windows-x86_64`, `darwin-aarch64`,
  `darwin-x86_64`, and `linux-x86_64`. macOS entries still use `.app.tar.gz`
  with signatures; DMGs are only user installers, never updater substitutes.
- `latest.json` has one Linux entry, `linux-x86_64`, pointing to the AppImage
  and requiring its non-empty signature.
- Neither manifest is uploaded under immutable `stable/{version}/`. Neither
  manifest nor `SHA256SUMS.txt` itself appears in the checksum entries.
- R2 re-download passes the same checksum manifest used by GitHub Release.
- Only after immutable versioned files verify and the GitHub Release succeeds,
  check both live manifest versions, publish `stable/downloads.json`, then
  publish `stable/latest.json` last. Both use `application/json` and `no-cache`.
- The live version gate rejects a numeric SemVer downgrade of either pointer
  and permits equal-version reruns only after immutable verification. A `404`
  allows a first manifest publication; other read/parse failures block it.
- If only downloads promotion succeeds, retry the same release after checking
  the failure; the updater can remain on the previous version until promotion
  completes. The two writes are not atomic.
- For the already published Stable version only, the operator manually creates
  and uploads `stable/downloads.json` as a one-time migration. No historical
  release rebuild or extra workflow is needed. Future new tags containing this
  change generate it automatically with the existing GitHub/R2 configuration.
- Manually exercise install, launch, update from the previous Stable version,
  MCP sidecar replacement, uninstall, and signature rejection.

For the published v0.9.0 release, install, launch, previous-Stable update, and
uninstall are `VERIFIED`. Manual updater signature rejection was not included in
the recorded live journey; artifact signatures and Store updater-policy tests do
not convert it to a manual `PASS`.

## Microsoft Store

- Use a clean Windows x64 release tree and exact Partner Center identity.
- Run the manual MSIX build and validator; do not add Store publication to CI.
- Confirm `X.Y.Z` became `X.Y.Z.0`.
- Inspect packaged build metadata for `distribution=microsoft-store`, Stable
  services, updater disabled, and null updater endpoint.
- Install a signed test package and exercise closed/running-app
  `unfour://auth/callback`, `unfour-mcp.exe` alias, Store upgrade, uninstall,
  and NSIS coexistence.
- Confirm no request is made to the Standard updater endpoint and no internal
  installer can be launched.

## Go/no-go

Do not publish with a required automated `FAIL`, a secret finding, a modified
historical migration, a version/tag mismatch, or different GitHub/R2 bytes.
Manual and real-service items that were not run must stay visibly `NOT
VERIFIED`; a successful build does not convert them to `PASS`.
