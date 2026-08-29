# Changelog

This file is the user-facing change history for Unfour, following
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html)

## [Unreleased]

## [0.9.0] - 2026-08-29

Released after `v0.8.0`, adding account-gated Cloud Sync, unified desktop and
MCP runtimes, and signed in-app updates with multi-channel distribution. An
active Pro subscription unlocks Cloud Sync in the same Unfour application.

### Added

- **Account and billing** — Add GitHub browser sign-in with PKCE and
  `unfour://auth/callback`, OS-keychain desktop sessions, account and
  entitlement state, and checkout/account-portal actions.
- **Cloud Sync** — Add local-first synchronization unlocked by an active Pro
  subscription for workspaces,
  API collections/folders/requests, environments and variables, SSH tasks, and
  SSH/Database connection definitions, with outbox retries, snapshots,
  tombstones, remote-workspace download, conflict resolution, and recovery.
- **Unified desktop and MCP runtime** — Share storage, command-bus setup,
  account context, and Cloud Sync initialization between the Tauri desktop and
  standalone `unfour-mcp` process, while retaining an ephemeral registry mode
  for CI and protocol smoke tests.
- **MCP client setup** — Add one-click Codex and Cursor configuration from
  Settings, including platform-aware sidecar/Store alias commands and safe
  preservation or backup of existing configuration.
- **Signed in-app updates and Store distribution** — Add in-app updates backed
  by signed updater artifacts for Standard builds and a separately
  contract-tested Windows x64 MSIX path managed by Microsoft Store.
- **Release candidate automation** — Add a reusable Standard build workflow for
  Windows x64, macOS arm64/x64, and Linux x64 AppImage candidates, with
  canonical artifacts, signatures, release checksums, and contract coverage.

### Changed

- **Distribution and release authority** — Split `standard` and
  `microsoft-store` at build time. Standard publishes NSIS, macOS, and Linux
  AppImage artifacts through GitHub/R2 and owns the Unfour updater, while Store
  builds disable the internal updater and use the Microsoft Store update path.
- **Release publication flow** — Resolve tags to exact commits, build once,
  reuse identical staged bytes for GitHub and R2, and promote
  `stable/latest.json` only after immutable artifacts and checksums pass.
  Production signing and publication secrets are read from the GitHub
  `production` Environment.
- **Local data profiles** — Keep Stable, Test, and Dev product roots isolated
  while desktop and MCP share the same resolver; preserve historical Community
  and Cloud Sync migration data.

### Fixed

- **MCP process lifecycle** — Handle EOF, broken stdout, and termination
  readiness cleanly, and avoid shutting down the Tokio runtime from an async
  context.
- **Windows Store and MSIX integration** — Use the stable `unfour-mcp.exe`
  execution alias and invoke PowerShell 7 for MSIX tooling.
- **Release artifact staging** — Correct the updater host, macOS updater
  artifacts, Linux publication to AppImage only, and stable updater promotion
  ordering.

### Security

- **Credential and sync isolation** — Keep account sessions in the OS keychain;
  keep secrets, credential references, local paths, and runtime history out of
  Cloud Sync payloads; and preserve workspace ownership and local-secret
  boundaries during external applies.
- **Artifact trust and Store boundaries** — Keep updater private keys and R2
  credentials out of tracked files, require signed Standard updater artifacts,
  and prevent Store builds from registering or invoking the internal updater.

### Verification notes

- **Verified real-environment journeys** — Complete Windows install, launch,
  uninstall, and previous-Stable-to-new-Stable update; macOS arm64/x64 install
  and run; GitHub browser OAuth and Desktop callback/login; Creem Test checkout,
  webhook, entitlement, and billing portal; PostgreSQL and MySQL; SSH Terminal,
  SFTP, and SSH Tasks; and real Codex and Cursor MCP initialization, discovery,
  tool calls, and Unfour data/tool access.
- **Scoped follow-up verification** — Historical live multi-device Cloud Sync
  evidence exists, but the v0.9.0 unified-client multi-device regression
  remains `NOT VERIFIED` and will include single-device coverage. The first real
  Creem Production end-to-end transaction, MCP prod policy/confirmation,
  Linux AppImage runtime, real Microsoft Store/MSIX, and macOS Gatekeeper trust
  journeys also remain `NOT VERIFIED` without reducing the completed platform
  and Test-environment results.

## [0.8.0] - 2026-08-26

### Changed

- **Faster desktop startup** — Load feature modules on demand and pause
  inactive SSH and Database queries while preserving previously opened work.

### Fixed

- **Database query compatibility** — Improve PostgreSQL and MySQL/MariaDB
  execution for selected catalogs and commands that require the simple query
  protocol, including session, transaction, DDL, and `SHOW` statements.

## [0.7.0] - 2026-08-23

Feature release adding the connection sync-domain foundation for SSH and
database connections, while separating device-local saves from cloud-visible
mutations.

### Added

- **Connection sync-domain foundation** — Add revisioned connection snapshots,
  tombstones, external apply, shared domain contracts, workspace ownership
  validation, and command-bus transaction integration for SSH and Database
  connections. This is a local foundation for optional edition sync and does
  not enable a hosted sync service by itself.
- **Connection domain coverage** — Add command-bus coverage for local and
  external create, update, delete, no-op, workspace-boundary, and
  credential-cleanup flows.

### Changed

- **Device-local connection saves** — Keep local-only configuration and
  credential-reference changes out of cloud mutation streams while still
  allowing local activity to be recorded.
- **Connection CRUD paths** — Separate shared connection fields from
  device-local state for SSH and Database persistence, so revisions and sync
  status advance only when shared fields change.

### Security

- **Workspace-scoped connection handling** — Reject cross-workspace connection
  operations and keep credential references device-local, preserving compatible
  local credentials when external connection data is applied.

## [0.6.0] - 2026-08-21

Feature release adding the SSH task domain foundation for optional edition sync
and expanding MCP registry and response contracts, with workspace and
credential-scope hardening.

### Added

- **SSH task sync-domain foundation** — Add revisioned task and task-step
  snapshots, tombstones, external apply, workspace-delete cascade handling,
  and local-storage migrations. This is a local command-bus foundation and
  does not enable hosted sync by itself.
- **Connection-aware task entities** — Include connection context when
  listing SSH task entities so task-domain consumers can resolve their owning
  connection.
- **Ephemeral MCP registry mode** — Support an in-memory registry for MCP
  processes that should not persist registry state, with container packaging
  and registry smoke coverage.

### Changed

- **MCP response contracts** — Keep `structuredContent` aligned with declared
  `outputSchema` values and pass catalog context through `db.query_readonly`.
- **Workspace deletion consistency** — Cascade soft-delete tombstones to live
  API, Workspace, and SSH task children while preserving the caller-provided
  deletion timestamp.
- **SSH task persistence and transfer paths** — Preserve step-position and
  paging metadata and keep literal transfer paths device-local.
- **SSH terminal suggestions and history** — Stabilize command tracking and
  suggestion behavior.

### Fixed

- **Linux MCP build** — Enable the required Tokio macros so `unfour-mcp` builds
  on Linux.

### Security

- **Workspace-scoped database credentials** — Keep connection passwords and
  credential references scoped to the current workspace.

## [0.5.0] - 2026-08-18

Feature release adding persistent SSH command history and history-aware MCP and
terminal workflows, while extending the API domain foundation for optional
edition sync.

### Added

- **SSH command history and suggestions** — Persist workspace- and
  connection-scoped commands after remote echo, retain a bounded local history,
  and show prefix-based suggestions while typing at detected shell prompts.
  Arrow keys continue to reach the remote shell whenever the suggestion popup
  is closed.
- **MCP SSH history inspection** — Add the read-only
  `unfour.ssh.list_history` tool with workspace, connection, text, time-range,
  and result-limit filters so an agent can inspect recent commands and draft a
  reusable SSH task for explicit user confirmation.
- **API sync-domain coverage** — Add revisioned snapshots and external-apply
  handling for API collections, folders, and saved requests. This extends the
  local command-bus foundation for optional edition sync; it does not enable a
  hosted sync service by itself.

### Changed

- **Terminal history interaction** — Suggestions use an explicit popup:
  Up/Down selects an item, Tab or click inserts it, Escape dismisses it, and
  Enter always submits the user's current line.
- **Development hooks** — Install lefthook during `pnpm install` and format
  staged Rust and TypeScript files before commits.
- **Module responsibility splits** — Split oversized SSH task, MCP task,
  Workspace external-apply, Database controller, SSH Terminal, and API domain
  modules into focused files without changing their package ownership.

### Fixed

- **API external apply resilience** — Make collection, folder, and request
  apply behavior more robust around missing parents, rollback, OpenAPI import,
  and locally preserved redacted values.

### Security

- **History and API snapshot redaction** — Exclude password-prompt input and
  conservatively redact sensitive SSH commands. Redact credentials in API
  auth, headers, query parameters, URLs, JSON bodies, and form bodies while
  preserving existing local secrets when redacted snapshots are applied.

## [0.4.0] - 2026-08-11

Feature release extending the local MCP workflow with workspace variables and
SSH task automation.

### Added

- **MCP workspace variables and SSH tasks** — Add policy-aware workspace-global
  variable CRUD and complete SSH task management, execution, run inspection,
  cancellation, and cleanup tools over the existing command bus, including
  confirmation handshakes and LLM-facing masking.

### Changed

- **SSH Task workflow** — Add manual ordering and drag-and-drop task reordering,
  workspace/environment variable defaults, bounded live event handling, and
  cached run transcripts for more stable task execution.
- **Release and storage contracts** — Harden Stable/Test channel handling,
  release identity checks, and CI packaging safeguards for the NSIS sidecar
  lifecycle.

### Security

- **SSH task secret handling** — Mark task inputs as secret when requested and
  redact them from task output, errors, persisted logs, and MCP responses.

## [0.3.0] - 2026-07-30

Release candidate adding API request scripting and the transactional Workspace
domain foundation for future sync, together with desktop reliability and
workflow improvements.

### Added

- **API request scripts** — Save and run JavaScript before a request and after
  its response. Pre-request scripts can adjust the outgoing request and work
  with workspace or environment variables; post-response scripts can inspect
  responses, record tests, and write to a dedicated Console. Script status,
  timing, test results, and errors are shown in the response panel, and script
  definitions survive request persistence and OpenAPI import/export.
- **Transactional Workspace domain foundation** — Add revisioned Workspace,
  variable, and environment mutations; snapshots and tombstones; external
  apply support; and transaction-scoped hooks for edition-level sync
  composition. This is the local domain foundation and does not by itself
  enable a hosted sync service.
- **Isolated storage profiles** — Support stable, development, and test data
  roots, plus an explicit absolute data-directory override, while preserving
  the existing `~/.unfour` layout for stable installations without migration.
- **SSH terminal clipboard menu** — Add right-click actions for Copy, Paste,
  Paste Selected Text, and Select All, with platform-appropriate shortcuts.

### Changed

- **Workspace mutation consistency** — Route Workspace, variable, and
  environment writes through one transactional command path shared by desktop
  and MCP adapters. Active Workspace/environment selection, last-opened time,
  and default Workspace remain device-local preferences rather than sync
  mutations.
- **Edition extension surfaces** — Add app-shell hooks for Workspace actions,
  decorations, and variable decorations so edition-specific sync UI can
  integrate without moving business logic into the shell.

### Fixed

- **Windows installer with running MCP clients** — Detect and stop the
  `unfour-mcp` sidecar during NSIS install or uninstall, avoiding a stalled file
  replacement when an MCP client still holds the executable.
- **Database row actions** — Keep the row delete action visible when table
  editing becomes available after the initial grid layout.

## [0.2.0] - 2026-07-22

Minor release focused on SSH file transfer and task automation, shared workspace
variables, and multi-statement Database execution.

### Added

- **SSH SFTP remote files** — Browse remote directories, transfer files, and manage
  remote paths from a dedicated Files panel with context menus, multi-select, and
  drag-and-drop upload. The SSH sidebar adds Connections / Files / Tasks modes.
- **SSH Task automation** — Create and run multi-step SSH tasks (command, upload,
  download) with workspace-scoped templates, local path bindings, run history,
  streamed transcripts, and Save / Run editor UX. Run placeholders can prefill
  from the active workspace environment; executed commands are echoed in the run
  output.
- **Shared workspace variables** — Promote API environments to workspace-scoped
  variables with title-bar active-environment switching and a dedicated
  Environments editor (including dirty-leave confirmation). API request
  resolution overlays workspace defaults.
- **Database multi-statement Run** — Split editor SQL on semicolons and run
  Current / All statements sequentially, showing multiple result sets as
  sub-tabs.

### Fixed

- **Database table preview** — Stabilize table preview loading and remove
  placeholder loading rows that could flash incorrect grid content.

### Changed

- **API environments ownership** — Environment CRUD and storage move out of the
  API Client path into shared workspace variables; API Client consumes the
  shared workspace active environment.

### Docs

- Updated architecture docs for workspace variables package boundaries, data
  storage, and project structure.
- Updated SSH Terminal and API Client package docs for the new surfaces.

## [0.1.2] - 2026-07-20

Feature and reliability release focused on API interoperability, Database row
editing, and MCP/SSH stability.

### Added

- **OpenAPI collection export** — Export API collections as OpenAPI 3.1 from the
  collection toolbar, with shared dialog and tree actions in the API Client.
- **OpenAPI YAML import** — Import OpenAPI YAML into API collections through the
  http-engine OpenAPI import path.
- **Database table row editing** — Edit table rows with confirmation gating,
  optimistic concurrency checks, and bind-parameter SQL updates.
- **MCP API environment CRUD** — Manage API environments through MCP tools over
  the command bus.
- **Named secret operations** — Secret store supports named secret read/write
  helpers for credential-reference workflows.
- **Data grid UX** — Column resizing and JSON preview improvements in the shared
  data table / Database table grid.

### Fixed

- **SSH failed sessions** — Failed SSH session tabs and connection errors are
  preserved instead of being discarded silently.
- **MCP idle shutdown** — Idle shutdown is disabled by default so long-lived MCP
  clients are not interrupted unexpectedly.

### Changed

- **Database workspace controller** — Split Database page orchestration into
  dedicated hooks and connection/tree helpers while preserving existing query
  and schema flows.

### Refactored

- Split oversized backend modules into focused directories across
  `database-engine`, `http-engine` (api_client), `ssh-engine`,
  `unfour-command-bus`, and `unfour-mcp` tool handlers. Behavior is unchanged
  aside from the features listed above.
- Removed obsolete workspace implementation leftovers from the earlier
  workspace boundary cleanup.

### Docs

- Updated README screenshots and product overview copy for the current desktop
  modules.
- Documented MCP environment tools and idle-shutdown default in MCP docs.

## [0.1.1] - 2026-07-13

Maintenance and polish release following the 0.1.0 public launch.

### Added

- **Desktop extension slots** — The app shell now exposes module mount surfaces
  and extension slots (`packages/app-shell/src/extensions.ts`), enabling future
  pluggable desktop features without touching core layout code.
- **Release `core_commit` identity** — App system info and the About panel now
  surface the built `core_commit`, and the Community release identity config is
  unified across the build pipeline (`release.yml`, `build.rs`, `app.rs`).
- **Generic deep-link runtime support** — Deep links now resolve at runtime
  without hardcoded scheme handling.
- **i18n resource loading** — Extended the shared i18n provider to load
  additional resource bundles and added provider tests.

### Changed

- **Windows distribution** — The build now packages only the NSIS installer and
  drops the MSI requirement, simplifying the upgrade story (see
  `docs/release/distribution.md`).

### Fixed

- **Settings dialog** — Enlarged the settings window and removed the MCP tab
  height flash on open.

### Refactored

- **File-size discipline** — Split oversized source files into module
  directories across `unfour-core` (models), `unfour-mcp` (ssh tools),
  `workspace-engine`, `api-client`, `command-client` (types), and `packages/ui`
  (shell, tree-view). Behavior is unchanged; this improves maintainability and
  keeps the CI large-file gate green.
- **Shared styles** — Moved global styles out of `apps/desktop/src/styles.css`
  into dedicated `packages/app-shell/src/styles` modules (animations, host,
  index) and tightened the shared-token checks.

### Docs

- Marked API, SQLite, SSH, PostgreSQL, MySQL, and MCP release-verification
  checks as PASS.
- Updated README and distribution/release documentation to reflect the NSIS-only
  Windows packaging.

## [0.1.0] - 2026-07-09

First public release.

### Added

- **API Client** — Compose, send, save, and inspect HTTP requests with workspace
  environments and redacted history.
- **SSH Terminal** — Manage SSH connections and terminal sessions with split
  panes, host-key trust, and redacted log export.
- **Database** — Manage connections, browse schemas, run SQL with confirmation
  guardrails, and preview query results.
- **Workspace** — Scope requests, environments, connections, activity, tabs, and
  layout to a local workspace with unique names and per-workspace persistence.
- **Local MCP server** — Expose safe local diagnostic tools (API replay, SSH
  connection) to MCP clients over the command bus.
- **App shell & platform** — Single-instance app, settings window, structured
  local logs, centralized design tokens, and shared i18n.

### Security

- Credentials stored as references only; sensitive headers redacted in history,
  activity, and logs; keychain purged on connection delete; MCP tools reject
  forbidden write/control operations.

### Known limitations

- Signing is not yet complete; unsigned artifacts may trigger OS warnings.
- Windows distributes both NSIS `.exe` and MSI `.msi` for the same version. NSIS
  is recommended for ordinary users; MSI is available for MSI preference or
  software deployment management. Choose one format because installing both
  may create duplicate shortcuts or uninstall entries and confuse upgrades.
- Cross-format detection, automatic uninstall, and NSIS/MSI cross-upgrade are
  not implemented at this stage.
- macOS Apple Silicon and Intel packages are real-device verified, but remain
  unsigned and unnotarized; Gatekeeper may block them.
- Linux artifacts remain experimental/unverified until real-device smoke checks
  are complete.

[0.9.0]: https://github.com/zyqzyq/Unfour/releases/tag/v0.9.0
[0.8.0]: https://github.com/zyqzyq/Unfour/releases/tag/v0.8.0
[0.7.0]: https://github.com/zyqzyq/Unfour/releases/tag/v0.7.0
[0.6.0]: https://github.com/zyqzyq/Unfour/releases/tag/v0.6.0
[0.5.0]: https://github.com/zyqzyq/Unfour/releases/tag/v0.5.0
[0.4.0]: https://github.com/zyqzyq/Unfour/releases/tag/v0.4.0
[0.3.0-rc.1]: https://github.com/zyqzyq/Unfour/releases/tag/v0.3.0-rc.1
[0.2.0]: https://github.com/zyqzyq/Unfour/releases/tag/v0.2.0
[0.1.2]: https://github.com/zyqzyq/Unfour/releases/tag/v0.1.2
[0.1.1]: https://github.com/zyqzyq/Unfour/releases/tag/v0.1.1
[0.1.0]: https://github.com/zyqzyq/Unfour/releases/tag/v0.1.0
