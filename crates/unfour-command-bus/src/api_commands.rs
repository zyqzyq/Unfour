//! API command facade for the existing `CommandBus` API.
//!
//! Private child modules provide inherent methods on [`crate::CommandBus`];
//! callers keep the same methods without an extra forwarding or execution layer.
//! Authoring continues through `transaction`, while execution delegates to the
//! existing `api_routes`, `api_script_commands`, and `api_execution` modules.
//! Shared environment resolution remains in `workspace_variable_commands`.

mod collections;
mod environments;
mod execution;
mod requests;
