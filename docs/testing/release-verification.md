# v0.9.0 Final Release Verification Record

This document records the final status of the published `v0.9.0` release. It
distinguishes implemented behavior from real-environment verification and
formal release publication. A published build does not turn an unrecorded
manual, platform, signing, updater, or live-service check into `PASS`.

## Status terms

```text
Implemented
  = the feature exists in the product

Verified
  = the behavior was exercised against the stated real environment

Released
  = the feature is included in the formally published v0.9.0 release
```

## Final release status

- Status: `RELEASED`
- Exact `v0.9.0` tag commit SHA:
  `1dc7c1cc6430e546689fde5206599a31f36b17a1`
- Tag resolution: `VERIFIED` through the local Git tag and GitHub tag ref
- CI workflow on the tagged commit: `PASS`
- Standard Release Candidate workflow on the tagged commit: `PASS`
- Standard Release workflow on the tagged commit: `PASS`
- GitHub Release: `VERIFIED` as published on 2026-08-29, non-draft and
  non-prerelease
- Published GitHub asset inventory: `VERIFIED` (12 uploaded assets, including
  platform packages, updater signatures, `SHA256SUMS.txt`, and `latest.json`)
- Release-operator working-tree cleanliness: `NOT RECORDED`; this cannot be
  inferred from the immutable tag or successful workflows

Traceable publication evidence:

- [CI run 33183827600](https://github.com/zyqzyq/Unfour/actions/runs/33183827600)
- [Standard Release Candidate run 33233154945](https://github.com/zyqzyq/Unfour/actions/runs/33233154945)
- [Standard Release run 33233946389](https://github.com/zyqzyq/Unfour/actions/runs/33233946389)
- [Unfour v0.9.0 GitHub Release](https://github.com/zyqzyq/Unfour/releases/tag/v0.9.0)

## Verification layers

```text
CI
  = unit tests / Rust checks / release contracts

Release Candidate
  = the same full verification plus real signed cross-platform Standard
    Tauri bundles, uploaded only as GitHub Actions artifacts

Release
  = the same reusable build core, then immutable R2 files, byte verification,
    GitHub Release, update-order gate, and stable/latest.json promotion last

Microsoft Store
  = independent manual Windows x64 MSIX build, validation, and submission
```

An RC workflow success is build evidence, not publication evidence. For
`v0.9.0`, publication is separately evidenced by the successful Standard
Release workflow and the published GitHub Release. Installed behavior, updater
behavior, and OS signing/notarization trust still require their own evidence.

## Final feature status

| Capability | Implemented | Verified | Released | Final v0.9.0 record |
| --- | --- | --- | --- | --- |
| SQLite | Yes | Yes | Yes | Verified for v0.9.0 |
| PostgreSQL | Yes | Yes | Yes | Verified against a real PostgreSQL environment |
| MySQL | Yes | Yes | Yes | Verified against a real MySQL environment |
| MariaDB | Through the MySQL compatibility path | Not independently recorded | Compatibility path included | Do not claim a separate MariaDB verification matrix for v0.9.0 |
| SSH Terminal | Yes | Yes | Yes | Release-level verification completed against a real SSH server |

Compatible MariaDB servers use the MySQL driver path where protocol and SQL
behavior are compatible. That implementation and release status is not the
same as an independent MariaDB verification claim.

## Earlier automated evidence (historical)

The PASS values in this section are retained from the earlier `74c7270` run.
They are historical supporting evidence, not claims that these commands were
rerun locally while preparing this documentation update. The separate workflow
results above are the final tagged-commit automation record.

| Area | Command | Historical result at `74c7270` |
| --- | --- | --- |
| Patch hygiene | `git diff --check` | PASS (historical documentation diff) |
| Version identity | `node scripts/sync-version.mjs --check` | PASS (`0.9.0`) |
| Release/distribution/RC contracts | `pnpm run test:release-env` | PASS (41 tests; shared signed build core, zero-publication RC policy, Store policy, and Linux AppImage contract covered) |
| Historical migration integrity | `node scripts/check-migrations.mjs` | PASS (18 files) |
| MSIX PowerShell syntax | PowerShell parser over `scripts/msix/*.ps1` | PASS (4 files) |
| Publishable-tree secret audit | `node scripts/audit-public-secrets.mjs` | PASS (1023 publishable files scanned; no secret values found) |
| Large-file guard | `pnpm run check:large-files` | PASS (0 blocking violations; 5 grandfathered files) |
| Shared-token guard | `pnpm run check:tokens` | PASS (107 shared tokens; no host redefinitions) |
| Frontend production build | TypeScript plus Vite build | PASS (2383 modules) |
| Frontend lint | ESLint | PASS (0 errors, 89 existing warnings) |
| Frontend unit tests | Vitest | PASS (108 files, 541 tests) |
| Browser smoke | Playwright Chromium | PASS (2 tests) |
| Rust workspace check | `cargo check --workspace` | PASS |
| Rust SSH feature check | `cargo check -p unfour --features ssh-native` | PASS |
| Rust workspace tests | `cargo test --workspace` | PASS (715 passed, 0 failed; one OS keychain smoke intentionally ignored) |
| Desktop account/update tests | `cargo test --workspace` | PASS (17 desktop tests; Store updater rejection covered) |
| Windows/macOS/Linux RC build | `Standard Release Candidate` workflow on tagged commit | PASS |
| Windows Standard installed upgrade | install and update from the previous Stable release | NOT VERIFIED |
| Store MSIX build/validate/install | manual Windows candidate | NOT VERIFIED |

The local machine does not have `gitleaks`; the repository audit therefore
uses the tracked deterministic scanner. A separate full-history scanner should
still be run before changing repository visibility if Git history from another
repository is ever imported. This merge copies reviewed source snapshots and
does not import Unfour-pro Git history.

## Remaining verification limits

The completed PostgreSQL, MySQL, and SSH checks are recorded in the final
feature table above. The following unrelated items retain their actual status:

| Gate | Required behavior | Result |
| --- | --- | --- |
| Account | GitHub login, closed/running deep link, sign out | Automated account tests PASS; installed/live callback NOT VERIFIED |
| Cloud | entitlement, push/pull, conflicts, snapshots, second device | Automated domain tests PASS; live multi-device NOT VERIFIED |
| Windows Standard | unsigned NSIS install, launch, previous-version update, SmartScreen behavior | NOT VERIFIED as an installed v0.9.0 journey |
| macOS signing and notarization | Apple signing, notarization, and Gatekeeper trust | NOT APPLICABLE to the unsigned and unnotarized v0.9.0 packages; trust behavior remains NOT VERIFIED |
| Linux | x64 AppImage launch, desktop integration, and updater behavior | NOT VERIFIED; Linux remains Experimental |
| Standard updater | installed update and signature-rejection behavior | NOT VERIFIED |
| Published Standard artifacts | tagged workflow publication and GitHub asset inventory | VERIFIED; installed behavior is not inferred |
| Store | MSIX, validator, callback, MCP alias, no internal updater | Static contracts PASS; installed package NOT VERIFIED |
| Migration | old Community DB, old Pro DB, clean DB | PASS (9 storage migration tests, including exact old-Pro data preservation) |

Windows code signing, macOS signing/notarization, Microsoft Store submission,
multi-device Cloud Sync, Linux runtime behavior, and installed updater behavior
are deliberately not promoted to verified status by this release-status
cleanup.

## Commands for future release regression checks

```powershell
$env:CI = "true"
pnpm run check:version
pnpm run check:secrets
pnpm run check:migrations
pnpm run check:large-files
pnpm run check:tokens
pnpm run test:release-env
pnpm run build
pnpm run lint
pnpm run test
pnpm run check:rust
pnpm run check:rust:ssh
pnpm run test:rust
pnpm run test:e2e
```

Run these commands when a future change needs fresh regression evidence. Keep
manual Store, updater, platform trust, signing, and multi-device checks as
`NOT VERIFIED` until evidence is recorded for the applicable release and
environment.
