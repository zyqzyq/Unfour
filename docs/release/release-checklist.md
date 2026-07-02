# Release Checklist

This checklist is for the first public `v0.1` release preparation.

## Release Candidate Setup

- Choose the release candidate commit.
- Confirm the working tree is clean before building release artifacts.
- Confirm version fields are correct:
  - root `package.json`;
  - `apps/desktop/src-tauri/tauri.conf.json`;
  - Rust crate versions if they are published or packaged.
- Review `README.md`, `docs/user/USER_GUIDE.md`, `SECURITY.md`, and
  `LICENSE`.
- Confirm release notes do not claim unverified checks.

## Required Verification

Complete and record the matrix in `docs/testing/release-verification.md`.

Minimum automated checks before public v0.1:

- `git status --short`
- `git diff --check`
- `pnpm run build`
- `pnpm run check:large-files`
- `pnpm run lint`
- `pnpm run test`
- `pnpm run test:e2e`
- `pnpm run check:rust`
- `pnpm run check:rust:ssh`
- `pnpm run test:rust`
- `pnpm run tauri build`

Minimum manual gates:

- API Client smoke.
- Database smoke.
- Live SSH smoke against a disposable server.
- MCP smoke.
- Installer launch on each release platform.
- Signing/notarization status recorded.

## Documentation

- README describes Unfour accurately and does not overclaim release readiness.
- Architecture docs are current:
  - `docs/architecture/package-boundaries.md`
  - `docs/architecture/project-structure.md`
  - `docs/architecture/data-storage.md`
  - `docs/architecture/security-model.md`
- MCP docs are current:
  - `docs/mcp/overview.md`
  - `docs/mcp/tools.md`
  - `docs/mcp/codex-setup.md`
- Testing docs are current:
  - `docs/testing/release-verification.md`
  - `docs/testing/manual-test-cases.md`
- Release docs are current:
  - `docs/release/distribution.md`
  - `docs/release/signing.md`

## Artifact Review

- Build artifacts are generated from the release candidate commit.
- Checksums are generated for every artifact.
- Artifact names include app name, version, platform, and architecture.
- Unsigned artifacts are clearly labeled in release notes.
- Install and first-launch behavior is verified on each target platform.
- No generated build output is committed unless explicitly required.

## Security Review

- No plaintext credential storage was introduced.
- Redaction policy covers API history, activity, terminal logs, and MCP results.
- Database mutation confirmation behavior is verified.
- SSH host-key trust behavior is verified against a live test host before
  claiming release readiness.
- MCP database and SSH tools reject forbidden write/control operations.

## Go / No-Go

Do not publish the release if any required verification is `FAIL`.

`NOT RUN` or `NOT VERIFIED` may be acceptable only if the release notes clearly
scope the limitation and the maintainer accepts the risk. For public v0.1, live
SSH verification and installer launch checks should be treated as release gates.
