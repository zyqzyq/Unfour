# Release Verification

This is the release verification matrix for public `v0.1` preparation.

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

| Area | Command | Required before public v0.1 |
| --- | --- | --- |
| Working tree | `git status --short` | Yes |
| Patch hygiene | `git diff --check` | Yes |
| Frontend build | `pnpm run build` | Yes |
| Large-file guard | `pnpm run check:large-files` | Yes |
| Frontend lint | `pnpm run lint` | Yes, warnings must be reviewed |
| Frontend unit tests | `pnpm run test` | Yes |
| Playwright smoke | `pnpm run test:e2e` | Yes |
| Rust workspace check | `pnpm run check:rust` | Yes |
| Rust SSH feature check | `pnpm run check:rust:ssh` | Yes |
| Rust tests | `pnpm run test:rust` | Yes |
| Tauri release bundle | `pnpm run tauri build` | Yes, per target platform |

If `pnpm run check` is used, record the subcommands it ran and still record
any checks not included in that aggregate command.

## Platform Checks

| Platform | Required checks |
| --- | --- |
| Windows | Release bundle builds, installer launches cleanly, first viewport renders, OS credential create/read/delete works for release credential categories, uninstall does not leave a broken install state. |
| macOS | Bundle builds, app launches, notarization/signing status is recorded, Apple Keychain create/read/delete is verified, first viewport renders. |
| Linux | AppImage/deb or selected package builds, app launches, Linux Secret Service create/read/delete is verified, first viewport renders. |

Platform checks that cannot be run must be recorded as `NOT VERIFIED` with a
reason.

## Live-Service Gates

Some workflows require external services. Automated tests do not replace these
release checks.

| Gate | Required coverage |
| --- | --- |
| SSH live server | Password auth, private-key auth, passphrase credential path when supported, PTY input/output, resize, search, terminal history restore, host-key first trust, host-key mismatch rejection, fingerprint reset, keepalive, reconnect, close, log copy/export redaction. |
| SQLite | Connection creation, schema browse, read-only query, mutation confirmation, table browse, empty result, query error. |
| PostgreSQL | Connection test, schema browse, read-only query, mutation confirmation, table browse, invalid credential error, unavailable server error. Required when DB behavior or release claim covers PostgreSQL. |
| MySQL/MariaDB | Connection test, schema browse, read-only query, mutation confirmation, table browse, invalid credential error, unavailable server error. Required when DB behavior or release claim covers MySQL/MariaDB. |
| MCP | Initialize, tools/list, workspace read, API list/read, database list/read-only query, activity list, and SSH diagnostic when a live SSH connection is available. |

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
Release candidate:
Commit:
Platform:

Automated checks:
- git status --short: PASS/FAIL/NOT RUN
- git diff --check: PASS/FAIL/NOT RUN
- pnpm run build: PASS/FAIL/NOT RUN
- pnpm run lint: PASS/FAIL/NOT RUN
- pnpm run test: PASS/FAIL/NOT RUN
- pnpm run test:e2e: PASS/FAIL/NOT RUN
- pnpm run check:rust: PASS/FAIL/NOT RUN
- pnpm run check:rust:ssh: PASS/FAIL/NOT RUN
- pnpm run test:rust: PASS/FAIL/NOT RUN
- pnpm run tauri build: PASS/FAIL/NOT RUN

Manual checks:
- API Client:
- SSH Terminal:
- Database:
- Workspace:
- MCP:
- Installer:
- Signing/notarization:

Known unresolved risks:
```
