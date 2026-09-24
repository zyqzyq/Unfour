use super::*;
use crate::command_bus_adapter::CommandBusAdapterError;

pub(super) const DEFAULT_QUERY_LIMIT: u32 = 100;
pub(super) const MAX_QUERY_LIMIT: u32 = 1000;
pub(super) const DEFAULT_TABLE_LIMIT: u32 = 200;
pub(super) const MAX_TABLE_LIMIT: u32 = 500;
pub(super) const MAX_QUERY_RESULT_BYTES: usize = 20 * 1024;

// --- Connection summary sanitization ---

/// Convert a DatabaseConnection into a safe summary that excludes credentials,
/// usernames, and internal metadata.
pub(super) fn safe_connection_summary(conn: &DatabaseConnection) -> Value {
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
pub(super) fn truncate_query_rows(
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

pub(super) fn result_columns(columns: &[unfour_core::models::DatabaseResultColumn]) -> Vec<Value> {
    columns
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "dataType": c.data_type
            })
        })
        .collect()
}

// --- Helpers ---

pub(super) fn resolve_workspace_id(
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

pub(super) fn parse_required_string(
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

pub(super) fn parse_optional_string(
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

pub(super) fn parse_optional_u32(
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

pub(super) fn parse_optional_u64(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<u64>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::Number(n)) => {
            let val = n.as_u64().ok_or_else(|| {
                ToolCallError::InvalidArguments(format!(
                    "argument `{}` must be a positive integer",
                    key
                ))
            })?;
            Ok(Some(val))
        }
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a number",
            key
        ))),
    }
}

pub(super) fn parse_optional_bool(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<bool>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a boolean",
            key
        ))),
    }
}

pub(super) fn parse_optional_limit(
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

pub(super) fn adapter_execution(error: CommandBusAdapterError) -> ToolCallError {
    if error
        .details
        .as_object()
        .is_some_and(|details| !details.is_empty())
    {
        ToolCallError::ExecutionWithDetails {
            code: error.code,
            message: error.message,
            details: error.details,
        }
    } else {
        ToolCallError::Execution {
            code: error.code,
            message: error.message,
        }
    }
}

pub(super) fn parse_export_columns(
    arguments: &Map<String, Value>,
) -> Result<Option<Vec<String>>, ToolCallError> {
    match arguments.get("columns") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| {
                        ToolCallError::InvalidArguments(
                            "argument `columns` must be an array of column names".into(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `columns` must be an array of column names".into(),
        )),
    }
}

pub(super) fn parse_export_filters(
    arguments: &Map<String, Value>,
) -> Result<Vec<unfour_core::models::DatabaseExportFilter>, ToolCallError> {
    let Some(value) = arguments.get("filters") else {
        return Ok(Vec::new());
    };
    let Value::Array(items) = value else {
        return Err(ToolCallError::InvalidArguments(
            "argument `filters` must be an array".into(),
        ));
    };
    items
        .iter()
        .map(|item| {
            let Some(object) = item.as_object() else {
                return Err(ToolCallError::InvalidArguments(
                    "each export filter must be an object".into(),
                ));
            };
            if object
                .keys()
                .any(|key| !matches!(key.as_str(), "column" | "op" | "values"))
            {
                return Err(ToolCallError::InvalidArguments(
                    "unexpected export filter field".into(),
                ));
            }
            let column = object
                .get("column")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ToolCallError::InvalidArguments(
                        "export filter `column` must be a non-empty string".into(),
                    )
                })?
                .to_string();
            let op = match object.get("op").and_then(Value::as_str) {
                Some("eq") => unfour_core::models::DatabaseExportFilterOp::Eq,
                Some("in") => unfour_core::models::DatabaseExportFilterOp::In,
                _ => {
                    return Err(ToolCallError::InvalidArguments(
                        "export filter `op` must be eq or in".into(),
                    ))
                }
            };
            let values = object
                .get("values")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    ToolCallError::InvalidArguments(
                        "export filter `values` must be an array".into(),
                    )
                })?
                .iter()
                .map(export_filter_value)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(unfour_core::models::DatabaseExportFilter { column, op, values })
        })
        .collect()
}

fn export_filter_value(value: &Value) -> Result<Option<String>, ToolCallError> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text.clone())),
        Value::Number(number) => Ok(Some(number.to_string())),
        Value::Bool(flag) => Ok(Some(flag.to_string())),
        _ => Err(ToolCallError::InvalidArguments(
            "export filter values must be strings, numbers, booleans, or null".into(),
        )),
    }
}

pub(super) fn unexpected_result() -> ToolCallError {
    ToolCallError::Execution {
        code: "COMMAND_BUS_RESULT_MISMATCH",
        message: "The command-bus returned an unexpected result.",
    }
}
