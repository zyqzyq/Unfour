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

[AGENTS.md](AGENTS.md) defines repository-wide architecture and security
constraints. Consult [the context map](docs/agents/START_HERE.md) when locating
an unfamiliar owner or domain reference, and the affected package/crate's local
instructions for its scope. Read the detailed boundary document when changing
ownership or dependency direction.

## Making Changes

1. Fork the repo and create a topic branch off `main`.
2. Keep the change set small and focused; avoid unrelated refactors.
3. Add or update tests where it makes sense.
4. Run the checks below and make sure they pass.
5. Open a pull request describing the change and how you verified it.

## Verification

Choose checks by the change's impact using the
[verification guide](docs/agents/EXECUTION_PROTOCOL.md#choose-verification-by-impact).
Documentation edits need diff and reference checks; local implementation changes
need affected tests/checks. Shared contracts and releases warrant broader coverage.
Record results and any unavailable verification in the pull request.

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
