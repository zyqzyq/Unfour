# Roadmap

This roadmap is a product-direction summary. It is not release evidence. Use
`docs/testing/release-verification.md` and `docs/release/release-checklist.md`
for release readiness.

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
  external apply for optional edition sync foundations;
- shared connection domain contracts and command-bus transaction integration;
- device-local connection saves that do not create cloud mutations when shared
  connection fields are unchanged, while preserving local activity recording.

The connection domain work is a local command-bus foundation and does not by
itself provide a hosted sync service. Release claims remain limited by the
current verification matrix.

## v0.8.0 (Current published release)

Current status:

- Published Community release / Preview `v0.8.0`;
- automated, installer, platform, and live-service verification recorded in
  the active release matrix;
- continued platform hardening without expanding the current package or
  command-bus boundaries;
- MCP tools are provided over local stdio through the command bus and support
  the product's troubleshooting loop: Codex and Cursor can use saved API, SSH,
  and database connections to reproduce issues, inspect logs and database
  state, and act on the findings; Unfour does not ship an automatic
  troubleshooting playbook or workflow runner.

Release claims remain limited by the current verification matrix.

## v0.9.0 (Current release candidate)

Implemented areas:

- GitHub browser sign-in with PKCE, deep-link completion, OS-keychain desktop
  sessions, account entitlements, and billing destinations;
- Pro-gated, local-first Cloud Sync for workspaces, API trees, variables,
  connections, and SSH tasks, including retries, conflict resolution, remote
  workspace download, and recovery;
- one unified desktop/MCP runtime with shared storage, command-bus setup,
  account context, and an ephemeral registry mode for CI and smoke checks;
- one-click Codex and Cursor MCP configuration from Settings;
- signed in-app updates for Standard builds and a separately validated Windows
  x64 Microsoft Store MSIX distribution; and
- a reusable Release Candidate/Release pipeline for canonical Windows, macOS,
  and Linux AppImage artifacts, checksum manifests, and ordered updater
  promotion.

Current status:

- source version is `0.9.0`; the last published tag remains `v0.8.0`;
- current automated checks pass as recorded in
  `docs/testing/release-verification.md`; and
- candidate installer, OS trust, live account/billing, and multi-device Cloud
  Sync checks remain subject to manual verification before publication.

Release claims remain limited by the current verification matrix.

## Beyond v0.9

Likely follow-up areas:

- broader live SSH verification and platform hardening;
- database driver smoke coverage across supported engines;
- signed and notarized distribution;
- screenshots and fuller user documentation;
- query cancellation and richer database result interactions;
- optional AI/automation adapters over the command bus;
- hosted sync adapters and UI over the optional domain foundation;
- plugin extension points.

## Release Rule

Do not claim a roadmap item is complete in release notes unless it is backed by
current verification or repository evidence.
