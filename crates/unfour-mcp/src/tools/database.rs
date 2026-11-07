use serde_json::{json, Map, Value};
use unfour_core::models::{
    DatabaseConnection, DatabaseExportContent, DatabaseExportFormat, DatabaseExportTableInput,
    DatabaseQueryInput, DatabaseTableStructureInput,
};

use crate::command_bus_adapter::CommandBusAdapter;

use super::policy::ToolPolicyEvaluation;
use super::{
    confirmation::{ensure_confirmed, ensure_confirmed_if_guarded},
    object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition,
};

#[path = "database_create.rs"]
mod database_create;

#[path = "database_helpers.rs"]
mod database_helpers;
#[path = "database_sql.rs"]
mod database_sql;

use database_helpers::*;
use database_sql::*;

#[path = "database_registry.rs"]
mod registry;
pub(super) use registry::registered_tools;

// --- Tool handlers ---

fn db_list_connections(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId"])?;
    let workspace_id = resolve_workspace_id(evaluation, &arguments)?;

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
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &["connectionId", "workspaceId", "limit", "catalog", "schema"],
    )?;
    let connection_id = parse_required_string(&arguments, "connectionId", "unfour.db.list_tables")?;
    let workspace_id = resolve_workspace_id(evaluation, &arguments)?;
    let limit = parse_optional_limit(&arguments, "limit", DEFAULT_TABLE_LIMIT, MAX_TABLE_LIMIT)?;
    let catalog = parse_optional_string(&arguments, "catalog")?;
    let schema = parse_optional_string(&arguments, "schema")?;

    let listing = command_bus
        .list_db_tables(
            &workspace_id,
            &connection_id,
            catalog.as_deref(),
            schema.as_deref(),
            limit,
        )
        .map_err(adapter_execution)?;

    let total = listing.total_tables;
    let tables: Vec<Value> = listing
        .tables
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "catalog": t.catalog,
                "schema": t.schema,
                "kind": t.kind,
                "columnCount": t.column_count
            })
        })
        .collect();

    let truncated = total > tables.len() as u64;

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
    evaluation: &ToolPolicyEvaluation,
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
    let workspace_id = resolve_workspace_id(evaluation, &arguments)?;
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

fn db_export_table(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
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
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.db.export_table")?;
    let table_name = parse_required_string(&arguments, "tableName", "unfour.db.export_table")?;
    let workspace_id = resolve_workspace_id(evaluation, &arguments)?;
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
    if !matches!(content, DatabaseExportContent::Structure) && limit.is_none() {
        ensure_confirmed(
            &arguments,
            "DB_EXPORT_UNBOUNDED_DATA",
            "Exporting all matching data rows can consume substantial resources and leave a persistent local file.",
            json!({
                "workspaceId": workspace_id,
                "connectionId": connection_id,
                "catalog": catalog,
                "schema": schema,
                "tableName": table_name,
                "content": content,
                "format": format,
                "columns": columns,
                "filters": filters,
                "limit": limit,
            }),
        )?;
    }
    // Completed files remain in the managed exports directory; the engine
    // removes only staging files left by failed exports.
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
        ],
    )?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.db.query_readonly")?;
    let sql = parse_required_string(&arguments, "sql", "unfour.db.query_readonly")?;
    let workspace_id = resolve_workspace_id(evaluation, &arguments)?;
    let limit = parse_optional_limit(&arguments, "limit", DEFAULT_QUERY_LIMIT, MAX_QUERY_LIMIT)?;
    let catalog = parse_optional_string(&arguments, "catalog")?;
    let schema = parse_optional_string(&arguments, "schema")?;
    let timeout_ms = parse_optional_u64(&arguments, "timeoutMs")?;

    // MCP-layer read-only validation (defense-in-depth). The engine classification
    // is authoritative; a statement that is not a proven read never reaches SQL.
    let driver = sql_driver_hint(command_bus, &workspace_id, &connection_id, &sql);
    validate_readonly_sql_for_driver(&sql, driver.as_deref())?;

    let input = DatabaseQueryInput {
        workspace_id,
        connection_id: connection_id.clone(),
        sql,
        limit: Some(limit),
        confirm_mutation: None,
        catalog,
        schema,
        timeout_ms,
        // The tool contract is read-only in every policy, including when the
        // saved connection itself is writable.
        enforce_read_only: true,
    };

    match command_bus.execute_db_query(input) {
        Ok(result) => {
            let (truncated_rows, was_truncated) = cap_query_rows(result.rows, result.truncated);

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
    let workspace_id = resolve_workspace_id(evaluation, &arguments)?;
    let limit = parse_optional_limit(&arguments, "limit", DEFAULT_QUERY_LIMIT, MAX_QUERY_LIMIT)?;
    let catalog = parse_optional_string(&arguments, "catalog")?;
    let schema = parse_optional_string(&arguments, "schema")?;
    let timeout_ms = parse_optional_u64(&arguments, "timeoutMs")?;
    let dry_run = parse_optional_bool(&arguments, "dryRun")?.unwrap_or(false);
    let transaction = parse_optional_bool(&arguments, "transaction")?.unwrap_or(false);
    let driver = sql_driver_hint(command_bus, &workspace_id, &connection_id, &sql);
    validate_single_statement_for_execute(&sql, driver.as_deref())?;
    let proven_read = sql_is_proven_read(&sql, driver.as_deref());
    let risk = classify_sql_risk(&sql, driver.as_deref());
    // Policy and this handler share the engine classifier. If policy still saw a
    // read, do not execute a statement this check can no longer prove is read-only.
    if !proven_read && evaluation.risk == super::policy::McpRisk::Read {
        return Err(ToolCallError::Execution {
            code: "READONLY_SQL_REJECTED",
            message: "Only read-only SQL is permitted in this workspace.",
        });
    }

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
        // Proven reads do not carry a mutation confirmation. Writes receive one
        // only after workspace policy allowed the write and any required
        // content-bound confirmation succeeded.
        confirm_mutation: if proven_read { None } else { Some(true) },
        catalog,
        schema,
        timeout_ms,
        // Read-only and production-auto workspaces stay constrained even when
        // this tool is allowed to run a proven read. Writable policies keep
        // normal statement execution.
        enforce_read_only: evaluation.resolved_policy == super::policy::ResolvedMcpPolicy::ReadOnly,
    };

    match command_bus.execute_db_query(input) {
        Ok(result) => {
            let (truncated_rows, was_truncated) = cap_query_rows(result.rows, result.truncated);
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
                "warnings": result.warnings,
                "source": "command-bus"
            }))
        }
        Err(error) => Err(adapter_execution(error)),
    }
}

fn db_explain(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
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
    let workspace_id = resolve_workspace_id(evaluation, &arguments)?;
    let limit = parse_optional_limit(&arguments, "limit", DEFAULT_QUERY_LIMIT, MAX_QUERY_LIMIT)?;
    let catalog = parse_optional_string(&arguments, "catalog")?;
    let schema = parse_optional_string(&arguments, "schema")?;
    let timeout_ms = parse_optional_u64(&arguments, "timeoutMs")?;

    let driver = sql_driver_hint(command_bus, &workspace_id, &connection_id, &sql);
    validate_readonly_sql_for_driver(&sql, driver.as_deref())?;
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
        enforce_read_only: true,
    };

    match command_bus.execute_db_query(input) {
        Ok(result) => {
            let (truncated_rows, was_truncated) = cap_query_rows(result.rows, result.truncated);
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

/// Resolve the saved connection driver only when the statement is not already
/// a proven read on every dialect. Universal reads never need a connection lookup.
fn sql_driver_hint(
    command_bus: &dyn CommandBusAdapter,
    workspace_id: &str,
    connection_id: &str,
    sql: &str,
) -> Option<String> {
    if unfour_command_bus::sql_is_proven_read_for_driver(sql, None)
        && unfour_command_bus::sql_is_single_statement_for_driver(sql, None)
    {
        return None;
    }
    command_bus
        .list_db_connections(workspace_id)
        .ok()
        .and_then(|connections| {
            connections
                .into_iter()
                .find(|connection| {
                    connection.id == connection_id
                        && connection.workspace_id == workspace_id
                        && connection.deleted_at.is_none()
                })
                .map(|connection| connection.driver)
        })
}

fn db_test_connection(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["connectionId", "workspaceId"])?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.db.test_connection")?;
    let workspace_id = resolve_workspace_id(evaluation, &arguments)?;

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
