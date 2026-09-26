use super::*;
use serde_json::json;

fn request() -> ApiRequestInput {
    serde_json::from_value(json!({"workspaceId":"test", "method":"POST", "url":"https://example.test/", "headers":[], "query":[], "bodyKind":"json"})).unwrap()
}

#[test]
fn runtime_values_cover_nested_json_and_enabled_rows_without_ordinary_data() {
    let mut input = request();
    input.body = Some(json!({"token":"saved-secret", "nested":[{"apiKey":"camel-secret", "password":"password-secret", "name":"ordinary-body"}], "secret":{"data":"nested-secret"}}).to_string());
    input.headers = serde_json::from_value(json!([
        {"key":"Authorization", "value":"Bearer header-secret", "enabled":true},
        {"key":"Cookie", "value":"disabled-secret", "enabled":false},
        {"key":"X-Trace", "value":"ordinary-header", "enabled":true}]))
    .unwrap();
    input.query = serde_json::from_value(json!([
        {"key":"token", "value":"query-secret", "enabled":true},
        {"key":"page", "value":"ordinary-query", "enabled":true}]))
    .unwrap();
    let values = runtime_request_secret_values(&input).unwrap();
    for secret in [
        "saved-secret",
        "camel-secret",
        "password-secret",
        "nested-secret",
        "Bearer header-secret",
        "header-secret",
        "query-secret",
    ] {
        assert!(values.contains(&secret.into()), "{values:?}");
    }
    for plain in [
        "ordinary-body",
        "ordinary-header",
        "ordinary-query",
        "disabled-secret",
    ] {
        assert!(!values.contains(&plain.into()), "{values:?}");
    }
}

#[test]
fn runtime_custom_auth_slots_and_encoded_url_values() {
    let mut input = request();
    input.auth_json = Some(
        json!({"type":"api-key", "key":"company", "value":"auth-secret", "addTo":"query"})
            .to_string(),
    );
    input.url = "https://alice:pass%2Bword+literal@example.test/?company=url%2Bkey&token=one&token=two&page=ordinary-query".into();
    input.query = serde_json::from_value(
        json!([{"key":"company", "value":"override-secret", "enabled":true}]),
    )
    .unwrap();
    let values = runtime_request_secret_values(&input).unwrap();
    for secret in [
        "auth-secret",
        "override-secret",
        "url+key",
        "url%2Bkey",
        "pass+word+literal",
        "pass%2Bword+literal",
        "one",
        "two",
    ] {
        assert!(values.contains(&secret.into()), "{values:?}");
    }
    for plain in ["alice", "company", "query", "api-key", "ordinary-query"] {
        assert!(!values.contains(&plain.into()), "{values:?}");
    }
}

#[test]
fn runtime_form_values_cover_both_storage_formats() {
    for body in [
        "password=form%2Bsecret&name=ordinary-body",
        r#"[{"key":"password","value":"form+secret","enabled":true},{"key":"name","value":"ordinary-body","enabled":true},{"key":"token","value":"disabled-secret","enabled":false}]"#,
    ] {
        let mut input = request();
        input.body_kind = "form-urlencoded".into();
        input.body = Some(body.into());
        let values = runtime_request_secret_values(&input).unwrap();
        assert!(values.contains(&"form+secret".into()));
        assert!(!values.contains(&"ordinary-body".into()));
        assert!(!values.contains(&"disabled-secret".into()));
    }
}

#[test]
fn runtime_userinfo_password_keeps_colons_and_literal_plus() {
    let mut input = request();
    input.url = "https://alice:pass:word+literal@example.test/".into();
    let values = runtime_request_secret_values(&input).unwrap();
    assert!(values.contains(&"pass:word+literal".into()), "{values:?}");
    assert!(!values.contains(&"alice".into()));
}
