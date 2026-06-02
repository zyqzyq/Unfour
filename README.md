# Unfour Workspace

Unfour Workspace is a Tauri 2 desktop app for unified operations and development work: API debugging, SSH sessions, and database management inside one local-first workspace.

## Current Slice

- React + TypeScript + Vite frontend.
- shadcn-style UI primitives with Tailwind CSS.
- Workspace shell with API, SSH, and database tabs.
- Rust Command Bus with Workspace and API services.
- SQLite migrations for workspaces, API requests/history, connections, and audit events.
- Workspace environment variables with `{{variable}}` resolution for API requests.
- Saved API requests can be loaded back into the editor.
- Database connection metadata, SQLite connection test, SQLite schema browsing, and SQLite SQL execution.
- AI and cloud sync extension points reserved.

## Commands

```bash
pnpm install
pnpm run build
pnpm run tauri dev
```

Rust checks should be run from `src-tauri`:

```bash
cargo check
cargo check --features ssh-native
```

The project currently expects a modern stable Rust toolchain. This workspace was verified with `rustc 1.96.0`.

## Documentation

- `AGENTS.md`: rules for coding agents.
- `docs/engineering`: architecture and implementation notes.
- `docs/engineering/progress.md`: current progress and next work slices.
- `docs/decisions`: ADRs.
- `docs/user/USER_GUIDE.md`: user-facing guide.
