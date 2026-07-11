use serde_json::json;

use super::fixtures::registry;

#[test]
fn exec_allows_dev_regular_command() {
    let result = registry()
        .call(
            "unfour.ssh.exec",
            json!({ "connectionId": "conn-1", "command": "echo ok" }),
        )
        .expect("dev command should execute");

    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["environment"], "dev");
    assert_eq!(result["structuredContent"]["risk_level"], "medium");
    assert!(result["structuredContent"]["stdout"]
        .as_str()
        .unwrap()
        .contains("echo ok"));
}

#[test]
fn exec_rm_rf_requires_confirmation_then_executes() {
    let first = registry()
        .call(
            "unfour.ssh.exec",
            json!({ "connectionId": "conn-1", "command": "rm -rf /tmp/app" }),
        )
        .expect("confirmation should be structured");
    assert_eq!(first["isError"], true);
    assert_eq!(first["structuredContent"]["requires_confirmation"], true);
    let confirmation = first["structuredContent"]["confirmation_text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(confirmation.starts_with("SSH_DELETE_COMMAND:"));

    let confirmed = registry()
        .call(
            "unfour.ssh.exec",
            json!({
                "connectionId": "conn-1",
                "command": "rm -rf /tmp/app",
                "confirm": true,
                "confirmation_text": confirmation
            }),
        )
        .expect("confirmed dev command should execute");
    assert_eq!(confirmed["isError"], false);
    assert!(confirmed["structuredContent"]["stdout"]
        .as_str()
        .unwrap()
        .contains("rm -rf /tmp/app"));
}
