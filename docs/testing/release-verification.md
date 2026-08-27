# Release Verification

This is the active verification matrix for the published Community release /
Preview `v0.8.0`. Results must come from this release commit; evidence from earlier
releases is historical context only. An artifact build is not proof of
platform, live-service, credential-store, or feature-level verification.

## Status Labels

- `PASS`: executed successfully for this release commit.
- `FAIL`: executed and failed.
- `NOT RUN`: automated check was not executed; include the reason.
- `NOT VERIFIED`: manual, platform, network, or live-service behavior was not
  verified.
- `N/A`: not applicable to the target platform or release artifact.

## Automated Checks

Run from the repository root and replace each placeholder with current
evidence.

| Area | Command | Required for v0.8.0 | Result |
| --- | --- | --- | --- |
| Working tree | `git status --short` | Yes | NOT RUN (release-preparation changes are present) |
| Patch hygiene | `git diff --check` | Yes | PASS (current working diff) |
| Dependency installation | `pnpm install --frozen-lockfile` | Yes | NOT RUN |
| Frontend build | `pnpm run build` | Yes | NOT RUN after pull (runner disk space insufficient) |
| Large-file guard | `pnpm run check:large-files` | Yes | PASS (0 blocking violations) |
| Frontend lint | `pnpm run lint` | Yes | NOT RUN after pull |
| Frontend unit tests | `pnpm run test` | Yes | NOT RUN after pull |
| Release/environment unit tests | `pnpm run test:release-env` | Yes | PASS (9 tests) |
| Playwright browser install | `pnpm exec playwright install chromium` | When required by the runner | NOT RUN |
| Playwright smoke | `pnpm run test:e2e` | Yes | NOT RUN |
| Rust workspace check | `pnpm run check:rust` | Yes | NOT RUN after pull (runner disk space insufficient) |
| Rust SSH feature check | `pnpm run check:rust:ssh` | Yes | NOT RUN after pull (runner disk space insufficient) |
| Aggregate repository check | `pnpm run check` | Yes | NOT RUN after pull (runner disk space insufficient) |
| Rust tests | `pnpm run test:rust` | Yes | NOT RUN |
| Windows NSIS bundle | `pnpm run tauri build` | Yes on Windows | NOT RUN |
| macOS Tauri bundles | `pnpm run tauri build` | Yes on macOS target | PASS (Apple Silicon and Intel packages are real-device verified; unsigned/not notarized) |
| Linux Tauri bundles | `pnpm run tauri build` | Yes on Linux target | NOT VERIFIED (requires target runner and real-device smoke) |

If `pnpm run check` is used, record its subcommands and separately record any
checks not included in the aggregate command.

## Platform Checks

| Platform | Required checks | Result |
| --- | --- | --- |
| Windows | NSIS install, launch, first viewport, upgrade from `v0.2.0`, running-`unfour-mcp` install/uninstall handling, credential behavior, quit/relaunch, and uninstall. | NOT VERIFIED |
| macOS package/launch | Bundle build, install/launch, and first viewport on Apple Silicon and Intel real devices. | PASS (real-device verified) |
| macOS Keychain | Keychain behavior on the target devices. | NOT VERIFIED |
| macOS signing/notarization | Apple signing, notarization, and Gatekeeper assessment. | NOT VERIFIED (artifacts are unsigned and not notarized) |
| Linux | Selected package build, install/launch, first viewport, Secret Service, and package-signing status. | NOT VERIFIED |

Platform checks that cannot be run must remain `NOT VERIFIED` with a reason.

## Feature And Live-Service Gates

| Gate | Required coverage for this candidate | Result |
| --- | --- | --- |
| API request scripts | Persist pre/post scripts; request and temporary-variable mutation; environment reads and writes; console output; passing/failing tests; pre-script failure/timeout; post-script failure; OpenAPI import/export round trip. | NOT VERIFIED |
| API sync domain foundation | API collection/folder/request snapshots; revision and tombstone behavior; redaction of auth, headers, query, URL, JSON, and form secrets; external apply ordering, local-secret preservation, rollback, and OpenAPI import interaction. This does not claim a hosted sync service. | NOT VERIFIED |
| Workspace domain foundation | Existing-workspace migration; Workspace/variable/environment CRUD; revision and tombstone behavior; transactional rollback; external apply; local active/default preferences remain local; desktop and MCP paths agree. | NOT VERIFIED |
| Connection sync domain foundation | SSH and Database connection snapshots; revision and tombstone behavior; external apply ordering; workspace ownership validation; device-local save separation; and credential cleanup/rollback. This does not claim a hosted sync service. | NOT VERIFIED |
| SSH task domain foundation | Task and step snapshots; revision and tombstone behavior; external apply ordering; workspace-delete cascades; connection-aware task listing; migrations; and local-secret preservation. This does not claim a hosted sync service. | NOT VERIFIED |
| Release/storage environment | Local Tauri dev defaults to Test; build defaults to Stable; build:test forces Test; invalid channel/profile values fail; Stable uses `~/.unfour`; `dev` and `test` use sibling roots; absolute override works; relative override is rejected; desktop and MCP resolve the same root. | NOT VERIFIED |
| SSH live server | Password/key auth, terminal input/output, resize, clipboard menu, SFTP, task automation, command-history persistence and suggestions, literal transfer paths, password-prompt exclusion, host-key checks, reconnect, and redacted log export. | NOT VERIFIED |
| Database | SQLite/PostgreSQL/MySQL connection and query flows, table edit/delete actions, multi-statement execution, workspace-scoped credential behavior, errors, and confirmation gates. | NOT VERIFIED |
| MCP | Initialize, tools/list, Workspace and environment operations, API reads, database read-only query and catalog context, activity list, SSH diagnostics, workspace-scoped redacted SSH history, output-schema alignment, ephemeral registry mode, and selected storage profile. | NOT VERIFIED |

Automated tests may support these gates but do not replace live server,
installer, operating-system, or credential-store verification.

## Documentation Checks

For release documentation changes:

```bash
git diff --check
```

Search active documentation for stale release identities and retired names:

```bash
rg -n "v?0[.]1[.]0|api-debugger|packages/terminal|@unfour/terminal|PROJECT_STATE|NEXT_STEPS|OPEN_ISSUES|DOCS_AUDIT" README.md README.zh-CN.md AGENTS.md docs --glob "!docs/archive/**" --glob "!docs/testing/release-verification.md"
```

Version-like values in protocol examples may remain when they identify the
example client rather than the Unfour release. Historical references inside
`docs/archive/` remain archived context.

## Release Evidence Template

```text
Release: v0.8.0
Commit: <release commit>
Platform: <runner or physical device>

Automated checks:
- pnpm install --frozen-lockfile: PASS / FAIL / NOT RUN
- pnpm run lint: PASS / FAIL / NOT RUN
- pnpm run test: PASS / FAIL / NOT RUN
- pnpm run test:release-env: PASS / FAIL / NOT RUN
- pnpm run check: PASS / FAIL / NOT RUN
- pnpm run check:rust:ssh: PASS / FAIL / NOT RUN
- pnpm run test:rust: PASS / FAIL / NOT RUN
- pnpm run test:e2e: PASS / FAIL / NOT RUN
- pnpm run tauri build: PASS / FAIL / NOT RUN

Manual checks:
- Windows NSIS install/upgrade/uninstall: PASS / FAIL / NOT VERIFIED
- running unfour-mcp installer handling: PASS / FAIL / NOT VERIFIED
- application startup and first viewport: PASS / FAIL / NOT VERIFIED
- API request scripts: PASS / FAIL / NOT VERIFIED
- API sync domain snapshots/external apply: PASS / FAIL / NOT VERIFIED
- Workspace domain migration and CRUD: PASS / FAIL / NOT VERIFIED
- SSH and Database connection sync domain snapshots/external apply: PASS / FAIL / NOT VERIFIED
- device-local connection saves and credential cleanup: PASS / FAIL / NOT VERIFIED
- SSH task domain snapshots/external apply: PASS / FAIL / NOT VERIFIED
- storage profile isolation: PASS / FAIL / NOT VERIFIED
- SSH Terminal/SFTP/tasks/clipboard/history suggestions: PASS / FAIL / NOT VERIFIED
- Database and row actions: PASS / FAIL / NOT VERIFIED
- MCP including SSH history: PASS / FAIL / NOT VERIFIED
- macOS installer smoke: PASS / FAIL / NOT VERIFIED
- Linux installer smoke: PASS / FAIL / NOT VERIFIED
- macOS signing/notarization: PASS / FAIL / NOT VERIFIED

Known unresolved risks:
- <failed, unverified, or accepted release risks>
```
