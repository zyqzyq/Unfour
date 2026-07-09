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

## v0.2 and Beyond

Likely follow-up areas:

- broader live SSH verification and platform hardening;
- database driver smoke coverage across supported engines;
- signed and notarized distribution;
- screenshots and fuller user documentation;
- query cancellation and richer database result interactions;
- SSH file-transfer or multiplexing exploration;
- optional AI/automation adapters over the command bus;
- optional cloud sync and plugin extension points.

## Release Rule

Do not claim a roadmap item is complete in release notes unless it is backed by
current verification or repository evidence.
