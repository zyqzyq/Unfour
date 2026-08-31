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
behavior, and OS signing/notarization trust require their own evidence; the
real-environment evidence actually recorded for v0.9.0 is listed below.

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

## Recorded live and manual verification

The release operator supplied the following real-environment results. They are
recorded here without rerunning the completed manual journeys, and are not
inferred from automated tests or artifact generation.

| Area | Real behavior exercised | v0.9.0 result |
| --- | --- | --- |
| Windows Standard | NSIS install, installed launch, and uninstall | VERIFIED |
| Windows Stable updater | A real installation of the previous Stable release upgraded to the new Stable release | VERIFIED |
| Account and GitHub OAuth | Browser GitHub sign-in, Desktop GitHub login, `unfour://auth/callback`, and basic signed-in account state | VERIFIED |
| Creem Test environment | Checkout, webhook delivery, entitlement activation, and billing portal | VERIFIED |
| Database | PostgreSQL and MySQL against real database environments | VERIFIED |
| SSH | SSH Terminal, SFTP, and SSH Tasks against a real SSH environment | VERIFIED |
| macOS arm64 | Install and run on the target architecture | VERIFIED |
| macOS x64 | Install and run on the target architecture | VERIFIED |
| MCP real clients | Codex and Cursor each started Unfour MCP, completed `initialize`, `tools/list`, and `tools/call`, and accessed real Unfour data/tools | VERIFIED |

The real Codex and Cursor client journeys cover more than the standalone MCP
protocol smoke. The basic manual smoke remains useful as a future diagnostic or
regression procedure, but it is not a separate outstanding v0.9.0 verification
item.

## Linux AppImage compatibility

Linux remains Experimental: x86_64 (x64) only, AppImage only, with Ubuntu 22.04+
as the current runtime/test baseline. Ubuntu 20.04 is not supported. Other
distributions are not guaranteed compatible merely because they use glibc 2.35
or newer.

The release operator reported the v0.9.0 Ubuntu 20.04 startup failure and
confirmed that its formal Linux release build ran on `ubuntu-latest`, then
Ubuntu 24.04 / glibc 2.39. These runtime results are supplied evidence, not a
local rerun during the baseline fix.

| v0.9.0 Linux AppImage check | Result / evidence |
| --- | --- |
| Artifact build | PASS; the recorded Standard Release workflow produced and published the AppImage |
| Ubuntu 20.04 x64 runtime | FAIL; missing `GLIBC_2.32`, `GLIBC_2.33`, `GLIBC_2.34`, `GLIBC_2.35`, `GLIBC_2.38`, `GLIBC_2.39`, `GLIBCXX_3.4.29`, and `GLIBCXX_3.4.30` |
| Root cause | Binary/runtime dependencies were built against Ubuntu 24.04-era GLIBC/GLIBCXX; the release build baseline was too new, not a Rust business-logic, chmod, or FUSE defect |
| Ubuntu 22.04+ regression after the build-baseline fix | NOT VERIFIED until a new artifact is built on the pinned runner and tested |
| Linux desktop integration and updater | NOT VERIFIED |

The fix pins the shared Standard Linux `build` job to `ubuntu-22.04` and
isolates its Rust cache from older runner builds. `verify` may stay on
`ubuntu-latest` because it supplies no packaged native artifacts. This changes
future builds only: do not move the v0.9.0 tag, rebuild/overwrite its Release or
R2 files, or describe its Linux runtime as PASS. Existing VERIFIED Windows and
macOS results remain unchanged.

### Next-artifact Linux regression gates

Use a new candidate from the fixed workflow; record the commit, Actions run,
artifact filename/SHA-256, Ubuntu version, architecture, desktop session, and
startup logs with each result. Build success and static contracts alone do not
satisfy these runtime gates.

| Environment | Minimum real verification | Current result |
| --- | --- | --- |
| Ubuntu 22.04 x64 | `chmod +x` the AppImage; launch; first window renders; open API Client, SSH Terminal, and Database; quit and relaunch | NOT VERIFIED |
| Ubuntu 24.04 x64 | Launch smoke test with the same candidate AppImage | NOT VERIFIED |
| Linux Standard updater | Signed AppImage update from a runnable earlier installation, restart into the expected version, and record signature-rejection behavior separately | NOT VERIFIED |

The Linux `linux-x86_64` signed AppImage remains part of the Standard updater
contract. If no runnable previous build or safe update feed is available,
record that blocker instead of PASS; do not change stable metadata just to
exercise a candidate. Detailed steps are in
[Linux manual cases](manual-test-cases.md#linux-appimage-experimental).

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

The local machine does not have `gitleaks`; the repository audit therefore
uses the tracked deterministic scanner. A separate full-history scanner should
still be run before changing repository visibility if Git history from another
repository is ever imported. This merge copies reviewed source snapshots and
does not import Unfour-pro Git history.

## Remaining verification limits

These limits are scoped so that one unverified trust or policy variant does not
erase a completed install, updater, account, platform, or MCP client journey.

| Gate | Required behavior | Result |
| --- | --- | --- |
| Cloud Sync v0.9.0 unified client | Single-device behavior and a real multi-device push/pull, conflict, snapshot, and second-device regression | Historical live multi-device verification exists for an earlier version; the v0.9.0 unified-client regression remains NOT VERIFIED. Record single-device coverage with this regression rather than creating a separate large gate. |
| Creem Production | First real production checkout -> webhook -> active entitlement -> Desktop account refresh -> Cloud Sync entitlement -> billing portal | NOT VERIFIED until the first successful real production flow is recorded; Creem Test is VERIFIED, and this is not a failure or a request to repeat Test validation. |
| MCP production policy | Read-only operations in a prod workspace, blocked writes, `CONFIRMATION_REQUIRED`, `confirmation_text`/payload binding, and confirmed retry behavior | NOT VERIFIED in a real prod workspace |
| Windows trust prompt | SmartScreen/certificate trust behavior for the unsigned NSIS package | NOT VERIFIED; Windows install, launch, uninstall, and Stable upgrade remain VERIFIED |
| Standard updater rejection | Manual rejection of an invalid updater signature | NOT VERIFIED manually; artifact signatures and Store updater-policy tests do not establish this result. The real previous-Stable-to-new-Stable success path is VERIFIED. |
| macOS signing and notarization | Apple signing and notarization | NOT APPLICABLE because neither is enabled for v0.9.0; this is not a test failure |
| macOS Gatekeeper trust | Exact warning/trust behavior for the unsigned and unnotarized packages | NOT VERIFIED; arm64 and x64 install/run remain VERIFIED |
| Linux | Experimental x86_64 AppImage; Ubuntu 22.04+ baseline | v0.9.0 Ubuntu 20.04 launch FAIL (unsupported baseline); new-artifact Ubuntu 22.04/24.04 regression, desktop integration, and updater remain NOT VERIFIED; see Linux compatibility record above |
| Published Standard artifacts | tagged workflow publication and GitHub asset inventory | VERIFIED; installed behavior is not inferred |
| Microsoft Store / MSIX | Real MSIX install, callback, MCP alias, Store servicing, Partner Center acceptance, and coexistence behavior | Static contract/build-policy tests PASS; real package and Store journeys remain NOT VERIFIED |
| Migration | old Community DB, old Pro DB, clean DB | PASS (9 storage migration tests, including exact old-Pro data preservation) |

MariaDB remains a MySQL compatibility-path claim rather than an independent
verification matrix. Windows code signing is not claimed. Apple
signing/notarization is not enabled and therefore is not described as a failed
test.

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
the scoped manual Store/MSIX, updater-signature rejection, platform trust,
Linux, v0.9.0 Cloud Sync multi-device regression, Creem Production, and MCP
production-policy checks as `NOT VERIFIED` until evidence is recorded for the
applicable release and environment.
