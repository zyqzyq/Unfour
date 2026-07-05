use super::*;
use crate::tools::ToolRegistry;

// --- list_connections tests ---

#[test]
fn create_connection_stores_password_as_credential_and_returns_safe_summary() {
    let result = registry()
        .call(
            "unfour.db.create_connection",
            json!({
                "name": "Remote PG15",
                "driver": "postgres",
                "host": "192.168.57.128",
                "port": 5432,
                "database": "unfour",
                "username": "unfour",
                "password": "test-db-password",
                "sslMode": "disable"
            }),
        )
        .expect("create connection should succeed");

    assert_eq!(result["isError"], false);
    let content = &result["structuredContent"];
    assert_eq!(content["credentialStored"], true);
    assert_eq!(content["credentialSource"], "created");
    assert_eq!(content["connection"]["id"], "created-db-1");
    assert_eq!(content["connection"]["name"], "Remote PG15");
    assert_eq!(content["connection"]["databaseType"], "postgres");
    assert_eq!(content["connection"]["host"], "192.168.57.128");
    assert_eq!(content["connection"]["port"], 5432);
    assert_eq!(content["connection"]["database"], "unfour");

    let serialized = serde_json::to_string(content).unwrap();
    assert!(!serialized.contains("test-db-password"));
    assert!(!serialized.contains("credentialRef"));
    assert!(!serialized.contains("database-password:cred-1"));
}

#[test]
fn create_connection_rejects_password_and_credential_ref_together() {
    let result = registry().call(
        "unfour.db.create_connection",
        json!({
            "name": "Remote PG15",
            "driver": "postgres",
            "credentialRef": "unfour:workspace-1:database-password:cred-1",
            "password": "test-db-password"
        }),
    );
    assert!(result.is_err());
}

#[test]
fn create_connection_prod_workspace_is_blocked_by_policy() {
    let registry = ToolRegistry::with_command_bus(Arc::new(ProdDbStubCommandBus));
    let result = registry
        .call(
            "unfour.db.create_connection",
            json!({
                "name": "Remote PG15",
                "driver": "postgres",
                "host": "192.168.57.128",
                "port": 5432
            }),
        )
        .expect("policy denial should be structured");

    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"]["ok"], false);
    assert_eq!(
        result["structuredContent"]["error"]["code"],
        "WORKSPACE_POLICY_BLOCKED"
    );
    assert_eq!(result["structuredContent"]["environment"], "prod");
}

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

    let reg = ToolRegistry::with_command_bus(Arc::new(EmptyDbStub));
    let result = reg
        .call("unfour.db.list_connections", json!({}))
        .expect("should succeed");
    assert_eq!(result["structuredContent"]["count"], 0);
}
