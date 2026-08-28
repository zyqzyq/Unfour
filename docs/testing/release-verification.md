# v0.9.0 Release Verification

This matrix applies to the `v0.9.0` release candidate at source revision
`74c7270`, immediately after the published `v0.8.0`. It covers the unified
desktop/MCP runtime, account and Cloud Sync integration, and Standard/Store
distribution pipeline. `PASS` means executed on the recorded commit;
`NOT VERIFIED` is intentionally not inferred from builds or unit tests.
`NOT CLEAN` records a working-tree change that must be reviewed before creating
the release tag.

## Verification layers

```text
CI
  = unit tests / Rust checks / release contracts

Release Candidate
  = the same full verification plus real signed cross-platform Standard
    Tauri bundles, uploaded only as GitHub Actions artifacts

Release
  = the same reusable build core, then immutable R2 files, byte verification,
    GitHub Release, update-order gate, and stable/latest.json promotion last

Microsoft Store
  = independent manual Windows x64 MSIX build, validation, and submission
```

An RC workflow success is build evidence, not publication evidence. Installed
behavior, updater behavior, OS signing/notarization trust, and architecture
must still be recorded from its downloaded artifacts. The RC workflow does not
write a tag, GitHub Release, R2 object, `stable/latest.json`, or Store state.

## Automated evidence

| Area | Command | Current result |
| --- | --- | --- |
| Working tree baseline | `git status --short` before edits | NOT CLEAN (pre-existing user change in `apps/desktop/src-tauri/Cargo.toml`; release commit must be clean) |
| Patch hygiene | `git diff --check` | PASS (current documentation diff) |
| Version identity | `node scripts/sync-version.mjs --check` | PASS (`0.9.0`) |
| Release/distribution/RC contracts | `pnpm run test:release-env` | PASS (41 tests; shared signed build core, zero-publication RC policy, Store policy, and Linux AppImage contract covered) |
| Historical migration integrity | `node scripts/check-migrations.mjs` | PASS (18 files) |
| MSIX PowerShell syntax | PowerShell parser over `scripts/msix/*.ps1` | PASS (4 files) |
| Publishable-tree secret audit | `node scripts/audit-public-secrets.mjs` | PASS (1023 publishable files scanned; no secret values found) |
| Large-file guard | `pnpm run check:large-files` | PASS (0 blocking violations; 5 grandfathered files) |
| Shared-token guard | `pnpm run check:tokens` | PASS (107 shared tokens; no host redefinitions) |
| Frontend production build | TypeScript plus Vite build | PASS (2383 modules) |
| Frontend lint | ESLint | PASS (0 errors, 89 existing warnings) |
| Frontend unit tests | Vitest | PASS (108 files, 541 tests) |
| Browser smoke | Playwright Chromium | PASS (2 tests) |
| Rust workspace check | `cargo check --workspace` | PASS |
| Rust SSH feature check | `cargo check -p unfour --features ssh-native` | PASS |
| Rust workspace tests | `cargo test --workspace` | PASS (715 passed, 0 failed; one OS keychain smoke intentionally ignored) |
| Desktop account/update tests | `cargo test --workspace` | PASS (17 desktop tests; Store updater rejection covered) |
| Windows/macOS/Linux RC bundles | manual `Standard Release Candidate` run on the recorded commit | NOT VERIFIED |
| Windows Standard bundle/update | signed CI build and installed upgrade | NOT VERIFIED |
| Store MSIX build/validate/install | manual Windows candidate | NOT VERIFIED |

The local machine does not have `gitleaks`; the repository audit therefore
uses the tracked deterministic scanner. A separate full-history scanner should
still be run before changing repository visibility if Git history from another
repository is ever imported. This merge copies reviewed source snapshots and
does not import Unfour-pro Git history.

## Feature and migration matrix

| Gate | Required behavior | Result |
| --- | --- | --- |
| Local | API, real SSH, SQLite/PostgreSQL/MySQL, desktop MCP | NOT VERIFIED as a complete live matrix |
| Account | GitHub login, closed/running deep link, sign out | Automated account tests PASS; installed/live callback NOT VERIFIED |
| Cloud | entitlement, push/pull, conflicts, snapshots, second device | Automated domain tests PASS; live multi-device NOT VERIFIED |
| Standard | RC Windows/macOS bundles, Linux x64 AppImage + signature, identical formal GitHub/R2 hash, signed updater | Shared RC/Release contracts PASS; real candidate and published artifact NOT VERIFIED |
| Store | MSIX, validator, callback, MCP alias, no internal updater | Static contracts PASS; installed package NOT VERIFIED |
| Migration | old Community DB, old Pro DB, clean DB | PASS (9 storage migration tests, including exact old-Pro data preservation) |

## Commands for the final local rerun

```powershell
$env:CI = "true"
pnpm run check:version
pnpm run check:secrets
pnpm run check:migrations
pnpm run check:large-files
pnpm run check:tokens
pnpm run test:release-env
pnpm run build
pnpm run lint
pnpm run test
pnpm run check:rust
pnpm run check:rust:ssh
pnpm run test:rust
pnpm run test:e2e
```

Manual Store, updater, real-service, signing, and multi-device evidence must be
attached to the actual new version/tag candidate. The existing `v0.8.0` tag
predates the unified release implementation and cannot be reused; create and
verify a new `v0.9.0` tag only after the remaining gates pass.
