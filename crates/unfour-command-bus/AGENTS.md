# Command Bus Scope

Own the reusable Rust command entry point for Workspace, API, Database, SSH,
credentials, system health, and read-only adapter operations used by Tauri,
MCP, and future AI/CLI surfaces.

- Orchestrate domain services here; capability logic stays in owning engines.
  Do not depend on frontend packages or Tauri-specific UI concerns.
- Keep adapter paths consistent in workspace scoping, credential references,
  redaction, activity-log safety, and `AppError` handling.
- New dangerous commands need an explicit confirmation/capability policy before
  adapters expose them. Adapters must not bypass that policy or expose raw secrets.
