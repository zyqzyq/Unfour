use super::*;

// --- history tests ---

#[test]
fn list_history_masks_url_and_returns_status() {
    let result = api_registry()
        .call("unfour.api.list_history", json!({}))
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    let content = &result["structuredContent"];
    assert_eq!(content["count"], 1);
    let item = &content["history"][0];
    assert_eq!(item["status"], 500);
    let url = item["url"].as_str().unwrap();
    assert!(url.contains("token=[mask "), "token should be masked");
    assert!(!url.contains("secret123"), "raw token should not appear");
    assert!(url.contains("page=2"), "safe params preserved");
}

#[test]
fn get_history_masks_request_and_response() {
    let result = api_registry()
        .call("unfour.api.get_history", json!({ "historyId": "hist-1" }))
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    let h = &result["structuredContent"]["history"];
    assert_eq!(h["status"], 401);

    let url = h["url"].as_str().unwrap();
    assert!(url.contains("api_key=[mask "));
    assert!(!url.contains("=secret"));

    let req_headers = h["requestHeaders"].as_array().unwrap();
    let auth = req_headers
        .iter()
        .find(|x| x["key"] == "Authorization")
        .unwrap();
    let auth_val = auth["value"].as_str().unwrap();
    assert!(auth_val.starts_with("[mask "));
    assert!(auth_val.contains("scheme=Bearer"));
    assert!(!auth_val.contains("secret-token"));

    let resp_headers = h["responseHeaders"].as_array().unwrap();
    let cookie = resp_headers
        .iter()
        .find(|x| x["key"] == "Set-Cookie")
        .unwrap();
    assert!(cookie["value"].as_str().unwrap().starts_with("[mask "));

    let req_body = h["requestBody"].as_str().unwrap();
    assert!(req_body.contains("[mask "));
    assert!(!req_body.contains("secret123"));

    let resp_body = h["responseBodyPreview"].as_str().unwrap();
    assert!(resp_body.contains("[mask "));
    assert!(!resp_body.contains("secret-jwt"));
}

#[test]
fn get_history_requires_history_id() {
    let result = api_registry().call("unfour.api.get_history", json!({}));
    assert!(result.is_err(), "should fail without historyId");
}
