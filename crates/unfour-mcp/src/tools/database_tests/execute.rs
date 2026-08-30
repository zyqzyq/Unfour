use super::*;
use crate::tools::ToolRegistry;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Default)]
struct RecordingBus {
    read_only: AtomicBool,
    writes: Mutex<Vec<DatabaseQueryInput>>,
}

impl CommandBusAdapter for RecordingBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        let mut result = DbStubCommandBus.execute_read(command)?;
        let policy = if self.read_only.load(Ordering::SeqCst) {
            "read_only"
        } else {
            "guarded"
        };
        match &mut result {
            ReadCommandResult::CurrentWorkspace(workspace) => workspace.mcp_policy = policy.into(),
            ReadCommandResult::Workspaces(list) => {
                list.workspaces[0].mcp_policy = policy.into();
                let mut second = list.workspaces[0].clone();
                second.id = "workspace-2".into();
                second.is_active = false;
                list.workspaces.push(second);
                list.count = 2;
            }
            _ => {}
        }
        Ok(result)
    }

    fn execute_saved_api_request(
        &self,
        _: &str,
        _: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        panic!("database confirmation must not send API requests")
    }

    fn execute_db_query(
        &self,
        input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        let result = DbStubCommandBus.execute_db_query(input.clone());
        self.writes.lock().unwrap().push(input);
        result
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
}

fn request_confirmation(registry: &ToolRegistry) -> serde_json::Value {
    let request =
        json!({"workspaceId":"workspace-1", "connectionId":"conn-1", "sql":"DELETE FROM users"});
    let result = registry.call("unfour.db.execute", request.clone()).unwrap();
    let error = crate::response::error_json(&result);
    assert_eq!(error["error"]["code"], "CONFIRMATION_REQUIRED");
    let mut confirmed = request;
    confirmed["confirm"] = json!(true);
    confirmed["confirmation_text"] = error["confirmation_text"].clone();
    confirmed
}

#[test]
fn confirmation_is_bound_to_workspace_connection_sql_catalog_schema_and_transaction() {
    let bus = Arc::new(RecordingBus::default());
    let registry = ToolRegistry::with_command_bus(bus.clone());
    let confirmed = request_confirmation(&registry);
    assert!(bus.writes.lock().unwrap().is_empty());
    for (key, value) in [
        ("workspaceId", json!("workspace-2")),
        ("connectionId", json!("conn-2")),
        ("sql", json!("DELETE FROM orders")),
        ("catalog", json!("production")),
        ("schema", json!("private")),
        ("transaction", json!(true)),
    ] {
        let mut changed = confirmed.clone();
        changed[key] = value;
        let result = registry.call("unfour.db.execute", changed).unwrap();
        assert_eq!(
            crate::response::error_json(&result)["error"]["code"],
            "CONFIRMATION_REQUIRED",
            "changed {key}"
        );
        assert!(
            bus.writes.lock().unwrap().is_empty(),
            "changed {key} reached execution"
        );
    }
    let result = registry.call("unfour.db.execute", confirmed).unwrap();
    assert_eq!(result["isError"], false);
    let writes = bus.writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].workspace_id, "workspace-1");
    assert_eq!(writes[0].confirm_mutation, Some(true));
}

#[test]
fn confirmed_dry_run_never_executes_and_policy_is_rechecked_on_retry() {
    let bus = Arc::new(RecordingBus::default());
    let registry = ToolRegistry::with_command_bus(bus.clone());
    let confirmed = request_confirmation(&registry);
    let mut dry_run = confirmed.clone();
    dry_run["dryRun"] = json!(true);
    let result = registry.call("unfour.db.execute", dry_run).unwrap();
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["dryRun"], true);
    assert!(bus.writes.lock().unwrap().is_empty());
    bus.read_only.store(true, Ordering::SeqCst);
    let result = registry.call("unfour.db.execute", confirmed).unwrap();
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "WORKSPACE_POLICY_BLOCKED"
    );
    assert!(
        bus.writes.lock().unwrap().is_empty(),
        "old confirmation cannot override current policy"
    );
}

// --- execute tests ---

#[test]
fn execute_allows_dev_update_with_where() {
    let registry = registry();
    let result = registry
        .call(
            "unfour.db.execute",
            json!({
                "connectionId": "conn-1",
                "sql": "UPDATE users SET email = 'new@example.com' WHERE id = 1"
            }),
        )
        .expect("dev update should execute");
    crate::output_schema::assert_success_matches_output_schema(
        &registry,
        "unfour.db.execute",
        &result,
    );

    let content = &result["structuredContent"];
    assert_eq!(result["isError"], false);
    crate::response::assert_call_meta(&result, "dev", "medium");
    assert_eq!(content["affectedRows"], 2);
    assert_eq!(content["truncated"], false);
    assert_eq!(content["transaction"], false);
    assert_eq!(content["safety"]["confirmed"], true);
}

#[test]
fn execute_delete_without_where_requires_confirmation_then_executes() {
    let first = registry()
        .call(
            "unfour.db.execute",
            json!({ "connectionId": "conn-1", "sql": "DELETE FROM users" }),
        )
        .expect("confirmation should be structured");

    assert_eq!(first["isError"], true);
    let payload = crate::response::error_json(&first);
    assert_eq!(payload["requires_confirmation"], true);
    assert_eq!(payload["error"]["code"], "CONFIRMATION_REQUIRED");
    crate::response::assert_call_meta(&first, "dev", "high");
    let confirmation = payload["confirmation_text"].as_str().unwrap().to_string();
    assert!(confirmation.starts_with("DELETE_WITHOUT_WHERE:"));

    let confirmed = registry()
        .call(
            "unfour.db.execute",
            json!({
                "connectionId": "conn-1",
                "sql": "DELETE FROM users",
                "confirm": true,
                "confirmation_text": confirmation
            }),
        )
        .expect("confirmed delete should execute in dev");
    assert_eq!(confirmed["isError"], false);
    assert_eq!(confirmed["structuredContent"]["affectedRows"], 2);
}

#[test]
fn execute_prod_update_is_blocked_by_policy() {
    let registry = ToolRegistry::with_command_bus(Arc::new(ProdDbStubCommandBus));
    let result = registry
        .call(
            "unfour.db.execute",
            json!({
                "connectionId": "conn-1",
                "sql": "UPDATE users SET email = 'new@example.com' WHERE id = 1"
            }),
        )
        .expect("policy denial should be structured");

    assert_eq!(result["isError"], true);
    let payload = crate::response::error_json(&result);
    assert!(payload.get("ok").is_none());
    assert_eq!(payload["error"]["code"], "WORKSPACE_POLICY_BLOCKED");
    crate::response::assert_call_meta(&result, "prod", "medium");
}
