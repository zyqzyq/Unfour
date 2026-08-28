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

## Standard

- CI exports `UNFOUR_DISTRIBUTION=standard` and `stable`.
- The updater private signing key exists only in Actions secrets; the tracked
  public key exactly matches the updater configuration.
- One matrix build produces each installer and updater signature.
- Linux Standard staging contains only the x64 AppImage and its `.sig`;
  `.deb`, `.rpm`, and Linux ARM64 are not canonical public release assets.
- The aggregation job creates `SHA256SUMS.txt` and `latest.json`.
- `latest.json` has one Linux entry, `linux-x86_64`, pointing to the AppImage
  and requiring its non-empty signature.
- R2 re-download passes the same checksum manifest used by GitHub Release.
- `stable/latest.json` is uploaded only after immutable versioned files verify.
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
