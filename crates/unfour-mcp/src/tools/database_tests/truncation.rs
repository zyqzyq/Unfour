use std::sync::Arc;

use super::*;
use crate::tools::database::MAX_QUERY_RESULT_BYTES;
use crate::tools::ToolRegistry;

struct TruncationBus {
    engine_truncated: bool,
    row_limit: u32,
    rows: Vec<Vec<Option<String>>>,
}

impl CommandBusAdapter for TruncationBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        DbStubCommandBus.execute_read(command)
    }

    fn execute_saved_api_request(
        &self,
        _: &str,
        _: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        unreachable!("query truncation must not send API requests")
    }

    fn list_db_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        DbStubCommandBus.list_db_connections(workspace_id)
    }

    fn get_db_schema(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        DbStubCommandBus.get_db_schema(workspace_id, connection_id)
    }

    fn execute_db_query(
        &self,
        _: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        Ok(DatabaseQueryResult {
            warnings: Vec::new(),
            columns: vec![DatabaseResultColumn {
                name: "value".to_string(),
                data_type: "text".to_string(),
            }],
            rows: self.rows.clone(),
            affected_rows: 0,
            duration_ms: 3,
            safety: DatabaseQuerySafety {
                classification: "read".to_string(),
                requires_confirmation: false,
                confirmed: true,
                message: None,
            },
            truncated: self.engine_truncated,
            row_limit: self.row_limit,
        })
    }
}

fn small_rows(count: usize) -> Vec<Vec<Option<String>>> {
    (0..count)
        .map(|index| vec![Some(index.to_string())])
        .collect()
}

fn oversized_rows() -> Vec<Vec<Option<String>>> {
    let cell = "x".repeat(1024);
    (0..100)
        .map(|index| vec![Some(index.to_string()), Some(cell.clone())])
        .collect()
}

/// One unescaped cell whose JSON array is exactly `target` bytes.
fn rows_with_json_len(target: usize) -> Vec<Vec<Option<String>>> {
    let rows = vec![vec![Some("a".repeat(target - 6))]];
    assert_eq!(serde_json::to_string(&rows).unwrap().len(), target);
    rows
}

fn assert_tool_truncated(
    tool: &str,
    engine_truncated: bool,
    rows: Vec<Vec<Option<String>>>,
    row_limit: u32,
    expect_truncated: bool,
    expect_fewer_rows: bool,
) -> serde_json::Value {
    let original_len = rows.len();
    let bus = Arc::new(TruncationBus {
        engine_truncated,
        row_limit,
        rows,
    });
    let registry = ToolRegistry::with_command_bus(bus);
    let result = registry
        .call(
            tool,
            json!({
                "connectionId": "conn-1",
                "sql": "SELECT value FROM samples"
            }),
        )
        .unwrap_or_else(|error| panic!("{tool} failed: {error:?}"));
    assert_eq!(result["isError"], false, "{tool}");
    crate::output_schema::assert_success_matches_output_schema(&registry, tool, &result);
    let content = &result["structuredContent"];
    assert_eq!(content["truncated"], expect_truncated, "{tool}");
    let row_count = content["rowCount"].as_u64().unwrap() as usize;
    assert_eq!(
        content["rows"].as_array().unwrap().len(),
        row_count,
        "{tool}"
    );
    if expect_fewer_rows {
        assert!(row_count < original_len, "{tool}");
        let serialized = serde_json::to_string(&content["rows"]).unwrap();
        assert!(serialized.len() <= MAX_QUERY_RESULT_BYTES, "{tool}");
    } else {
        assert_eq!(row_count, original_len, "{tool}");
    }
    content.clone()
}

const QUERY_TOOLS: [&str; 3] = [
    "unfour.db.query_readonly",
    "unfour.db.execute",
    "unfour.db.explain",
];

#[test]
fn query_tools_report_row_truncation_without_a_byte_cap() {
    for tool in QUERY_TOOLS {
        assert_tool_truncated(tool, true, small_rows(2), 2, true, false);
    }
}

#[test]
fn query_tools_report_byte_truncation_without_a_row_cap() {
    for tool in QUERY_TOOLS {
        assert_tool_truncated(tool, false, oversized_rows(), 1000, true, true);
    }
}

#[test]
fn query_tools_report_truncation_when_both_limits_apply() {
    for tool in QUERY_TOOLS {
        assert_tool_truncated(tool, true, oversized_rows(), 100, true, true);
    }
}

#[test]
fn query_tools_leave_truncation_false_when_neither_limit_applies() {
    for tool in QUERY_TOOLS {
        assert_tool_truncated(tool, false, small_rows(2), 100, false, false);
    }
}

#[test]
fn query_tools_do_not_truncate_a_result_that_exactly_meets_both_caps() {
    let rows = rows_with_json_len(MAX_QUERY_RESULT_BYTES);
    for tool in QUERY_TOOLS {
        // Engine `truncated == false` with `row_limit == row count` is the exact
        // row-cap outcome. This payload is also exactly the MCP byte budget.
        let content = assert_tool_truncated(tool, false, rows.clone(), 1, false, false);
        assert_eq!(
            serde_json::to_string(&content["rows"]).unwrap().len(),
            MAX_QUERY_RESULT_BYTES,
            "{tool}"
        );
    }
}

#[test]
fn query_tools_truncate_when_the_byte_cap_is_exceeded_by_one() {
    let rows = rows_with_json_len(MAX_QUERY_RESULT_BYTES + 1);
    for tool in QUERY_TOOLS {
        assert_tool_truncated(tool, false, rows.clone(), 1, true, false);
    }
}
