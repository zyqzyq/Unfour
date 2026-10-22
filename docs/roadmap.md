# Roadmap

This roadmap is a product-direction summary. It is not release evidence. Use
[release verification](testing/release-verification.md) and
[the release checklist](release/release-checklist.md) for release readiness.
Sections through v0.10.0 record what those releases shipped. They do not
describe the current source, and they do not change any `NOT VERIFIED` result.

Current source version: `0.11.0`. The latest published release is
[v0.11.0](https://github.com/zyqzyq/Unfour/releases/tag/v0.11.0), published on
2026-10-10. Current development on `main` targets **v0.12.0**, in documentation
preparation only: **NOT RELEASED / RC NOT STARTED**. Earlier verification records
remain historical and do not establish v0.11.0 manual coverage.

## Published capabilities through v0.11.0

Shipped through v0.11.0:

- API Client, SSH Terminal, Database, and local Flow runbooks in one workspace.
- Local stdio MCP over the command bus, including Flow save, run, cancel, and
  query. MCP does not generate a troubleshooting playbook. Boundaries,
  environment permissions, and confirmation are in the
  [MCP overview](mcp/overview.md).
- Whole-Workspace import/export, including Flow definitions, plus
  password-encrypted local backups. Run history is not exported.
- Pro Cloud Sync for supported non-secret workspace data. Flow definitions,
  Flow run history, and sensitive values are not synchronized.

## v0.12.0 (Planned, current main)

- Flow Definition Cloud Sync is enabled in the client: whole definitions,
  durable offline intent, tombstones and keep-local/use-remote conflicts.
  Runtime inputs/outputs, run history and credentials remain local. Input
  declarations and ordinary defaults are definition data; recognized sensitive
  and device-local material is rejected, but arbitrary secrets in free text
  cannot be reliably detected. The cloud payload limit is 64 KiB.
- SSH Task previous-parameter recall, reviewed history/result-panel reruns,
  stale-input protection and sensitivity/source locks.
- MCP/Database/SSH read-only enforcement, retained completed results after
  activity logging failures, API history/diagnostic safety and Flow Runtime
  environment/timeout fixes, plus first-use and navigation improvements.

Deployed-server Flow support, real two-device convergence, offline/conflict/delete
recovery, history isolation and old-version upgrade are **NOT VERIFIED**. See
the [draft notes](release/v0.12.0-release-notes.md) and
[prospective RC checklist](testing/v0.12.0-rc-checklist.md). No v0.12.0 workflow,
artifact or release acceptance is established by this roadmap.

## Not yet verified for v0.11.0

Publication does not make these checks `PASS`. They remain `NOT VERIFIED` in
the [v0.11.0 verification record](testing/release-verification.md#v0110-final-release-verification-record):

- native install, launch, upgrade, uninstall, MCP sidecar replacement, and OS
  trust, including Linux 22.04/24.04 runtime;
- v0.10.0-to-v0.11.0 in-app update and manual invalid-signature rejection;
- released-artifact Workspace exchange, credential, Auth, Database, MCP, and
  Flow regressions, plus live API, SSH, and database journeys;
- Account, Cloud Sync, and MCP production-policy journeys;
- macOS and Linux native credential stores, and the manual MSIX/Store path.

## v0.11.0 (Published)

The published follow-up from `v0.10.0` includes:

- whole-Workspace import/export of variables, environments, API definitions,
  SSH and Database connection settings, SSH tasks, Saved SQL, and Flow
  definitions, with an import preview that creates an independent Workspace;
- password-encrypted local backups that can optionally include supported saved
  secrets, without embedding private-key files, database files, or histories;
- independent editing of workspace environment type and MCP permissions,
  including after import.

Flow definitions are part of that Workspace exchange. Flow run history is not
exported. Flow definitions and run history remain outside Cloud Sync. Sensitive
data is not synchronized. MCP Flow tools shipped in v0.10.0 and remain the way
to save, run, cancel, and query a Flow; they still do not author a playbook.

Current status:

- `v0.11.0` was released on 2026-10-10;
- CI, Standard Release Candidate, Standard Release, artifacts, checksums, and
  manifest evidence are recorded in the
  [final verification record](testing/release-verification.md#v0110-final-release-verification-record);
- the items in [Not yet verified for v0.11.0](#not-yet-verified-for-v0110)
  remain `NOT VERIFIED`;
- current `main` continues from v0.11.0 without an assigned next version.

Release claims remain limited by the current verification matrix.

## v0.1 (First Public Release)

Release focus:

- local-first desktop workbench;
- API Client request editing, Send, saved requests, collections, environments,
  response inspection, and redacted history;
- SSH Terminal connection/session workflows with host-key trust and redacted
  logs;
- Database connection, schema, SQL execution, table preview, and confirmation
  guardrails;
- workspace-scoped local data and layout state;
- local stdio MCP diagnostics over the command bus;
- release verification, distribution, and signing documentation.

Remaining readiness work must be tracked through release verification, not
through progress logs.

## v0.2 (SSH And Workspace Workflows)

Delivered areas:

- SFTP remote file browsing and transfer;
- serial SSH task automation;
- shared Workspace variables and environments;
- Database multi-statement execution;
- OpenAPI import/export and table-row editing improvements.

## v0.3.0-rc.1

Release-candidate focus:

- saved pre-request and post-response API scripts with tests and console output;
- transactional Workspace domain mutations, snapshots, tombstones, and
  external-apply foundations for optional edition sync;
- isolated stable/development/test storage profiles;
- SSH terminal clipboard actions;
- NSIS handling for a running MCP sidecar;
- Database row-action visibility.

These items are release claims only to the extent recorded in
`docs/testing/release-verification.md`.

## v0.4.0

Delivered areas:

- policy-aware MCP workspace variable CRUD over the command bus;
- MCP SSH task lifecycle tools for save, run, cancellation, inspection, and
  cleanup;
- SSH task ordering, workspace/environment defaults, bounded event handling,
  and secret-safe task logs.

These items are release claims only to the extent recorded in the current
verification matrix.

## v0.5.0

Delivered areas:

- persistent, redacted SSH command history and terminal command suggestions;
- read-only MCP SSH history inspection for user-confirmed task drafting;
- API collection, folder, and saved-request snapshots and external apply for
  optional edition sync foundations;
- API snapshot and external-apply secret redaction and resilience hardening.

The API domain work is a local command-bus foundation and does not by itself
provide a hosted sync service. Release claims remain limited by the current
verification matrix.

## v0.6.0

Delivered areas:

- SSH task snapshots, tombstones, external apply, workspace-delete cascades,
  and connection-aware task entities for optional edition sync foundations;
- ephemeral MCP registry storage and container/sidecar packaging support;
- MCP output-schema alignment and catalog-context propagation for database
  read-only queries;
- workspace-scoped database credentials and consistent child tombstones on
  workspace deletion;
- stable SSH command suggestions, history tracking, and device-local transfer
  paths.

The SSH task and MCP domain work is a local command-bus foundation and does not
by itself provide a hosted sync service. Release claims remain limited by the
current verification matrix.

## v0.7.0

Delivered areas:

- SSH and Database connection snapshots, revisioned mutations, tombstones, and
  external apply for optional Cloud Sync foundations;
- shared connection domain contracts and command-bus transaction integration;
- device-local connection saves that do not create cloud mutations when shared
  connection fields are unchanged, while preserving local activity recording.

The connection domain work is a local command-bus foundation and does not by
itself provide a hosted sync service. Release claims remain limited by the
current verification matrix.

## v0.8.0 (Earlier published release)

Historical status:

- Published Unfour release `v0.8.0`;
- automated, installer, platform, and live-service verification recorded in
  the active release matrix;
- continued platform hardening without expanding the current package or
  command-bus boundaries;
- MCP tools are provided over local stdio through the command bus and support
  the product's troubleshooting loop: Codex and Cursor can use saved API, SSH,
  and database connections to reproduce issues, inspect logs and database
  state, and act on the findings. At this checkpoint, troubleshooting steps
  were directed through individual tools; local Flow runbooks arrive in v0.10.0.

Release claims remain limited by the current verification matrix.

## v0.9.0 (Earlier published release)

Implemented in v0.9.0:

- GitHub browser sign-in with PKCE, deep-link completion, OS-keychain desktop
  sessions, account entitlements, and billing destinations;
- local-first Cloud Sync unlocked by an active Pro subscription for workspaces,
  API trees, variables, connections, and SSH tasks, including retries, conflict
  resolution, remote workspace download, and recovery;
- one unified desktop/MCP runtime with shared storage, command-bus setup,
  account context, and an ephemeral registry mode for CI and smoke checks;
- one-click Codex and Cursor MCP configuration from Settings;
- signed in-app updates for Standard builds and a separately contract-tested
  Windows x64 Microsoft Store MSIX distribution; and
- a reusable Release Candidate/Release pipeline for canonical Windows, macOS,
  and Linux AppImage artifacts, checksum manifests, and ordered updater
  promotion.

Current status:

- `v0.9.0` is published from commit
  `1dc7c1cc6430e546689fde5206599a31f36b17a1`;
- PostgreSQL and MySQL are implemented, verified against real environments, and
  released; compatible MariaDB servers use the MySQL driver path, without a
  separate v0.9.0 MariaDB verification claim;
- SSH is implemented, release-level verified against a real server, and
  released; and
- final publication evidence and the remaining platform-specific limits are
  recorded in `docs/testing/release-verification.md`.

Release claims remain limited by the current verification matrix.

## v0.9.1 (Previous source checkpoint)

Maintenance and release-hardening follow-up to the published v0.9.0 release:

- frontend lifecycle hardening and broader high-risk execution coverage;
- Cloud Sync repository and worker responsibility splits;
- a separate public `stable/downloads.json` installer manifest;
- an Ubuntu 22.04 baseline for Standard Linux x86_64 AppImage builds; and
- Codex and Cursor MCP runtime verification guidance and troubleshooting
  documentation.

Current status:

- the `v0.9.1` tag is a historical source checkpoint, not the current release baseline;
- the active release verification record still treats the v0.9.0 publication
  evidence as historical.

Release claims remain limited by the current verification matrix.

## v0.9.2 (Previous source checkpoint)

The follow-up from the `v0.9.1` tag focuses on MCP correctness, privacy, and
desktop usability:

- anonymous active-install telemetry with a first-run notice, Privacy opt-out,
  UTC daily gating, and a test/dev network-disable policy;
- per-call MCP environment selection, saved-request script replay, API
  environment-variable tools, and fail-closed capability/risk policy checks;
- independent persisted sidebar widths for API, SSH, and Database, including
  legacy single-width migration;
- the Windows Cursor MCP path-with-spaces fix; and
- refreshed README/product screenshots plus the Microsoft Store listing icon.

Current status:

- the `v0.9.2` tag is a historical source checkpoint, not the current release baseline;
- the active release verification record still treats the v0.9.0 publication
  evidence as historical.

Release claims remain limited by the current verification matrix.

## v0.9.3 (Previous source checkpoint)

The follow-up from the `v0.9.2` tag focuses on API execution controls and
Cloud Sync reliability:

- global and per-request API timeouts, active-request cancellation, and
  explicit timeout/cancellation states;
- clearer API request naming and history display;
- authoritative Cloud Sync workspace ownership, durable outbox preservation
  during account inactivity, orphan/legacy binding recovery, and legacy API
  entity bootstrap; and
- a consolidated desktop settings navigation surface.

Current status:

- the `v0.9.3` tag is a historical source checkpoint, not the current release baseline;
- the active release verification record still treats the v0.9.0 publication
  evidence as historical.

Release claims remain limited by the current verification matrix.

## v0.9.4 (Previous source checkpoint)

The follow-up from the `v0.9.3` tag focuses on multipart API requests and Cloud
Sync Protocol 5 compatibility:

- ordered multipart text/file fields, transient file bindings, streamed upload,
  and body-kind-aware request history; and
- Protocol 5 client freeze, entity/reader-revision tracking, compatibility
  hardening, fail-closed incomplete apply, and workspace delete resolution.

Current status:

- the `v0.9.4` tag is a historical source checkpoint, not the current release baseline;
- the active release verification record still treats the v0.9.0 publication
  evidence as historical.

Release claims remain limited by the current verification matrix.

## v0.9.5 (Previous source checkpoint)

The follow-up from the `v0.9.4` tag focuses on Database SQL script execution,
Cloud Sync defaults for new accounts, and workbench navigation:

- one-connection SQL script batches with whole-script confirmation, per-statement
  results, Stop that waits for the current statement, and credential-reference
  save safety;
- first account activation on a device turning Global Sync on while preserving
  existing Off preferences; and
- preserved unsaved editor drafts, simpler workbench chrome, and About-page
  feedback links.

Current status:

- the `v0.9.5` tag is a historical source checkpoint, not the current release baseline;
- the active release verification record still treats the v0.9.0 publication
  evidence as historical.

Release claims remain limited by the current verification matrix.

## v0.9.6 (Earlier published release)

The follow-up from the `v0.9.5` tag focuses on MCP diagnostics and long-running
call control, plus Database, API, SSH, and Workspace polish:

- workspace-scoped database query history, database and SSH connection metadata
  updates and deletes, SSH connection tests, and stored host fingerprint reads;
- stdio-responsive MCP cancellation, execution propagation where supported, a
  120-second safety deadline, and bounded queue/shutdown cleanup; and
- SQL draft close protection, safer dangerous-SQL confirmation, Saved SQL
  open/save polish, API save/history feedback, SSH batch close, and workspace
  label polish.

Historical checkpoint status:

- `v0.9.6` was the published baseline compared with v0.10.0;
- the recorded v0.9.6 preparation checks remain historical evidence in the
  active release matrix and do not verify a new candidate.

Release claims remain limited by the current verification matrix.

## v0.10.0 (Previous published release)

The published follow-up from `v0.9.6` includes:

- Flow V1 local runbooks combining saved API requests, SSH tasks, and Database
  queries, with Condition, Wait Until, Canvas authoring, and run history;
- MCP management and execution of the same Flow definitions and runs, including
  cancellation and shared run history;
- API Collection and Environment import/export with import previews;
- Database table structure/data export and improved openGauss support; and
- preserved SQL execution and Saved SQL database/schema context.

Flow V1 at the v0.10.0 release was local-only. Its definitions and run history
did not support Cloud Sync or workspace import/export. v0.11.0 later included
Flow definitions in whole-Workspace import/export; run history and Cloud Sync
still exclude Flow.

Historical status:

- `v0.10.0` was released on 2026-09-30 from commit
  `96f0022ec344fd643b567a54fe6f3686c2272dc0`;
- CI, Standard Release Candidate, Standard Release, artifacts, checksums, and
  manifest evidence are recorded in the
  [final verification record](testing/release-verification.md#v0100-final-release-verification-record);
- unrecorded native platform, live-service, and manual checks for that release
  remain `NOT VERIFIED`; publication does not establish manual coverage.

Release claims remain limited by the current verification matrix.

## Beyond v0.12.0

No version beyond v0.12.0 is assigned. Likely follow-up areas, none of which are
release claims:

- operating-system code signing and macOS notarization;
- richer database result interactions;
- sensitive-data Cloud Sync and synchronization of Flow run history/runtime data;
- plugin extension points.

MCP and local Flow runbooks are current capabilities, not future AI work. A
separate CLI is not part of the current product. Adding another adapter later
would still go through the command bus.

## Release Rule

Do not claim a roadmap item is complete in release notes unless it is backed by
current verification or repository evidence. Roadmap text does not change a
`NOT VERIFIED` result into a verified one.
