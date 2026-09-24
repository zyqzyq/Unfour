use serde_json::{json, Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::{
    DatabaseConnection, DatabaseExportContent, DatabaseExportFormat, DatabaseExportTableInput,
    DatabaseQueryInput, DatabaseTableStructureInput,
};

use crate::command_bus_adapter::CommandBusAdapter;

use super::policy::ToolPolicyEvaluation;
use super::{
    confirmation::ensure_confirmed_if_guarded, object_with_allowed_keys, RegisteredTool,
    ToolAnnotations, ToolCallError, ToolDefinition,
};

#[path = "database_create.rs"]
mod database_create;

#[path = "database_helpers.rs"]
mod database_helpers;
#[path = "database_sql.rs"]
mod database_sql;

use database_helpers::*;
use database_sql::*;

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.export_table",
                title: "Export Database Table",
                description: "Exports one table to a file in Unfour's managed exports directory. Optional columns, structured filters, and limit apply to the data portion. Omitted selectors export the whole table. Returns file metadata; no destination path is accepted.",
                input_schema: json!({
                    "type": "object", "properties": {
                        "connectionId": {"type": "string"}, "tableName": {"type": "string"},
                        "workspaceId": {"type": "string"}, "catalog": {"type": "string"}, "schema": {"type": "string"},
                        "content": {"type": "string", "enum": ["structure", "data", "structure-and-data"]},
                        "format": {"type": "string", "enum": ["sql", "csv", "json"]},
                        "columns": {"type": "array", "items": {"type": "string"}, "description": "Optional columns to include in the data portion. Omit to export every column."},
                        "filters": {"type": "array", "description": "Optional structured row predicates combined with AND. Each value is a string, number, or null and is bound; this is not a raw WHERE clause.", "items": {"type": "object", "properties": {
                            "column": {"type": "string"},
                            "op": {"type": "string", "enum": ["eq", "in"]},
                            "values": {"type": "array", "items": {"type": ["string", "number", "null"]}}
                        }, "required": ["column", "op", "values"], "additionalProperties": false}},
                        "limit": {"type": "integer", "minimum": 1, "description": "Optional maximum number of data rows."}
                    }, "required": ["connectionId", "tableName", "content", "format"], "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object", "properties": {
                        "path": {"type": "string"}, "rowCount": {"type": "integer", "minimum": 0},
                        "bytesWritten": {"type": "integer", "minimum": 0}, "format": {"type": "string", "enum": ["sql", "csv", "json"]}
                    }, "required": ["path", "rowCount", "bytesWritten", "format"], "additionalProperties": false
                }),
                annotations: ToolAnnotations::remote_action(),
            },
            handler: db_export_table,
        },
        database_create::registered_tool(),
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
                annotations: ToolAnnotations::local_read(),
            },
            handler: db_list_connections,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.list_tables",
                title: "List Database Tables",
                description:
                    "Lists tables and views for a saved database connection through the Unfour command bus. Optional catalog selects a database other than the connection default. Requires a saved connectionId; does not accept ad-hoc connection strings.",
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
                        },
                        "catalog": {
                            "type": "string",
                            "description": "Optional database catalog. Omit to use the connection's default database."
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
                                    "catalog": { "type": ["string", "null"] },
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
                annotations: ToolAnnotations::remote_read(),
            },
            handler: db_list_tables,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.describe_table",
                title: "Describe Database Table",
                description:
                    "Describes a table's complete structure (columns, indexes, foreign keys, DDL) for a saved database connection through the Unfour command bus. Does not read table data.",
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
                        "catalog": { "type": "string", "description": "Optional database catalog." },
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID. Uses the active workspace if omitted."
                        }
                    },
                    "required": ["connectionId", "tableName"],
                    "additionalProperties": false
                }),
                output_schema: describe_table_output_schema(),
                annotations: ToolAnnotations::remote_read(),
            },
            handler: db_describe_table,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.query_readonly",
                title: "Execute Read-Only SQL Query",
                description:
                    "Executes a read-only SQL query against a saved database connection through the Unfour command bus. Only SELECT, WITH, SHOW, DESCRIBE, DESC, and EXPLAIN statements are allowed. Write operations, DDL, and multi-statement queries are rejected. Optional catalog, schema, and timeoutMs match unfour.db.execute and unfour.db.explain.",
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
                        },
                        "catalog": {
                            "type": "string",
                            "description": "Optional catalog (database) to run the query against."
                        },
                        "schema": {
                            "type": "string",
                            "description": "Optional schema to resolve unqualified names against."
                        },
                        "timeoutMs": {
                            "type": "integer",
                            "description": "Optional per-statement timeout in milliseconds."
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
                annotations: ToolAnnotations::remote_read(),
            },
            handler: db_query_readonly,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.execute",
                title: "Execute Database SQL",
                description:
                    "Executes one SQL statement against a saved database connection through the Unfour command bus. Use for INSERT/UPDATE/DELETE/DDL when an agent needs to repair dev/test data. Dev allows non-high-risk writes by default; test allows small writes but high-risk SQL requires confirm; prod blocks writes. DELETE/UPDATE without WHERE, DROP, TRUNCATE, ALTER, and multi-row destructive statements require a second call with the returned confirmation_text. Returns affectedRows, statementType, durationMs, and engine safety metadata.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "sql": { "type": "string" },
                        "limit": { "type": "integer" },
                        "workspaceId": { "type": "string" },
                        "catalog": { "type": "string" },
                        "schema": { "type": "string" },
                        "timeoutMs": { "type": "integer" },
                        "dryRun": { "type": "boolean" },
                        "transaction": { "type": "boolean" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["connectionId", "sql"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean" },
                        "connectionId": { "type": "string" },
                        "statementType": { "type": "string" },
                        "affectedRows": { "type": "integer" },
                        "columns": { "type": "array" },
                        "rows": { "type": "array" },
                        "rowCount": { "type": "integer" },
                        "durationMs": { "type": "integer" },
                        "truncated": { "type": "boolean" },
                        "dryRun": { "type": "boolean" },
                        "transaction": { "type": "boolean" },
                        "safety": { "type": "object" },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["ok", "connectionId", "statementType", "dryRun", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::remote_action(),
            },
            handler: db_execute,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.explain",
                title: "Explain Database Query",
                description:
                    "Runs EXPLAIN for a read query against a saved database connection through the Unfour command bus. Use before optimizing slow SELECT/WITH queries. This is read-only in dev/test/prod; it returns structured columns/rows, durationMs, and truncation metadata.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "sql": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "limit": { "type": "integer" },
                        "catalog": { "type": "string" },
                        "schema": { "type": "string" },
                        "timeoutMs": { "type": "integer" }
                    },
                    "required": ["connectionId", "sql"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean" },
                        "connectionId": { "type": "string" },
                        "sql": { "type": "string" },
                        "columns": { "type": "array" },
                        "rows": { "type": "array" },
                        "rowCount": { "type": "integer" },
                        "durationMs": { "type": "integer" },
                        "truncated": { "type": "boolean" },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["ok", "connectionId", "sql", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::remote_read(),
            },
            handler: db_explain,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.test_connection",
                title: "Test Database Connection",
                description:
                    "Tests connectivity to a saved database connection through the Unfour command bus. Returns whether the connection succeeded and the server version when available.",
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
                        }
                    },
                    "required": ["connectionId"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean" },
                        "connectionId": { "type": "string" },
                        "message": { "type": "string" },
                        "serverVersion": { "type": ["string", "null"] },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["ok", "connectionId", "message", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::remote_read(),
            },
            handler: db_test_connection,
        },
    ]
}

// --- Tool handlers ---

fn db_list_connections(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId"])?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;

    let connections = command_bus
        .list_db_connections(&workspace_id)
        .map_err(adapter_execution)?;

    let safe_connections: Vec<Value> = connections.iter().map(safe_connection_summary).collect();

    Ok(json!({
        "connections": safe_connections,
        "count": safe_connections.len(),
        "source": "command-bus"
    }))
}

fn db_list_tables(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &["connectionId", "workspaceId", "limit", "catalog"],
    )?;
    let connection_id = parse_required_string(&arguments, "connectionId", "unfour.db.list_tables")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let limit = parse_optional_limit(&arguments, "limit", DEFAULT_TABLE_LIMIT, MAX_TABLE_LIMIT)?;
    let catalog = parse_optional_string(&arguments, "catalog")?;

    let schema = command_bus
        .get_db_schema_for_catalog(&workspace_id, &connection_id, catalog.as_deref())
        .map_err(adapter_execution)?;

    let total = schema.tables.len();
    let tables: Vec<Value> = schema
        .tables
        .iter()
        .take(limit as usize)
        .map(|t| {
            json!({
                "name": t.name,
                "catalog": t.catalog,
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
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "connectionId",
            "tableName",
            "catalog",
            "schema",
            "workspaceId",
        ],
    )?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.db.describe_table")?;
    let table_name = parse_required_string(&arguments, "tableName", "unfour.db.describe_table")?;
    let catalog = parse_optional_string(&arguments, "catalog")?;
    let schema = parse_optional_string(&arguments, "schema")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let table = command_bus
        .get_db_table_structure(DatabaseTableStructureInput {
            workspace_id,
            connection_id: connection_id.clone(),
            catalog,
            schema,
            table_name,
        })
        .map_err(adapter_execution)?;
    let mut table = serde_json::to_value(table).map_err(|_| ToolCallError::Execution {
        code: "SERIALIZATION_ERROR",
        message: "Could not serialize table structure.",
    })?;
    table["columnCount"] = json!(table["columns"].as_array().map_or(0, Vec::len));
    Ok(json!({ "connectionId": connection_id, "table": table, "source": "command-bus" }))
}

fn describe_table_output_schema() -> Value {
    json!({
        "type": "object", "properties": {
            "connectionId": {"type": "string"}, "source": {"type": "string", "const": "command-bus"},
            "table": {"type": "object", "properties": {
                "catalog": {"type": ["string", "null"]}, "schema": {"type": ["string", "null"]},
                "name": {"type": "string"}, "kind": {"type": "string"}, "ddl": {"type": ["string", "null"]},
                "columnCount": {"type": "integer", "minimum": 0},
                "columns": {"type": "array", "items": {"type": "object", "properties": {
                    "name": {"type": "string"}, "dataType": {"type": "string"},
                    "nullable": {"type": "boolean"}, "primaryKey": {"type": "boolean"},
                    "defaultValue": {"type": ["string", "null"]}, "generated": {"type": "boolean"}, "autoIncrement": {"type": "boolean"}
                }, "required": ["name", "dataType", "nullable", "primaryKey", "defaultValue", "generated", "autoIncrement"], "additionalProperties": false}},
                "indexes": {"type": "array", "items": {"type": "object", "properties": {
                    "name": {"type": "string"}, "columns": {"type": "array", "items": {"type": "string"}},
                    "unique": {"type": "boolean"}, "primary": {"type": "boolean"}
                }, "required": ["name", "columns", "unique", "primary"], "additionalProperties": false}},
                "foreignKeys": {"type": "array", "items": {"type": "object", "properties": {
                    "name": {"type": "string"}, "columns": {"type": "array", "items": {"type": "string"}},
                    "referencedTable": {"type": "string"}, "referencedColumns": {"type": "array", "items": {"type": "string"}}
                }, "required": ["name", "columns", "referencedTable", "referencedColumns"], "additionalProperties": false}}
            }, "required": ["catalog", "schema", "name", "kind", "ddl", "columnCount", "columns", "indexes", "foreignKeys"], "additionalProperties": false}
        }, "required": ["connectionId", "table", "source"], "additionalProperties": false
    })
}

fn db_export_table(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "connectionId",
            "tableName",
            "catalog",
            "schema",
            "workspaceId",
            "content",
            "format",
            "columns",
            "filters",
            "limit",
        ],
    )?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.db.export_table")?;
    let table_name = parse_required_string(&arguments, "tableName", "unfour.db.export_table")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let catalog = parse_optional_string(&arguments, "catalog")?;
    let schema = parse_optional_string(&arguments, "schema")?;
    let columns = parse_export_columns(&arguments)?;
    let filters = parse_export_filters(&arguments)?;
    let limit = match parse_optional_u32(&arguments, "limit")? {
        Some(0) => {
            return Err(ToolCallError::InvalidArguments(
                "argument `limit` must be greater than zero".into(),
            ))
        }
        other => other,
    };
    let content: DatabaseExportContent = serde_json::from_value(json!(parse_required_string(
        &arguments,
        "content",
        "unfour.db.export_table"
    )?))
    .map_err(|_| ToolCallError::InvalidArguments("Invalid export content".into()))?;
    let format: DatabaseExportFormat = serde_json::from_value(json!(parse_required_string(
        &arguments,
        "format",
        "unfour.db.export_table"
    )?))
    .map_err(|_| ToolCallError::InvalidArguments("Invalid export format".into()))?;
    let root = unfour_paths::resolve_unfour_paths()
        .map_err(|_| ToolCallError::Execution {
            code: "IO_ERROR",
            message: "Could not resolve exports directory.",
        })?
        .product_data_dir
        .join("exports");
    std::fs::create_dir_all(&root).map_err(|_| ToolCallError::Execution {
        code: "IO_ERROR",
        message: "Could not create exports directory.",
    })?;
    let safe_name: String = table_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .take(60)
        .collect();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let extension = match format {
        DatabaseExportFormat::Sql => "sql",
        DatabaseExportFormat::Csv => "csv",
        DatabaseExportFormat::Json => "json",
    };
    let path = root.join(format!(
        "{}-{}-{}.{}",
        if safe_name.is_empty() {
            "table"
        } else {
            &safe_name
        },
        std::process::id(),
        unique,
        extension
    ));
    let result = command_bus
        .export_db_table(DatabaseExportTableInput {
            workspace_id,
            connection_id,
            catalog,
            schema,
            table_name,
            content,
            format,
            destination_path: path.to_string_lossy().into_owned(),
            columns,
            filters,
            limit,
        })
        .map_err(adapter_execution)?;
    serde_json::to_value(result).map_err(|_| ToolCallError::Execution {
        code: "SERIALIZATION_ERROR",
        message: "Could not serialize export result.",
    })
}

fn db_query_readonly(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "connectionId",
            "sql",
            "limit",
            "workspaceId",
            "catalog",
            "schema",
            "timeoutMs",
        ],
    )?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.db.query_readonly")?;
    let sql = parse_required_string(&arguments, "sql", "unfour.db.query_readonly")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let limit = parse_optional_limit(&arguments, "limit", DEFAULT_QUERY_LIMIT, MAX_QUERY_LIMIT)?;
    let catalog = parse_optional_string(&arguments, "catalog")?;
    let schema = parse_optional_string(&arguments, "schema")?;
    let timeout_ms = parse_optional_u64(&arguments, "timeoutMs")?;

    // MCP-layer read-only validation (defense-in-depth).
    validate_readonly_sql(&sql)?;

    let input = DatabaseQueryInput {
        workspace_id,
        connection_id: connection_id.clone(),
        sql,
        limit: Some(limit),
        confirm_mutation: None,
        catalog,
        schema,
        timeout_ms,
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
        Err(error) => Err(adapter_execution(error)),
    }
}

fn db_execute(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "connectionId",
            "sql",
            "limit",
            "workspaceId",
            "catalog",
            "schema",
            "timeoutMs",
            "dryRun",
            "transaction",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let connection_id = parse_required_string(&arguments, "connectionId", "unfour.db.execute")?;
    let sql = parse_required_string(&arguments, "sql", "unfour.db.execute")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let limit = parse_optional_limit(&arguments, "limit", DEFAULT_QUERY_LIMIT, MAX_QUERY_LIMIT)?;
    let catalog = parse_optional_string(&arguments, "catalog")?;
    let schema = parse_optional_string(&arguments, "schema")?;
    let timeout_ms = parse_optional_u64(&arguments, "timeoutMs")?;
    let dry_run = parse_optional_bool(&arguments, "dryRun")?.unwrap_or(false);
    let transaction = parse_optional_bool(&arguments, "transaction")?.unwrap_or(false);
    validate_single_statement_for_execute(&sql)?;
    let risk = classify_sql_risk(&sql);

    if risk.requires_confirmation {
        ensure_confirmed_if_guarded(
            evaluation,
            &arguments,
            risk.confirmation_code,
            risk.reason,
            json!({
                "tool": "unfour.db.execute",
                "workspaceId": workspace_id,
                "connectionId": connection_id,
                "sql": sql,
                "catalog": catalog,
                "schema": schema,
                "transaction": transaction
            }),
        )?;
    }

    if dry_run {
        return Ok(json!({
            "ok": true,
            "connectionId": connection_id,
            "statementType": risk.statement_type,
            "affectedRows": 0,
            "columns": [],
            "rows": [],
            "rowCount": 0,
            "durationMs": 0,
            "dryRun": true,
            "transaction": transaction,
            "safety": {
                "classification": risk.statement_type,
                "requiresConfirmation": risk.requires_confirmation,
                "confirmed": risk.requires_confirmation,
                "message": risk.reason
            },
            "source": "command-bus"
        }));
    }

    let input = DatabaseQueryInput {
        workspace_id,
        connection_id: connection_id.clone(),
        sql,
        limit: Some(limit),
        // The MCP layer has already applied environment policy and the
        // content-bound confirmation token; pass the engine confirmation flag
        // so non-high-risk dev/test writes can execute through the shared path.
        confirm_mutation: Some(true),
        catalog,
        schema,
        timeout_ms,
    };

    match command_bus.execute_db_query(input) {
        Ok(result) => {
            let (truncated_rows, was_truncated) =
                truncate_query_rows(result.rows, MAX_QUERY_RESULT_BYTES);
            let columns = result_columns(&result.columns);

            Ok(json!({
                "ok": true,
                "connectionId": connection_id,
                "statementType": result.safety.classification,
                "affectedRows": result.affected_rows,
                "columns": columns,
                "rows": truncated_rows,
                "rowCount": truncated_rows.len(),
                "durationMs": result.duration_ms,
                "truncated": was_truncated,
                "dryRun": false,
                "transaction": transaction,
                "safety": result.safety,
                "source": "command-bus"
            }))
        }
        Err(error) => Err(adapter_execution(error)),
    }
}

fn db_explain(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "connectionId",
            "sql",
            "workspaceId",
            "limit",
            "catalog",
            "schema",
            "timeoutMs",
        ],
    )?;
    let connection_id = parse_required_string(&arguments, "connectionId", "unfour.db.explain")?;
    let sql = parse_required_string(&arguments, "sql", "unfour.db.explain")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let limit = parse_optional_limit(&arguments, "limit", DEFAULT_QUERY_LIMIT, MAX_QUERY_LIMIT)?;
    let catalog = parse_optional_string(&arguments, "catalog")?;
    let schema = parse_optional_string(&arguments, "schema")?;
    let timeout_ms = parse_optional_u64(&arguments, "timeoutMs")?;

    validate_readonly_sql(&sql)?;
    let explain_sql = if sql.trim_start().to_ascii_lowercase().starts_with("explain") {
        sql
    } else {
        format!("EXPLAIN {}", sql.trim().trim_end_matches(';'))
    };

    let input = DatabaseQueryInput {
        workspace_id,
        connection_id: connection_id.clone(),
        sql: explain_sql.clone(),
        limit: Some(limit),
        confirm_mutation: None,
        catalog,
        schema,
        timeout_ms,
    };

    match command_bus.execute_db_query(input) {
        Ok(result) => {
            let (truncated_rows, was_truncated) =
                truncate_query_rows(result.rows, MAX_QUERY_RESULT_BYTES);
            Ok(json!({
                "ok": true,
                "connectionId": connection_id,
                "sql": explain_sql,
                "columns": result_columns(&result.columns),
                "rows": truncated_rows,
                "rowCount": truncated_rows.len(),
                "durationMs": result.duration_ms,
                "truncated": was_truncated,
                "source": "command-bus"
            }))
        }
        Err(error) => Err(adapter_execution(error)),
    }
}

fn db_test_connection(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["connectionId", "workspaceId"])?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.db.test_connection")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;

    match command_bus.test_db_connection(&workspace_id, &connection_id) {
        Ok(result) => Ok(json!({
            "ok": result.ok,
            "connectionId": connection_id,
            "message": result.message,
            "serverVersion": result.server_version,
            "source": "command-bus"
        })),
        Err(error) => Err(adapter_execution(error)),
    }
}

#[cfg(test)]
#[path = "database_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "database_schema_tests.rs"]
mod schema_tests;

#[cfg(test)]
#[path = "database_validation_tests.rs"]
mod validation_tests;
