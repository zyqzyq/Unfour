use super::*;
use crate::{response::content_json, ToolRegistry};
use serde_json::json;
use unfour_core::models::ApiRequestInput;

#[test]
fn saved_script_failure_returns_safe_diagnostics_through_the_real_mcp_registry() {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let workspace = adapter
        .run(adapter.bus.create_workspace_with_options(
            "Script diagnostics".into(),
            Some("dev".into()),
            Some("auto".into()),
        ))
        .unwrap()
        .id;
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    for tail in [
        "throw new Error(value)",
        "pm.request.method = ''",
        "Object.prototype.toJSON = function() { throw new Error(value); }",
    ] {
        let input: ApiRequestInput = serde_json::from_value(json!({
            "workspaceId": workspace,
            "name": "Script fixture",
            "method": "GET",
            "url": "http://127.0.0.1:1/never",
            "headers": [], "query": [], "bodyKind": "none",
            "preRequestScript": format!(r#"
                const value = ["mcp-generated", "canary"].join("-");
                pm.variables.set("ordinary", value);
                pm.request.headers.upsert({{key:"X-Opaque", value}});
                console.log(value);
                pm.test(value, () => pm.expect(value).to.equal("other"));
                {tail}
            "#),
        }))
        .unwrap();
        let saved = adapter.save_api_request(input).unwrap();
        let result = registry
            .call(
                "unfour.api.send_request",
                json!({
                    "workspaceId": workspace, "requestId": saved.id,
                }),
            )
            .unwrap();
        assert_eq!(
            content_json(&result)["error"]["code"],
            "API_SCRIPT_EXECUTION_FAILED"
        );
        assert!(!result.to_string().contains("ACTIVITY_RECORD_FAILED"));
        assert!(!result.to_string().contains("mcp-generated-canary"));
        assert!(adapter
            .run(adapter.bus.list_api_history(workspace.clone(), Some(10)))
            .unwrap()
            .is_empty());
    }
}
