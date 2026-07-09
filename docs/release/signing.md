# Signing

This document tracks release signing expectations. It does not claim signing is
configured or passing.

## Policy

Public artifacts should clearly state whether they are signed. If a release is
unsigned, release notes must tell users to expect operating-system warnings and
must provide checksums for manual verification.

## Windows

Expected production posture:

- code-sign `.msi` and `.exe` artifacts with an Authenticode certificate;
- verify signatures after build;
- record SmartScreen behavior during installer smoke.

Verification examples:

```powershell
Get-AuthenticodeSignature <artifact>
```

Unsigned Windows artifacts must be labeled as unsigned in the release notes.

## macOS

Expected production posture:

- sign the `.app` bundle and `.dmg`;
- notarize with Apple;
- staple notarization where appropriate;
- verify Gatekeeper behavior on a clean machine.

Verification examples:

```bash
codesign --verify --deep --strict <Unfour.app>
spctl --assess --type execute <Unfour.app>
```

Unnotarized macOS artifacts must be labeled clearly.

## Linux

Linux desktop artifacts often are not code-signed in the same way as Windows or
macOS. Release notes should still publish SHA-256 checksums and describe the
package format.

For `.deb` packages, record whether repository signing or package signing is in
scope for the release channel.

## Secrets

Signing credentials must not be committed to the repository. Store certificates,
private keys, notarization credentials, and tokens only in the appropriate
platform secret store or CI secret manager.

## Release Gate

For v0.1.0:

- record whether each platform artifact is signed;
- record verification command output or mark the item `NOT VERIFIED`;
- record OS trust prompts observed during installer smoke;
- publish checksums for every artifact.

## v0.1.0 Release Status

- Windows: NOT VERIFIED — unsigned; Authenticode signing is not configured. Artifacts
  must be labeled unsigned and shipped with SHA-256 checksums.
- macOS: NOT VERIFIED — not built or notarized in this environment.
- Linux: NOT VERIFIED — not built in this environment; publish SHA-256 checksums.

Signing/notarization is incomplete for v0.1.0. Release notes must tell users to expect
OS trust warnings and must publish checksums for manual verification.
