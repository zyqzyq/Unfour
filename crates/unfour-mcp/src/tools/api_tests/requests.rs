use super::*;

// --- list_collections tests ---

#[test]
fn list_collections_returns_structured_result() {
    let result = api_registry()
        .call("unfour.api.list_collections", json!({}))
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["count"], 1);
    assert_eq!(
        result["structuredContent"]["collections"][0]["name"],
        "Users"
    );
    assert_eq!(
        result["structuredContent"]["collections"][0]["requestCount"],
        3
    );
    assert_eq!(result["structuredContent"]["source"], "command-bus");
}

// --- list_requests tests ---

#[test]
fn list_requests_redacts_sensitive_url_params() {
    let result = api_registry()
        .call("unfour.api.list_requests", json!({}))
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    let requests = &result["structuredContent"]["requests"];
    assert_eq!(requests[0]["id"], "req-1");
    // token should be redacted in urlPreview
    let url_preview = requests[0]["urlPreview"].as_str().unwrap();
    assert!(
        url_preview.contains("token=[mask "),
        "token should be masked in urlPreview"
    );
    assert!(
        !url_preview.contains("secret123"),
        "raw token should not appear"
    );
    assert!(url_preview.contains("page=1"), "safe params preserved");
}

// --- get_request tests ---

#[test]
fn get_request_redacts_sensitive_data() {
    let result = api_registry()
        .call("unfour.api.get_request", json!({ "requestId": "req-1" }))
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    let request = &result["structuredContent"]["request"];

    // URL query params masked
    let url = request["url"].as_str().unwrap();
    assert!(
        url.contains("api_key=[mask "),
        "api_key should be masked in URL"
    );
    assert!(!url.contains("=secret"), "raw secret should not appear");

    // Authorization header masked (scheme preserved for diagnosis)
    let headers = request["headers"].as_array().unwrap();
    let auth_header = headers
        .iter()
        .find(|h| h["key"] == "Authorization")
        .unwrap();
    let auth_value = auth_header["value"].as_str().unwrap();
    assert!(auth_value.starts_with("[mask "));
    assert!(auth_value.contains("scheme=Bearer"));
    assert!(!auth_value.contains("secret-token"));

    // Content-Type preserved
    let ct_header = headers.iter().find(|h| h["key"] == "Content-Type").unwrap();
    assert_eq!(ct_header["value"], "application/json");

    // Query param token masked
    let query = request["query"].as_array().unwrap();
    let token_param = query.iter().find(|q| q["key"] == "token").unwrap();
    assert!(token_param["value"].as_str().unwrap().starts_with("[mask "));

    // Body password masked
    let body = request["bodyPreview"].as_str().unwrap();
    assert!(body.contains("[mask "), "password should be masked in body");
    assert!(
        !body.contains("secret123"),
        "raw password should not appear"
    );
    assert!(body.contains("test"), "non-sensitive body values preserved");

    assert_eq!(request["collectionId"], "users");
    assert_eq!(result["structuredContent"]["source"], "command-bus");
}

#[test]
fn get_request_requires_request_id() {
    let result = api_registry().call("unfour.api.get_request", json!({}));
    assert!(result.is_err(), "should fail without requestId");
}

#[test]
fn request_update_keeps_an_edit_committed_after_policy_evaluation() {
    let adapter =
        crate::LocalCommandBusAdapter::from_storage_mode(crate::StorageMode::Ephemeral).unwrap();
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    let created = registry
        .call(
            "unfour.api.create_request",
            json!({
                "name": "Catalog", "method": "GET", "url": "https://example.test/catalog",
            }),
        )
        .unwrap();
    let id = created["structuredContent"]["request"]["id"]
        .as_str()
        .unwrap();
    let rename = json!({"requestId": id, "name": "Renamed"});
    let tool = "unfour.api.update_request";
    let evaluation =
        crate::tools::policy::evaluate_tool_policy(adapter.as_ref(), tool, &rename).unwrap();

    let edit = registry
        .call(
            tool,
            json!({"requestId": id, "url": "https://example.test/items"}),
        )
        .unwrap();
    assert_eq!(edit["isError"], false);
    assert!(matches!(
        api_update_request(adapter.as_ref(), &evaluation, rename),
        Err(ToolCallError::Execution {
            code: "ROW_CONFLICT",
            ..
        })
    ));

    let detail = registry
        .call("unfour.api.get_request", json!({"requestId": id}))
        .unwrap();
    let request = &detail["structuredContent"]["request"];
    assert_eq!(request["name"], "Catalog");
    assert_eq!(request["url"], "https://example.test/items");
}
