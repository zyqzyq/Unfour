# unfour-command-bus

## Purpose

`unfour-command-bus` is the reusable Rust command entry point for manual UI
actions and future automated adapters.

## Boundaries

- Can orchestrate workspace, API, Database, SSH, credential, system-health, and
  read-only adapter commands through domain services.
- Should keep Tauri and MCP layers as thin adapters.
- Should leave low-level domain behavior in the owning engine crate when
  practical.
- Should not depend on frontend UI packages or expose raw secrets.

## Key Files

- `src/lib.rs` - `CommandBus`, read commands, safe connection summaries, and
  domain command methods.
- `Cargo.toml` - crate dependencies and `ssh-native` feature forwarding.

## Current Capabilities

- Workspace CRUD, active workspace, environment, and layout commands.
- API send, saved request, history, and collection read commands.
- Database connection, schema, query, and browse commands.
- SSH connection/session/log/host-key commands.
- Credential create, inspect, rotate, and delete commands.
- Safe read commands for MCP and future AI surfaces.

## Known Gaps

- Current package status is centralized in `docs/project/PACKAGE_STATUS.md`.
- New dangerous commands need explicit capability and confirmation policy before
  adapters expose them.

## Test / Verify

- `cargo test -p unfour-command-bus`
- `cargo check -p unfour-command-bus`
- `cargo check -p unfour-command-bus --features ssh-native`
- For adapter-facing changes, also verify the relevant Tauri or MCP path.
