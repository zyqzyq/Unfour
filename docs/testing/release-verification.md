# Release Verification

This document records **v0.12.0 Standard stable**, released on **2026-10-23**,
compared with `v0.11.0`. Published, automated-verified and manually-verified
are separate states. Earlier release and RC/preparation records retain their
original source identities, dates and limits.

## v0.12.0 Final Release Verification Record

Evidence checked on **2026-10-23 (Asia/Shanghai)** through GitHub Release/tag,
workflow/job/artifact APIs and publish logs, plus direct public checksum and
manifest HTTP reads. This follow-up changes documentation only.

### Source and publication

- Status: **RELEASED** as Standard stable; version `0.12.0`, tag `v0.12.0`.
- Exact tag/CI/RC/Release commit:
  [`4883df8dbfaf4d9d69c94ed5bae75c627d146e34`](https://github.com/zyqzyq/Unfour/commit/4883df8dbfaf4d9d69c94ed5bae75c627d146e34).
  Local `main`/HEAD, local tag, GitHub tag and GitHub `main` agreed at entry;
  the local working tree was clean. This follow-up uses the existing `main` branch.
- [GitHub Release](https://github.com/zyqzyq/Unfour/releases/tag/v0.12.0):
  `Unfour v0.12.0`, Release ID `421419257`, `draft=false`, `prerelease=false`,
  published `2026-10-23T03:25:54Z` (11:25:54 Asia/Shanghai).
  GitHub's latest-release API also returned `v0.12.0`.
- [Version comparison](https://github.com/zyqzyq/Unfour/compare/v0.11.0...v0.12.0)
  and [Release Notes](../release/v0.12.0-release-notes.md) describe the tagged scope.
  The remote Release body remains the workflow's generic Standard distribution
  text; the detailed local notes do not imply an edit to that remote body.
- Release-operator working-tree cleanliness and manual RC sign-off at
  publication: **NOT RECORDED / NOT VERIFIED**. The later clean tree and
  successful workflows do not establish either earlier manual state.

### Workflow and build results

All three runs completed with `conclusion=success`, attempt **1**, on the
exact SHA above. Jobs and applicable execution steps were inspected.

| Workflow | Run / trigger | Result |
| --- | --- | --- |
| CI #320 | [39885807832](https://github.com/zyqzyq/Unfour/actions/runs/39885807832); `push`, `main` | PASS; 7/7 jobs: frontend lint/build/tests, Rust check/tests, release contracts, MCP contracts/tests |
| Standard Release Candidate #20 | [39885870422](https://github.com/zyqzyq/Unfour/actions/runs/39885870422); `workflow_dispatch`, `main` | PASS (automated); identity, reusable verify, four signed updater builds and four Actions artifacts; no publish job |
| Standard Release #33 | [39892644501](https://github.com/zyqzyq/Unfour/actions/runs/39892644501); `push`, `v0.12.0` | PASS; identity, reusable verify, four signed updater builds and publication |

Both RC and Release verify jobs passed `pnpm run lint`, `pnpm run test`,
`pnpm run test:release-env`, `pnpm run check`, `pnpm run check:rust:ssh`,
`pnpm run test:rust` and `pnpm run test:e2e`. These are hosted results,
not application suites rerun for this documentation task. Ordinary CI uses Test;
the reusable Standard verify/build uses Stable. Browser smoke does not verify
installed native behavior.

The actual RC and Release Linux build logs report `Image: ubuntu-22.04`;
the separate reusable verify job is configured on `ubuntu-latest`.
OS code signing and macOS notarization remain disabled; updater signatures
do not establish operating-system trust.

The Actions API listed four artifacts per run, all `expired=false` at this
check. Prefixes are `release-candidate-` and `release-assets-`, respectively.

| Target suffix | RC artifact ID | Release artifact ID / build job |
| --- | --- | --- |
| `x86_64-pc-windows-msvc` | `12473274349` | `12476206489` / [120219390466](https://github.com/zyqzyq/Unfour/actions/runs/39892644501/job/120219390466) |
| `aarch64-apple-darwin` | `12473686509` | `12475973401` / [120219390568](https://github.com/zyqzyq/Unfour/actions/runs/39892644501/job/120219390568) |
| `x86_64-apple-darwin` | `12473289611` | `12475943673` / [120219390630](https://github.com/zyqzyq/Unfour/actions/runs/39892644501/job/120219390630) |
| `x86_64-unknown-linux-gnu` | `12473736515` | `12476565796` / [120219390490](https://github.com/zyqzyq/Unfour/actions/runs/39892644501/job/120219390490) |

RC and formal artifacts are separate builds of the same source. Their ZIP
digests differ; no RC-to-Release byte-equality or manual artifact acceptance
is claimed. GitHub and R2 receive the same formal build's staged files.

### Published assets, checksums, and Stable manifests

The [Release API](https://api.github.com/repos/zyqzyq/Unfour/releases/tags/v0.12.0)
lists **13 assets**, all `state=uploaded`: six payloads, four updater signatures,
checksums and two manifests. The six payloads are Windows x64 NSIS, macOS
arm64/x64 DMG installers, both macOS updater `.app.tar.gz` archives, and Linux
x64 AppImage. No MSIX, `.deb`, `.rpm` or Linux ARM64 asset is in this inventory.

| Asset | Bytes | SHA-256 (Release API) |
| --- | --- | --- |
| [`downloads.json`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/downloads.json) | 504 | `d8021883e95bae81b86f5f7b0416829f6957b48db5b8e3b0d294efe4380f40cf` |
| [`latest.json`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/latest.json) | 2,362 | `52e8c6af8b9f20cf541b5d83d46634c286bb1f3facf2c363f92336782efde053` |
| [`SHA256SUMS.txt`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/SHA256SUMS.txt) | 1,004 | `94c89e15dba2a923ae01af6eccb8888647a27228527ac5391724a0174a5640c6` |
| [`Unfour_0.12.0_linux_x64.AppImage`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/Unfour_0.12.0_linux_x64.AppImage) | 116,357,624 | `bcb09776db7f86e2ba1eb3b19cf392daad38fc4640825b789520b7b9fb7fb813` |
| [`Unfour_0.12.0_linux_x64.AppImage.sig`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/Unfour_0.12.0_linux_x64.AppImage.sig) | 420 | `94f7134ebc687cc11376cfaf616aac2743d5ce17a1a143f3f1fca114d639cd66` |
| [`Unfour_0.12.0_macos_arm64.app.tar.gz`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/Unfour_0.12.0_macos_arm64.app.tar.gz) | 38,876,471 | `ed712ccfa30fecd45ffe4ba94b4736d622ac8f0bc58de35b3c37c35765321aaf` |
| [`Unfour_0.12.0_macos_arm64.app.tar.gz.sig`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/Unfour_0.12.0_macos_arm64.app.tar.gz.sig) | 404 | `6ecf1a73b37c8e2506d14140e635cf5ba4dba9d84b0dc493ee0e81b1dd98222c` |
| [`Unfour_0.12.0_macos_arm64.dmg`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/Unfour_0.12.0_macos_arm64.dmg) | 39,398,486 | `ab9dd9d5f498e8c1aae50a953c5fb8b3225d0bfc7000a666ec2a96d9f2cc9f12` |
| [`Unfour_0.12.0_macos_x64.app.tar.gz`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/Unfour_0.12.0_macos_x64.app.tar.gz) | 41,580,901 | `4a1bdc02510c95e25762c67aa59d762c2d774272bfd133c1b1e8cdb85d136c81` |
| [`Unfour_0.12.0_macos_x64.app.tar.gz.sig`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/Unfour_0.12.0_macos_x64.app.tar.gz.sig) | 404 | `56fcd1849f350c197875ea185710d1593778418cb9c9d2f445858be9ad3d52ae` |
| [`Unfour_0.12.0_macos_x64.dmg`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/Unfour_0.12.0_macos_x64.dmg) | 42,428,910 | `b02cca99255470bddad2f9af42b4573dcce4dc8355d6a58b93d72302ae810224` |
| [`Unfour_0.12.0_windows_x64.exe`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/Unfour_0.12.0_windows_x64.exe) | 25,478,591 | `da1da0f54359e920366acc943a44973e9ff8645bc282a11e745efdd7ac81fab5` |
| [`Unfour_0.12.0_windows_x64.exe.sig`](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/Unfour_0.12.0_windows_x64.exe.sig) | 416 | `2f5d5a32f151dbd9d6c3b88101f85121d1e19217b3f0fe82381f4b59e4d871b3` |

The [publish job 120222954228](https://github.com/zyqzyq/Unfour/actions/runs/39892644501/job/120222954228)
passed finalization, immutable R2 upload/re-download, GitHub publication and
ordered Stable promotion. Its log reports all ten `sha256sum -c` entries
as `OK` at `03:25:40–03:25:41Z`. The update-order check allowed `0.12.0`
over `0.11.0`, then uploaded `stable/downloads.json` at `03:25:57Z` and
`stable/latest.json` last at `03:25:58Z`.

Direct public reads completed at **2026-10-23T11:18:46Z**
(**19:18:46 Asia/Shanghai**); all six requests returned HTTP 200:

- [Versioned R2 checksums](https://releases.unfour.dev/stable/0.12.0/SHA256SUMS.txt)
  and [GitHub checksums](https://github.com/zyqzyq/Unfour/releases/download/v0.12.0/SHA256SUMS.txt)
  are byte-identical: 1,004 bytes, SHA-256
  `94c89e15dba2a923ae01af6eccb8888647a27228527ac5391724a0174a5640c6`.
  Exactly ten payload/signature entries match all ten Release API digests;
  neither manifest nor the checksum file itself is included.
- [Stable downloads](https://releases.unfour.dev/stable/downloads.json):
  `version=0.12.0`, exactly `windows-x64`, `macos-arm64`, `macos-x64`,
  `linux-x64`, pointing to NSIS, DMG, DMG and AppImage.
- [Stable updater](https://releases.unfour.dev/stable/latest.json):
  `version=0.12.0`, exactly `windows-x86_64`, `darwin-aarch64`,
  `darwin-x86_64`, `linux-x86_64`, each with a non-empty signature.
  macOS updater URLs use `.app.tar.gz`. Its `pub_date` is
  `2026-10-23T03:25:19.475Z`, the metadata-generation time before GitHub publication.
- Both online manifests are byte-identical to their respective GitHub assets
  and match the sizes/digests in the table. Both R2 responses have
  `Content-Type: application/json` and `Cache-Control: no-cache`.
  All eight payload URLs use canonical filenames under `stable/0.12.0/`.

Stable pointers are mutable; these values describe the dated observation.
Full R2 payload verification relies on the workflow's re-download/checksum log.
No installer or Actions ZIP was downloaded, installed or rebuilt locally in
this follow-up. Digest agreement and non-empty signatures do not establish
an in-app update or manual invalid-signature-rejection result.

### Manual limits and preserved preparation evidence

| Gate | v0.12.0 release-level result |
| --- | --- |
| Windows/macOS/Linux install, launch, upgrade/uninstall, MCP sidecar replacement and OS trust; Linux 22.04/24.04 runtime | NOT VERIFIED; Linux remains Experimental; Windows/macOS OS signing and macOS notarization are not enabled |
| v0.11.0-to-v0.12.0 in-app update and manual invalid-signature rejection | NOT VERIFIED |
| Released-artifact Workspace exchange/backup, Auth/redaction, API/Database/MCP/Flow regressions and SSH Task reruns on real hosts | NOT VERIFIED; historical implementation and automated evidence retain their scope |
| General Flow Sync manual validation | USER CONFIRMED on 2026-10-22; no scenario/build/service/device details supplied |
| Detailed Flow Sync server contract, independent devices, offline/retry/conflict/deletion, history isolation, native view refresh and old-client upgrades | NOT VERIFIED where scenario evidence is absent; publication does not fill the FS/UP/RG rows |
| Account/OAuth/billing, live API/SSH/database journeys and MCP production-policy clients | NOT VERIFIED |
| Manual RC sign-off, macOS/Linux native credential stores and separate MSIX/Store build/install/servicing | NOT VERIFIED |

The [original RC checklist](v0.12.0-rc-checklist.md) and
[dated source preparation](v0.12.0-release-preparation-2026-10-22.md) are preserved
unchanged. Their historical feature audit at `9f02b16`, version preparation and
CI #319 at `8f5d9db`, unselected final candidate and
`NOT RELEASED / RC NOT STARTED` statements describe the earlier checkpoint.
The later SHA, CI #320, RC #20 and Release #33 above supersede only that
publication/workflow status. They do not rewrite prior test results or turn
unrecorded manual rows into `PASS`. Use those scenario steps against the exact
released artifacts when manual verification is performed.

### Documentation follow-up validation

- `node scripts/sync-version.mjs --check`: **PASS**; all tracked consumers
  remain `0.12.0`. `node scripts/release-contract.mjs v0.12.0`: **PASS**.
- Local Markdown paths/anchors in all 13 changed documents: **PASS**;
  168 local links and 41 anchors checked. Current-state references use v0.12.0;
  old release and preparation statements retain explicit historical scope.
- Published inventory comparison: **PASS**; all 13 documented asset names,
  URLs, sizes and digests, three workflow identities and eight Actions artifact
  IDs match the inspected GitHub API evidence.
- `node scripts/audit-public-secrets.mjs`: **PASS**; 1,526 publishable files,
  no findings. `git diff --check`: **PASS**.
- Original v0.12.0 RC/preparation files, older changelog entries and the entire
  v0.11.0-and-earlier verification body are unchanged (Git checkout line endings
  normalized for content comparison). Only Markdown documents are modified.
- Application build/tests and native/manual journeys: **NOT RUN** during this
  prose-only follow-up; hosted results above remain attributed to their runs.
  No version, dependency, workflow, business code, tag or remote publication is
  changed; no branch, push, dispatch, deployment or package rebuild is performed.
  A local documentation commit is subsequently authorized by the user.

Release documentation is closed out; the next code-quality phase can begin
without assigning a new version here. The manual checks above remain explicit
follow-up work and are not represented as completed by this documentation task.

## Historical v0.11.0 release record

This document records the published v0.11.0 release against `v0.10.0`
(`96f0022ec344fd643b567a54fe6f3686c2272dc0`). Earlier release and RC
preparation records retain their original scope and results below or in their
linked reports; they do not establish new native/manual coverage.

## v0.11.0 Final Release Verification Record

Evidence checked on 2026-10-10 through GitHub tag, Release, workflow, job,
artifact APIs and publish logs, plus direct public checksum/manifest HTTP reads.
This follow-up archives existing results and changes documentation only.

### Source and publication

- Status: **RELEASED** as Standard stable; version `0.11.0`, tag `v0.11.0`.
- Exact tag/release commit:
  [`311910dedaaa849ee9130e9d03a1301714b02726`](https://github.com/zyqzyq/Unfour/commit/311910dedaaa849ee9130e9d03a1301714b02726).
  Local `main`/HEAD, local tag, GitHub tag and GitHub `main` agreed at the start
  of this follow-up. The initial local working tree was clean.
- [GitHub Release](https://github.com/zyqzyq/Unfour/releases/tag/v0.11.0):
  `Unfour v0.11.0`, Release ID `408823362`, `draft=false`, `prerelease=false`,
  published `2026-10-10T08:57:14Z` (2026-10-10 16:57:14 Asia/Shanghai).
  Its actual body is the workflow's generic Standard distribution text; the
  [detailed local Release Notes](../release/v0.11.0-release-notes.md) archive
  user-facing changes without claiming a remote body edit.
- Release-operator tree review/cleanliness at publication: `NOT RECORDED`.
  The later clean tree and successful workflows do not establish that state.

### Workflow and build results

All three runs resolved to the exact SHA above, completed with
`conclusion=success`, and used attempt 1:

| Workflow | Run / trigger | Result |
| --- | --- | --- |
| CI | [38035203118](https://github.com/zyqzyq/Unfour/actions/runs/38035203118); `push`, `main` | PASS; all seven jobs: frontend lint/build/tests, Rust check/tests, release contracts, MCP contracts/tests |
| Standard Release Candidate | [38036217463](https://github.com/zyqzyq/Unfour/actions/runs/38036217463); `workflow_dispatch`, `main` | PASS; identity, reusable verify, four signed native builds; four Actions artifacts, no publish job |
| Standard Release | [38038525070](https://github.com/zyqzyq/Unfour/actions/runs/38038525070); `push`, `v0.11.0` | PASS; identity, reusable verify, four signed native builds, publish |

Both RC and formal verify jobs passed `pnpm run lint`, `pnpm run test`,
`pnpm run test:release-env`, `pnpm run check`, `pnpm run check:rust:ssh`,
`pnpm run test:rust`, and `pnpm run test:e2e`. These are hosted workflow
results, not application suites rerun for this documentation follow-up.
The Linux build jobs used `ubuntu-22.04`; the independent verify jobs used
`ubuntu-latest`. Browser mocks/builds do not verify installed native behavior.

The Actions API listed these artifacts with `expired=false`. RC and formal
artifacts are separate builds of the same source; no RC/Release byte-equality
claim is made.

| Target suffix | RC artifact ID | Release artifact ID / build job |
| --- | --- | --- |
| `x86_64-pc-windows-msvc` | `11663579938` | `11664992824` / [114175104007](https://github.com/zyqzyq/Unfour/actions/runs/38038525070/job/114175104007) |
| `aarch64-apple-darwin` | `11664329333` | `11664808397` / [114175104066](https://github.com/zyqzyq/Unfour/actions/runs/38038525070/job/114175104066) |
| `x86_64-apple-darwin` | `11664569226` | `11664833480` / [114175104044](https://github.com/zyqzyq/Unfour/actions/runs/38038525070/job/114175104044) |
| `x86_64-unknown-linux-gnu` | `11663574922` | `11664918338` / [114175104075](https://github.com/zyqzyq/Unfour/actions/runs/38038525070/job/114175104075) |

### Published assets, checksums, and Stable manifests

GitHub lists 13 assets, all `state=uploaded`: six payloads, four updater
signatures, checksums and two manifests. The API's ten payload/signature
SHA-256 digests agree with the downloaded checksum entries. No MSIX, `.deb`,
`.rpm`, or Linux ARM64 asset is in this release inventory.

| Asset | Bytes | SHA-256 (Release API) |
| --- | --- | --- |
| `downloads.json` | 504 | `31dc60cd38b683abd2999e31206934cba2fefcf7342151d1cfe89fecbe86e5e2` |
| `latest.json` | 2,362 | `491969ed74b2da9a1be2088c2d36deeb990077f4e058b678d326eded35a01fe5` |
| `SHA256SUMS.txt` | 1,004 | `decd1b918426bbd90dc227d4c172ab07d88c5f63df41d928d92358d935dee494` |
| `Unfour_0.11.0_linux_x64.AppImage` | 115,350,008 | `c2e505a7a1d68068ca682abee46a1826ec9a7f24cafaff079ae494ca0d33e40b` |
| `Unfour_0.11.0_linux_x64.AppImage.sig` | 420 | `f8a848e38f7af8bd554a0ec1220bd6017bc2a69f55b47a633c03927c690639a8` |
| `Unfour_0.11.0_macos_arm64.app.tar.gz` | 37,827,355 | `db2a453e8e1916702ca28db196bd50e4651cd04a735b9c6f01e357f0c53a0567` |
| `Unfour_0.11.0_macos_arm64.app.tar.gz.sig` | 404 | `13869dd98b50f0ecb6a6af97ba044c9683b9da6b594dcacfbc0b12e0b721eb6e` |
| `Unfour_0.11.0_macos_arm64.dmg` | 38,375,159 | `455d0a654f350308eabfbb0e22948e970f401c9191fdae5f20b16edfe456e1ea` |
| `Unfour_0.11.0_macos_x64.app.tar.gz` | 40,381,001 | `b52fd5f88d163cc061b9257105ea83f00c617f56132697959d403df23e7594c6` |
| `Unfour_0.11.0_macos_x64.app.tar.gz.sig` | 404 | `0c9e26ec3f548bed36d9f68c9161459372d41cb7b6b8e6aef9f482f0e9d0613c` |
| `Unfour_0.11.0_macos_x64.dmg` | 41,258,489 | `c5cc5eebd63bc35344463a5cec68c6c3b2355d5c671b7ff1f1f9b6154b6e75ac` |
| `Unfour_0.11.0_windows_x64.exe` | 24,801,880 | `dba2e7f72f14e4c6f06a80ea7f20c9d181e2db6454058e1f5fd44985184bb267` |
| `Unfour_0.11.0_windows_x64.exe.sig` | 416 | `14344faf3018425a6e305c9560ab4415f4bed9b1dbdecbdd5c8d40235cbd4476` |

The [publish job 114177060956](https://github.com/zyqzyq/Unfour/actions/runs/38038525070/job/114177060956)
passed finalization, immutable R2 upload/re-download, GitHub publication, and
ordered Stable promotion. Its log reports all ten `sha256sum -c` entries as
`OK`, then uploads `downloads.json` before `latest.json`.

Direct public reads returned HTTP 200:

- [Versioned R2 checksums](https://releases.unfour.dev/stable/0.11.0/SHA256SUMS.txt)
  are byte-identical to the [GitHub checksum asset](https://github.com/zyqzyq/Unfour/releases/download/v0.11.0/SHA256SUMS.txt).
  Exactly ten entries exclude the checksum file itself and both manifests.
- [Stable downloads](https://releases.unfour.dev/stable/downloads.json):
  `version=0.11.0`; exactly `windows-x64`, `macos-arm64`, `macos-x64`,
  `linux-x64`, pointing to NSIS, DMG, DMG, AppImage respectively.
- [Stable updater](https://releases.unfour.dev/stable/latest.json):
  `version=0.11.0`; exactly `windows-x86_64`, `darwin-aarch64`,
  `darwin-x86_64`, `linux-x86_64`, each with a non-empty signature.
  macOS updater URLs use `.app.tar.gz`.
- Both manifests are byte-identical to their GitHub assets, with
  `Content-Type: application/json` and `Cache-Control: no-cache`. All eight
  payload URLs reference canonical assets under `stable/0.11.0/`.

Full R2 payload checks rely on the workflow's re-download/checksum log; no
installers were downloaded or installed locally in this follow-up. API digests
and signature presence do not establish manual update/signature-rejection PASS.

### Manual limits and historical RC evidence

| Gate | v0.11.0 release-level result |
| --- | --- |
| Windows/macOS/Linux native install, launch, upgrade/uninstall, MCP sidecar replacement, OS trust and Linux 22.04/24.04 runtime/updater | NOT VERIFIED; Linux remains Experimental; Windows/macOS OS signing and macOS notarization are not enabled |
| v0.10.0-to-v0.11.0 in-app update and manual invalid-signature rejection | NOT VERIFIED |
| Released-artifact Workspace exchange/encrypted backup, credential lifecycle, Auth/OpenAPI/redaction, Database/MCP/Flow and live API/SSH/database/client regressions | NOT VERIFIED; scoped automated/implementation evidence is retained |
| Account/OAuth/billing, Cloud Sync single/multi-device and MCP prod-policy real journeys | NOT VERIFIED |
| macOS/Linux native credential stores and manual MSIX/Store build/install/servicing | NOT VERIFIED; Store remains a separate process |

The [2026-10-10 source preparation](v0.11.0-release-preparation-2026-10-10.md),
[2026-10-09 RC readiness](v0.11.0-rc-readiness-2026-10-09.md), and
[Auth/export follow-up](v0.11.0-rc-openapi-auth-history-2026-10-10.md)
are preserved unchanged. Their preparation-time outstanding RC/publication
gates describe historical checkpoints. The preparation's real Windows
credential-store smoke remains scoped local evidence, not an installed-artifact
or cross-platform result. Final source also includes the
[Workspace MCP permission fixes](workspace-mcp-permissions.md).
Use the [manual acceptance steps](v0.11.0-release-preparation-2026-10-10.md#manual-rc-acceptance)
against the exact released artifacts for future verification. Formal publication,
automated verification, and manual verification remain separate states.

### Documentation follow-up validation

- `node scripts/sync-version.mjs --check`: PASS; all tracked consumers remain
  `0.11.0`. `node scripts/release-contract.mjs v0.11.0`: PASS.
- Local Markdown paths/anchors: PASS; 58 local links and 19 anchors in the
  seven changed documents. Documented sizes/digests match all 13 Release assets.
- `node scripts/audit-public-secrets.mjs`: PASS; 1,380 publishable files, no findings.
- `git diff --check`: PASS. Older changelog/release results and all three v0.11.0
  RC/preparation records are preserved. No business source, versions, AGENTS,
  tags, workflows, or published files are changed; no push or dispatch is performed.

## Historical v0.10.0 release record

This document records the published v0.10.0 release, compared with
`v0.9.6` (`87bea5d6f337cc6d1d10055972bce2f3ee24aaee`). The local v0.10.0
preparation checks, v0.9.6 preparation checks, and published v0.9.0 results
remain below as historical evidence. They do not establish new manual coverage.

## v0.10.0 Final Release Verification Record

Evidence checked on 2026-09-30 through GitHub tag, workflow, job, artifact, and
Release APIs, plus public checksum and manifest HTTP reads.

### Status distinctions

| Term | Meaning | v0.10.0 record |
| --- | --- | --- |
| Implemented | Behavior exists in the tagged product source | Flow V1/MCP, Collection/Environment exchange, table export, openGauss, and SQL/Saved SQL context changes are included; scoped implementation reports are linked below |
| Automated verified | The stated automated check completed successfully on the recorded SHA or published files | CI, RC/Release verification and four-platform builds, publication, R2 checksum verification, and manifest reads: PASS |
| Manually verified | Installed behavior or a real-service journey was actually exercised in the stated environment | Release-level native/manual/live-service regressions without fresh evidence: NOT VERIFIED; earlier scoped and v0.9.x records retain their original limits |
| Released | The tagged build is formally published | RELEASED as Standard stable; this status does not imply manual verification |

### Source and workflow evidence

- Status: `RELEASED`; version `0.10.0`, tag `v0.10.0`.
- Exact tag/release commit:
  [`96f0022ec344fd643b567a54fe6f3686c2272dc0`](https://github.com/zyqzyq/Unfour/commit/96f0022ec344fd643b567a54fe6f3686c2272dc0).
  The GitHub tag ref and local tag agree. Local and GitHub `main` resolved
  to the same commit at the start of this documentation follow-up.
- [GitHub Release](https://github.com/zyqzyq/Unfour/releases/tag/v0.10.0):
  `Unfour v0.10.0`, Release ID `399805354`, `draft=false`,
  `prerelease=false`, published `2026-09-30T07:24:24Z`
  (2026-09-30 15:24:24 Asia/Shanghai).
- Release-operator working-tree cleanliness at publication: `NOT RECORDED`;
  the clean tree observed during this documentation follow-up does not
  establish the operator's earlier state.

All three runs below resolved to the exact tag commit, completed with
`conclusion=success`, and used attempt 1:

| Workflow | Run | Trigger | Automated result |
| --- | --- | --- | --- |
| CI | [36676317345](https://github.com/zyqzyq/Unfour/actions/runs/36676317345) | `push`, `main` | PASS: frontend lint/build/tests, release contracts, secret audit, migration integrity, Rust checks/tests, MCP contracts/tests |
| Standard Release Candidate | [36676343771](https://github.com/zyqzyq/Unfour/actions/runs/36676343771) | `workflow_dispatch`, `main` | PASS: source identity, reusable verification, and exactly four signed Standard native builds; four Actions artifacts, no publish job |
| Standard Release | [36681185573](https://github.com/zyqzyq/Unfour/actions/runs/36681185573) | `push`, `v0.10.0` | PASS: source identity, reusable verification, four signed Standard builds, and publication |

The RC and Release verify jobs both passed `pnpm run lint`, `pnpm run test`,
`pnpm run test:release-env`, `pnpm run check`, `pnpm run check:rust:ssh`,
`pnpm run test:rust`, and `pnpm run test:e2e`. These are workflow results,
not commands rerun during this documentation-only follow-up. Both Linux build
jobs used `ubuntu-22.04`; runner/build success does not verify installed
Ubuntu 22.04/24.04 runtime behavior.

### Actions artifacts

The API lists the following artifacts, all `expired=false` at the evidence
check. RC artifacts use `release-candidate-`; formal build artifacts use
`release-assets-`. The formal workflow rebuilt the same source SHA; RC and
Release artifact ZIPs are separate builds, without a byte-equality claim.

| Target suffix | RC artifact ID | Release artifact ID |
| --- | --- | --- |
| `x86_64-pc-windows-msvc` | `11080319728` | `11082627057` |
| `aarch64-apple-darwin` | `11080454267` | `11081764712` |
| `x86_64-apple-darwin` | `11080519299` | `11082144589` |
| `x86_64-unknown-linux-gnu` | `11079648849` | `11082487720` |

### Published assets, checksums, and manifests

GitHub lists 13 assets, all `state=uploaded`: six installer/updater payloads,
four updater `.sig` files, `SHA256SUMS.txt`, `latest.json`, and
`downloads.json`. Sizes and SHA-256 digests below come from the Release API.
The ten payload/signature digests agree with the downloaded checksum entries.
No MSIX, `.deb`, `.rpm`, or Linux ARM64 asset was published by this workflow.

| Published asset | Bytes | SHA-256 |
| --- | --- | --- |
| [`downloads.json`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/downloads.json) | 504 | `576b889ae527d7660d76ba43afff4ca6ffce46e615309e492219cee5c3ea12e1` |
| [`latest.json`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/latest.json) | 2,362 | `4ee56c43aee534f026dfce5a3cb89b4aacf443c64ceb77bf2b7c7a1efde9edca` |
| [`SHA256SUMS.txt`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/SHA256SUMS.txt) | 1,004 | `e8e0162e617ab3949c6b07d1b48b27bae190fc9c7a84c2ebf73c2e9ea139874c` |
| [`Unfour_0.10.0_linux_x64.AppImage`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/Unfour_0.10.0_linux_x64.AppImage) | 111,036,920 | `b1bc430aecc6671cad217f14c6cb4a52c532c84945916979608995c211acb8d1` |
| [`Unfour_0.10.0_linux_x64.AppImage.sig`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/Unfour_0.10.0_linux_x64.AppImage.sig) | 420 | `651f7f3394465791c7d37250863bf77e46d340e3115cf1f5d785afa350b63f45` |
| [`Unfour_0.10.0_macos_arm64.app.tar.gz`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/Unfour_0.10.0_macos_arm64.app.tar.gz) | 33,488,159 | `1fa1fd8df2c31c58823978f0816f3d36d9965d86d8082cde65977cfa6e69ed71` |
| [`Unfour_0.10.0_macos_arm64.app.tar.gz.sig`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/Unfour_0.10.0_macos_arm64.app.tar.gz.sig) | 404 | `54ae1f23030e27b690f1ae22a53286a627abe4982a9dc829a1441822b44a7005` |
| [`Unfour_0.10.0_macos_arm64.dmg`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/Unfour_0.10.0_macos_arm64.dmg) | 34,062,338 | `96762b945ac98939cec0766f170fa9607e5a7a4f06c113634ff8f346886d3aa8` |
| [`Unfour_0.10.0_macos_x64.app.tar.gz`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/Unfour_0.10.0_macos_x64.app.tar.gz) | 36,050,897 | `06b50c440ee1e6b6e80a7de87f0efd8719c0a4cb8d05a6685540eb691bf8272e` |
| [`Unfour_0.10.0_macos_x64.app.tar.gz.sig`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/Unfour_0.10.0_macos_x64.app.tar.gz.sig) | 404 | `3d856521e7dbe604196fea68f98573759a25ce4f2594464f1dba38081749ac71` |
| [`Unfour_0.10.0_macos_x64.dmg`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/Unfour_0.10.0_macos_x64.dmg) | 36,956,905 | `fb084c7ab334d1fc7756359298e83f94c34d2861f4d7b17ee7498f0d64c5a441` |
| [`Unfour_0.10.0_windows_x64.exe`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/Unfour_0.10.0_windows_x64.exe) | 20,919,321 | `ba6ecc94844f1c69d8addeac42264d3098b9c13556ddda7fe3fb7c13e01ca3f8` |
| [`Unfour_0.10.0_windows_x64.exe.sig`](https://github.com/zyqzyq/Unfour/releases/download/v0.10.0/Unfour_0.10.0_windows_x64.exe.sig) | 416 | `7cba841f6c9cfdf0bcb0ca5e715a4b0f40e762c4ff2b541dbdef1336e34ccd50` |

The [publish job 109783752704](https://github.com/zyqzyq/Unfour/actions/runs/36681185573/job/109783752704)
passed manifest finalization, immutable R2 upload/re-download, GitHub Release
creation, and ordered stable-manifest promotion. Its log records all ten
`sha256sum -c` entries as `OK`, allows promotion from updater `0.9.6` to
`0.10.0`, uploads `downloads.json` first, then `latest.json` last.

Direct public reads during this follow-up returned HTTP 200:

- [R2 versioned checksums](https://releases.unfour.dev/stable/0.10.0/SHA256SUMS.txt)
  and the GitHub checksum asset are byte-identical. The checksum file contains
  exactly ten payload/signature entries, excluding itself and both manifests.
- [Stable updater manifest](https://releases.unfour.dev/stable/latest.json):
  version `0.10.0`; exactly `windows-x86_64`, `darwin-aarch64`,
  `darwin-x86_64`, and `linux-x86_64`, each with a non-empty signature.
  macOS updater URLs use `.app.tar.gz`.
- [Stable download manifest](https://releases.unfour.dev/stable/downloads.json):
  version `0.10.0`; exactly `windows-x64`, `macos-arm64`, `macos-x64`,
  and `linux-x64`, using NSIS, DMG, DMG, and AppImage respectively.
- Both live manifests are byte-identical to their GitHub assets and return
  `Content-Type: application/json` and `Cache-Control: no-cache`. All eight
  payload URLs point into `https://releases.unfour.dev/stable/0.10.0/`.

Full payload byte verification is evidenced by the workflow's R2 re-download
and checksum log, not a new local download/install of all packages. Manifest
signature presence does not establish cryptographic rejection or a successful
in-app update journey.

### Remaining v0.10.0 manual and live-service limits

| Gate | Release-level result |
| --- | --- |
| Windows NSIS install, installed launch, upgrade, MCP sidecar replacement, uninstall, and SmartScreen/trust behavior | NOT VERIFIED |
| macOS arm64/x64 install/run and Gatekeeper/trust behavior | NOT VERIFIED; Apple signing/notarization is not enabled |
| Linux AppImage Ubuntu 22.04/24.04 launch, rendering, module opening, quit/relaunch, desktop integration, and updater | NOT VERIFIED; Linux remains Experimental |
| Previous-Stable-to-v0.10.0 in-app update and manual invalid-signature rejection | NOT VERIFIED |
| Native Desktop/MCP Flow lifecycle, API exchange, table export, openGauss, and SQL/Saved SQL context regression on released artifacts | NOT VERIFIED; scoped implementation records retain their narrower evidence |
| Real API, SSH/SFTP/Tasks, supported database-engine, and real Codex/Cursor MCP-client regressions | NOT VERIFIED |
| Account/OAuth/callback, Creem Test/Production billing, and Cloud Sync single/multi-device regressions | NOT VERIFIED |
| MCP prod read-only, blocked-write, confirmation binding, and confirmed retry | NOT VERIFIED |
| Platform OS-keychain release smoke | NOT VERIFIED; the ignored smoke has no new manual evidence |
| Manual MSIX build/install, Store submission/servicing, callback, alias, and NSIS coexistence | NOT VERIFIED; separate from Standard publication |

Historical v0.9.x live/manual outcomes and the scoped 2026-09-28 openGauss
record are preserved below and in their owning reports. They are not promoted
to final v0.10.0 manual `PASS` results.

### Documentation follow-up validation

- Version check: `PASS` via
  `pnpm --config.verify-deps-before-run=false run check:version`; the existing
  version script confirms all tracked consumers match `0.10.0`. The default
  invocation failed before that script because pnpm's automatic dependency
  installation could not open its local cache database in the sandbox. The
  temporary CLI setting skips that installation; repository settings and
  dependencies were not changed.
- `git diff --check`: `PASS`.
- Local Markdown links/anchors: `PASS` across 98 tracked Markdown files,
  99 local links, and 18 anchors, including links into the new final record.
- Historical v0.9.x release evidence and v0.10.0 release-note content:
  unchanged. Only documentation files are modified by this follow-up.

## Historical v0.10.0 preparation status

This is the original local preparation record, before the RC, tag, and formal
publication recorded above. Its `NOT RELEASED` and `NOT VERIFIED` statements
describe that preparation checkpoint, not the current publication state.

- Source version: `0.10.0`; release status: `NOT RELEASED`.
- Local automated verification date: 2026-09-30, Windows.
- Evidence applies to the local release-preparation diff on `main` base
  `e3912f53949436c6108a0e63a2410c6bd3a0d03d`; the eventual candidate must record
  its own committed source SHA and workflow run.

| Command | Recorded local preparation result |
| --- | --- |
| `pnpm run check:version` | PASS: all package/Tauri consumers match `0.10.0` |
| `pnpm run check` | PASS: secret audit, frontend build, Rust workspace check, migrations, large-file guard, shared tokens |
| `pnpm run test` | PASS: 157 files, 924 tests |
| `pnpm run test:rust` | PASS: 1,042 passed, 1 ignored, no failures |
| `pnpm run test:e2e` | PASS: 13 Chromium browser smoke tests |
| `pnpm run test:release-env` | PASS: 73 tests |

The initial sandboxed synchronization attempt stopped at a denied package
write, and the first version-check wrapper could not open pnpm's local cache.
Approved retries completed synchronization and the requested checks outside
the sandbox. The frontend build retains its non-blocking bundle-size warning;
Cargo uses the default Test release channel for these local checks.
Rust also reports an existing non-blocking unused-method warning. The ignored
OS keychain release smoke requires platform credential-store access and remains
`NOT VERIFIED`; it was not manually enabled during this run. Documentation
links/anchors and `git diff --check` pass. Cargo regenerated the lockfile with
only the 18 workspace package version changes; no dependencies changed.

Local preparation is ready for another **Standard Release Candidate** after
reviewing and committing this diff. Run it against that exact source commit
(or `main` once it contains the commit), record the resolved SHA and workflow
result, and collect fresh native/manual evidence from its artifacts. No RC
workflow, tag, or publication was performed during this local preparation.

Candidate artifacts, native platform installation/update, live-service
regressions, and manual gates remain `NOT VERIFIED`. Local automated checks
do not establish a successful Standard Release Candidate or formal release.

## v0.10.0 candidate scope

- **Flow V1:** Canvas authoring, typed inputs, API/SSH/Database actions,
  Condition branches, Wait Until attempts/timeouts, validation, confirmation,
  cancellation, and run-history detail. Repeat the lifecycle through MCP,
  including history pagination and retained history after definition deletion.
  Flow definitions and history are local-only and do not support Cloud Sync.
- **API Collection/Environment exchange:** preview and apply imports; export
  and re-import supported Unfour/Postman/OpenAPI collections and Unfour/Postman
  environments; check distinct import names, credential redaction, and empty
  environment secret values. Review scripts/free-form content before sharing.
- **Database table export:** structure-only, data-only, and combined SQL;
  CSV/JSON data; MCP column/filter/limit options and confirmation for data
  export without a limit. Check the supported engines on disposable data.
- **openGauss:** connection detection, catalog/schema/table browsing, columns,
  primary keys, indexes, native DDL, queries, and structure/data exports.
  Preserve the scoped 2026-09-28 openGauss 6.0.3 live record; it is not a new
  candidate-wide certification or evidence for untested server versions.
- **SQL and Saved SQL context:** selected database/schema on New Query,
  execution, saved queries, reopened history, and MySQL scripts containing
  `USE`; schema discovery must not silently change the query context.
- Retain the existing API, SSH, Database, MCP policy, account, updater,
  migration, and supported Cloud Sync regression gates. Flow is excluded
  from Cloud Sync; no Flow Cloud Sync gate or capability is being added.

Scoped implementation evidence and its limitations:
[Flow V1](flow-v1.md), [Flow MCP](flow-mcp-v1.md),
[Collection/Environment exchange](collection-environment-exchange.md),
[table export](database-table-export.md), [openGauss](database-opengauss.md),
and [SQL execution context](database-sql-execution-context.md).
Release-level native/manual coverage of these changes remains `NOT VERIFIED`,
including after formal publication.

## Historical v0.9.6 preparation status

The following checks were recorded during v0.9.6 preparation against the
`v0.9.5` comparison tag. They are retained as history, not rerun results or
the current publication status of v0.9.6.

- Source version: `0.9.6`; release status: `NOT RELEASED`.
- Version synchronization: `PASS` (`node scripts/sync-version.mjs --check`).
- Cargo workspace lock/check: `PASS` (`cargo check --workspace --locked`).
- Release contract suite: `PASS` (`pnpm run test:release-env`; 73 tests).
- Candidate artifact, platform, live-service, and manual gates: `NOT VERIFIED`.

### Historical v0.9.6 candidate scope

The candidate adds MCP diagnostic tools for database query history, database
and SSH connection metadata updates and deletes, SSH connection tests, and
stored host fingerprint reads; long-running MCP call cancellation with a
120-second safety deadline; and Database, API, SSH, and Workspace editor
polish. Fresh candidate evidence is required for MCP diagnostics and
cancellation, SQL draft close and confirmation, API save and history feedback,
SSH batch close, and the existing supported-module regression matrix.

## Database SQL execution baseline — 2026-09-11

This is scoped implementation evidence, not a new release or a claim of live
PostgreSQL/MySQL verification. The manual SQL matrix is in
`docs/testing/manual-test-cases.md`.

- Replaced frontend per-statement requests with one command-bus script call and
  one checked-out physical connection. No implicit batch transaction is added;
  explicit BEGIN/COMMIT/ROLLBACK retain database semantics. Open transactions and
  TEMP/session state are discarded when the batch connection is released.
- Whole-script read-only/confirmation preflight precedes all SQL. Execution stops
  at the first error. Each statement retains its source range, SQL, ordinal,
  success/failed/skipped state, result, and original driver error.
- SQL is no longer rewritten by appending LIMIT or dispatched using a row-return
  keyword guess. The driver stream is drained while retained rows are capped, so
  RETURNING writes are not partially consumed. Multiple rowsets from one statement
  are not merged; the first is retained and the script surfaces an omission warning.
- Parsing uses `sqlparser` 0.62's dialect tokenizer. SQLite trigger completeness
  uses the same `libsqlite3-sys` version already used by SQLx. `futures-util` is a
  direct dependency for bounded stream consumption. These three direct dependencies
  are task-specific; only sqlparser adds a new package to the lockfile.
- Stop prevents later statements, waits for the current statement/timeout, and
  preserves its outcome. Confirmation edits, repeated Run, tab remounts, and history
  workspace capture are guarded. Local activity-log failure becomes a warning, not
  a false SQL failure that could encourage duplicate mutation execution.
- Newly found issues addressed: mutation PRAGMAs classified as reads, stale
  confirmation/keyboard closures, permission errors marking the connection failed,
  late history writing to a newly selected workspace, and incompatible result shapes
  being merged into one table.

Deliberate limits: MySQL DELIMITER/executable-comment client scripts are rejected
before execution; no stored-routine client interpreter, transaction manager,
continue-on-error, parallel execution, or background job system was added. Stop
is not an immediate server interrupt. Zero-row results may still lack driver column
metadata. Safety classification is conservative lexical policy, not a replacement
for database permissions or a full engine semantic analyzer.

Verification: final command outcomes for this change are recorded in the task
report. Automated coverage includes SQLite execution, dialect boundaries, read-only
preflight for all three drivers, RETURNING/WITH, repeat failures, statement results,
Stop, confirmation, and history scoping. PostgreSQL/MySQL live service and desktop
manual UI checks remain NOT VERIFIED in this environment (no configured test
servers or installed database/container clients).

### Modified-file responsibilities

- `crates/database-engine/Cargo.toml`, `Cargo.lock`: parser and stream dependencies.
- `crates/database-engine/src/database.rs`: register scoped execution modules.
- `crates/database-engine/src/database/{script_parser,scripts,script_connection}.rs`:
  source boundaries, batch preflight/Stop/results, and one-session driver execution.
- `crates/database-engine/src/database/{queries,sql}.rs`: reuse execution for the
  single-statement API; remove unsafe splitting, LIMIT and row-return heuristics;
  retain conservative safety checks using lexical tokens.
- `crates/database-engine/src/database_tests/{mod,scripts}.rs`: engine regression tests.
- `crates/unfour-core/src/models/database.rs`,
  `packages/command-client/src/types/database.ts`: matching script contracts.
- `crates/unfour-command-bus/src/database_commands.rs`,
  `crates/unfour-command-bus/tests/database_script.rs`: adapters, activity outcome
  preservation, and regression coverage.
- `crates/unfour-app/src/commands/database.rs`, `crates/unfour-app/src/lib.rs`,
  `packages/command-client/src/tauri/database.ts`: Tauri registration and typed calls.
- `packages/command-client/src/tauri/browser-mocks/database.ts`: explicit desktop
  requirement instead of pretending a browser mock has physical SQL sessions.
- `packages/database/src/model/{run-sql-batch,run-sql-batch.test,types}.ts`: script
  snapshot/confirmation inputs and frontend state; the obsolete `sql-statements.ts`
  and its test were removed, with parser cases moved to Rust.
- `packages/database/src/hooks/{useDatabaseSqlRunner.ts,useDatabaseSqlRunner.test.tsx}`:
  outcome/Stop/confirmation/repeat-run handling and tests.
- `packages/database/src/hooks/{useQueryHistory.ts,useQueryHistory.test.tsx,
  useDatabaseQueryWorkspaceActions.ts}`: original-workspace history capture.
- `packages/database/src/components/{SqlEditorTab,DatabaseWorkspace,QueryResultPanel,
  QueryResultPanel.test,SqlBatchMessages}.tsx`: statement UX, keyboard callbacks,
  pending-run close handling, and UI regressions.
- `packages/ui/src/i18n/locales/{en,zh-CN}.json`: shared localized execution messages.
- `docs/testing/{manual-test-cases,release-verification}.md`: manual matrix and scope.

Business logic and cross-layer command contracts changed; package ownership and
dependency direction did not. Cross-package changes are limited to the Database
execution contract, its adapters, and shared locale dictionaries. Prioritize human
review of the parser, script connection lifetime, safety policy, and SQL runner.

## Previous v0.9.0 Final Release Verification Record

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
