# Release Checklist

This checklist is for the Community Stable `v0.5.0` release.

## Release setup

- Confirm the release commit and a clean working tree.
- Confirm the unique version source is `0.5.0` in root
  `[workspace.package]`; run the version sync and confirm the root package,
  desktop package,
  Tauri configuration, and any packaged Rust crates.
- Confirm the release tag is exactly `v0.5.0` and points to the verified
  release commit. Community rejects every pre-release tag.
- Confirm the release workflow resolves Stable with `prerelease = false`,
  exports `UNFOUR_RELEASE_CHANNEL=stable`, and embeds the exact checked-out
  commit as `UNFOUR_BUILD_COMMIT` throughout verification and artifact builds.
- Review `README.md`, `README.zh-CN.md`, `CHANGELOG.md`, `SECURITY.md`, and
  `LICENSE`.
- Confirm release notes describe this as a release and do not claim
  unverified platforms or live-service checks are supported.

## Required automated verification

The release workflow must complete its independent `verify` job before any
platform build job:

- `pnpm install --frozen-lockfile`
- `pnpm run lint`
- `pnpm run test`
- `pnpm run check`
- `pnpm run test:rust`
- Playwright Chromium installation and `pnpm run test:e2e` when the GitHub
  Actions runner can execute the existing local smoke suite.

The workflow then builds macOS and Linux with their existing targets and builds
the Windows NSIS target. The Windows release asset set must contain one NSIS
`.exe` installer for this version and must not include stale MSI output.

## Artifact review

- Build artifacts come from the verified release commit.
- The single aggregation job generates and uploads `SHA256SUMS.txt` alongside
  the installers.
- Artifact names identify the app, version, platform, and architecture where
  Tauri provides those fields.
- Windows release notes identify NSIS as the only Windows installer format.
- Unsigned artifacts and possible SmartScreen/security warnings are stated in
  the Release body.
- macOS/Linux artifacts remain experimental or unverified until real-device
  smoke checks are complete.

## Manual gates

- Windows NSIS install and app launch: record the actual result.
- Verify upgrade from `v0.2.0` and install/uninstall while `unfour-mcp.exe` is
  held by an MCP client; the installer should prompt and complete rather than
  stall.
- Windows first viewport, quit/relaunch, uninstall, and upgrade behavior:
  record the actual result; do not infer it from bundle generation.
- macOS and Linux launch/install smoke: `NOT VERIFIED` until run on real
  devices.
- API request scripts, API snapshot/external-apply behavior, Workspace
  transactional domain behavior, storage profiles, Database row actions, SSH
  clipboard/SFTP/tasks/command history and suggestions, and MCP history smoke:
  record only what was actually tested.
- Live SSH, PostgreSQL, MySQL/MariaDB, and system credential-store checks:
  require the corresponding real server, OS, or credential environment.
- Signing/notarization status: record as unsigned/not verified until completed.

## Go / no-go

Do not publish if a required automated verification step is `FAIL`. A
`NOT RUN` or `NOT VERIFIED` item requires maintainer acceptance; it must not be
rewritten as `PASS`.
