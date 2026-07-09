use serde_json::{json, Map, Value};
use unfour_core::models::{CredentialCreateInput, DatabaseConnectionInput};

use crate::command_bus_adapter::CommandBusAdapter;

use super::super::policy::ToolPolicyEvaluation;
use super::super::{
    object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition,
};
use super::{
    parse_optional_bool, parse_optional_string, parse_required_string, resolve_workspace_id,
    safe_connection_summary,
};

pub(super) fn registered_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "unfour.db.create_connection",
            title: "Create Database Connection",
            description:
                "Creates a saved database connection through the Unfour command bus. Optional password input is stored in the OS credential store and only a credential reference is persisted; the tool never returns the password or credential reference.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "name": { "type": "string" },
                    "driver": { "type": "string", "enum": ["sqlite", "postgres", "mysql"] },
                    "host": { "type": "string" },
                    "port": { "type": "integer", "minimum": 1, "maximum": 65535 },
                    "database": { "type": "string" },
                    "username": { "type": "string" },
                    "sslMode": {
                        "type": "string",
                        "enum": ["disable", "prefer", "require", "verify-ca", "verify-full"]
                    },
                    "sqlitePath": { "type": "string" },
                    "credentialRef": { "type": "string" },
                    "password": { "type": "string" },
                    "credentialLabel": { "type": "string" },
                    "readOnly": { "type": "boolean" }
                },
                "required": ["name", "driver"],
                "additionalProperties": false
            }),
            output_schema: json!({ "type": "object" }),
            annotations: ToolAnnotations::local_write(),
        },
        handler: db_create_connection,
    }
}

fn db_create_connection(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "name",
            "driver",
            "host",
            "port",
            "database",
            "username",
            "sslMode",
            "sqlitePath",
            "credentialRef",
            "password",
            "credentialLabel",
            "readOnly",
        ],
    )?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let name = parse_required_string(&arguments, "name", "unfour.db.create_connection")?;
    let driver = parse_required_string(&arguments, "driver", "unfour.db.create_connection")?;
    let supplied_credential_ref = parse_optional_string(&arguments, "credentialRef")?;
    let password = parse_optional_secret(&arguments, "password")?;
    if supplied_credential_ref.is_some() && password.is_some() {
        return Err(ToolCallError::InvalidArguments(
            "unfour.db.create_connection accepts either `password` or `credentialRef`, not both"
                .to_string(),
        ));
    }

    let credential_source = if password.is_some() {
        "created"
    } else if supplied_credential_ref.is_some() {
        "provided"
    } else {
        "none"
    };
    let credential_ref = if let Some(secret) = password {
        let label = parse_optional_string(&arguments, "credentialLabel")?
            .unwrap_or_else(|| format!("{name} password"));
        let credential = command_bus
            .create_credential(CredentialCreateInput {
                workspace_id: workspace_id.clone(),
                kind: "database-password".to_string(),
                label,
                secret,
            })
            .map_err(|error| ToolCallError::Execution {
                code: error.code,
                message: error.message,
            })?;
        Some(credential.credential_ref)
    } else {
        supplied_credential_ref
    };

    let connection = command_bus
        .save_db_connection(DatabaseConnectionInput {
            id: None,
            workspace_id,
            name,
            driver,
            host: parse_optional_string(&arguments, "host")?,
            port: parse_optional_port(&arguments)?,
            database: parse_optional_string(&arguments, "database")?,
            username: parse_optional_string(&arguments, "username")?,
            ssl_mode: parse_optional_string(&arguments, "sslMode")?,
            sqlite_path: parse_optional_string(&arguments, "sqlitePath")?,
            credential_ref,
            read_only: parse_optional_bool(&arguments, "readOnly")?.unwrap_or(false),
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    Ok(json!({
        "connection": safe_connection_summary(&connection),
        "credentialStored": connection.credential_ref.is_some(),
        "credentialSource": credential_source,
        "source": "command-bus"
    }))
}

fn parse_optional_port(arguments: &Map<String, Value>) -> Result<Option<u16>, ToolCallError> {
    let Some(value) = arguments.get("port") else {
        return Ok(None);
    };
    let Some(port) = value.as_u64() else {
        return Err(ToolCallError::InvalidArguments(
            "argument `port` must be a positive integer".to_string(),
        ));
    };
    if !(1..=65535).contains(&port) {
        return Err(ToolCallError::InvalidArguments(
            "argument `port` must be between 1 and 65535".to_string(),
        ));
    }
    Ok(Some(port as u16))
}

fn parse_optional_secret(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a string"
        ))),
    }
}
