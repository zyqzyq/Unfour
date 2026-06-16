use serde_json::{json, Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::{DatabaseConnection, DatabaseQueryInput};

use crate::command_bus_adapter::CommandBusAdapter;

use super::{object_with_allowed_keys, RegisteredTool, ToolCallError, ToolDefinition, ToolHandler};

const DEFAULT_QUERY_LIMIT: u32 = 100;
const MAX_QUERY_LIMIT: u32 = 1000;
const DEFAULT_TABLE_LIMIT: u32 = 200;
const MAX_TABLE_LIMIT: u32 = 500;
const MAX_QUERY_RESULT_BYTES: usize = 20 * 1024;

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.list_connections",
                title: "List Database Connections",
                description:
                    "Lists saved database connections for the active workspace through the Unfour command bus. Returns safe summaries without passwords, tokens, or connection strings.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID. Uses the active workspace if omitted."
                        }
                    },
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "connections": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "name": { "type": "string" },
                                    "databaseType": { "type": "string" },
                                    "host": { "type": ["string", "null"] },
                                    "port": { "type": ["integer", "null"] },
                                    "database": { "type": ["string", "null"] },
                                    "workspaceId": { "type": "string" }
                                },
                                "required": ["id", "name", "databaseType", "workspaceId"],
                                "additionalProperties": false
                            }
                        },
                        "count": { "type": "integer", "minimum": 0 },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["connections", "count", "source"],
                    "additionalProperties": false
                }),
            },
            handler: ToolHandler::Real(db_list_connections),
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.list_tables",
                title: "List Database Tables",
                description:
                    "Lists tables and views for a saved database connection through the Unfour command bus. Requires a saved connectionId; does not accept ad-hoc connection strings.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": {
                            "type": "string",
                            "description": "Required saved database connection ID."
                        },
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID. Uses the active workspace if omitted."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of tables to return (default 200, max 500)."
                        }
                    },
                    "required": ["connectionId"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "tables": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "schema": { "type": ["string", "null"] },
                                    "kind": { "type": "string" },
                                    "columnCount": { "type": "integer", "minimum": 0 }
                                },
                                "required": ["name", "kind", "columnCount"],
                                "additionalProperties": false
                            }
                        },
                        "count": { "type": "integer", "minimum": 0 },
                        "totalTables": { "type": "integer", "minimum": 0 },
                        "truncated": { "type": "boolean" },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["connectionId", "tables", "count", "totalTables", "truncated", "source"],
                    "additionalProperties": false
                }),
            },
            handler: ToolHandler::Real(db_list_tables),
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.describe_table",
                title: "Describe Database Table",
                description:
                    "Describes a table's structure (columns, types, nullability, primary keys) for a saved database connection through the Unfour command bus. Does not read table data.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": {
                            "type": "string",
                            "description": "Required saved database connection ID."
                        },
                        "tableName": {
                            "type": "string",
                            "description": "Required table name to describe."
                        },
                        "schema": {
                            "type": "string",
                            "description": "Optional schema name filter (e.g. 'public')."
                        },
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID. Uses the active workspace if omitted."
                        }
                    },
                    "required": ["connectionId", "tableName"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "table": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "schema": { "type": ["string", "null"] },
                                "kind": { "type": "string" },
                                "columns": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "name": { "type": "string" },
                                            "dataType": { "type": "string" },
                                            "nullable": { "type": "boolean" },
                                            "primaryKey": { "type": "boolean" }
                                        },
                                        "required": ["name", "dataType", "nullable", "primaryKey"],
                                        "additionalProperties": false
                                    }
                                },
                                "columnCount": { "type": "integer", "minimum": 0 }
                            },
                            "required": ["name", "kind", "columns", "columnCount"],
                            "additionalProperties": false
                        },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["connectionId", "table", "source"],
                    "additionalProperties": false
                }),
            },
            handler: ToolHandler::Real(db_describe_table),
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.query_readonly",
                title: "Execute Read-Only SQL Query",
                description:
                    "Executes a read-only SQL query against a saved database connection through the Unfour command bus. Only SELECT, WITH, SHOW, DESCRIBE, DESC, and EXPLAIN statements are allowed. Write operations, DDL, and multi-statement queries are rejected.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": {
                            "type": "string",
                            "description": "Required saved database connection ID."
                        },
                        "sql": {
                            "type": "string",
                            "description": "Required SQL query. Only read-only statements are allowed."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of rows to return (default 100, max 1000)."
                        },
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID. Uses the active workspace if omitted."
                        }
                    },
                    "required": ["connectionId", "sql"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean" },
                        "connectionId": { "type": "string" },
                        "columns": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "dataType": { "type": "string" }
                                },
                                "required": ["name", "dataType"],
                                "additionalProperties": false
                            }
                        },
                        "rows": { "type": "array" },
                        "rowCount": { "type": "integer", "minimum": 0 },
                        "durationMs": { "type": "integer", "minimum": 0 },
                        "truncated": { "type": "boolean" },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["ok", "connectionId", "source"],
                    "additionalProperties": false
                }),
            },
            handler: ToolHandler::Real(db_query_readonly),
        },
    ]
}

// --- Tool handlers ---

fn db_list_connections(
    command_bus: &dyn CommandBusAdapter,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId"])?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;

    let connections = command_bus
        .list_db_connections(&workspace_id)
        .map_err(|e| ToolCallError::Execution {
            code: e.code,
            message: e.message,
        })?;

    let safe_connections: Vec<Value> = connections.iter().map(safe_connection_summary).collect();

    Ok(json!({
        "connections": safe_connections,
        "count": safe_connections.len(),
        "source": "command-bus"
    }))
}

fn db_list_tables(
    command_bus: &dyn CommandBusAdapter,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments =
        object_with_allowed_keys(arguments, &["connectionId", "workspaceId", "limit"])?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.db.list_tables")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let limit = parse_optional_limit(&arguments, "limit", DEFAULT_TABLE_LIMIT, MAX_TABLE_LIMIT)?;

    let schema = command_bus
        .get_db_schema(&workspace_id, &connection_id)
        .map_err(|e| ToolCallError::Execution {
            code: e.code,
            message: e.message,
        })?;

    let total = schema.tables.len();
    let tables: Vec<Value> = schema
        .tables
        .iter()
        .take(limit as usize)
        .map(|t| {
            json!({
                "name": t.name,
                "schema": t.schema,
                "kind": t.kind,
                "columnCount": t.columns.len()
            })
        })
        .collect();

    let truncated = total > tables.len();

    Ok(json!({
        "connectionId": connection_id,
        "tables": tables,
        "count": tables.len(),
        "totalTables": total,
        "truncated": truncated,
        "source": "command-bus"
    }))
}

fn db_describe_table(
    command_bus: &dyn CommandBusAdapter,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &["connectionId", "tableName", "schema", "workspaceId"],
    )?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.db.describe_table")?;
    let table_name =
        parse_required_string(&arguments, "tableName", "unfour.db.describe_table")?;
    let schema_filter = parse_optional_string(&arguments, "schema")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;

    let schema = command_bus
        .get_db_schema(&workspace_id, &connection_id)
        .map_err(|e| ToolCallError::Execution {
            code: e.code,
            message: e.message,
        })?;

    let table = schema.tables.iter().find(|t| {
        t.name == table_name
            && match &schema_filter {
                Some(s) => t.schema.as_deref() == Some(s.as_str()),
                None => true,
            }
    });

    let Some(table) = table else {
        return Err(ToolCallError::Execution {
            code: "TABLE_NOT_FOUND",
            message: "The requested table was not found in the database schema.",
        });
    };

    let columns: Vec<Value> = table
        .columns
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "dataType": c.data_type,
                "nullable": c.nullable,
                "primaryKey": c.primary_key
            })
        })
        .collect();

    Ok(json!({
        "connectionId": connection_id,
        "table": {
            "name": table.name,
            "schema": table.schema,
            "kind": table.kind,
            "columns": columns,
            "columnCount": columns.len()
        },
        "source": "command-bus"
    }))
}

fn db_query_readonly(
    command_bus: &dyn CommandBusAdapter,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments =
        object_with_allowed_keys(arguments, &["connectionId", "sql", "limit", "workspaceId"])?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.db.query_readonly")?;
    let sql = parse_required_string(&arguments, "sql", "unfour.db.query_readonly")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let limit = parse_optional_limit(&arguments, "limit", DEFAULT_QUERY_LIMIT, MAX_QUERY_LIMIT)?;

    // MCP-layer read-only validation (defense-in-depth).
    validate_readonly_sql(&sql)?;

    let input = DatabaseQueryInput {
        workspace_id,
        connection_id: connection_id.clone(),
        sql,
        limit: Some(limit),
        confirm_mutation: None,
    };

    match command_bus.execute_db_query(input) {
        Ok(result) => {
            let (truncated_rows, was_truncated) =
                truncate_query_rows(result.rows, MAX_QUERY_RESULT_BYTES);

            let columns: Vec<Value> = result
                .columns
                .iter()
                .map(|c| {
                    json!({
                        "name": c.name,
                        "dataType": c.data_type
                    })
                })
                .collect();

            Ok(json!({
                "ok": true,
                "connectionId": connection_id,
                "columns": columns,
                "rows": truncated_rows,
                "rowCount": truncated_rows.len(),
                "durationMs": result.duration_ms,
                "truncated": was_truncated,
                "source": "command-bus"
            }))
        }
        Err(error) => Err(ToolCallError::Execution {
            code: error.code,
            message: error.message,
        }),
    }
}

// --- SQL validation ---

/// MCP-layer read-only SQL validation. Strips comments, rejects multi-statement
/// SQL, and only allows an explicit allowlist of read-only keywords.
fn validate_readonly_sql(sql: &str) -> Result<(), ToolCallError> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err(ToolCallError::Execution {
            code: "READONLY_SQL_REJECTED",
            message: "SQL cannot be empty.",
        });
    }

    // Strip leading comments to prevent bypass via `/* ... */ INSERT ...`.
    let stripped = strip_leading_comments(trimmed);
    if stripped.is_empty() {
        return Err(ToolCallError::Execution {
            code: "READONLY_SQL_REJECTED",
            message: "SQL cannot be empty after removing comments.",
        });
    }

    // Reject multi-statement SQL: after removing trailing semicolons, any
    // remaining semicolons indicate multiple statements.
    let without_trailing = stripped.trim_end_matches(';').trim_end();
    if without_trailing.contains(';') {
        return Err(ToolCallError::Execution {
            code: "READONLY_SQL_REJECTED",
            message: "Only one SQL statement is allowed.",
        });
    }

    let keyword = stripped
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();

    match keyword.as_str() {
        "select" | "with" | "show" | "describe" | "desc" | "explain" => Ok(()),
        _ => Err(ToolCallError::Execution {
            code: "READONLY_SQL_REJECTED",
            message: "Only read-only SQL is permitted (SELECT, WITH, SHOW, DESCRIBE, DESC, EXPLAIN).",
        }),
    }
}

/// Strip leading SQL line comments (`--`) and block comments (`/* ... */`).
fn strip_leading_comments(sql: &str) -> String {
    let mut s = sql.trim();
    loop {
        if s.starts_with("--") {
            // Line comment: skip to end of line.
            if let Some(pos) = s.find('\n') {
                s = s[pos + 1..].trim();
            } else {
                return String::new();
            }
        } else if s.starts_with("/*") {
            // Block comment: skip to closing `*/`.
            if let Some(pos) = s.find("*/") {
                s = s[pos + 2..].trim();
            } else {
                return String::new();
            }
        } else {
            break;
        }
    }
    s.to_string()
}

// --- Connection summary sanitization ---

/// Convert a DatabaseConnection into a safe summary that excludes credentials,
/// usernames, and internal metadata.
fn safe_connection_summary(conn: &DatabaseConnection) -> Value {
    json!({
        "id": conn.id,
        "name": conn.name,
        "databaseType": conn.driver,
        "host": conn.host,
        "port": conn.port,
        "database": conn.database,
        "workspaceId": conn.workspace_id
    })
}

// --- Result truncation ---

/// Truncate rows if their serialized JSON size exceeds `max_bytes`.
/// Returns `(kept_rows, was_truncated)`.
fn truncate_query_rows(
    rows: Vec<Vec<Option<String>>>,
    max_bytes: usize,
) -> (Vec<Vec<Option<String>>>, bool) {
    let serialized = serde_json::to_string(&rows).unwrap_or_default();
    if serialized.len() <= max_bytes {
        return (rows, false);
    }

    let mut kept = Vec::new();
    let mut current_size = 2; // for "[]"
    for row in rows {
        let row_json = serde_json::to_string(&row).unwrap_or_default();
        let row_size = row_json.len() + 1; // +1 for comma separator
        if current_size + row_size > max_bytes && !kept.is_empty() {
            return (kept, true);
        }
        current_size += row_size;
        kept.push(row);
    }
    (kept, true)
}

// --- Helpers ---

fn resolve_workspace_id(
    command_bus: &dyn CommandBusAdapter,
    arguments: &Map<String, Value>,
) -> Result<String, ToolCallError> {
    match parse_optional_string(arguments, "workspaceId")? {
        Some(id) => Ok(id),
        None => {
            let ws_result = command_bus
                .execute_read(ReadCommand::CurrentWorkspace)
                .map_err(|e| ToolCallError::Execution {
                    code: e.code,
                    message: e.message,
                })?;
            let ReadCommandResult::CurrentWorkspace(ws) = ws_result else {
                return Err(unexpected_result());
            };
            Ok(ws.workspace_id)
        }
    }
}

fn parse_required_string(
    arguments: &Map<String, Value>,
    key: &str,
    tool_name: &str,
) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        Some(Value::String(_)) => Err(ToolCallError::InvalidArguments(format!(
            "{} argument `{}` cannot be empty",
            tool_name, key
        ))),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "{} requires argument `{}`",
            tool_name, key
        ))),
    }
}

fn parse_optional_string(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a string",
            key
        ))),
    }
}

fn parse_optional_u32(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<u32>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::Number(n)) => {
            let val = n.as_u64().ok_or_else(|| {
                ToolCallError::InvalidArguments(format!(
                    "argument `{}` must be a positive integer",
                    key
                ))
            })?;
            Ok(Some(val as u32))
        }
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a number",
            key
        ))),
    }
}

fn parse_optional_limit(
    arguments: &Map<String, Value>,
    key: &str,
    default: u32,
    max: u32,
) -> Result<u32, ToolCallError> {
    match parse_optional_u32(arguments, key)? {
        None => Ok(default),
        Some(val) => Ok(val.clamp(1, max)),
    }
}

fn unexpected_result() -> ToolCallError {
    ToolCallError::Execution {
        code: "COMMAND_BUS_RESULT_MISMATCH",
        message: "The command-bus returned an unexpected result.",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use unfour_command_bus::{
        ConnectionListResult, CurrentWorkspaceResult, ReadCommand, ReadCommandResult,
    };
    use unfour_core::models::{
        ApiResponse, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult,
        DatabaseQuerySafety, DatabaseResultColumn, DatabaseSchema, DatabaseTable,
        DatabaseTableColumn,
    };

    use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};

    use super::*;

    // --- Stub implementations ---

    struct DbStubCommandBus;

    impl CommandBusAdapter for DbStubCommandBus {
        fn execute_read(
            &self,
            command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            Ok(match command {
                ReadCommand::CurrentWorkspace => {
                    ReadCommandResult::CurrentWorkspace(CurrentWorkspaceResult {
                        workspace_id: "workspace-1".to_string(),
                        workspace_name: "Workspace".to_string(),
                        workspace_root: None,
                        mode: "local".to_string(),
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ListConnections { .. } => {
                    ReadCommandResult::Connections(ConnectionListResult {
                        connections: vec![],
                        count: 0,
                        source: "command-bus".to_string(),
                    })
                }
                _ => ReadCommandResult::CurrentWorkspace(CurrentWorkspaceResult {
                    workspace_id: "workspace-1".to_string(),
                    workspace_name: "Workspace".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                }),
            })
        }

        fn execute_saved_api_request(
            &self,
            _request_id: &str,
            _timeout_ms: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            Ok(ApiResponse {
                history_id: "history-1".to_string(),
                status: 200,
                status_text: "OK".to_string(),
                headers: vec![],
                body: "{}".to_string(),
                duration_ms: 0,
            })
        }

        fn list_db_connections(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
            Ok(vec![DatabaseConnection {
                id: "conn-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                name: "Dev Database".to_string(),
                driver: "postgres".to_string(),
                host: Some("localhost".to_string()),
                port: Some(5432),
                database: Some("app_dev".to_string()),
                username: Some("admin".to_string()),
                sqlite_path: None,
                credential_ref: Some("secret-ref-123".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
                deleted_at: None,
                revision: 1,
                sync_status: "local".to_string(),
                remote_id: None,
            }])
        }

        fn get_db_schema(
            &self,
            _workspace_id: &str,
            connection_id: &str,
        ) -> Result<DatabaseSchema, CommandBusAdapterError> {
            Ok(DatabaseSchema {
                connection_id: connection_id.to_string(),
                tables: vec![
                    DatabaseTable {
                        schema: Some("public".to_string()),
                        name: "users".to_string(),
                        kind: "table".to_string(),
                        columns: vec![
                            DatabaseTableColumn {
                                name: "id".to_string(),
                                data_type: "integer".to_string(),
                                nullable: false,
                                primary_key: true,
                            },
                            DatabaseTableColumn {
                                name: "email".to_string(),
                                data_type: "varchar".to_string(),
                                nullable: false,
                                primary_key: false,
                            },
                            DatabaseTableColumn {
                                name: "created_at".to_string(),
                                data_type: "timestamp".to_string(),
                                nullable: true,
                                primary_key: false,
                            },
                        ],
                    },
                    DatabaseTable {
                        schema: Some("public".to_string()),
                        name: "orders".to_string(),
                        kind: "table".to_string(),
                        columns: vec![DatabaseTableColumn {
                            name: "id".to_string(),
                            data_type: "integer".to_string(),
                            nullable: false,
                            primary_key: true,
                        }],
                    },
                    DatabaseTable {
                        schema: Some("analytics".to_string()),
                        name: "events".to_string(),
                        kind: "view".to_string(),
                        columns: vec![],
                    },
                    DatabaseTable {
                        schema: Some("analytics".to_string()),
                        name: "summary".to_string(),
                        kind: "table".to_string(),
                        columns: vec![],
                    },
                    DatabaseTable {
                        schema: Some("audit".to_string()),
                        name: "logs".to_string(),
                        kind: "table".to_string(),
                        columns: vec![],
                    },
                ],
            })
        }

        fn execute_db_query(
            &self,
            _input: DatabaseQueryInput,
        ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
            Ok(DatabaseQueryResult {
                columns: vec![
                    DatabaseResultColumn {
                        name: "id".to_string(),
                        data_type: "integer".to_string(),
                    },
                    DatabaseResultColumn {
                        name: "email".to_string(),
                        data_type: "varchar".to_string(),
                    },
                ],
                rows: vec![
                    vec![Some("1".to_string()), Some("user@example.com".to_string())],
                    vec![Some("2".to_string()), Some("other@example.com".to_string())],
                ],
                affected_rows: 0,
                duration_ms: 42,
                safety: DatabaseQuerySafety {
                    classification: "read".to_string(),
                    requires_confirmation: false,
                    confirmed: true,
                    message: None,
                },
            })
        }
    }

    struct DbFailingCommandBus;

    impl CommandBusAdapter for DbFailingCommandBus {
        fn execute_read(
            &self,
            _command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_READ_FAILED",
                message: "The command-bus read operation failed.",
            })
        }

        fn execute_saved_api_request(
            &self,
            _request_id: &str,
            _timeout_ms: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_API_SEND_FAILED",
                message: "The command-bus API send operation failed.",
            })
        }

        fn list_db_connections(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_DB_LIST_FAILED",
                message: "The command-bus database list operation failed.",
            })
        }

        fn get_db_schema(
            &self,
            _workspace_id: &str,
            _connection_id: &str,
        ) -> Result<DatabaseSchema, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_DB_SCHEMA_FAILED",
                message: "The command-bus database schema operation failed.",
            })
        }

        fn execute_db_query(
            &self,
            _input: DatabaseQueryInput,
        ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_DB_QUERY_FAILED",
                message: "The command-bus database query operation failed.",
            })
        }
    }

    fn registry() -> super::super::ToolRegistry {
        super::super::ToolRegistry::with_command_bus(Arc::new(DbStubCommandBus))
    }

    // --- Schema / registration tests ---

    #[test]
    fn db_tools_are_registered() {
        let definitions = registry().definitions();
        assert!(definitions
            .iter()
            .any(|d| d.name == "unfour.db.list_connections"));
        assert!(definitions
            .iter()
            .any(|d| d.name == "unfour.db.list_tables"));
        assert!(definitions
            .iter()
            .any(|d| d.name == "unfour.db.describe_table"));
        assert!(definitions
            .iter()
            .any(|d| d.name == "unfour.db.query_readonly"));
    }

    #[test]
    fn db_list_connections_input_schema() {
        let definitions = registry().definitions();
        let tool = definitions
            .iter()
            .find(|d| d.name == "unfour.db.list_connections")
            .unwrap();
        assert_eq!(tool.input_schema["type"], "object");
        assert!(tool.input_schema["properties"]["workspaceId"].is_object());
    }

    #[test]
    fn db_list_tables_input_schema() {
        let definitions = registry().definitions();
        let tool = definitions
            .iter()
            .find(|d| d.name == "unfour.db.list_tables")
            .unwrap();
        assert_eq!(tool.input_schema["type"], "object");
        assert_eq!(
            tool.input_schema["required"].as_array().unwrap(),
            &vec![json!("connectionId")]
        );
    }

    #[test]
    fn db_describe_table_input_schema() {
        let definitions = registry().definitions();
        let tool = definitions
            .iter()
            .find(|d| d.name == "unfour.db.describe_table")
            .unwrap();
        assert_eq!(tool.input_schema["type"], "object");
        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("connectionId")));
        assert!(required.contains(&json!("tableName")));
    }

    // --- list_connections tests ---

    #[test]
    fn list_connections_returns_safe_summary() {
        let result = registry()
            .call("unfour.db.list_connections", json!({}))
            .expect("should succeed");

        let content = &result["structuredContent"];
        assert_eq!(content["count"], 1);
        let conn = &content["connections"][0];
        assert_eq!(conn["id"], "conn-1");
        assert_eq!(conn["name"], "Dev Database");
        assert_eq!(conn["databaseType"], "postgres");
        assert_eq!(conn["host"], "localhost");
        assert_eq!(conn["port"], 5432);
        assert_eq!(conn["database"], "app_dev");

        // Ensure sensitive fields are NOT present.
        let serialized = serde_json::to_string(content).unwrap();
        assert!(!serialized.contains("admin"));
        assert!(!serialized.contains("secret-ref-123"));
        assert!(!serialized.contains("credentialRef"));
        assert!(!serialized.contains("credential_ref"));
    }

    #[test]
    fn list_connections_resolves_default_workspace() {
        let result = registry()
            .call("unfour.db.list_connections", json!({}))
            .expect("should succeed");
        assert_eq!(result["structuredContent"]["source"], "command-bus");
    }

    #[test]
    fn list_connections_handles_empty() {
        struct EmptyDbStub;
        impl CommandBusAdapter for EmptyDbStub {
            fn execute_read(
                &self,
                _command: ReadCommand,
            ) -> Result<ReadCommandResult, CommandBusAdapterError> {
                Ok(ReadCommandResult::CurrentWorkspace(CurrentWorkspaceResult {
                    workspace_id: "ws-1".to_string(),
                    workspace_name: "W".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                }))
            }
            fn execute_saved_api_request(
                &self,
                _: &str,
                _: Option<u64>,
            ) -> Result<ApiResponse, CommandBusAdapterError> {
                unreachable!()
            }
            fn list_db_connections(
                &self,
                _: &str,
            ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
                Ok(vec![])
            }
            fn get_db_schema(
                &self,
                _: &str,
                _: &str,
            ) -> Result<DatabaseSchema, CommandBusAdapterError> {
                unreachable!()
            }
            fn execute_db_query(
                &self,
                _: DatabaseQueryInput,
            ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
                unreachable!()
            }
        }

        let reg =
            super::super::ToolRegistry::with_command_bus(Arc::new(EmptyDbStub));
        let result = reg
            .call("unfour.db.list_connections", json!({}))
            .expect("should succeed");
        assert_eq!(result["structuredContent"]["count"], 0);
    }

    // --- list_tables tests ---

    #[test]
    fn list_tables_returns_table_summaries() {
        let result = registry()
            .call(
                "unfour.db.list_tables",
                json!({ "connectionId": "conn-1" }),
            )
            .expect("should succeed");

        let content = &result["structuredContent"];
        assert_eq!(content["connectionId"], "conn-1");
        assert_eq!(content["totalTables"], 5);
        assert_eq!(content["count"], 5);
        assert_eq!(content["truncated"], false);

        let first = &content["tables"][0];
        assert_eq!(first["name"], "users");
        assert_eq!(first["schema"], "public");
        assert_eq!(first["kind"], "table");
        assert_eq!(first["columnCount"], 3);
    }

    #[test]
    fn list_tables_respects_limit() {
        let result = registry()
            .call(
                "unfour.db.list_tables",
                json!({ "connectionId": "conn-1", "limit": 2 }),
            )
            .expect("should succeed");

        let content = &result["structuredContent"];
        assert_eq!(content["count"], 2);
        assert_eq!(content["totalTables"], 5);
        assert_eq!(content["truncated"], true);
    }

    #[test]
    fn list_tables_requires_connection_id() {
        let result = registry().call("unfour.db.list_tables", json!({}));
        assert!(result.is_err(), "should fail without connectionId");
    }

    #[test]
    fn list_tables_clamps_limit_to_500() {
        let result = registry()
            .call(
                "unfour.db.list_tables",
                json!({ "connectionId": "conn-1", "limit": 9999 }),
            )
            .expect("should succeed");

        let content = &result["structuredContent"];
        // We have 5 tables, limit clamped to 500, so all 5 returned.
        assert_eq!(content["count"], 5);
        assert_eq!(content["truncated"], false);
    }

    // --- describe_table tests ---

    #[test]
    fn describe_table_returns_columns() {
        let result = registry()
            .call(
                "unfour.db.describe_table",
                json!({ "connectionId": "conn-1", "tableName": "users" }),
            )
            .expect("should succeed");

        let content = &result["structuredContent"];
        assert_eq!(content["connectionId"], "conn-1");
        let table = &content["table"];
        assert_eq!(table["name"], "users");
        assert_eq!(table["schema"], "public");
        assert_eq!(table["kind"], "table");
        assert_eq!(table["columnCount"], 3);

        let id_col = &table["columns"][0];
        assert_eq!(id_col["name"], "id");
        assert_eq!(id_col["dataType"], "integer");
        assert_eq!(id_col["nullable"], false);
        assert_eq!(id_col["primaryKey"], true);
    }

    #[test]
    fn describe_table_with_schema_filter() {
        let result = registry()
            .call(
                "unfour.db.describe_table",
                json!({ "connectionId": "conn-1", "tableName": "events", "schema": "analytics" }),
            )
            .expect("should succeed");

        let content = &result["structuredContent"];
        assert_eq!(content["table"]["name"], "events");
        assert_eq!(content["table"]["schema"], "analytics");
        assert_eq!(content["table"]["kind"], "view");
    }

    #[test]
    fn describe_table_not_found_returns_error() {
        let result = registry()
            .call(
                "unfour.db.describe_table",
                json!({ "connectionId": "conn-1", "tableName": "nonexistent" }),
            )
            .expect("should return error result");

        assert_eq!(result["isError"], true);
        assert_eq!(
            result["structuredContent"]["error"]["code"],
            "TABLE_NOT_FOUND"
        );
    }

    #[test]
    fn describe_table_requires_table_name() {
        let result = registry().call(
            "unfour.db.describe_table",
            json!({ "connectionId": "conn-1" }),
        );
        assert!(result.is_err(), "should fail without tableName");
    }

    // --- query_readonly tests ---

    #[test]
    fn query_readonly_executes_select() {
        let result = registry()
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "SELECT id, email FROM users LIMIT 10"
                }),
            )
            .expect("should succeed");

        let content = &result["structuredContent"];
        assert_eq!(content["ok"], true);
        assert_eq!(content["connectionId"], "conn-1");
        assert_eq!(content["columns"].as_array().unwrap().len(), 2);
        assert_eq!(content["rowCount"], 2);
        assert_eq!(content["durationMs"], 42);
        assert_eq!(content["source"], "command-bus");
    }

    #[test]
    fn query_readonly_allows_with_cte() {
        let result = registry()
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "WITH cte AS (SELECT 1) SELECT * FROM cte"
                }),
            )
            .expect("should succeed");
        assert_eq!(result["structuredContent"]["ok"], true);
    }

    #[test]
    fn query_readonly_allows_explain() {
        let result = registry()
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "EXPLAIN SELECT * FROM users"
                }),
            )
            .expect("should succeed");
        assert_eq!(result["structuredContent"]["ok"], true);
    }

    #[test]
    fn query_readonly_rejects_insert() {
        let result = registry()
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "INSERT INTO users (email) VALUES ('evil@test.com')"
                }),
            )
            .expect("should return error result");
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn query_readonly_rejects_update() {
        let result = registry()
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "UPDATE users SET email = 'hacked' WHERE id = 1"
                }),
            )
            .expect("should return error result");
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn query_readonly_rejects_delete() {
        let result = registry()
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "DELETE FROM users WHERE id = 1"
                }),
            )
            .expect("should return error result");
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn query_readonly_rejects_drop_alter_create() {
        for sql in &[
            "DROP TABLE users",
            "ALTER TABLE users ADD COLUMN foo TEXT",
            "CREATE TABLE evil (id INT)",
            "TRUNCATE TABLE users",
        ] {
            let result = registry()
                .call(
                    "unfour.db.query_readonly",
                    json!({ "connectionId": "conn-1", "sql": sql }),
                )
                .expect("should return error result");
            assert_eq!(result["isError"], true, "should reject: {}", sql);
        }
    }

    #[test]
    fn query_readonly_rejects_multi_statement() {
        let result = registry()
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "SELECT 1; DROP TABLE users"
                }),
            )
            .expect("should return error result");
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn query_readonly_rejects_comment_bypass() {
        let result = registry()
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "/* harmless comment */ INSERT INTO users VALUES (1)"
                }),
            )
            .expect("should return error result");
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn query_readonly_clamps_limit_to_1000() {
        let result = registry()
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "SELECT * FROM users",
                    "limit": 99999
                }),
            )
            .expect("should succeed");
        assert_eq!(result["structuredContent"]["ok"], true);
    }

    #[test]
    fn query_readonly_truncates_large_results() {
        struct LargeResultStub;
        impl CommandBusAdapter for LargeResultStub {
            fn execute_read(
                &self,
                _: ReadCommand,
            ) -> Result<ReadCommandResult, CommandBusAdapterError> {
                Ok(ReadCommandResult::CurrentWorkspace(CurrentWorkspaceResult {
                    workspace_id: "ws-1".to_string(),
                    workspace_name: "W".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                }))
            }
            fn execute_saved_api_request(
                &self,
                _: &str,
                _: Option<u64>,
            ) -> Result<ApiResponse, CommandBusAdapterError> {
                unreachable!()
            }
            fn list_db_connections(
                &self,
                _: &str,
            ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
                unreachable!()
            }
            fn get_db_schema(
                &self,
                _: &str,
                _: &str,
            ) -> Result<DatabaseSchema, CommandBusAdapterError> {
                unreachable!()
            }
            fn execute_db_query(
                &self,
                _: DatabaseQueryInput,
            ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
                // Generate rows that will exceed 20KB.
                let big_value = "x".repeat(1024);
                let rows: Vec<Vec<Option<String>>> = (0..100)
                    .map(|i| vec![Some(i.to_string()), Some(big_value.clone())])
                    .collect();
                Ok(DatabaseQueryResult {
                    columns: vec![
                        DatabaseResultColumn {
                            name: "id".to_string(),
                            data_type: "integer".to_string(),
                        },
                        DatabaseResultColumn {
                            name: "data".to_string(),
                            data_type: "text".to_string(),
                        },
                    ],
                    rows,
                    affected_rows: 0,
                    duration_ms: 10,
                    safety: DatabaseQuerySafety {
                        classification: "read".to_string(),
                        requires_confirmation: false,
                        confirmed: true,
                        message: None,
                    },
                })
            }
        }

        let reg =
            super::super::ToolRegistry::with_command_bus(Arc::new(LargeResultStub));
        let result = reg
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "SELECT id, data FROM big_table"
                }),
            )
            .expect("should succeed");

        let content = &result["structuredContent"];
        assert_eq!(content["ok"], true);
        assert_eq!(content["truncated"], true);
        // Should have fewer rows than the original 100.
        assert!(content["rowCount"].as_u64().unwrap() < 100);
    }

    #[test]
    fn query_readonly_command_bus_failure() {
        let reg =
            super::super::ToolRegistry::with_command_bus(Arc::new(DbFailingCommandBus));
        let result = reg
            .call(
                "unfour.db.query_readonly",
                json!({
                    "connectionId": "conn-1",
                    "sql": "SELECT 1",
                    "workspaceId": "workspace-1"
                }),
            )
            .expect("should return error result");
        assert_eq!(result["isError"], true);
        assert_eq!(
            result["structuredContent"]["error"]["code"],
            "COMMAND_BUS_DB_QUERY_FAILED"
        );
    }

    // --- SQL validation unit tests ---

    #[test]
    fn validate_readonly_sql_case_insensitive() {
        assert!(validate_readonly_sql("SELECT 1").is_ok());
        assert!(validate_readonly_sql("select 1").is_ok());
        assert!(validate_readonly_sql("Select 1").is_ok());
        assert!(validate_readonly_sql("  SELECT 1  ").is_ok());
    }

    #[test]
    fn validate_readonly_sql_rejects_empty() {
        assert!(validate_readonly_sql("").is_err());
        assert!(validate_readonly_sql("   ").is_err());
    }

    #[test]
    fn validate_readonly_sql_rejects_transaction_control() {
        assert!(validate_readonly_sql("BEGIN").is_err());
        assert!(validate_readonly_sql("COMMIT").is_err());
        assert!(validate_readonly_sql("ROLLBACK").is_err());
    }

    #[test]
    fn validate_readonly_sql_strips_leading_comments() {
        // Block comment followed by valid SELECT.
        assert!(validate_readonly_sql("/* comment */ SELECT 1").is_ok());
        // Line comment followed by valid SELECT.
        assert!(validate_readonly_sql("-- comment\nSELECT 1").is_ok());
        // Block comment followed by forbidden INSERT.
        assert!(validate_readonly_sql("/* comment */ INSERT INTO t VALUES (1)").is_err());
        // Multiple comments then valid query.
        assert!(validate_readonly_sql("-- a\n-- b\nSELECT 1").is_ok());
        assert!(validate_readonly_sql("/* a */ /* b */ SELECT 1").is_ok());
    }

    // --- Truncation unit tests ---

    #[test]
    fn truncate_query_rows_preserves_small_results() {
        let rows = vec![
            vec![Some("1".to_string()), Some("a".to_string())],
            vec![Some("2".to_string()), Some("b".to_string())],
        ];
        let (kept, truncated) = truncate_query_rows(rows.clone(), 1024);
        assert_eq!(kept.len(), 2);
        assert!(!truncated);
    }

    #[test]
    fn truncate_query_rows_truncates_at_limit() {
        let big = "x".repeat(500);
        let rows: Vec<Vec<Option<String>>> = (0..50)
            .map(|i| vec![Some(i.to_string()), Some(big.clone())])
            .collect();
        let (kept, truncated) = truncate_query_rows(rows, 1024);
        assert!(truncated);
        assert!(kept.len() < 50);
        assert!(!kept.is_empty());
    }
}
