use serde_json::json;

use super::{
    assert_call_meta, assert_success_payload_is_business_value, content_json, error_json,
    structured_confirmation_required, structured_policy_error, structured_tool_error,
    structured_tool_result,
};

#[test]
fn success_result_uses_business_value_as_structured_content() {
    let value = json!({
        "connections": [],
        "count": 0,
        "source": "command-bus"
    });
    let result =
        structured_tool_result("unfour.db.list_connections", "dev", "low", 3, value.clone());

    assert_success_payload_is_business_value(&result, &value);
    assert_call_meta(&result, "dev", "low");
    assert_eq!(result["_meta"]["tool"], "unfour.db.list_connections");
    assert_eq!(result["_meta"]["durationMs"], 3);
    for field in [
        "ok",
        "tool",
        "environment",
        "risk_level",
        "duration_ms",
        "data",
        "warnings",
        "redactions",
    ] {
        assert!(
            result["structuredContent"].get(field).is_none(),
            "success structuredContent must not include envelope field `{field}`"
        );
    }
}

#[test]
fn execution_error_omits_structured_content() {
    let result = structured_tool_error(
        "unfour.workspace.current",
        "dev",
        "low",
        4,
        "COMMAND_BUS_READ_FAILED",
        "The command-bus read operation failed.",
    );

    let payload = error_json(&result);
    assert_eq!(payload["error"]["code"], "COMMAND_BUS_READ_FAILED");
    assert_eq!(
        payload["error"]["message"],
        "The command-bus read operation failed."
    );
    assert_call_meta(&result, "dev", "low");
    assert_eq!(&content_json(&result), &payload);
}

#[test]
fn policy_error_keeps_denial_fields_in_text_payload() {
    let denial = json!({
        "blocked": true,
        "reason": "Production workspaces are read-only.",
        "error": {
            "code": "WORKSPACE_POLICY_BLOCKED",
            "message": "blocked"
        },
        "workspaceId": "workspace-1",
        "workspaceName": "Local Workspace",
        "environmentType": "prod",
        "mcpPolicy": "auto",
        "resolvedPolicy": "read_only",
        "capability": "ssh:exec",
        "risk": "execute",
        "riskLevel": "medium"
    });
    let result = structured_policy_error("unfour.ssh.exec", "prod", "medium", 2, denial);

    let payload = error_json(&result);
    assert_eq!(payload["error"]["code"], "WORKSPACE_POLICY_BLOCKED");
    assert_eq!(payload["blocked"], true);
    assert_eq!(payload["workspaceId"], "workspace-1");
    assert_eq!(payload["environmentType"], "prod");
    assert_eq!(payload["capability"], "ssh:exec");
    assert!(payload.get("ok").is_none());
    assert!(payload.get("environment").is_none());
    assert!(payload.get("risk_level").is_none());
    assert_call_meta(&result, "prod", "medium");
}

#[test]
fn confirmation_required_exposes_confirmation_text_in_content() {
    let confirmation = json!({
        "riskLevel": "high",
        "reason": "Destructive SQL requires confirmation.",
        "confirmationText": "DELETE_WITHOUT_WHERE:abcd1234",
        "confirmationHint": "Re-run with confirm=true and the exact confirmation_text.",
        "details": {
            "riskCode": "DELETE_WITHOUT_WHERE",
            "dryRun": true
        }
    });
    let result =
        structured_confirmation_required("unfour.db.execute", "test", "high", 5, confirmation);

    let payload = error_json(&result);
    assert_eq!(payload["error"]["code"], "CONFIRMATION_REQUIRED");
    assert_eq!(payload["requires_confirmation"], true);
    assert_eq!(
        payload["confirmation_text"],
        "DELETE_WITHOUT_WHERE:abcd1234"
    );
    assert_eq!(
        payload["confirmation_hint"],
        "Re-run with confirm=true and the exact confirmation_text."
    );
    assert_eq!(payload["reason"], "Destructive SQL requires confirmation.");
    assert_call_meta(&result, "test", "high");
}
