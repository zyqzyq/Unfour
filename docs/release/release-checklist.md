# Final release checklist

## Shared gate

- Working tree and intended release commit are reviewed.
- `pnpm run check:version`, `pnpm run check:secrets`, migration checks,
  frontend/Rust tests, and release contract tests pass.
- The release version is unused, exactly `X.Y.Z`, and the tag is `vX.Y.Z`.
- Historical `pro_*` SQL migration files pass their immutable checksum guard.
- Community DB, Pro DB, and clean DB migrations have current test evidence.
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
- Download all four `release-candidate-*` Actions artifacts. Verify canonical
  filenames, non-empty updater signatures, and that each file belongs to the
  expected architecture.
- Confirm Windows contains one NSIS installer plus `.sig`; each macOS artifact
  contains its DMG, `.app.tar.gz`, and `.app.tar.gz.sig`; Linux contains only
  the public x64 AppImage and `.sig` even if Tauri also built `.deb`/`.rpm`.
- Record install, launch, upgrade, updater, signature rejection, OS trust, and
  uninstall results from the downloaded candidate artifacts.
- Confirm the RC run created no tag or GitHub Release, accessed no R2 path,
  changed no `stable/latest.json`, and built or published no MSIX.

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
- The aggregation job creates `SHA256SUMS.txt` and `latest.json`.
- `latest.json` has one Linux entry, `linux-x86_64`, pointing to the AppImage
  and requiring its non-empty signature.
- R2 re-download passes the same checksum manifest used by GitHub Release.
- `stable/latest.json` is uploaded only after immutable versioned files verify
  and the GitHub Release succeeds; its live version gate rejects numeric
  SemVer downgrades and permits equal-version reruns only after that check.
- Install, launch, update from the previous Stable version, MCP sidecar
  replacement, uninstall, and signature rejection are manually exercised.

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
