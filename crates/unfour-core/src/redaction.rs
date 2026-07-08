pub const REDACTED_VALUE: &str = "<redacted>";

pub fn is_sensitive_key(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "authorization"
            | "cookie"
            | "proxy-authorization"
            | "x-api-key"
            | "x-auth-token"
            | "password"
            | "passwd"
            | "token"
            | "access_token"
            | "refresh_token"
            | "secret"
            | "private_key"
            | "api_key"
            | "license_key"
    )
}

pub fn is_sensitive_log_line(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "authorization",
        "cookie",
        "proxy-authorization",
        "x-api-key",
        "x-auth-token",
        "password",
        "passphrase",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub fn redact_key_values<T, Key, Set>(items: Vec<T>, key: Key, set_value: Set) -> Vec<T>
where
    Key: Fn(&T) -> &str,
    Set: Fn(&mut T, String),
{
    items
        .into_iter()
        .map(|mut item| {
            if is_sensitive_key(key(&item)) {
                set_value(&mut item, REDACTED_VALUE.to_string());
            }
            item
        })
        .collect()
}

/// Redact sensitive values from a JSON body string.
///
/// Walks the JSON structure recursively and replaces values whose keys
/// match the sensitive-key list with `<redacted>`. Non-JSON input is returned
/// unchanged with `false` as the second element.
pub fn redact_json_body(body: &str) -> (String, bool) {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return (body.to_string(), false);
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(mut value) => {
            let changed = redact_json_value(&mut value);
            if changed {
                (
                    serde_json::to_string(&value).unwrap_or_else(|_| body.to_string()),
                    true,
                )
            } else {
                (body.to_string(), false)
            }
        }
        Err(_) => (body.to_string(), false),
    }
}

fn redact_json_value(value: &mut serde_json::Value) -> bool {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut changed = false;
            for (key, val) in map.iter_mut() {
                if is_sensitive_key(key) {
                    if !val.is_string() || val.as_str() != Some(REDACTED_VALUE) {
                        *val = Value::String(REDACTED_VALUE.to_string());
                        changed = true;
                    }
                } else if redact_json_value(val) {
                    changed = true;
                }
            }
            changed
        }
        Value::Array(items) => {
            let mut changed = false;
            for item in items.iter_mut() {
                if redact_json_value(item) {
                    changed = true;
                }
            }
            changed
        }
        _ => false,
    }
}

pub fn redact_sensitive_lines(value: &str) -> (String, bool) {
    let mut redacted = false;
    let lines = value
        .lines()
        .map(|line| {
            if is_sensitive_log_line(line) {
                redacted = true;
                REDACTED_VALUE.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>();

    let mut output = lines.join("\n");
    if value.ends_with('\n') {
        output.push('\n');
    }

    (output, redacted)
}

pub fn redact_url_query(value: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(value) else {
        return value.to_string();
    };

    if url.query().is_none() {
        return value.to_string();
    }

    let pairs = url
        .query_pairs()
        .map(|(key, val)| {
            if is_sensitive_key(&key) {
                (key.into_owned(), REDACTED_VALUE.to_string())
            } else {
                (key.into_owned(), val.into_owned())
            }
        })
        .collect::<Vec<_>>();

    url.set_query(None);
    {
        let mut query_pairs = url.query_pairs_mut();
        for (key, val) in pairs {
            query_pairs.append_pair(&key, &val);
        }
    }
    url.to_string()
}

pub fn redact_connection_string(value: &str) -> String {
    if reqwest::Url::parse(value).is_ok() {
        return redact_url_password_for_display(&redact_url_query(value));
    }

    // The url crate rejects non-special schemes (postgres://, mysql://) that
    // carry a `:port`, so the parse above fails and would otherwise leave a
    // userinfo password in the clear. Scrub the credential directly from the
    // raw string instead.
    if value.contains("://") && value.contains('@') {
        return redact_url_password_for_display(value);
    }

    let mut changed = false;
    let redacted = value
        .split(';')
        .map(|segment| {
            let Some((key, _val)) = segment.split_once('=') else {
                return segment.to_string();
            };
            if is_sensitive_key(key) {
                changed = true;
                format!("{key}={REDACTED_VALUE}")
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(";");

    if changed {
        redacted
    } else {
        value.to_string()
    }
}

fn redact_url_password_for_display(value: &str) -> String {
    let Some(authority_start) = value.find("://").map(|index| index + 3) else {
        return value.to_string();
    };
    let Some(at_offset) = value[authority_start..].find('@') else {
        return value.to_string();
    };
    let authority_end = authority_start + at_offset;
    let authority = &value[authority_start..authority_end];
    let Some(password_start_offset) = authority.rfind(':').map(|index| index + 1) else {
        return value.to_string();
    };
    if password_start_offset >= authority.len() {
        return value.to_string();
    }

    let password_start = authority_start + password_start_offset;
    format!(
        "{}{}{}",
        &value[..password_start],
        REDACTED_VALUE,
        &value[authority_end..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_keys_are_detected_case_insensitively() {
        assert!(is_sensitive_key("Authorization"));
        assert!(is_sensitive_key("x-AUTH-token"));
        assert!(is_sensitive_key(" proxy-authorization "));
        assert!(is_sensitive_key("password"));
        assert!(is_sensitive_key("passwd"));
        assert!(is_sensitive_key("token"));
        assert!(is_sensitive_key("access_token"));
        assert!(is_sensitive_key("refresh_token"));
        assert!(is_sensitive_key("private_key"));
        assert!(is_sensitive_key("license_key"));
        assert!(!is_sensitive_key("x-request-id"));
    }

    #[test]
    fn key_values_are_redacted_without_mutating_safe_values() {
        let headers = vec![
            (
                "Authorization".to_string(),
                "Bearer secret".to_string(),
                true,
            ),
            ("Accept".to_string(), "application/json".to_string(), true),
        ];

        let redacted = redact_key_values(
            headers,
            |item| &item.0,
            |item, value| {
                item.1 = value;
            },
        );

        assert_eq!(redacted[0].1, "<redacted>");
        assert_eq!(redacted[1].1, "application/json");
    }

    #[test]
    fn sensitive_log_lines_are_replaced() {
        let (output, redacted) =
            redact_sensitive_lines("ok\nAuthorization: Bearer secret\npassword=secret");

        assert_eq!(output, "ok\n<redacted>\n<redacted>");
        assert!(redacted);
    }

    #[test]
    fn json_body_redacts_nested_sensitive_keys() {
        let body = r#"{"user":"alice","auth":{"Authorization":"Bearer secret","token":"ok"}}"#;
        let (redacted, changed) = redact_json_body(body);
        assert!(changed);
        let parsed: serde_json::Value = serde_json::from_str(&redacted).unwrap();
        assert_eq!(parsed["user"], "alice");
        assert_eq!(parsed["auth"]["Authorization"], "<redacted>");
        assert_eq!(parsed["auth"]["token"], "<redacted>");
    }

    #[test]
    fn json_body_redacts_inside_arrays() {
        let body = r#"[{"Authorization":"secret1"},{"Authorization":"secret2"}]"#;
        let (redacted, changed) = redact_json_body(body);
        assert!(changed);
        let parsed: serde_json::Value = serde_json::from_str(&redacted).unwrap();
        assert_eq!(parsed[0]["Authorization"], "<redacted>");
        assert_eq!(parsed[1]["Authorization"], "<redacted>");
    }

    #[test]
    fn json_body_preserves_non_sensitive_values() {
        let body = r#"{"name":"test","method":"GET","count":42}"#;
        let (redacted, changed) = redact_json_body(body);
        assert!(!changed);
        assert_eq!(redacted, body);
    }

    #[test]
    fn json_body_handles_invalid_json() {
        let body = "not valid json at all";
        let (redacted, changed) = redact_json_body(body);
        assert!(!changed);
        assert_eq!(redacted, body);
    }

    #[test]
    fn json_body_handles_plain_text() {
        let body = "Hello, world!";
        let (redacted, changed) = redact_json_body(body);
        assert!(!changed);
        assert_eq!(redacted, body);
    }

    #[test]
    fn json_body_handles_empty_input() {
        let (redacted, changed) = redact_json_body("");
        assert!(!changed);
        assert_eq!(redacted, "");
    }

    #[test]
    fn json_body_preserves_json_structure() {
        let body = r#"{"items":[{"x-api-key":"secret","data":"safe"}],"meta":{"count":1}}"#;
        let (redacted, changed) = redact_json_body(body);
        assert!(changed);
        let parsed: serde_json::Value = serde_json::from_str(&redacted).unwrap();
        assert_eq!(parsed["items"][0]["x-api-key"], "<redacted>");
        assert_eq!(parsed["items"][0]["data"], "safe");
        assert_eq!(parsed["meta"]["count"], 1);
    }

    #[test]
    fn connection_string_passwords_are_redacted() {
        let input = "postgres://alice:secret@db.internal/app?sslmode=require&access_token=abc";

        let redacted = redact_connection_string(input);

        assert!(redacted.contains("<redacted>"));
        assert!(!redacted.contains("secret"));
        assert!(!redacted.contains("abc"));
    }

    #[test]
    fn semicolon_connection_string_passwords_are_redacted() {
        let input = "Server=db.internal;User Id=alice;Password=secret;Database=app";

        let redacted = redact_connection_string(input);

        assert_eq!(
            redacted,
            "Server=db.internal;User Id=alice;Password=<redacted>;Database=app"
        );
    }

    #[test]
    fn non_special_scheme_url_with_port_scrubs_userinfo() {
        // postgres:// / mysql:// are non-special schemes; the url crate refuses
        // to parse them when a :port is present, so the userinfo password must
        // still be scrubbed from the raw string.
        let input = "postgres://alice:secret@db.internal:5432/app";

        let redacted = redact_connection_string(input);

        assert!(redacted.contains("alice"));
        assert!(!redacted.contains("secret"));
        assert!(redacted.contains("<redacted>"));
    }

    #[test]
    fn url_query_tokens_are_redacted() {
        let input = "https://api.example.test/v1/users?access_token=abc&page=1&api_key=secret";

        let redacted = redact_url_query(input);

        assert_eq!(
            redacted,
            "https://api.example.test/v1/users?access_token=%3Credacted%3E&page=1&api_key=%3Credacted%3E"
        );
        assert!(!redacted.contains("abc"));
        assert!(!redacted.contains("secret"));
    }
}
