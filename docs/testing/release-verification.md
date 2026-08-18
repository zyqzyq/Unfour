# Release Verification

This is the active verification matrix for the Community Stable `v0.5.0`
release. Results must come from this release commit; evidence from earlier
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

| Area | Command | Required for v0.5.0 | Result |
| --- | --- | --- | --- |
| Working tree | `git status --short` | Yes | NOT RUN (release-preparation changes are present) |
| Patch hygiene | `git diff --check` | Yes | PASS (documentation update) |
| Dependency installation | `pnpm install --frozen-lockfile` | Yes | NOT RUN |
| Frontend build | `pnpm run build` | Yes | NOT RUN |
| Large-file guard | `pnpm run check:large-files` | Yes | NOT RUN |
| Frontend lint | `pnpm run lint` | Yes | NOT RUN |
| Frontend unit tests | `pnpm run test` | Yes | NOT RUN |
| Release/environment unit tests | `pnpm run test:release-env` | Yes | PASS (9 tests) |
| Playwright browser install | `pnpm exec playwright install chromium` | When required by the runner | NOT RUN |
| Playwright smoke | `pnpm run test:e2e` | Yes | NOT RUN |
| Rust workspace check | `pnpm run check:rust` | Yes | NOT RUN |
| Rust SSH feature check | `pnpm run check:rust:ssh` | Yes | NOT RUN |
| Aggregate repository check | `pnpm run check` | Yes | NOT RUN |
| Rust tests | `pnpm run test:rust` | Yes | NOT RUN |
| Windows NSIS bundle | `pnpm run tauri build` | Yes on Windows | NOT RUN |
| macOS/Linux Tauri bundles | `pnpm run tauri build` | Yes on each target | NOT VERIFIED (requires target runners and real-device smoke) |

If `pnpm run check` is used, record its subcommands and separately record any
checks not included in the aggregate command.

## Platform Checks

| Platform | Required checks | Result |
| --- | --- | --- |
| Windows | NSIS install, launch, first viewport, upgrade from `v0.2.0`, running-`unfour-mcp` install/uninstall handling, credential behavior, quit/relaunch, and uninstall. | NOT VERIFIED |
| macOS | Bundle build, install/launch, first viewport, Keychain, signing, and notarization status. | NOT VERIFIED |
| Linux | Selected package build, install/launch, first viewport, Secret Service, and package-signing status. | NOT VERIFIED |

Platform checks that cannot be run must remain `NOT VERIFIED` with a reason.

## Feature And Live-Service Gates

| Gate | Required coverage for this candidate | Result |
| --- | --- | --- |
| API request scripts | Persist pre/post scripts; request and temporary-variable mutation; environment reads and writes; console output; passing/failing tests; pre-script failure/timeout; post-script failure; OpenAPI import/export round trip. | NOT VERIFIED |
| API sync domain foundation | API collection/folder/request snapshots; revision and tombstone behavior; redaction of auth, headers, query, URL, JSON, and form secrets; external apply ordering, local-secret preservation, rollback, and OpenAPI import interaction. This does not claim a hosted sync service. | NOT VERIFIED |
| Workspace domain foundation | Existing-workspace migration; Workspace/variable/environment CRUD; revision and tombstone behavior; transactional rollback; external apply; local active/default preferences remain local; desktop and MCP paths agree. | NOT VERIFIED |
| Release/storage environment | Local Tauri dev defaults to Test; build defaults to Stable; build:test forces Test; invalid channel/profile values fail; Stable uses `~/.unfour`; `dev` and `test` use sibling roots; absolute override works; relative override is rejected; desktop and MCP resolve the same root. | NOT VERIFIED |
| SSH live server | Password/key auth, terminal input/output, resize, clipboard menu, SFTP, task automation, command-history persistence and suggestions, password-prompt exclusion, host-key checks, reconnect, and redacted log export. | NOT VERIFIED |
| Database | SQLite/PostgreSQL/MySQL connection and query flows, table edit/delete actions, multi-statement execution, errors, and confirmation gates. | NOT VERIFIED |
| MCP | Initialize, tools/list, Workspace and environment operations, API reads, database read-only query, activity list, SSH diagnostics, workspace-scoped redacted SSH history, and selected storage profile. | NOT VERIFIED |

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
Release: v0.5.0
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
- storage profile isolation: PASS / FAIL / NOT VERIFIED
- SSH Terminal/SFTP/tasks/clipboard/history suggestions: PASS / FAIL / NOT VERIFIED
- Database and row actions: PASS / FAIL / NOT VERIFIED
- MCP including SSH history: PASS / FAIL / NOT VERIFIED
- macOS/Linux installer smoke: PASS / FAIL / NOT VERIFIED
- signing/notarization: PASS / FAIL / NOT VERIFIED

Known unresolved risks:
- <failed, unverified, or accepted release risks>
```
