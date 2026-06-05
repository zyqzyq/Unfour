# Roadmap

## v0.1 Foundation

- Tauri 2 + React TypeScript app.
- Workspace shell.
- Command Bus.
- SQLite migrations.
- API request send/history/save.
- Workspace environments.
- Engineering docs.

Status: implemented. Workspace layout restore, selected resource slots, and resource-tree navigation are in place.

## v0.2 API Client

- Request collections and folders.
- Response headers/cookies/timing panels.
- Import/export without secrets.

Status: request execution, history, saved requests, environments, collections/folders, import/export, and richer response panels are implemented.

## v0.3 SSH

- Password and private-key login.
- Multi-session tabs.
- PTY resize and event streaming.
- Session logs.

Status: UI preview and backend boundary exist. Real session lifecycle is next.

## v0.4 Database

- PostgreSQL/MySQL/SQLite connection management.
- Schema browser.
- SQL execution.
- Paginated result grids.

Status: SQLite connection metadata, test, schema browsing, SQL execution, read-only table browsing, pagination, and result virtualization are implemented. PostgreSQL/MySQL live connections and controlled editing remain.

## v0.5 Hardening

- OS keychain/Stronghold.
- Capability review.
- Cross-platform packaging.
- User guide with screenshots.

Status: OS keychain-backed credential references are implemented with frontend attach/rotate/delete flows. Sync/AI boundaries, packaging, and screenshots remain.

## Later

- AI command adapter.
- Optional cloud sync.
- Workflow runner.
- Plugin model.
