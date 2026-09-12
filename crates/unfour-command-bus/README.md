# unfour-command-bus

## Purpose

`unfour-command-bus` is the reusable Rust command entry point for manual UI
actions and future automated adapters.

## Local constraints

See [AGENTS.md](AGENTS.md) for this crate's scope and invariants.

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

- Release readiness and current verification evidence live in `docs/release/`
  and `docs/testing/`.

## Test / Verify

Choose checks for the changed behavior using the
[verification guide](../../docs/agents/EXECUTION_PROTOCOL.md#choose-verification-by-impact).
The commands below are examples, not a checklist for every edit.

- `cargo test -p unfour-command-bus`
- `cargo check -p unfour-command-bus`
- `cargo check -p unfour-command-bus --features ssh-native`
- For adapter-facing changes, also verify the relevant Tauri or MCP path.
