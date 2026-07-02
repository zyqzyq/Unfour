# Distribution

This document describes how public release artifacts should be built, named,
and verified.

## Build Command

From the repository root:

```bash
pnpm run tauri build
```

Run the full release verification matrix before publishing artifacts. A bundle
that builds successfully is not automatically release-ready.

## Target Artifacts

Expected Tauri artifact types depend on the target platform and local Tauri
configuration:

| Platform | Typical artifact types |
| --- | --- |
| Windows | `.msi`, `.exe` |
| macOS | `.dmg`, `.app` bundle, optional archive |
| Linux | `.AppImage`, `.deb`, optional `.rpm` depending on configuration |

Record the exact artifact names produced for the release candidate.

## Naming

Artifact names should make the target obvious:

```text
Unfour-<version>-windows-x64.<ext>
Unfour-<version>-macos-arm64.<ext>
Unfour-<version>-macos-x64.<ext>
Unfour-<version>-linux-x64.<ext>
```

If Tauri produces different default names, the release page should still label
the platform and architecture clearly.

## Checksums

Generate a checksum for every published artifact. SHA-256 is the default:

```bash
sha256sum <artifact>
```

On Windows PowerShell:

```powershell
Get-FileHash -Algorithm SHA256 <artifact>
```

Publish checksums with the release notes.

## Installer Smoke

For each release artifact:

- install on a clean or disposable test profile;
- launch the app;
- confirm the first viewport renders;
- switch between API Client, SSH Terminal, and Database;
- quit and relaunch;
- uninstall or remove the artifact;
- record warnings from the OS, especially for unsigned builds.

## Upgrade Smoke

When a previous build exists:

- install the previous build;
- create a disposable workspace with sample API, SSH, and Database metadata;
- install the release candidate over it;
- confirm the app launches and existing local data is still readable;
- confirm no secrets are exposed in SQLite or logs.

## Release Notes

Release notes should include:

- version and commit;
- supported platforms;
- artifact checksums;
- signing/notarization status;
- verification summary;
- known limitations;
- security reporting link;
- warning if live SSH, database, or platform smoke checks are not complete.

Do not describe unverified behavior as passing.
