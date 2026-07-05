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
    assert_eq!(content["environment"], "dev");
    assert_eq!(content["risk_level"], "medium");
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
    assert_eq!(first["structuredContent"]["requires_confirmation"], true);
    let confirmation = first["structuredContent"]["confirmation_text"]
        .as_str()
        .unwrap()
        .to_string();
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
    assert_eq!(result["structuredContent"]["ok"], false);
    assert_eq!(
        result["structuredContent"]["error"]["code"],
        "WORKSPACE_POLICY_BLOCKED"
    );
    assert_eq!(result["structuredContent"]["environment"], "prod");
}
