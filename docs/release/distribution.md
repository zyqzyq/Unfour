# Distribution

This document describes the public distribution format and release-asset
verification for Community Stable Unfour `v0.6.0`.

## Release workflow

GitHub Actions runs the release workflow in three gates:

1. `verify` installs the frozen dependency graph and runs lint, unit tests,
   repository checks, Rust tests, and Playwright Chromium smoke tests.
2. The platform matrix builds the existing macOS and Linux targets. The
   Windows matrix builds and stages the NSIS installer only.
3. `checksum-release` downloads all platform artifacts, generates one
   `SHA256SUMS.txt` from the actual files, and creates the release with the
   installers and checksum manifest.

If `verify` fails, the build jobs do not run and no release assets are created.

Local `pnpm tauri build` bundles default to the Stable channel. For isolated
local test builds, use `pnpm tauri build:test`; the root launcher exports the
Test channel to Tauri, sidecar builds, and the complete Cargo graph. A formal
publishable Stable build is CI-owned and must explicitly provide
`UNFOUR_RELEASE_CHANNEL=stable` and the exact `UNFOUR_BUILD_COMMIT`. On
Windows, the configured release target produces an NSIS installer.

## Target artifacts

| Platform | Official distribution status | Format |
| --- | --- | --- |
| Windows x64 | Community Stable distribution | NSIS `.exe` |
| macOS arm64/x64 | Experimental / unverified until real-device smoke checks | Existing Tauri `.dmg` and archive outputs |
| Linux x64 | Experimental / unverified until real-device smoke checks | Existing Tauri `.AppImage`, `.deb`, and available package outputs |

Windows ships a single NSIS `.exe` installer. Before collecting release assets,
the workflow removes cached Windows bundle output and then selects `.exe`
artifacts only, preventing stale MSI files from being published.

The NSIS installer checks for a running `unfour-mcp.exe` during install and
uninstall. It prompts before stopping the sidecar and retries the process check
so an MCP host that respawns the process does not leave file replacement
stalled. This behavior still requires Windows installer smoke verification for
the release.

## Checksums

The final `checksum-release` job generates a single `SHA256SUMS.txt` using
`sha256sum` over the exact staged release assets. It uploads that file to the
same GitHub Release as the installers. Each line contains the SHA-256 followed
by the exact installer filename.

PowerShell can verify a downloaded Windows installer with:

```powershell
Get-FileHash -Algorithm SHA256 .\Unfour-*.exe
```

## Release caveats

- Installers are unsigned and may trigger SmartScreen or other operating-system
  warnings.
- macOS and Linux must remain labeled experimental/unverified until real-device
  launch and smoke checks are recorded; a successful CI bundle build is not
  platform verification.
- Real SSH, PostgreSQL, MySQL/MariaDB, and system Keychain/Secret Service checks
  are not represented as automated passes unless they were run against those
  real systems.

## Installer smoke

For each platform that is claimed as verified, use a clean or disposable test
profile to install, launch, render the first viewport, exercise the documented
module navigation, quit and relaunch, and uninstall. Record OS warnings,
signing status, and upgrade behavior in the release verification matrix.
