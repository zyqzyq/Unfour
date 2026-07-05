use super::*;

// --- test_connection tests ---

#[test]
fn test_connection_returns_ok_with_server_version() {
    let result = registry()
        .call(
            "unfour.db.test_connection",
            json!({ "connectionId": "conn-1" }),
        )
        .expect("should succeed");

    let content = &result["structuredContent"];
    assert_eq!(content["ok"], true);
    assert_eq!(content["connectionId"], "conn-1");
    assert_eq!(content["message"], "Connection successful");
    assert_eq!(content["serverVersion"], "PostgreSQL 16.1");
    assert_eq!(content["source"], "command-bus");
}

#[test]
fn test_connection_requires_connection_id() {
    let result = registry().call("unfour.db.test_connection", json!({}));
    assert!(result.is_err(), "should fail without connectionId");
}
