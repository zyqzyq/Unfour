mod connection;
mod helpers;
mod operations;

use serde_json::json;

use self::connection::{ssh_create_connection, ssh_list_connections, ssh_run_diagnostic};
use self::operations::{ssh_exec, ssh_list_dir, ssh_patch_file, ssh_read_file, ssh_write_file};
use super::{RegisteredTool, ToolAnnotations, ToolDefinition};

const MAX_DIAGNOSTIC_TIMEOUT_MS: u64 = 60_000;
const MAX_ONE_SHOT_COMMAND_CHARS: usize = 4096;
const DEFAULT_FILE_LIMIT: u64 = 20 * 1024;
const MAX_FILE_LIMIT: u64 = 128 * 1024;

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.create_connection",
                title: "Create SSH Connection",
                description:
                    "Creates a saved SSH connection through the Unfour command bus. Optional secret input is stored in the OS credential store by the SSH engine and only a credential reference is persisted; the tool never returns the secret or credential reference.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "name": { "type": "string" },
                        "host": { "type": "string" },
                        "port": { "type": "integer", "minimum": 1, "maximum": 65535 },
                        "username": { "type": "string" },
                        "authKind": {
                            "type": "string",
                            "enum": ["password", "private-key", "none"]
                        },
                        "keyPath": { "type": "string" },
                        "credentialRef": { "type": "string" },
                        "secret": { "type": "string" }
                    },
                    "required": ["name", "host", "username", "authKind"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: ssh_create_connection,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.list_connections",
                title: "List SSH Connections",
                description:
                    "Lists saved SSH connections for a workspace through the Unfour command bus. Returns connection id, name, host, port, username, and environment; never returns passwords, private keys, passphrases, or credential references.",
                input_schema: json!({
                    "type": "object",
                    "properties": { "workspaceId": { "type": "string" } },
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_read(),
            },
            handler: ssh_list_connections,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.run_diagnostic",
                title: "Run SSH Diagnostic Command",
                description:
                    "Runs a single read-only diagnostic command on a saved SSH connection through the Unfour command bus and returns captured stdout/stderr. Safe in dev/test/prod for allowlisted diagnostics. For broader command execution use unfour.ssh.exec, which applies environment policy and high-risk confirmation.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "command": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "timeoutMs": { "type": "integer" }
                    },
                    "required": ["connectionId", "command"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_read(),
            },
            handler: ssh_run_diagnostic,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.exec",
                title: "Execute SSH Command",
                description:
                    "Executes one non-interactive SSH command on a saved connection. Use for dev/test repair loops after diagnostics identify a fix. Dev allows ordinary commands; test allows safe diagnostics and guarded commands; prod only allows read-only diagnostic commands. High-risk commands such as rm -rf, restart/shutdown, kill, docker/kubectl delete, and curl-pipe-shell require confirm with the returned confirmation_text.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "command": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "cwd": { "type": "string" },
                        "env": { "type": "object" },
                        "timeoutMs": { "type": "integer" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["connectionId", "command"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_action(),
            },
            handler: ssh_exec,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.read_file",
                title: "Read SSH File",
                description:
                    "Reads a bounded remote file slice or tail through SSH. Useful for logs and config inspection. Dev/test/prod allow ordinary reads; output is capped and line-redacted by the SSH engine. Sensitive paths may still be blocked by connection or OS permissions.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "path": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "offset": { "type": "integer" },
                        "limit": { "type": "integer" },
                        "tailLines": { "type": "integer" },
                        "timeoutMs": { "type": "integer" }
                    },
                    "required": ["connectionId", "path"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_read(),
            },
            handler: ssh_read_file,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.write_file",
                title: "Write SSH File",
                description:
                    "Writes or appends a remote file through SSH. Dev allows ordinary project paths; test and high-risk paths require confirmation; prod is blocked by policy. The returned result contains only path/mode/byte counts and command status, never file content.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "path": { "type": "string" },
                        "content": { "type": "string" },
                        "mode": { "type": "string", "enum": ["overwrite", "append", "create"] },
                        "workspaceId": { "type": "string" },
                        "timeoutMs": { "type": "integer" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["connectionId", "path", "content"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_action(),
            },
            handler: ssh_write_file,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.patch_file",
                title: "Patch SSH File",
                description:
                    "Applies a small search/replace patch to a remote file through SSH and returns a diff summary without file content. Dev allows single-match project-file patches; test, system paths, or multi-match replacements require confirmation; prod is blocked by policy.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "path": { "type": "string" },
                        "search": { "type": "string" },
                        "replace": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "timeoutMs": { "type": "integer" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["connectionId", "path", "search", "replace"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_action(),
            },
            handler: ssh_patch_file,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.list_dir",
                title: "List SSH Directory",
                description:
                    "Lists a bounded remote directory through SSH and returns structured entry summaries when the remote find utility is available. Safe in dev/test/prod.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "path": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "limit": { "type": "integer" },
                        "timeoutMs": { "type": "integer" }
                    },
                    "required": ["connectionId", "path"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_read(),
            },
            handler: ssh_list_dir,
        },
    ]
}

#[cfg(test)]
#[path = "ssh_tests.rs"]
mod tests;
