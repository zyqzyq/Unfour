use serde_json::Value;

pub const REDACTED: &str = "[REDACTED]";
pub const MAX_BODY_PREVIEW_BYTES: usize = 20 * 1024;

fn normalize(name: &str) -> String {
    name.to_ascii_lowercase()
        .replace(['-', '_'], "")
}

pub fn is_sensitive_key(name: &str) -> bool {
    matches!(
        normalize(name).as_str(),
        "password"
            | "passwd"
            | "pwd"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "apikey"
            | "secret"
            | "clientsecret"
            | "authorization"
            | "cookie"
            | "setcookie"
            | "proxyauthorization"
            | "xapikey"
            | "xauthtoken"
            | "privatekey"
            | "connectionstring"
            | "databaseurl"
            | "credentialref"
    )
}

pub fn redact_header_value(name: &str, value: &str) -> String {
    if is_sensitive_key(name) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

pub fn redact_url_query(url: &str) -> String {
    let Some(question_mark) = url.find('?') else {
        return url.to_string();
    };

    let (base, query_and_fragment) = url.split_at(question_mark);
    let query_str = &query_and_fragment[1..]; // skip '?'

    // Separate fragment if present
    let (query_part, fragment) = match query_str.find('#') {
        Some(pos) => (&query_str[..pos], Some(&query_str[pos..])),
        None => (query_str, None),
    };

    if query_part.is_empty() {
        return url.to_string();
    }

    let redacted_pairs: Vec<String> = query_part
        .split('&')
        .map(|pair| {
            if let Some(eq_pos) = pair.find('=') {
                let key = &pair[..eq_pos];
                let value = &pair[eq_pos + 1..];
                if is_sensitive_key(key) {
                    format!("{}={}", key, REDACTED)
                } else {
                    format!("{}={}", key, value)
                }
            } else {
                pair.to_string()
            }
        })
        .collect();

    let mut result = format!("{}?{}", base, redacted_pairs.join("&"));
    if let Some(frag) = fragment {
        result.push_str(frag);
    }
    result
}

pub fn redact_body(body: &str, body_kind: &str) -> String {
    if body_kind != "json" {
        return body.to_string();
    }

    let Ok(mut json) = serde_json::from_str::<Value>(body) else {
        return body.to_string();
    };

    redact_json_value(&mut json);
    serde_json::to_string(&json).unwrap_or_else(|_| body.to_string())
}

fn redact_json_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if is_sensitive_key(key) {
                    *val = Value::String(REDACTED.to_string());
                } else {
                    redact_json_value(val);
                }
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                redact_json_value(item);
            }
        }
        _ => {}
    }
}

pub fn truncate_body(body: &str, max_bytes: usize) -> (String, bool) {
    if body.len() <= max_bytes {
        (body.to_string(), false)
    } else {
        let safe_end = body[..max_bytes]
            .char_indices()
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        let truncated = body[..safe_end.min(max_bytes)].to_string();
        (truncated, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_key_detects_common_names() {
        assert!(is_sensitive_key("password"));
        assert!(is_sensitive_key("Password"));
        assert!(is_sensitive_key("PASSWORD"));
        assert!(is_sensitive_key("Authorization"));
        assert!(is_sensitive_key("authorization"));
        assert!(is_sensitive_key("x-api-key"));
        assert!(is_sensitive_key("X-Api-Key"));
        assert!(is_sensitive_key("cookie"));
        assert!(is_sensitive_key("Set-Cookie"));
        assert!(is_sensitive_key("access_token"));
        assert!(is_sensitive_key("accessToken"));
        assert!(is_sensitive_key("private_key"));
        assert!(is_sensitive_key("privateKey"));
        assert!(is_sensitive_key("connection_string"));
        assert!(is_sensitive_key("connectionString"));
        assert!(is_sensitive_key("client_secret"));
        assert!(is_sensitive_key("refresh_token"));
        assert!(is_sensitive_key("database_url"));
        assert!(is_sensitive_key("databaseUrl"));
        assert!(is_sensitive_key("credential_ref"));
        assert!(is_sensitive_key("credentialRef"));
    }

    #[test]
    fn sensitive_key_allows_safe_names() {
        assert!(!is_sensitive_key("Content-Type"));
        assert!(!is_sensitive_key("Accept"));
        assert!(!is_sensitive_key("Host"));
        assert!(!is_sensitive_key("name"));
        assert!(!is_sensitive_key("username"));
        assert!(!is_sensitive_key("page"));
    }

    #[test]
    fn header_redaction_replaces_sensitive_values() {
        assert_eq!(
            redact_header_value("Authorization", "Bearer secret123"),
            REDACTED
        );
        assert_eq!(
            redact_header_value("Content-Type", "application/json"),
            "application/json"
        );
        assert_eq!(
            redact_header_value("x-api-key", "my-key"),
            REDACTED
        );
    }

    #[test]
    fn url_query_redaction_replaces_sensitive_params() {
        let url = "https://api.example.com/users?token=abc123&page=1&api_key=secret";
        let redacted = redact_url_query(url);

        assert!(redacted.contains(&format!("token={}", REDACTED)));
        assert!(redacted.contains("page=1"));
        assert!(redacted.contains(&format!("api_key={}", REDACTED)));
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("secret"));
    }

    #[test]
    fn url_query_redaction_preserves_safe_urls() {
        let url = "https://api.example.com/users?page=1&sort=name";
        assert_eq!(redact_url_query(url), url);
    }

    #[test]
    fn url_query_redaction_handles_invalid_url() {
        let invalid = "not a url";
        assert_eq!(redact_url_query(invalid), invalid);
    }

    #[test]
    fn body_redaction_replaces_sensitive_json_keys() {
        let body = r#"{"user":"alice","password":"secret123","data":"safe","nested":{"token":"abc"}}"#;
        let redacted = redact_body(body, "json");
        let parsed: Value = serde_json::from_str(&redacted).unwrap();

        assert_eq!(parsed["user"], "alice");
        assert_eq!(parsed["password"], REDACTED);
        assert_eq!(parsed["data"], "safe");
        assert_eq!(parsed["nested"]["token"], REDACTED);
    }

    #[test]
    fn body_redaction_skips_non_json() {
        let body = "password=secret&user=alice";
        assert_eq!(redact_body(body, "text"), body);
        assert_eq!(redact_body(body, "form"), body);
    }

    #[test]
    fn body_redaction_handles_invalid_json() {
        let body = "{invalid json";
        assert_eq!(redact_body(body, "json"), body);
    }

    #[test]
    fn truncate_body_preserves_small_bodies() {
        let (result, truncated) = truncate_body("hello", 100);
        assert_eq!(result, "hello");
        assert!(!truncated);
    }

    #[test]
    fn truncate_body_truncates_large_bodies() {
        let large = "a".repeat(100);
        let (result, truncated) = truncate_body(&large, 50);
        assert_eq!(result.len(), 50);
        assert!(truncated);
    }
}
