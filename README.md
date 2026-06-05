# Unfour Workspace

Unfour Workspace is a Tauri 2 desktop app for unified operations and development work: API debugging, SSH sessions, and database management inside one local-first workspace.

## Current Slice

- React + TypeScript + Vite frontend.
- shadcn-style UI primitives with Tailwind CSS.
- Workspace shell with API, SSH, and database tabs.
- Rust Command Bus with Workspace and API services.
- SQLite migrations for workspaces, API requests/history, connections, and local activity events.
- Workspace environment variables with `{{variable}}` resolution for API requests.
- Saved API requests can be grouped into folders, duplicated, deleted, and loaded back into the editor.
- Database connection metadata, SQLite connection test, SQLite schema browsing, and SQLite SQL execution.
- SSH connection metadata with credential references; live sessions remain reserved.
- AI and cloud sync extension points reserved.

## Commands

```bash
pnpm install
pnpm run build
pnpm run check
pnpm run test:rust
pnpm run tauri dev
```

Rust checks can be run from the repository root:

```bash
pnpm run check:rust
pnpm run check:rust:ssh
```

On Windows, Rust and Vite build steps write generated artifacts under `src-tauri/target` and `dist`.
When running inside a restricted automation sandbox, those steps may need permission to spawn helper processes
or write build artifacts.

The project currently expects a modern stable Rust toolchain. This workspace was verified with `rustc 1.96.0`.

## Documentation

- `AGENTS.md`: rules for coding agents.
- `docs/engineering`: architecture and implementation notes.
- `docs/engineering/progress.md`: current progress and next work slices.
- `docs/decisions`: ADRs.
- `docs/user/USER_GUIDE.md`: user-facing guide.
