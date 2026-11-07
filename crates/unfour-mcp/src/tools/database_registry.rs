//! MCP database definitions; execution and policy remain in database.rs.
use super::*;

pub(in crate::tools) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.db.export_table",
                title: "Export Database Table",
                description: "Exports one table to a file in Unfour's managed exports directory. Optional columns, structured filters, and limit apply to the data portion. Data exports without limit require content-bound confirmation; omitted limit still means all matching rows. Returns file metadata; no destination path is accepted.",
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
                        "limit": {"type": "integer", "minimum": 1, "description": "Optional maximum number of data rows. Omit for all matching rows, which requires confirmation when content includes data."},
                        "confirm": {"type": "boolean"},
                        "confirmationText": {"type": "string"},
                        "confirmation_text": {"type": "string"}
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
                    "Lists tables and views for a saved database connection through the Unfour command bus. Optional catalog and schema scope the listing. Requires a saved connectionId; does not accept ad-hoc connection strings.",
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
                        },
                        "schema": {
                            "type": "string",
                            "description": "Optional schema name. pg_catalog and information_schema are excluded even when specified."
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
                    "Describes a table's columns, available indexes, foreign keys and DDL, with runtime metadata capabilities, through the Unfour command bus. Does not read table data.",
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
                        "warnings": { "type": "array", "items": { "type": "string" } },
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

fn describe_table_output_schema() -> Value {
    json!({
        "type": "object", "properties": {
            "connectionId": {"type": "string"}, "source": {"type": "string", "const": "command-bus"},
            "table": {"type": "object", "properties": {
                "catalog": {"type": ["string", "null"]}, "schema": {"type": ["string", "null"]},
                "name": {"type": "string"}, "kind": {"type": "string"}, "ddl": {"type": ["string", "null"]},
                "columnCount": {"type": "integer", "minimum": 0},
                "capabilities": {"type": "object", "properties": {
                    "indexes": {"type": "boolean"}, "foreignKeys": {"type": "boolean"}, "ddl": {"type": "boolean"}
                }, "required": ["indexes", "foreignKeys", "ddl"], "additionalProperties": false},
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
            }, "required": ["catalog", "schema", "name", "kind", "ddl", "columnCount", "columns", "indexes", "foreignKeys", "capabilities"], "additionalProperties": false}
        }, "required": ["connectionId", "table", "source"], "additionalProperties": false
    })
}
