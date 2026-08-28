# Final merge release verification

This matrix applies to the current release candidate after Unfour-pro code was
migrated into the unified repository. `PASS` means executed on the recorded
commit; `NOT VERIFIED` is intentionally not inferred from builds or unit tests.

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
| Working tree baseline | `git status --short` before edits | PASS (clean at `1d0bb08`) |
| Patch hygiene | `git diff --check` | PASS |
| Version identity | `node scripts/sync-version.mjs --check` | PASS (`0.8.0`) |
| Release/distribution contracts | direct Node test suite | PASS (39 tests; shared RC/Release core, zero-publication RC policy, and Linux AppImage contract covered) |
| Historical migration integrity | `node scripts/check-migrations.mjs` | PASS (18 files) |
| MSIX PowerShell syntax | PowerShell parser over `scripts/msix/*.ps1` | PASS (4 files) |
| Publishable-tree secret audit | `node scripts/audit-public-secrets.mjs` | PASS (tracked and non-ignored candidate files; no secret values found) |
| Frontend production build | TypeScript plus Vite build | PASS (2383 modules) |
| Frontend lint | ESLint | PASS (0 errors, 89 existing warnings) |
| Frontend unit tests | Vitest | PASS (107 files, 539 tests) |
| Browser smoke | Playwright Chromium | PASS (2 tests) |
| Rust workspace check | `cargo check --workspace --offline` | PASS |
| Rust workspace tests | `cargo test --workspace --offline` | PASS (one OS keychain smoke intentionally ignored) |
| Desktop Standard policy | Stable `standard` desktop tests | PASS (17 tests) |
| Desktop Store policy | Stable `microsoft-store` desktop tests | PASS (17 tests) |
| RC/shared workflow contract | direct Node release contract suite | PASS (shared build core and zero-publication RC policy covered) |
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
| Account | GitHub login, closed/running deep link, sign out | NOT VERIFIED on candidate installers |
| Cloud | entitlement, push/pull, conflicts, snapshots, second device | Automated domain tests PASS; live multi-device NOT VERIFIED |
| Standard | RC Windows/macOS bundles, Linux x64 AppImage + signature, identical formal GitHub/R2 hash, signed updater | Shared RC/Release workflow contract present; real candidate and published artifact NOT VERIFIED |
| Store | MSIX, validator, callback, MCP alias, no internal updater | Static contract covered; installed package NOT VERIFIED |
| Migration | old Community DB, old Pro DB, clean DB | PASS (9 storage migration tests, including exact old-Pro data preservation) |

## Commands for the final local rerun

```powershell
$env:CI = "true"
pnpm run check:secrets
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
predates the unified release implementation and cannot be reused.
