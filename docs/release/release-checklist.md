# Release checklist

## v0.14.0 preparation status

Status: **NOT RELEASED / RC NOT STARTED** (2026-11-09, Asia/Shanghai).
Version `0.14.0` is prepared locally; final complete candidate SHA is not selected.
Latest published stable remains [v0.13.0](#v0130-released-status).

- [Draft Release Notes](v0.14.0-release-notes.md),
  [Unreleased changelog](../../CHANGELOG.md#unreleased), and
  [source, local results and remaining acceptance](../testing/v0.14.0-release-preparation-2026-11-09.md).
- Scope: C2 workbench, API/DB/SSH Task recovery, Flow/workspace/environment guards,
  MCP resource management and fixes to existing SSH file tools, layout and selection.
  JumpServer/AccessClient, website updates and additional refactoring are deferred.
- Fresh GitHub checks: v0.13.0 is the latest non-draft/non-prerelease release;
  `v0.14.0` tag absent and Release endpoint returns 404. Local/remote main at
  `d3b35b91bd0e888c60719010a34babd78a6ce543`; entry working tree clean.
- [CI 42577948027](https://github.com/zyqzyq/Unfour/actions/runs/42577948027)
  passed 7/7 jobs on that entry SHA. It does not include this uncommitted preparation.

| Required gate | Current disposition |
| --- | --- |
| Version, changelog, notes and local verification | See the dated preparation record; local evidence applies to HEAD plus the preparation diff |
| Reviewed complete commit, push and CI on that exact SHA | PENDING; no commit/push performed in this task |
| Standard Release Candidate on complete SHA | NOT RUN; do not dispatch entry HEAD or moving main as evidence for local edits |
| Windows x64, macOS arm64/x64, Linux x64 candidate builds | NOT RUN for v0.14.0; all four required from the same RC run |
| Candidate identity/checksum/signature and focused native acceptance | PENDING; local debug checks do not establish installed RC behavior |
| Real SSH file create/overwrite/append/patch and basic permissions | NOT VERIFIED without a designated disposable remote directory and test connection |
| Final go/no-go and manual-risk disposition | PENDING; no claim of all gates passing |
| Formal tag / GitHub Release / Stable manifest promotion | OUT OF SCOPE; not performed |

Commit the reviewed preparation, push it, wait for CI, then dispatch
`release-candidate.yml` with `ref=<full-complete-commit-SHA>` as described in the
dated record. Download and verify that run's artifacts, finish scoped acceptance,
and record any remaining risk before a separately authorized formal release.
Retain all historical records below with their original evidence boundaries.

## v0.13.0 released status

- Status: **RELEASED** as Standard stable on **2026-11-07**, at
  `2026-11-07T15:19:26Z` (23:19:26 Asia/Shanghai); version `0.13.0`, tag `v0.13.0`.
- Exact tag, CI, RC and formal Release SHA:
  `e9fd2f7189e2569f9710588e5330a6da287e3cc3`.
  [Comparison](https://github.com/zyqzyq/Unfour/compare/v0.12.0...v0.13.0).
- [GitHub Release](https://github.com/zyqzyq/Unfour/releases/tag/v0.13.0):
  ID `437654776`, non-draft, non-prerelease, 13 assets.
- [Release Notes](v0.13.0-release-notes.md),
  [Changelog](../../CHANGELOG.md#0130---2026-11-07) and
  [final release verification](../testing/release-verification.md#v0130-final-release-verification-record)
  record the published scope and evidence checked on 2026-11-08.

| Gate | v0.13.0 result and evidence |
| --- | --- |
| CI on exact release SHA | PASS: [#334 / 42380089617](https://github.com/zyqzyq/Unfour/actions/runs/42380089617), 7/7 jobs |
| Standard Release Candidate | PASS: [#22 / 42380108572](https://github.com/zyqzyq/Unfour/actions/runs/42380108572), 6/6 jobs, all four builds including macOS x64 |
| Windows RC artifact and native regression | PASS within the [recorded scope](../testing/v0.13.0-rc-final-verification-2026-11-07.md): identity/signatures, isolated backend checks, Flow/Run Again -> Database, API -> Database, history and restart; DB-01 closed |
| Standard Release | PASS: [#34 / 42383703464](https://github.com/zyqzyq/Unfour/actions/runs/42383703464), same SHA, 7/7 jobs, all four builds and publish |
| GitHub / R2 distribution | PASS: 13 assets, ten R2 re-download checksum checks OK in publish logs; checksum entries match GitHub digests |
| Stable manifests | PASS: both `0.13.0`, four platforms, public bytes match GitHub assets; downloads promoted before updater |
| Installed upgrade, updater, formal-artifact native checks, macOS/Linux runtime/keychains | NOT VERIFIED; formal builds differ from tested RC bytes |
| Real SSH/SFTP/Tasks, OAuth/accounts, remote DB, Cloud Sync and remaining MCP/manual scenarios | NOT VERIFIED where the RC matrix lacks evidence; no dedicated real resources |
| Release-operator clean tree and explicit disposition of outstanding manual risks | NOT RECORDED / NOT VERIFIED |
| Microsoft Store/MSIX | Separate path; NOT VERIFIED |

The [preparation](../testing/v0.13.0-release-preparation-2026-11-07.md),
[hardening](../testing/release-hardening-2026-11-07.md) and
[final RC record](../testing/v0.13.0-rc-final-verification-2026-11-07.md) retain
historical checkpoints. Their NOT RELEASED and HOLD statements predate the
completed publication; publication does not make missing manual checks PASS.
This documentation follow-up includes prior local RC report edits, without
moving the released tag, changing version or repeating publication.

## v0.12.0 released status

- Status: **RELEASED** as Standard stable on **2026-10-23**
  (`2026-10-23T03:25:54Z`, 11:25:54 Asia/Shanghai); version `0.12.0`, tag `v0.12.0`.
- Exact tag, CI, RC and Release SHA:
  `4883df8dbfaf4d9d69c94ed5bae75c627d146e34`.
  [Comparison](https://github.com/zyqzyq/Unfour/compare/v0.11.0...v0.12.0).
- [GitHub Release](https://github.com/zyqzyq/Unfour/releases/tag/v0.12.0):
  Release ID `421419257`, `draft=false`, `prerelease=false`, 13 uploaded assets.
- [Release Notes](v0.12.0-release-notes.md), [CHANGELOG](../../CHANGELOG.md#0120---2026-10-23)
  and the [final verification record](../testing/release-verification.md#v0120-final-release-verification-record)
  record formal publication and its actual distribution inventory.

| Gate | v0.12.0 result and evidence |
| --- | --- |
| CI on exact release SHA | PASS: [#320 / 39885807832](https://github.com/zyqzyq/Unfour/actions/runs/39885807832), all seven jobs |
| Standard Release Candidate | PASS (automated): [#20 / 39885870422](https://github.com/zyqzyq/Unfour/actions/runs/39885870422), identity, full verify and four native builds / Actions artifacts; no publish job |
| Standard Release | PASS: [#33 / 39892644501](https://github.com/zyqzyq/Unfour/actions/runs/39892644501), same SHA, full verify, four native builds and publish |
| GitHub / R2 distribution | PASS: 13 GitHub assets; publish job re-download reports ten checksum entries OK; public R2/GitHub checksum files and API digests agree |
| Stable manifests | PASS at 2026-10-23 check: downloads/updater both `0.12.0`, four platforms each, byte-identical to GitHub assets; downloads promoted before updater |
| Native install/launch/upgrade/uninstall, updater/signature rejection, OS trust, real SSH Task reruns and released-artifact feature regressions | NOT VERIFIED |
| Detailed Flow Sync service/device/offline/conflict/deletion/upgrade scenarios | NOT VERIFIED where scenario evidence is absent; general manual validation remains USER CONFIRMED |
| Manual RC sign-off, account/live-service/MCP prod journeys, macOS/Linux credential stores and separate MSIX/Store path | NOT VERIFIED |

The [original RC checklist](../testing/v0.12.0-rc-checklist.md) and
[dated source preparation](../testing/v0.12.0-release-preparation-2026-10-22.md)
are preserved unchanged. Their `NOT RELEASED / RC NOT STARTED`, unselected-SHA
and CI #319 statements describe the preparation checkpoint, before the runs above.
Publication and automated checks do not turn those manual scenario rows into `PASS`.
The v0.11.0 and older records below retain their original scope.

## v0.11.0 released status

- Status: **RELEASED** as Standard stable on **2026-10-10**
  (`2026-10-10T08:57:14Z`); version `0.11.0`, tag `v0.11.0`.
- Exact tag/release commit: `311910dedaaa849ee9130e9d03a1301714b02726`;
  comparison: `v0.10.0...v0.11.0`.
- [GitHub Release](https://github.com/zyqzyq/Unfour/releases/tag/v0.11.0):
  published, non-draft, non-prerelease, Release ID `408823362`.
- Community/free and Pro use the same application, Cargo workspace, Tauri
  configuration, and package versions; no separate Pro build is introduced.
- Evidence checked on 2026-10-10; exact workflow, artifact, checksum, and live
  manifest results are in the
  [v0.11.0 final verification record](../testing/release-verification.md#v0110-final-release-verification-record).
- [Release Notes](v0.11.0-release-notes.md) and
  [published changelog](../../CHANGELOG.md#0110---2026-10-10) cover changes after
  v0.10.0, including the final Workspace MCP permission and safety-hint fixes.
- Preserve the [source preparation checkpoint](../testing/v0.11.0-release-preparation-2026-10-10.md),
  [initial RC fixes](../testing/v0.11.0-rc-readiness-2026-10-09.md), and
  [latest Auth/export-safety evidence](../testing/v0.11.0-rc-openapi-auth-history-2026-10-10.md#export-compatibility-and-rotated-credential-follow-up-latest)
  as historical scoped evidence. Their preparation-time `NOT RELEASED` and
  `NOT RUN` values do not describe the completed publication.
- Workspace encrypted backups remain local files. Sensitive data and Flow
  definitions/history are not added to Cloud Sync.

| Gate | v0.11.0 status and evidence |
| --- | --- |
| Formal tag and release commit | VERIFIED: local/GitHub tag and release workflow agree on the SHA above |
| CI on release commit | PASS: [run 38035203118](https://github.com/zyqzyq/Unfour/actions/runs/38035203118) |
| Standard Release Candidate on release commit | PASS: [run 38036217463](https://github.com/zyqzyq/Unfour/actions/runs/38036217463); verify and four signed native builds |
| Standard Release on formal tag | PASS: [run 38038525070](https://github.com/zyqzyq/Unfour/actions/runs/38038525070); identity, verify, four builds, and publish |
| Automated frontend/Rust/MCP, SSH-native, version, secret, migration, release-contract, and browser checks | PASS: recorded CI and reusable RC/Release jobs; browser smoke does not establish native/manual coverage |
| Canonical build/staging and GitHub assets | PASS / VERIFIED: four Release Actions artifacts and 13 uploaded Release assets; Linux used `ubuntu-22.04` |
| Immutable R2 checksums | PASS: publish log reports ten checksum entries as OK; GitHub/R2 checksum files and API digests agree |
| Stable downloads/updater manifests | PASS: HTTP 200, version `0.11.0`, expected keys/URLs/signatures; byte-identical to GitHub assets; downloads promoted before updater |
| Changelog date, pinned comparison, Release Notes, README, and User Guide | COMPLETE: post-release documentation archived for `2026-10-10` |
| Release-operator tree review/cleanliness at publication | NOT RECORDED; successful workflows do not establish it |
| Native installation, launch, upgrade/uninstall, MCP sidecar replacement, OS trust, previous-Stable updater and invalid-signature rejection | NOT VERIFIED |
| Released-artifact Workspace exchange/backup, Auth/OpenAPI, credential, Database/MCP/Flow and live-service/Cloud Sync regressions | NOT VERIFIED; earlier implementation and local Windows Keychain evidence retain their original scope |
| Linux Ubuntu 22.04/24.04 runtime, macOS/Linux native credential stores, and manual MSIX/Store journey | NOT VERIFIED; Linux remains Experimental; Store is separate |

Publication and automated verification do not turn unrun manual checks into
`PASS`. Use the historical [manual acceptance checklist](../testing/v0.11.0-release-preparation-2026-10-10.md#manual-rc-acceptance)
for any follow-up against the exact released artifacts. This documentation
follow-up does not push, retag, rebuild, redispatch, or republish the release.

The published v0.10.0 evidence below is retained with its original scope.

## v0.10.0 released status

- Status: `RELEASED` as Standard stable on 2026-09-30
  (`2026-09-30T07:24:24Z`); source version `0.10.0`, formal tag `v0.10.0`.
- Tag/release commit: `96f0022ec344fd643b567a54fe6f3686c2272dc0`;
  comparison tag: `v0.9.6` (`87bea5d6f337cc6d1d10055972bce2f3ee24aaee`).
- [GitHub Release](https://github.com/zyqzyq/Unfour/releases/tag/v0.10.0):
  published, non-draft, non-prerelease, Release ID `399805354`.
- Evidence checked on 2026-09-30. Exact workflow, artifact, checksum, and
  manifest evidence is recorded in the
  [v0.10.0 final verification record](../testing/release-verification.md#v0100-final-release-verification-record).
- Scope: Flow V1 and MCP Flow execution, API Collection/Environment exchange,
  Database table export, openGauss support, and SQL/Saved SQL context fixes.
  Flow V1 is local-only and does not support Cloud Sync.
- Native platform, live-service, and manual regressions without v0.10.0
  evidence remain `NOT VERIFIED`. Historical v0.9.0 release outcomes and
  v0.9.6 preparation checks do not verify the released v0.10.0 artifacts.

| Gate | v0.10.0 status and evidence |
| --- | --- |
| Formal tag and release commit | VERIFIED: local/GitHub tag agree on the SHA above |
| CI on release commit | PASS: [run 36676317345](https://github.com/zyqzyq/Unfour/actions/runs/36676317345) |
| Standard Release Candidate on release commit | PASS: [run 36676343771](https://github.com/zyqzyq/Unfour/actions/runs/36676343771); verify job and four signed native builds |
| Standard Release on formal tag | PASS: [run 36681185573](https://github.com/zyqzyq/Unfour/actions/runs/36681185573); verify/build/publish jobs |
| Automated version, secret, migration, frontend/Rust/MCP, SSH-native, release-contract, and browser-smoke checks | PASS: the recorded CI and reusable RC/Release verification jobs; browser smoke is not native manual coverage |
| Windows x64, macOS arm64/x64, Linux x64 canonical build/staging | PASS: four RC and four Release Actions artifacts; Linux builds used `ubuntu-22.04` |
| GitHub Release assets | VERIFIED: 13 uploaded assets, including payloads, updater signatures, checksums, and both manifests |
| Checksums and immutable R2 bytes | PASS: publish job re-download reports ten checksum entries as OK; direct GitHub/R2 checksum-file reads agree |
| Updater/download manifests and promotion order | PASS: publish log uploads downloads first, updater last; live HTTP 200 reads show `0.10.0`, expected platform keys, and bytes matching GitHub assets |
| Changelog publication date and pinned comparison | COMPLETE: `2026-09-30`, `v0.9.6...v0.10.0`; synchronized in this post-release documentation follow-up |
| Release-operator tree review/cleanliness at publication | NOT RECORDED; cannot be inferred from successful workflows |
| Native install, launch, upgrade, uninstall, MCP sidecar replacement, and OS trust checks | NOT VERIFIED |
| Previous-Stable update, manual invalid-signature rejection, and Linux 22.04/24.04 runtime/updater | NOT VERIFIED |
| Desktop/MCP Flow and new API/Database regression on released artifacts | NOT VERIFIED; existing scoped implementation evidence is retained |
| Real API/SSH/database/MCP-client, Account/OAuth/billing, Cloud Sync, multi-device, and MCP prod-policy regressions | NOT VERIFIED |
| OS-keychain release smoke and manual MSIX/Store journey | NOT VERIFIED; Store publication is separate |

Implemented, automated verified, manually verified, and released are separate
states. Standard publication completes the release gates evidenced above; it
does not turn unrun manual or live-service checks into `PASS`.

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

The sections below retain the reusable release procedure. Actual v0.12.0, v0.11.0 and
historical v0.10.0 outcomes are recorded above; imperative steps alone are not
`PASS` claims.
The v0.9.0 table above and the v0.9.6 preparation section in
`docs/testing/release-verification.md` remain
historical records.

## Shared gate

- Working tree and intended release commit are reviewed.
- `pnpm run check:version`, `pnpm run check:secrets`, migration checks,
  frontend/Rust tests, and release contract tests pass.
- The release version is unused, exactly `X.Y.Z`, and the tag is `vX.Y.Z`.
- Historical `pro_*` SQL migration files pass their immutable checksum guard.
- Historical Community DB, historical Pro DB, and clean DB migrations have
  current test evidence.
- API, SSH, Database, Flow, MCP, Account, Cloud, and multi-device manual results are
  recorded; unavailable live services remain `NOT VERIFIED`.
- Exercise Flow Canvas authoring, API/SSH/Database actions, Condition branches,
  Wait Until, cancellation and run history through both Desktop and MCP.
  For v0.12.0 and later regressions, verify definition-only Cloud Sync on two
  real devices, including offline recovery, whole-definition conflicts,
  tombstones, upgrade backfill and history isolation using the
  [v0.12.0 checklist](../testing/v0.12.0-rc-checklist.md).
  Actual runtime inputs/outputs, history and credentials stay local;
  deployed-server/two-device acceptance remains `NOT VERIFIED` until recorded.
  Use the [v0.11.0 acceptance checklist](../testing/v0.11.0-release-preparation-2026-10-10.md#manual-rc-acceptance)
  for v0.11.0 regressions; existing implementation reports do not establish
  released-artifact native/manual verification.

## Release Candidate

- After source review, commit and push, manually run **Standard Release Candidate**
  with `ref` set to the reviewed full commit SHA and record the resolved SHA.
  Do not dispatch the pre-preparation base SHA or a moving branch in place of
  the reviewed candidate.
- Confirm the GitHub Environment `production` contains
  `TAURI_SIGNING_PRIVATE_KEY` and, when the key is encrypted,
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The reusable workflow's actual
  `build` job binds that Environment and a missing private key must fail the
  shared build rather than produce unsigned updater artifacts.
- Confirm the reusable workflow completes the full verify job and exactly four
  native builds: Windows x64, macOS arm64, macOS x64, and Linux x64.
- For any candidate, confirm the actual Linux build uses `ubuntu-22.04`
  with a runner-specific Rust cache key. The independent verify job may use
  `ubuntu-latest`; it must not supply packaged native artifacts.
- Download all four `release-candidate-*` Actions artifacts. Verify canonical
  filenames, non-empty updater signatures, and that each file belongs to the
  expected architecture.
- Record SHA-256 hashes for the downloaded RC files. The RC uploads per-platform
  artifacts only; `SHA256SUMS.txt`, `latest.json` and `downloads.json` are
  finalized by the formal Release publish job, not by the RC workflow.
- Confirm Windows contains one NSIS installer plus `.sig`; each macOS artifact
  contains its DMG, `.app.tar.gz`, and `.app.tar.gz.sig`; Linux contains only
  the public x64 AppImage and `.sig` even if Tauri also built `.deb`/`.rpm`.
- Record install, launch, upgrade, updater, signature rejection, OS trust, and
  uninstall results from the downloaded candidate artifacts.
- Exercise candidate updater bytes/signatures through an isolated test fixture;
  do not promote public Stable manifests to test an unreleased RC. The formal
  Release rebuilds the accepted source through the same reusable workflow;
  it does not automatically promote the downloaded RC artifacts.
- For Linux Experimental (x86_64 AppImage only), record Ubuntu 22.04 executable
  permission, launch, first-window rendering, API/SSH/Database/Flow module opening,
  and quit/relaunch, plus an Ubuntu 24.04 launch smoke with the same artifact.
  Retain Linux updater smoke and signature-rejection checks; unrun items remain
  `NOT VERIFIED`. Follow `docs/testing/manual-test-cases.md`. Do not rebuild or
  overwrite v0.9.0 artifacts to exercise the new baseline.
- Confirm the RC run created no tag or GitHub Release, accessed no R2 path,
  changed neither `stable/latest.json` nor `stable/downloads.json`, and built
  or published no MSIX.

## Standard

- v0.11.0 documentation follow-up: `COMPLETE`.
  [CHANGELOG.md](../../CHANGELOG.md) records the actual publication date
  `2026-10-10` and pins `[0.11.0]` to
  `https://github.com/zyqzyq/Unfour/compare/v0.10.0...v0.11.0`.
- v0.12.0 documentation follow-up: `COMPLETE`; actual publication date
  `2026-10-23`, comparison `v0.11.0...v0.12.0`, and the final verification
  record above. Outstanding manual checks remain `NOT VERIFIED`.
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
