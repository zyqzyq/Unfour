use super::*;
use crate::tools::ToolRegistry;

// --- execute tests ---

#[test]
fn execute_allows_dev_update_with_where() {
    let result = registry()
        .call(
            "unfour.db.execute",
            json!({
                "connectionId": "conn-1",
                "sql": "UPDATE users SET email = 'new@example.com' WHERE id = 1"
            }),
        )
        .expect("dev update should execute");

    let content = &result["structuredContent"];
    assert_eq!(result["isError"], false);
    crate::response::assert_call_meta(&result, "dev", "medium");
    assert_eq!(content["affectedRows"], 2);
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
