<div align="center">

[English](README.md) · [简体中文](README.zh-CN.md)

# Unfour

**A local-first desktop workspace for backend developers that combines API debugging, SSH terminals, and database management — and exposes them to Codex and Cursor through a local MCP server.**

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![CI](https://github.com/zyqzyq/Unfour/actions/workflows/ci.yml/badge.svg)](https://github.com/zyqzyq/Unfour/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/zyqzyq/Unfour?include_prereleases&sort=semver)](https://github.com/zyqzyq/Unfour/releases)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB.svg)](https://tauri.app)

![Unfour overview](docs/screenshots/app-overview.png)

</div>

> [!WARNING]
> This source tree targets Unfour v0.9.0. Windows NSIS
> installers are unsigned and may trigger SmartScreen or other operating-system
> security warnings. Use `SHA256SUMS.txt` from the GitHub Release to verify
> downloaded files.

## Download

Download the [latest Unfour release](https://github.com/zyqzyq/Unfour/releases/latest)
from GitHub Releases.

- Windows is the primary distribution path: NSIS `.exe` installer. It is
  unsigned and may trigger SmartScreen.
- macOS has Apple Silicon and Intel packages that have been verified on real
  devices, but they are not Apple-signed or notarized; Gatekeeper may block
  them.
- Linux publishes an x64 AppImage only. It is Experimental; `.deb` and `.rpm`
  packages are not formally supported or published.
- Verify downloaded installers with the release `SHA256SUMS.txt` asset.

## What Is Unfour?

Unfour is a local-first desktop workspace for backend and operations work.
It keeps API requests, SSH connections, database connections, local activity,
and workspace layout in one local-first application, and exposes those
capabilities to Codex and Cursor through a local MCP server. The desktop
workbench and the local MCP server share the same command bus and the same
saved API, SSH, and database connections, so you and Codex or Cursor can
reproduce an issue, inspect logs and database state, and then make a fix in the
same troubleshooting loop. Troubleshooting is Unfour's core product loop.
Unfour does not ship an automatic troubleshooting playbook: you and Codex or
Cursor work through the steps together, using ready-made diagnostic tools
rather than a workflow runner.

Unfour is one application and one product. Its core desktop features are free
and open source under Apache-2.0. An active Pro subscription unlocks Cloud Sync
in the same application. Pro is an entitlement within Unfour, not a separate
client, package, repository, or release.

The app is built with Tauri 2, React, TypeScript, and Rust. The frontend owns
the workbench UI, while security-sensitive execution such as HTTP, SSH,
database drivers, local storage, and credential references lives behind Rust
capability crates and the command bus.

## Modules

- **API Client** - Compose and send HTTP requests, organize saved requests into
  collections and folders, resolve shared workspace variables, inspect response
  body/headers/cookies/timing, run saved pre-request and post-response scripts,
  review script tests and console output, and keep redacted history.
- **SSH Terminal** - Manage SSH connections and terminal sessions (split panes,
  search, clipboard context menu, persistent redacted command history and
  typing suggestions, host-key trust, redacted logs), browse and
  transfer remote files over SFTP, and automate multi-step SSH tasks (command,
  upload, download) from the Connections / Files / Tasks sidebar.
- **Database** - Manage database connections, browse schemas, run SQL with
  confirmation-aware safety checks (including multi-statement Run Current /
  Run All), preview and edit table rows, and review query output.
- **Workspace** - Scope saved requests, shared environments/variables,
  connections, activity, tabs, and layout state to a local workspace, with
  title-bar active-environment switching.
- **MCP integration for Codex and Cursor** - Expose safe local stdio diagnostic
  tools through the same command bus used by the desktop app. Codex and Cursor
  can use the same saved API, SSH, and database connections to reproduce
  issues, inspect logs and database state, and make a fix. The user and Codex
  or Cursor work through the steps together; Unfour does not ship an automatic
  troubleshooting playbook or workflow runner.

> [Connect Codex and Cursor to Unfour MCP →](docs/mcp/client-setup.md)

## Screenshots

**App overview — sidebar with module switcher and the API Client workspace**

![Unfour overview](docs/screenshots/app-overview.png)

**API Client — request builder with params, auth, headers, body, and response**

![API Client](docs/screenshots/api-client.png)

**SSH Terminal — connections, sessions, remote files, and tasks**

![SSH Terminal](docs/screenshots/ssh-terminal.png)

**Database — schema browsing and SQL query output**

![Database](docs/screenshots/database.png)

## Local Development

Requirements:

- Node.js and pnpm.
- A stable Rust toolchain.
- Tauri 2 prerequisites for your operating system.

Install and run:

```bash
pnpm install
pnpm tauri dev
```

`pnpm install` also installs Git hooks through lefthook. A commit formats staged
Rust files with `cargo fmt` and auto-fixes staged TypeScript with ESLint.
Skip once with `LEFTHOOK=0 git commit`.

Common commands:

```bash
pnpm tauri build        # create local Stable-channel Tauri bundles
pnpm tauri build:test   # create isolated Test-channel Tauri bundles
pnpm run build          # build the desktop frontend only
pnpm run check          # frontend build + Rust check + large-file check
pnpm run lint           # ESLint
pnpm run test           # frontend unit tests (Vitest)
pnpm run test:e2e       # Playwright smoke tests
pnpm run check:rust     # cargo check --workspace
pnpm run check:rust:ssh # cargo check with the ssh-native feature
pnpm run test:rust      # cargo test --workspace
pnpm run test:release-env # release/channel contract unit tests
```

Run commands from the repository root unless a package document says otherwise.
`pnpm tauri dev` defaults to the Test release channel, while local
`pnpm tauri build` defaults to Stable. Use `pnpm tauri build:test` for an
isolated Test-channel bundle. Set `UNFOUR_STORAGE_PROFILE=dev` when development
data should use `~/.unfour-dev`; this storage override is independent from
release identity. Only CI should create formal publishable Stable artifacts,
with `UNFOUR_RELEASE_CHANNEL=stable` and an exact `UNFOUR_BUILD_COMMIT`.

## Project Layout

| Path | Role |
| --- | --- |
| `apps/desktop` | Tauri/Vite desktop app entry and Tauri adapter layer. |
| `packages/app-shell` | Global shell composition and module mount slots. |
| `packages/api-client` | API Client frontend module. |
| `packages/ssh-terminal` | SSH Terminal frontend module. |
| `packages/database` | Database frontend module. |
| `packages/workspace-core` | Shared frontend workspace state. |
| `packages/workspace-environments` | Workspace environments and variables management UI. |
| `packages/workspace-local` | Reserved local workspace lifecycle boundary. |
| `packages/ui` | Shared UI primitives and stateless layout helpers. |
| `packages/command-client` | Typed Tauri command wrappers and frontend command types. |
| `crates/*` | Rust backend capability crates and adapters. |

See `docs/architecture/project-structure.md` for the full package and crate
map.

## Release Status

This source tree targets Unfour v0.9.0. Release
verification evidence is documented in:

- `docs/testing/release-verification.md`
- `docs/testing/manual-test-cases.md`
- `docs/release/release-checklist.md`
- `docs/release/distribution.md`
- `docs/release/signing.md`

Windows is the primary distribution path and ships an unsigned NSIS `.exe`
installer that may trigger SmartScreen. macOS has Apple Silicon and Intel
packages verified on real devices, but they are not Apple-signed or notarized
and Gatekeeper may block them. Linux publishes an x64 AppImage only and
remains Experimental. `.deb` and `.rpm` packages are not formally supported
or published. Use the release `SHA256SUMS.txt` to verify downloaded artifacts,
and do not claim a release check passes unless it was run successfully for the
target platform or is backed by current repository evidence.

## Documentation

- `AGENTS.md` - repository rules for coding agents.
- `docs/agents/START_HERE.md` - scoped onboarding path for AI agents.
- `docs/architecture/package-boundaries.md` - package ownership and forbidden
  dependency directions.
- `docs/architecture/project-structure.md` - repository, package, crate, and
  call-chain map.
- `docs/architecture/data-storage.md` - workspace data, SQLite, credential
  references, and local activity rules.
- `docs/architecture/diagnostics.md` - local structured logs, redaction,
  retention, diagnostic bundles, and developer logging guidance.
- `docs/architecture/security-model.md` - security posture, redaction, host-key
  policy, and dangerous-action rules.
- `docs/mcp/overview.md` and `docs/mcp/tools.md` - local MCP server behavior.
- `docs/mcp/client-setup.md` - installed-user setup for Codex and Cursor.
- `docs/testing/release-verification.md` - release verification matrix.
- `docs/release/release-checklist.md` - public release checklist.
- `docs/user/USER_GUIDE.md` - user-facing workflow guide.

## Contributing

Please read `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, and the package boundary
rules in `AGENTS.md` before opening a pull request.

Security issues should be reported through `SECURITY.md`, not a public issue.

## Support Unfour

If Unfour is useful to you, you can support its continued open-source development through [GitHub Sponsors](https://github.com/sponsors/zyqzyq).

Sponsorship is optional and does not include Unfour Pro or paid cloud services.

## Built with Codex & GPT-5.6

Codex was used to review the Rust and TypeScript architecture, implement and
refactor Tauri commands, add tests, and investigate build failures and MCP
process lifecycle issues.

GPT-5.6 helped analyze SSH and database permission boundaries, refine MCP tool
design, and plan the project architecture and release process.

The local Unfour MCP server lets Codex and Cursor use the same saved API, SSH,
and database connections for diagnostic inspection and fixes. It follows the
same command bus, workspace scope, credential handling, and confirmation
controls as the desktop app. Codex and Cursor can participate in the
troubleshooting loop through MCP, but connecting them does not automatically
run a complete root-cause playbook.

## License

Licensed under the [Apache License 2.0](LICENSE).
