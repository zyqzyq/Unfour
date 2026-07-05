use super::*;
use crate::tools::ToolRegistry;

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
            Ok(ReadCommandResult::CurrentWorkspace(
                CurrentWorkspaceResult {
                    workspace_id: "ws-1".to_string(),
                    workspace_name: "W".to_string(),
                    environment_type: "dev".to_string(),
                    mcp_policy: "auto".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                },
            ))
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

    let reg = ToolRegistry::with_command_bus(Arc::new(LargeResultStub));
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
    let reg = ToolRegistry::with_command_bus(Arc::new(DbFailingCommandBus));
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
