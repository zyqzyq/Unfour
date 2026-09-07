use unfour_core::models::{parse_multipart_definition, ApiMultipartPart};
use unfour_core::redaction::{is_sensitive_key, REDACTED_VALUE};

pub(super) fn snapshot(body: &str) -> String {
    let Ok(mut parts) = parse_multipart_definition(Some(body)) else {
        return REDACTED_VALUE.into();
    };
    for part in &mut parts {
        if let ApiMultipartPart::Text { key, value, .. } = part {
            if is_sensitive_key(key) {
                *value = REDACTED_VALUE.into();
            }
        }
    }
    serde_json::to_string(&parts).unwrap_or_else(|_| REDACTED_VALUE.into())
}

pub(super) fn restore(body: &str, current: Option<&str>) -> String {
    let Ok(mut parts) = parse_multipart_definition(Some(body)) else {
        // Retain invalid input for the caller's strict validation to reject it.
        return body.into();
    };
    let current = parse_multipart_definition(current).unwrap_or_default();
    for part in &mut parts {
        if let ApiMultipartPart::Text { id, key, value, .. } = part {
            if is_sensitive_key(key) {
                *value = if value == REDACTED_VALUE {
                    current
                        .iter()
                        .find_map(|part| match part {
                            ApiMultipartPart::Text {
                                id: old_id,
                                key: old_key,
                                value,
                                ..
                            } if old_id == id && old_key.eq_ignore_ascii_case(key) => {
                                Some(value.clone())
                            }
                            _ => None,
                        })
                        .unwrap_or_default()
                } else {
                    String::new()
                };
            }
        }
    }
    serde_json::to_string(&parts).unwrap_or_else(|_| REDACTED_VALUE.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sensitive_text_is_redacted_and_restored_by_stable_id() {
        let current = r#"[{"id":"a","enabled":true,"key":"token","type":"text","value":"secret-a"},{"id":"b","enabled":false,"key":"token","type":"text","value":"secret-b"},{"id":"f","enabled":true,"key":"avatar","type":"file","fileName":"avatar.png"}]"#;
        let safe = snapshot(current);
        assert!(!safe.contains("secret-a"));
        assert!(!safe.contains("secret-b"));
        let mut reordered: Vec<serde_json::Value> = serde_json::from_str(&safe).unwrap();
        reordered.swap(0, 1);
        let restored = restore(&serde_json::to_string(&reordered).unwrap(), Some(current));
        let values: serde_json::Value = serde_json::from_str(&restored).unwrap();
        assert_eq!(values[0]["value"], "secret-b");
        assert_eq!(values[1]["value"], "secret-a");
        assert_eq!(values[2]["fileName"], "avatar.png");
        let remote_plaintext = restore(current, None);
        assert!(!remote_plaintext.contains("secret-a"));
        assert!(!remote_plaintext.contains("secret-b"));
    }
}
