use serde_json::Value;

pub const MAX_BODY_PREVIEW_BYTES: usize = 20 * 1024;

fn normalize(name: &str) -> String {
    name.to_ascii_lowercase().replace(['-', '_'], "")
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

/// Produce a partial-mask descriptor for a sensitive value.
///
/// The descriptor exposes diagnostic *shape* (auth scheme, structural kind,
/// length, and a stable fingerprint) while never revealing the usable secret.
/// This lets an LLM client diagnose the common auth failures (wrong scheme,
/// truncated/malformed token, wrong environment key, mismatched tokens across
/// fields) without exfiltrating the credential itself.
///
/// Examples:
/// - `Authorization: Bearer eyJ...` -> `[mask kind=jwt scheme=Bearer len=872 fp=a1b2c3]`
/// - `x-api-key: sk-live-...`        -> `[mask kind=prefixed:sk len=51 fp=9f8e7d]`
pub fn mask_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "[mask empty]".to_string();
    }

    let (scheme, secret) = split_scheme(trimmed);
    let len = secret.chars().count();
    let kind = classify_secret(scheme, secret, len);
    // Fingerprint the secret material (post-scheme) so the same token correlates
    // across header / query / body / environment fields.
    let fp = fnv1a_hex6(secret);

    let mut parts = vec![format!("kind={kind}")];
    if let Some(scheme) = scheme {
        parts.push(format!("scheme={scheme}"));
    }
    parts.push(format!("len={len}"));
    parts.push(format!("fp={fp}"));
    format!("[mask {}]", parts.join(" "))
}

/// Split a `Scheme credential` value (e.g. `Bearer abc`) into its scheme and
/// credential. Only a purely-alphabetic leading word followed by a remainder is
/// treated as a scheme (captures Bearer/Basic/Digest/Negotiate/NTLM); anything
/// else is treated as an opaque secret with no scheme.
fn split_scheme(value: &str) -> (Option<&str>, &str) {
    if let Some((head, rest)) = value.split_once(char::is_whitespace) {
        let rest = rest.trim();
        if !head.is_empty() && head.chars().all(|c| c.is_ascii_alphabetic()) && !rest.is_empty() {
            return (Some(head), rest);
        }
    }
    (None, value)
}

fn classify_secret(scheme: Option<&str>, secret: &str, len: usize) -> String {
    if scheme.is_some_and(|s| s.eq_ignore_ascii_case("basic")) {
        return "basic".to_string();
    }
    if is_jwt(secret) {
        return "jwt".to_string();
    }
    if is_uuid(secret) {
        return "uuid".to_string();
    }
    // Skip structured-prefix / hex classification for very short secrets to avoid
    // leaking a meaningful share of a low-entropy value.
    if len >= 8 {
        if let Some(prefix) = structured_prefix(secret) {
            return format!("prefixed:{prefix}");
        }
        if is_hex(secret) {
            return "hex".to_string();
        }
    }
    "opaque".to_string()
}

fn is_jwt(secret: &str) -> bool {
    let segments: Vec<&str> = secret.split('.').collect();
    segments.len() == 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
}

fn is_uuid(secret: &str) -> bool {
    let bytes = secret.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(i, &b)| {
        if matches!(i, 8 | 13 | 18 | 23) {
            b == b'-'
        } else {
            b.is_ascii_hexdigit()
        }
    })
}

fn is_hex(secret: &str) -> bool {
    secret.len() >= 16 && secret.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Reveal a structured prefix (e.g. `sk`, `ghp`) when a separator appears within
/// the first 8 characters. The prefix itself is not the secret and strongly aids
/// "wrong environment key" diagnosis.
fn structured_prefix(secret: &str) -> Option<String> {
    let window: String = secret.chars().take(8).collect();
    let idx = window.find(['_', '-'])?;
    if idx == 0 {
        return None;
    }
    let prefix = &window[..idx];
    if prefix.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(prefix.to_string())
    } else {
        None
    }
}

/// Dependency-free deterministic FNV-1a fingerprint, truncated to 6 hex digits.
/// Non-cryptographic; used only for cross-field correlation, never reversal.
fn fnv1a_hex6(value: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:06x}", hash & 0xff_ffff)
}

pub fn redact_header_value(name: &str, value: &str) -> String {
    if is_sensitive_key(name) {
        mask_secret(value)
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
                    format!("{}={}", key, mask_secret(value))
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

/// Recursively mask sensitive fields within an arbitrary JSON value in place.
/// Used as defense-in-depth over already-redacted stored summaries such as
/// activity-event details before they are surfaced to an LLM client.
pub fn redact_json_in_place(value: &mut Value) {
    redact_json_value(value);
}

fn redact_json_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if is_sensitive_key(key) {
                    let secret = match val.as_str() {
                        Some(s) => s.to_string(),
                        None => val.to_string(),
                    };
                    *val = Value::String(mask_secret(&secret));
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
    fn header_redaction_masks_sensitive_values() {
        let masked = redact_header_value("Authorization", "Bearer secret123");
        assert!(masked.starts_with("[mask "));
        assert!(masked.contains("scheme=Bearer"));
        assert!(masked.contains("len="));
        assert!(!masked.contains("secret123"));

        assert_eq!(
            redact_header_value("Content-Type", "application/json"),
            "application/json"
        );

        let key_masked = redact_header_value("x-api-key", "my-key");
        assert!(key_masked.starts_with("[mask "));
        assert!(!key_masked.contains("my-key"));
    }

    #[test]
    fn url_query_redaction_masks_sensitive_params() {
        let url = "https://api.example.com/users?token=abc123&page=1&api_key=secret";
        let redacted = redact_url_query(url);

        assert!(redacted.contains("token=[mask "));
        assert!(redacted.contains("page=1"));
        assert!(redacted.contains("api_key=[mask "));
        assert!(!redacted.contains("abc123"));
        // The raw secret value must not survive; "secret" only appears inside a
        // mask descriptor's fp/len fields by accident is impossible since fp is hex.
        assert!(!redacted.contains("=secret&"));
        assert!(!redacted.contains("=secret\""));
        assert!(!redacted.ends_with("=secret"));
    }

    #[test]
    fn mask_secret_classifies_common_shapes() {
        let jwt = mask_secret("Bearer aaa.bbb.ccc");
        assert!(jwt.contains("kind=jwt"));
        assert!(jwt.contains("scheme=Bearer"));

        let basic = mask_secret("Basic dXNlcjpwYXNz");
        assert!(basic.contains("kind=basic"));

        let prefixed = mask_secret("sk-live-0123456789abcdef");
        assert!(prefixed.contains("kind=prefixed:sk"));

        let hex = mask_secret("deadbeefcafef00d1234");
        assert!(hex.contains("kind=hex"));

        let uuid = mask_secret("123e4567-e89b-12d3-a456-426614174000");
        assert!(uuid.contains("kind=uuid"));

        let opaque = mask_secret("PlainOpaqueValue123");
        assert!(opaque.contains("kind=opaque"));

        let short = mask_secret("ab12");
        assert!(short.contains("kind=opaque"));
        assert!(short.contains("len=4"));

        assert_eq!(mask_secret(""), "[mask empty]");
    }

    #[test]
    fn mask_secret_fingerprint_is_deterministic_and_correlates() {
        // Same secret material correlates across scheme-prefixed and bare forms.
        let header = mask_secret("Bearer eyJabc.def.ghi");
        let bare = mask_secret("eyJabc.def.ghi");
        let fp = |s: &str| {
            s.split_whitespace()
                .find_map(|p| p.strip_prefix("fp="))
                .map(|f| f.trim_end_matches(']').to_string())
                .unwrap()
        };
        assert_eq!(fp(&header), fp(&bare), "fp ignores the auth scheme");
        assert_ne!(fp(&mask_secret("token-a")), fp(&mask_secret("token-b")));
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
        let body =
            r#"{"user":"alice","password":"secret123","data":"safe","nested":{"token":"abc"}}"#;
        let redacted = redact_body(body, "json");
        let parsed: Value = serde_json::from_str(&redacted).unwrap();

        assert_eq!(parsed["user"], "alice");
        assert!(parsed["password"].as_str().unwrap().starts_with("[mask "));
        assert_eq!(parsed["data"], "safe");
        assert!(parsed["nested"]["token"]
            .as_str()
            .unwrap()
            .starts_with("[mask "));
        assert!(!redacted.contains("secret123"));
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
