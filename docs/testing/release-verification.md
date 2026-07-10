# Release Verification

This is the release verification matrix for the published `v0.1.0`
release. Results below reflect the current repository state and the checks
run for this release follow-up. Do not treat an artifact build as proof of
platform, live-service, or credential-store verification.

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
| Working tree | `git status --short` | Yes | NOT RUN (working tree contains this task's changes; not a clean release candidate) |
| Patch hygiene | `git diff --check` | Yes | PASS (only line-ending normalization warnings) |
| Dependency installation | `pnpm install --frozen-lockfile` | Yes | PASS (lockfile up to date) |
| Frontend build | `pnpm run build` | Yes | PASS (executed inside `pnpm run check`) |
| Large-file guard | `pnpm run check:large-files` | Yes | FAIL (6 critical, 5 P0, and 14 P1 findings; 5 blocking findings) |
| Frontend lint | `pnpm run lint` | Yes, warnings must be reviewed | PASS (0 errors, 40 warnings) |
| Frontend unit tests | `pnpm run test` | Yes | PASS (58 files, 303 tests) |
| Playwright browser install | `pnpm exec playwright install chromium` | When E2E is enabled | PASS |
| Playwright smoke | `pnpm run test:e2e` | Yes | FAIL (2 smoke tests failed on existing UI selectors) |
| Rust workspace check | `pnpm run check:rust` | Yes | PASS (executed inside `pnpm run check`) |
| Rust SSH feature check | `pnpm run check:rust:ssh` | Yes | PASS |
| Aggregate repository check | `pnpm run check` | Yes | FAIL (build and Rust check passed; large-file guard failed) |
| Rust tests | `pnpm run test:rust` | Yes | PASS (all executed tests passed; platform keychain test ignored) |
| Windows NSIS + MSI bundles | `pnpm run tauri build` with `bundle.targets: "all"` | Yes | PASS (clean output produced `target/release/bundle/nsis/Unfour_0.1.0_x64-setup.exe` and `target/release/bundle/msi/Unfour_0.1.0_x64_en-US.msi`) |
| Explicit multi-bundle CLI experiment | `pnpm run tauri build --bundles nsis,msi` | Informational | FAIL (pnpm/PowerShell forwarded `nsis msi` as one invalid value; workflow intentionally relies on the tested shared `bundle.targets: "all"` configuration) |
| macOS/Linux Tauri release bundle | `pnpm run tauri build` | Yes, per target platform | NOT VERIFIED (no macOS/Linux release runner) |

If `pnpm run check` is used, record the subcommands it ran and still record
any checks not included in that aggregate command.

## Platform Checks

| Platform | Required checks | Result |
| --- | --- | --- |
| Windows | Both NSIS/MSI release bundles, installer launch, first viewport, OS credential behavior, uninstall and upgrade. | PARTIAL: PASS (user-reported Windows NSIS install and app launch); NOT VERIFIED (MSI install and remaining Windows smoke not run here) |
| Windows shortcut diagnosis | Standalone NSIS one shortcut, standalone MSI one shortcut, both formats causing two icons. | NOT VERIFIED for standalone counts; PASS for user-reported NSIS+MSI duplicate icons and source search finding no project-owned duplicate shortcut code |
| macOS | Bundle builds, app launches, notarization/signing status, Apple Keychain, first viewport. | NOT VERIFIED (no real macOS device) |
| Linux | AppImage/deb or selected package builds, app launches, Secret Service, first viewport. | NOT VERIFIED (no real Linux device) |

Platform checks that cannot be run must be recorded as `NOT VERIFIED` with a
reason.

## Live-Service Gates

Some workflows require external services. Automated tests do not replace these
release checks.

| Gate | Required coverage | Result |
| --- | --- | --- |
| SSH live server | Password auth, private-key auth, passphrase credential path when supported, PTY input/output, resize, search, terminal history restore, host-key first trust, host-key mismatch rejection, fingerprint reset, keepalive, reconnect, close, log copy/export redaction. | PASS (manual verification; user-reported live SSH behavior for this candidate) |
| SQLite | Connection creation, schema browse, read-only query, mutation confirmation, table browse, empty result, query error. | PASS (manual verification; user-reported running-app behavior for this candidate; partially covered by unit tests) |
| PostgreSQL | Connection test, schema browse, read-only query, mutation confirmation, table browse, invalid credential error, unavailable server error. Required when DB behavior or release claim covers PostgreSQL. | PASS (manual verification; user-reported live PostgreSQL behavior for this candidate) |
| MySQL/MariaDB | Connection test, schema browse, read-only query, mutation confirmation, table browse, invalid credential error, unavailable server error. Required when DB behavior or release claim covers MySQL/MariaDB. | PASS (manual verification; user-reported live MySQL/MariaDB behavior for this candidate) |
| MCP | Initialize, tools/list, workspace read, API list/read, database list/read-only query, activity list, and SSH diagnostic when a live SSH connection is available. | PASS (manual verification; user-reported live MCP behavior for this candidate) |

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
Commit: 7b5a499 (working tree includes this documentation follow-up)
Platform: Windows host used for local checks; clean NSIS and MSI bundles rebuilt here

Automated checks:
- pnpm install --frozen-lockfile: PASS
- pnpm run build: PASS (as part of `pnpm run check`)
- pnpm run lint: PASS (0 errors, 40 warnings)
- pnpm run test: PASS (58 files, 303 tests)
- pnpm exec playwright install chromium: PASS
- pnpm run test:e2e: FAIL (2 existing selector assertions failed)
- pnpm run check:rust: PASS (as part of `pnpm run check`)
- pnpm run check:rust:ssh: PASS
- pnpm run check: FAIL (large-file guard blocked the aggregate check)
- pnpm run test:rust: PASS (all executed tests passed; OS keychain test ignored)
- pnpm run tauri build: PASS (clean Windows output contains both NSIS and MSI)
- explicit `--bundles nsis,msi` experiment: FAIL (argument forwarding issue; not used by workflow)
- YAML lint for `.github/workflows/release.yml`: PASS

Manual checks:
- Windows NSIS install: PASS (user-reported)
- Windows MSI install: NOT VERIFIED (no standalone MSI install evidence)
- Application startup: PASS (user-reported after NSIS install)
- NSIS + MSI duplicate shortcuts: PASS (user-reported observation)
- Standalone NSIS one shortcut: NOT VERIFIED
- Standalone MSI one shortcut: NOT VERIFIED
- API Client: PASS (manual verification; user-reported live API behavior for this candidate)
- SSH Terminal: PASS (manual verification; user-reported live SSH behavior for this candidate)
- Database: PASS (manual verification; SQLite, PostgreSQL, and MySQL/MariaDB all user-reported live for this candidate)
- Workspace: NOT VERIFIED as a release manual smoke
- MCP: PASS (manual verification; user-reported live MCP behavior for this candidate)
- Windows uninstall/upgrade: NOT VERIFIED
- macOS/Linux installer smoke: NOT VERIFIED
- Signing/notarization: NOT VERIFIED; installers are unsigned and may trigger SmartScreen

Known unresolved risks:
- `pnpm run check` is currently blocked by the repository's existing large-file guard findings.
- Playwright smoke currently fails on two existing selectors and therefore blocks the release workflow.
- macOS/Linux release bundle generation and real-device smoke remain unverified here.
- Signing/notarization and system credential-store checks are not complete.
```
