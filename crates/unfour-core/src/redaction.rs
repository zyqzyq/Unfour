pub const REDACTED_VALUE: &str = "<redacted>";

pub fn is_sensitive_key(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "proxy-authorization" | "x-api-key" | "x-auth-token"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_keys_are_detected_case_insensitively() {
        assert!(is_sensitive_key("Authorization"));
        assert!(is_sensitive_key("x-AUTH-token"));
        assert!(is_sensitive_key(" proxy-authorization "));
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
}
