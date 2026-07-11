use std::sync::Arc;

use serde_json::json;

use crate::tools::ToolRegistry;

use super::fixtures::{registry, UnsupportedSshCommandBus};

#[test]
fn run_diagnostic_returns_captured_output() {
    let result = registry()
        .call(
            "unfour.ssh.run_diagnostic",
            json!({ "connectionId": "conn-1", "command": "df -h" }),
        )
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    let content = &result["structuredContent"];
    assert_eq!(content["connectionId"], "conn-1");
    assert_eq!(content["command"], "df -h");
    assert_eq!(content["exitStatus"], 0);
    assert!(content["stdout"].as_str().unwrap().contains("Filesystem"));
    assert_eq!(content["source"], "command-bus");
}

#[test]
fn run_diagnostic_requires_connection_and_command() {
    assert!(registry()
        .call("unfour.ssh.run_diagnostic", json!({ "command": "df -h" }))
        .is_err());
    assert!(registry()
        .call(
            "unfour.ssh.run_diagnostic",
            json!({ "connectionId": "conn-1" })
        )
        .is_err());
}

#[test]
fn run_diagnostic_surfaces_unsupported_when_native_disabled() {
    let registry = ToolRegistry::with_command_bus(Arc::new(UnsupportedSshCommandBus));
    let result = registry
        .call(
            "unfour.ssh.run_diagnostic",
            json!({ "connectionId": "conn-1", "command": "uptime" }),
        )
        .expect("execution errors become MCP tool results");

    assert_eq!(result["isError"], true);
    assert_eq!(
        result["structuredContent"]["error"]["code"],
        "COMMAND_BUS_OPERATION_UNSUPPORTED"
    );
}
