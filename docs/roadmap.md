# Roadmap

## v0.1 Foundation

- Tauri 2 + React TypeScript app.
- Workspace shell.
- Command Bus.
- SQLite migrations.
- API request send/history/save.
- Workspace environments.
- Engineering docs.

Status: mostly implemented. Remaining foundation work is automated testing and workspace layout/tab restore.

## v0.2 API Client

- Request collections and folders.
- Response headers/cookies/timing panels.
- Import/export without secrets.

Status: request execution, history, saved requests, and environments are implemented. Collections/folders and richer response panels remain.

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

Status: SQLite connection metadata, test, schema browsing, and SQL execution are implemented. PostgreSQL/MySQL live connections, pagination polish, and controlled editing remain.

## v0.5 Hardening

- OS keychain/Stronghold.
- Capability review.
- Cross-platform packaging.
- User guide with screenshots.

Status: secret/sync/AI boundaries are reserved. Actual credential provider, packaging, and screenshots remain.

## Later

- AI command adapter.
- Optional cloud sync.
- Workflow runner.
- Plugin model.
