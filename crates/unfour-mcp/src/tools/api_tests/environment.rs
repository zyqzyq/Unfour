use super::*;

// --- environment tests ---

#[test]
fn list_environments_masks_sensitive_variables_only() {
    let result = api_registry()
        .call("unfour.api.list_environments", json!({}))
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    let env = &result["structuredContent"]["environments"][0];
    assert_eq!(env["name"], "Staging");
    assert_eq!(env["isActive"], true);
    assert_eq!(env["variableCount"], 2);

    let vars = env["variables"].as_array().unwrap();
    let base = vars.iter().find(|v| v["key"] == "baseUrl").unwrap();
    // Non-sensitive value is shown verbatim so requests are intelligible.
    assert_eq!(base["value"], "https://api.staging.example.com");

    let token = vars.iter().find(|v| v["key"] == "token").unwrap();
    let token_val = token["value"].as_str().unwrap();
    assert!(token_val.starts_with("[mask "));
    assert!(token_val.contains("scheme=Bearer"));
    assert!(!token_val.contains("secret-token"));
}
