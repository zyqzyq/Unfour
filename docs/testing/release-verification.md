# Release Verification

This is the release verification matrix for the `v0.1.0` public release.

Do not mark a verification item as passing unless the command or manual check
was run successfully for the release candidate, or the result is backed by
current repository evidence cited in the release notes.

## Status Labels

Use these labels consistently:

- `PASS`: executed successfully for this release candidate.
- `FAIL`: executed and failed.
- `NOT RUN`: not executed; include the reason.
- `NOT VERIFIED`: manual, platform, network, or live-service behavior was not
  verified.
- `N/A`: not applicable to the target platform or release artifact.

## Automated Checks

Run from the repository root.

| Area | Command | Required for v0.1.0 | Result |
| --- | --- | --- | --- |
| Working tree | `git status --short` | Yes | PASS (clean) |
| Patch hygiene | `git diff --check` | Yes | PASS |
| Frontend build | `pnpm run build` | Yes | PASS |
| Large-file guard | `pnpm run check:large-files` | Yes | PASS (pre-existing P1 warnings, non-blocking) |
| Frontend lint | `pnpm run lint` | Yes, warnings must be reviewed | PASS (0 errors, 40 warnings) |
| Frontend unit tests | `pnpm run test` | Yes | PASS (303 passed) |
| Playwright smoke | `pnpm run test:e2e` | Yes | NOT RUN (no Playwright browser provisioned in this environment) |
| Rust workspace check | `pnpm run check:rust` | Yes | PASS |
| Rust SSH feature check | `pnpm run check:rust:ssh` | Yes | PASS |
| Rust tests | `pnpm run test:rust` | Yes | PENDING |
| Tauri release bundle | `pnpm run tauri build` | Yes, per target platform | PENDING |

If `pnpm run check` is used, record the subcommands it ran and still record
any checks not included in that aggregate command.

## Platform Checks

| Platform | Required checks | Result |
| --- | --- | --- |
| Windows | Release bundle builds, installer launches cleanly, first viewport renders, OS credential create/read/delete works for release credential categories, uninstall does not leave a broken install state. | NOT VERIFIED (no release artifact / OS smoke in this environment) |
| macOS | Bundle builds, app launches, notarization/signing status is recorded, Apple Keychain create/read/delete is verified, first viewport renders. | NOT VERIFIED |
| Linux | AppImage/deb or selected package builds, app launches, Linux Secret Service create/read/delete is verified, first viewport renders. | NOT VERIFIED |

Platform checks that cannot be run must be recorded as `NOT VERIFIED` with a
reason.

## Live-Service Gates

Some workflows require external services. Automated tests do not replace these
release checks.

| Gate | Required coverage | Result |
| --- | --- | --- |
| SSH live server | Password auth, private-key auth, passphrase credential path when supported, PTY input/output, resize, search, terminal history restore, host-key first trust, host-key mismatch rejection, fingerprint reset, keepalive, reconnect, close, log copy/export redaction. | NOT VERIFIED (no live SSH host available) |
| SQLite | Connection creation, schema browse, read-only query, mutation confirmation, table browse, empty result, query error. | NOT VERIFIED (requires running app + SQLite; partially covered by unit tests) |
| PostgreSQL | Connection test, schema browse, read-only query, mutation confirmation, table browse, invalid credential error, unavailable server error. Required when DB behavior or release claim covers PostgreSQL. | NOT VERIFIED |
| MySQL/MariaDB | Connection test, schema browse, read-only query, mutation confirmation, table browse, invalid credential error, unavailable server error. Required when DB behavior or release claim covers MySQL/MariaDB. | NOT VERIFIED |
| MCP | Initialize, tools/list, workspace read, API list/read, database list/read-only query, activity list, and SSH diagnostic when a live SSH connection is available. | NOT VERIFIED |

## Documentation Checks

For release docs changes:

```bash
git diff --check
```

Also search active docs for stale names and retired progress docs:

```bash
rg -n "api-debugger|packages/terminal|@unfour/terminal|PROJECT_STATE|NEXT_STEPS|OPEN_ISSUES|DOCS_AUDIT" README.md AGENTS.md docs --glob "!docs/archive/**"
```

Historical references inside `docs/archive/` may remain as archived context.

## Release Evidence Template

Record release evidence in release notes or a release-candidate checklist:

```text
Release candidate: v0.1.0
Commit: aeeed0e (release commit to be set when docs are committed)
Platform: Windows x64 (primary build target in this environment)

Automated checks:
- git status --short: PASS (clean)
- git diff --check: PASS
- pnpm run build: PASS
- pnpm run lint: PASS (0 errors, 40 warnings — pre-existing style warnings reviewed)
- pnpm run test: PASS (303 passed)
- pnpm run test:e2e: NOT RUN (no Playwright Chromium browser provisioned: ~/.cache/ms-playwright absent; run `npx playwright install chromium` then re-run)
- pnpm run check:rust: PASS
- pnpm run check:rust:ssh: PASS
- pnpm run test:rust: PENDING
- pnpm run tauri build: PENDING

Manual checks:
- API Client: NOT VERIFIED
- SSH Terminal: NOT VERIFIED (no live SSH host)
- Database: NOT VERIFIED (no live DB engines)
- Workspace: NOT VERIFIED
- MCP: NOT VERIFIED
- Installer: NOT VERIFIED
- Signing/notarization: NOT VERIFIED (unsigned; see docs/release/signing.md)

Known unresolved risks:
- Signing/notarization incomplete; unsigned artifacts may trigger OS warnings.
- Playwright e2e, live SSH/DB gates, and per-platform installer smoke not run in this environment.
```
