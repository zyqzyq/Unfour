# unfour-command-bus Agent Rules

## Scope

`crates/unfour-command-bus` owns the reusable Rust command entry point for
workspace, API, Database, SSH, credential, system health, and read-only adapter
operations used by Tauri, MCP, and future AI/CLI surfaces.

## Boundaries

- Tauri commands and MCP tools should remain adapters over this crate.
- This crate may orchestrate domain services, but domain-specific capability
  logic should stay in the owning engine crate when practical.
- It must not depend on frontend UI packages or Tauri-specific UI concerns.
- It must not expose raw secrets or bypass credential-reference boundaries.

## Rules

- Do not make unrelated cross-crate or cross-package changes.
- Prefer existing service interfaces, models, and `AppError` handling.
- Do not introduce new dependencies unless the task explicitly requires them.
- Preserve `workspace_id` scoping for persisted business records.
- Preserve redaction and activity-log safety for API, SSH, DB, credential, MCP,
  and future AI paths.
- New dangerous commands need an explicit confirmation/capability policy.

## Required Output

After changes here, report:

- Modified files
- Behavior changed
- Validation commands run
- Manual verification needed
- Follow-up risks
