# Contributing to Unfour

Thanks for your interest in contributing! This guide covers how to set up the
project, the rules that keep the codebase healthy, and how to get a change
merged.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By
participating, you are expected to uphold it.

## Development Setup

Requirements:

- A modern stable Rust toolchain (verified with `rustc 1.96.0`).
- Node.js and [pnpm](https://pnpm.io).
- Tauri [system prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```bash
git clone https://github.com/zyqzyq/Unfour.git
cd Unfour
pnpm install
pnpm tauri dev
```

## Project Architecture

Unfour is split into clear boundaries — please read these before changing code:

- [`AGENTS.md`](AGENTS.md) — package boundary, backend, and command-bus rules.
- [`docs/architecture/package-boundaries.md`](docs/architecture/package-boundaries.md) — package boundaries in depth.
- [`docs/project/PACKAGE_STATUS.md`](docs/project/PACKAGE_STATUS.md) — current status of every package and crate.

Key rules in short:

- Frontend UI lives in React/TypeScript (`packages/*`, `apps/desktop`);
  execution and security boundaries live in Rust (`crates/*`).
- Feature business logic stays in its owning package or crate — not in
  `packages/app-shell` (composition only) or `packages/ui` (shared primitives only).
- Business actions route through the Rust command bus boundary.
- Every persisted business record carries a `workspace_id`.
- Credentials (SSH keys, DB / API passwords) are stored as references in the OS
  keychain — never as SQLite plaintext.
- User-visible UI copy uses the shared i18n provider and locale keys, not
  hardcoded strings.

## Making Changes

1. Fork the repo and create a topic branch off `main`.
2. Keep the change set small and focused; avoid unrelated refactors.
3. Add or update tests where it makes sense.
4. Run the checks below and make sure they pass.
5. Open a pull request describing the change and how you verified it.

## Verification

Run the relevant commands from the repository root before pushing:

```bash
pnpm run build
pnpm run lint
pnpm run test
pnpm run check:rust
pnpm run check:rust:ssh
pnpm run test:rust
```

For UI changes, also run the app locally and inspect the first viewport.

## Commit Convention

This project uses [Conventional Commits](https://www.conventionalcommits.org):

```
feat(database): add connection import
fix(ssh-terminal): handle resize before PTY is ready
docs(readme): clarify build steps
```

Common types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`. The scope is
usually the package or crate you touched.

## Reporting Bugs & Requesting Features

Use the [issue templates](https://github.com/zyqzyq/Unfour/issues/new/choose).
For security issues, follow [SECURITY.md](SECURITY.md) instead of opening a
public issue.
