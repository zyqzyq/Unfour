# Database Scope

Own connection settings/tree, schema browser, SQL editor/execution UI, query
results, table preview, and database-local query history/view state.

- Use `@unfour/command-client` for backend actions. Selected connection state
  may use `@unfour/workspace-core` as the documented transitional boundary.
- Keep API request, SSH session, and app-shell behavior out.
- Preserve backend SQL safety classification, confirmation/capability handling,
  and stable safety codes. UI execution must not bypass high-risk SQL gates.
