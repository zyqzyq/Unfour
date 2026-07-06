# Diagnostics

Unfour's first diagnostics layer is local-first, redacted, and intended for
support troubleshooting. It is not an enterprise audit log and must not become
a place for storing raw secrets, request payloads, SQL results, terminal
buffers, or MCP arguments.

## Ownership

- `crates/unfour-diag` owns structured file logging, retention pruning,
  diagnostic bundle export, correlation ID helpers, and diagnostics metadata.
- `crates/unfour-paths` owns runtime directory resolution for the desktop app
  and standalone MCP server.
- `crates/unfour-core` owns shared redaction helpers used by persistence and
  diagnostics.
- `packages/command-client/src/logger.ts` is the frontend logging boundary.
  Frontend packages should use this wrapper instead of direct `console.*`
  calls for diagnostic events.
- `apps/desktop/src-tauri` and `crates/unfour-mcp` are adapters that initialize
  diagnostics; business capability crates emit module events through
  `unfour-diag`.

## Runtime Files

Runtime paths are resolved by `crates/unfour-paths` under the stable Unfour
data root `~/.unfour`. The desktop app and standalone MCP server use the same
path resolver so logs, diagnostics, backups, config, cache, and SQLite stay in
one predictable location.

- SQLite: `~/.unfour/unfour.sqlite`
- logs: `~/.unfour/logs`
- diagnostics bundles: `~/.unfour/diagnostics`
- backups: `~/.unfour/backups`
- config: `~/.unfour/config`
- cache: `~/.unfour/cache`

File logs use daily rolling files named `unfour.log*`. The default retention is
7 days. Retention pruning runs during diagnostics initialization and only
removes old files whose names start with `unfour.log`.

The diagnostic bundle command creates a new directory under
`~/.unfour/diagnostics`, writes a `manifest.json`, and copies the most recent
local log files. It must not copy the SQLite database, keychain material,
workspace exports, terminal buffers, or raw feature payloads.

## Event Model

Logs are JSON records emitted through Rust `tracing`. Common fields are:

- `event`
- `module`
- `operation`
- `status`
- `duration_ms`
- `error_kind`
- `command_id` for traced Tauri commands
- `request_id` for request-style operations such as API sends and MCP tool
  calls

Diagnostics metadata includes edition, channel, package kind, version, and
commit when available. OSS development builds default to debug-level logging;
release builds default to info-level logging. Future Pro or Team builds should
reuse the same crate and set edition/channel/package metadata from packaging
configuration instead of introducing a parallel diagnostics stack.

## Redaction Rules

Diagnostic fields must be sanitized before logging. The shared sensitive-key
policy covers:

- `authorization`
- `cookie`
- `proxy-authorization`
- `x-api-key`
- `x-auth-token`
- `password`
- `passwd`
- `token`
- `access_token`
- `refresh_token`
- `secret`
- `private_key`
- `api_key`
- `license_key`

URL query parameters and semicolon-style connection-string secrets with these
keys are redacted. Path values should use `safe_path_display` when the full
absolute path is not necessary.

Module code must not log:

- HTTP headers, request bodies, or response bodies
- raw SQL text, query parameters, or result rows
- SSH terminal input/output or private-key passphrases
- MCP tool arguments
- keychain credential refs when they are not needed for troubleshooting

## Developer Guidance

For Tauri commands, wrap high-value commands with the command tracing helper in
`crates/unfour-app/src/commands.rs` so start, success, failure, duration, and
stable `AppError` codes are recorded consistently.

For engine and storage code, use a small started/completed/failed sequence
around external or stateful boundaries:

- API request send
- database connection and query execution
- SSH connect, resize, and close
- SQLite initialization and migrations
- keychain create, read, rotate, and delete
- MCP tool calls

For frontend code, call `logFrontendEvent` or rely on the shared command-client
`invoke` wrapper. Logging failures must never break user workflows.

The desktop Command Palette exposes user-facing diagnostics actions:

- Open Log Directory
- Open Diagnostics Directory
- Export Diagnostics Bundle
