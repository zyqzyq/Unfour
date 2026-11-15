use super::*;
use crate::response::content_json;
use crate::tools::ToolRegistry;
use serde_json::{json, Value};

const SECRET: &str = "mcp-sentinel-6c8dbb09-ordinary-name";
const DISABLED: &str = "mcp-sentinel-e105b128-disabled";
const SENSITIVE: &str = "mcp-sentinel-710aaf38-password-false";
const COMPOSITE: &str = "mcp-sentinel-409fa361-db-password-false";

#[test]
fn environment_secret_projection_mask_does_not_echo_arbitrary_secret_fragments() {
    for value in ["CanaryScheme confidential", "Canary_suffix-16d2e5ab"] {
        assert!(!crate::sanitize::mask_secret(value).contains("Canary"));
    }
}

fn fixture() -> (Arc<LocalCommandBusAdapter>, ToolRegistry, String, String) {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let ReadCommandResult::CurrentWorkspace(workspace) =
        adapter.execute_read(ReadCommand::CurrentWorkspace).unwrap()
    else {
        panic!("workspace result");
    };
    let workspace_id = workspace.workspace_id;
    let environment = adapter
        .create_api_environment(&workspace_id, "Sentinel")
        .unwrap();
    for (index, (key, value, is_secret, is_enabled)) in [
        ("plain", "visible-value", false, true),
        ("display", SECRET, true, true),
        ("dormant", DISABLED, true, false),
        ("password", SENSITIVE, false, true),
        ("db_password", COMPOSITE, false, true),
    ]
    .into_iter()
    .enumerate()
    {
        let input = WorkspaceVariableInput {
            id: None,
            key: key.into(),
            value: value.into(),
            is_secret,
            is_enabled,
            description: None,
            sort_order: index as i64,
        };
        adapter
            .create_workspace_variable(&workspace_id, input.clone())
            .unwrap();
        adapter
            .create_api_environment_variable(&workspace_id, &environment.id, input)
            .unwrap();
    }
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    (adapter, registry, workspace_id, environment.id)
}

fn assert_safe(result: &Value) {
    assert_eq!(result["isError"], false);
    assert_eq!(content_json(result), result["structuredContent"]);
    for secret in [SECRET, DISABLED, SENSITIVE, COMPOSITE] {
        assert!(
            !result.to_string().contains(secret),
            "MCP result exposed sentinel"
        );
    }
}

#[test]
fn environment_secret_projection_lists_both_scopes_in_text_and_structured_content() {
    let (_adapter, registry, workspace_id, _) = fixture();
    for (tool, field) in [
        ("unfour.workspace.list_variables", "variables"),
        ("unfour.api.list_environments", "environments"),
    ] {
        let result = registry
            .call(tool, json!({"workspaceId": workspace_id}))
            .unwrap();
        assert_safe(&result);
        let variables = if field == "variables" {
            &result["structuredContent"][field]
        } else {
            &result["structuredContent"][field][0]["variables"]
        };
        assert_eq!(variables[0]["value"], "visible-value");
        assert_eq!(variables.as_array().unwrap().len(), 5);
    }
}

#[test]
fn environment_secret_projection_legacy_update_retains_explicit_secret_metadata() {
    let (adapter, registry, workspace_id, environment_id) = fixture();
    let result = registry
        .call(
            "unfour.api.update_environment",
            json!({
                "workspaceId": workspace_id,
                "environmentId": environment_id,
                "name": "Updated",
                "variables": [
                    {"key": "plain", "value": "visible-value", "enabled": true},
                    {"key": "display", "value": SECRET, "enabled": true},
                    {"key": "dormant", "value": DISABLED, "enabled": false},
                    {"key": "password", "value": SENSITIVE, "enabled": true}
                ]
            }),
        )
        .unwrap();
    assert_safe(&result);
    let stored = adapter.list_workspace_environments(&workspace_id).unwrap();
    assert!(stored[0].variables[1].is_secret);
    assert_eq!(stored[0].variables[1].value, SECRET);
    assert!(!stored[0].variables[2].is_enabled);
    assert_eq!(stored[0].variables[2].value, DISABLED);
}

#[test]
fn environment_secret_projection_single_variable_and_activity_are_safe() {
    let (_adapter, registry, workspace_id, environment_id) = fixture();
    let result = registry
        .call(
            "unfour.api.set_environment_variable",
            json!({
                "workspaceId": workspace_id, "environmentId": environment_id,
                "key": "display", "value": SECRET
            }),
        )
        .unwrap();
    assert_safe(&result);
    assert_eq!(result["structuredContent"]["variable"]["isSecret"], true);
    let result = registry
        .call("unfour.activity.list", json!({"workspaceId": workspace_id}))
        .unwrap();
    assert_safe(&result);
}

#[test]
fn environment_secret_projection_casing_change_does_not_remove_secret_classification() {
    let (adapter, registry, workspace_id, environment_id) = fixture();
    let result = registry
        .call(
            "unfour.api.update_environment",
            json!({
                "workspaceId": workspace_id, "environmentId": environment_id,
                "name": "Updated", "variables": [
                    {"key": " DISPLAY ", "value": SECRET, "enabled": true}
                ]
            }),
        )
        .unwrap();
    assert_safe(&result);
    let stored = adapter.list_workspace_environments(&workspace_id).unwrap();
    assert!(stored[0].variables[0].is_secret);
    assert_eq!(stored[0].variables[0].value, SECRET);
}

#[test]
fn environment_secret_projection_execution_uses_raw_credentials_but_masks_echoes() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;
    let (adapter, registry, workspace_id, environment_id) = fixture();
    // Explicitly allow the disposable workspace's loopback request under the existing policy.
    adapter
        .run(
            adapter
                .bus
                .update_workspace_environment(workspace_id.clone(), "dev".into()),
        )
        .unwrap();
    for saved_request in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 2048];
            while !bytes.windows(4).any(|chunk| chunk == b"\r\n\r\n") {
                let size = socket.read(&mut buffer).unwrap();
                assert!(size > 0);
                bytes.extend_from_slice(&buffer[..size]);
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request
                .to_lowercase()
                .contains(&format!("authorization: bearer {SECRET}")));
            let body = json!({"ordinary": SECRET}).to_string();
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nX-Display: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}", body.len(), SECRET, body).unwrap();
        });
        let mut arguments = json!({
            "workspaceId": workspace_id, "environmentId": environment_id,
            "method": "GET", "url": format!("http://{address}/"),
            "headers": [{"key":"Authorization", "value":"Bearer {{display}}", "enabled":true}],
            "timeoutMs": 5000
        });
        if saved_request {
            let mut input = arguments.clone();
            input.as_object_mut().unwrap().remove("environmentId");
            input["name"] = json!("Secret echo");
            input["query"] = json!([]);
            input["bodyKind"] = json!("none");
            input["postResponseScript"] = json!(format!("pm.test('raw response available', () => pm.expect(pm.response.json().ordinary).to.equal('{SECRET}'));"));
            let saved = adapter
                .save_api_request(serde_json::from_value(input).unwrap())
                .unwrap();
            arguments = json!({"workspaceId":workspace_id,"environmentId":environment_id,"requestId":saved.id,"timeoutMs":5000});
        }
        let result = registry.call("unfour.api.send_request", arguments).unwrap();
        server.join().unwrap();
        assert_safe(&result);
        assert_eq!(result["structuredContent"]["status"], 200);
        assert_eq!(
            result["structuredContent"]["sizeBytes"],
            json!({"ordinary": SECRET}).to_string().len()
        );
        for tool in ["unfour.api.list_history", "unfour.activity.list"] {
            let result = registry
                .call(tool, json!({"workspaceId":workspace_id}))
                .unwrap();
            assert_safe(&result);
            if tool == "unfour.api.list_history" {
                for item in result["structuredContent"]["history"].as_array().unwrap() {
                    let detail = registry
                        .call(
                            "unfour.api.get_history",
                            json!({
                                "workspaceId":workspace_id, "historyId": item["id"]
                            }),
                        )
                        .unwrap();
                    assert_safe(&detail);
                }
            }
        }
    }
}

#[test]
fn environment_secret_projection_failed_requests_keep_error_details_safe() {
    let (adapter, registry, workspace_id, environment_id) = fixture();
    adapter
        .run(
            adapter
                .bus
                .update_workspace_environment(workspace_id.clone(), "dev".into()),
        )
        .unwrap();
    let saved = adapter
        .save_api_request(
            serde_json::from_value(json!({
                "workspaceId":workspace_id, "name":"Failure", "method":"GET",
                "url":"http://127.0.0.1:1/", "headers":[], "query":[], "bodyKind":"none",
                "preRequestScript":"pm.request.method = pm.environment.get('display') + ' invalid';"
            }))
            .unwrap(),
        )
        .unwrap();
    for arguments in [
        json!({"workspaceId":workspace_id,"environmentId":environment_id,"requestId":saved.id,"timeoutMs":1000}),
        json!({"workspaceId":workspace_id,"environmentId":environment_id,"method":"GET", "url":"http://127.0.0.1:1/{{display}}?ordinary={{display}}", "timeoutMs":1000}),
    ] {
        let result = registry.call("unfour.api.send_request", arguments).unwrap();
        let error = crate::response::error_json(&result);
        assert!(error["error"]["code"].as_str().is_some());
        for sentinel in [SECRET, DISABLED, SENSITIVE] {
            assert!(!result.to_string().contains(sentinel));
        }
    }
}
