# Changelog

This file is the user-facing change history for Unfour, following
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html)

## [Unreleased]

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

[0.1.0]: https://github.com/zyqzyq/Unfour/releases/tag/v0.1.0
