use std::collections::{HashMap, VecDeque};

use reqwest::Url;
use serde_json::Value;
use unfour_core::models::KeyValue;
use unfour_core::redaction::{
    is_sensitive_key, redact_json_body, redact_key_values, REDACTED_VALUE,
};
use unfour_core::{AppError, AppResult};

use super::super::DEFAULT_AUTH_JSON;

/// Percent-encoded form of [`REDACTED_VALUE`] used inside URL-encoded
/// components (query strings and raw form bodies).
const ENCODED_REDACTED_VALUE: &str = "%3Credacted%3E";

/// Auth JSON fields that are safe to sync in plaintext.
///
/// Auth payloads are redacted with a field ALLOWLIST instead of a
/// sensitive-key list: every string-valued field whose key is not listed here
/// is treated as secret material (default-deny), so current and future auth
/// types cannot leak values whose key happens to be missing from
/// `is_sensitive_key` (e.g. the api-key auth's `value` field).
const AUTH_JSON_PLAIN_FIELDS: [&str; 4] = ["type", "addTo", "key", "username"];

fn is_auth_plain_field(key: &str) -> bool {
    AUTH_JSON_PLAIN_FIELDS
        .iter()
        .any(|field| field.eq_ignore_ascii_case(key))
}

pub(super) fn snapshot_auth_json(value: &str) -> String {
    let Ok(mut parsed) = serde_json::from_str::<Value>(value) else {
        return REDACTED_VALUE.to_string();
    };
    if parsed.is_string() {
        // A bare JSON string has no field key to classify; treat it like
        // non-JSON input and redact the whole value.
        return REDACTED_VALUE.to_string();
    }
    redact_auth_json_value(&mut parsed);
    serde_json::to_string(&parsed).unwrap_or_else(|_| REDACTED_VALUE.to_string())
}

fn redact_auth_json_value(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields {
                match field {
                    Value::String(text) => {
                        if !is_auth_plain_field(key) && !text.is_empty() {
                            *text = REDACTED_VALUE.to_string();
                        }
                    }
                    _ => redact_auth_json_value(field),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_auth_json_value(item);
            }
        }
        _ => {}
    }
}

pub(super) fn snapshot_key_values(value: &str) -> AppResult<Vec<KeyValue>> {
    let items = serde_json::from_str::<Vec<KeyValue>>(value).map_err(|_| {
        AppError::Config("stored API key-value configuration is invalid".to_string())
    })?;
    Ok(redact_key_values(
        items,
        |item| &item.key,
        |item, redacted| item.value = redacted,
    ))
}

pub(super) fn snapshot_body(value: Option<&str>, body_kind: &str) -> Option<String> {
    value.map(|value| {
        if is_form_urlencoded(body_kind) {
            snapshot_form_body(value)
        } else {
            redact_json_body(value).0
        }
    })
}

pub(super) fn snapshot_url(value: &str) -> String {
    transform_url(value, None, true)
}

pub(super) fn restore_auth_json(external: &str, current: Option<&str>) -> String {
    if external == REDACTED_VALUE {
        return current.unwrap_or(DEFAULT_AUTH_JSON).to_string();
    }
    restore_auth_json_string(external, current)
        .unwrap_or_else(|| current.unwrap_or(DEFAULT_AUTH_JSON).to_string())
}

fn restore_auth_json_string(external: &str, current: Option<&str>) -> Option<String> {
    let mut external = serde_json::from_str::<Value>(external).ok()?;
    if external.is_string() {
        // Mirrors `snapshot_auth_json`: a bare string is unclassifiable and
        // never accepted from remote; fall back to the local value.
        return None;
    }
    let current = current.and_then(|value| serde_json::from_str::<Value>(value).ok());
    restore_auth_json_value(&mut external, current.as_ref());
    serde_json::to_string(&external).ok()
}

/// Mirror of the allowlist classifier in `redact_auth_json_value`: redacted
/// secret fields are restored from the local auth JSON at the same path,
/// while a remote-supplied plaintext value for a secret field is never
/// accepted and becomes empty.
fn restore_auth_json_value(external: &mut Value, current: Option<&Value>) {
    match external {
        Value::Object(fields) => {
            for (key, field) in fields {
                let current = current
                    .and_then(Value::as_object)
                    .and_then(|map| map.get(key));
                match field {
                    Value::String(text) => {
                        if !is_auth_plain_field(key) && !text.is_empty() {
                            *text = if text.as_str() == REDACTED_VALUE {
                                current
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_string()
                            } else {
                                String::new()
                            };
                        }
                    }
                    _ => restore_auth_json_value(field, current),
                }
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                let current = current
                    .and_then(Value::as_array)
                    .and_then(|items| items.get(index));
                restore_auth_json_value(item, current);
            }
        }
        _ => {}
    }
}

pub(super) fn restore_key_values(
    external: Vec<KeyValue>,
    current_json: Option<&str>,
) -> Vec<KeyValue> {
    let current = current_json
        .and_then(|value| serde_json::from_str::<Vec<KeyValue>>(value).ok())
        .unwrap_or_default();
    external
        .into_iter()
        .map(|mut item| {
            if is_sensitive_key(&item.key) {
                item.value = if item.value == REDACTED_VALUE {
                    current
                        .iter()
                        .find(|candidate| candidate.key.eq_ignore_ascii_case(&item.key))
                        .map(|candidate| candidate.value.clone())
                        .unwrap_or_default()
                } else {
                    String::new()
                };
            }
            item
        })
        .collect()
}

pub(super) fn restore_body(
    external: Option<String>,
    current: Option<&str>,
    body_kind: &str,
) -> Option<String> {
    external.map(|value| {
        if is_form_urlencoded(body_kind) {
            restore_form_body(value, current)
        } else {
            restore_redacted_json(&value, current).unwrap_or(value)
        }
    })
}

pub(super) fn restore_url(external: &str, current: Option<&str>) -> String {
    transform_url(external, current, false)
}

fn is_form_urlencoded(body_kind: &str) -> bool {
    body_kind.eq_ignore_ascii_case("form-urlencoded")
}

/// Form bodies index secrets by form-field key, so generic JSON body
/// redaction (which matches object key names) cannot see them. They are
/// stored either as a JSON `KeyValue` list (the editor's save format) or as a
/// raw `key=value&...` string; both are redacted by form-field key like
/// headers and query parameters.
fn snapshot_form_body(value: &str) -> String {
    if let Ok(items) = serde_json::from_str::<Vec<KeyValue>>(value) {
        let items = redact_key_values(
            items,
            |item| &item.key,
            |item, redacted| item.value = redacted,
        );
        return serde_json::to_string(&items).unwrap_or_else(|_| REDACTED_VALUE.to_string());
    }
    transform_pair_components(value, None, true)
}

fn restore_form_body(external: String, current: Option<&str>) -> String {
    if let Ok(items) = serde_json::from_str::<Vec<KeyValue>>(&external) {
        let items = restore_key_values(items, current);
        return serde_json::to_string(&items).unwrap_or(external);
    }
    transform_pair_components(&external, current, false)
}

fn restore_redacted_json(external: &str, current: Option<&str>) -> Option<String> {
    let mut external = serde_json::from_str::<Value>(external).ok()?;
    let current = current.and_then(|value| serde_json::from_str::<Value>(value).ok());
    restore_redacted_value(&mut external, current.as_ref());
    serde_json::to_string(&external).ok()
}

fn restore_redacted_value(external: &mut Value, current: Option<&Value>) {
    match external {
        Value::Object(fields) => {
            for (key, value) in fields {
                let current = current
                    .and_then(Value::as_object)
                    .and_then(|map| map.get(key));
                if is_sensitive_key(key) {
                    *value = if value.as_str() == Some(REDACTED_VALUE) {
                        current
                            .cloned()
                            .unwrap_or_else(|| Value::String(String::new()))
                    } else {
                        Value::String(String::new())
                    };
                } else {
                    restore_redacted_value(value, current);
                }
            }
        }
        Value::Array(items) => {
            for (index, value) in items.iter_mut().enumerate() {
                let current = current
                    .and_then(Value::as_array)
                    .and_then(|items| items.get(index));
                restore_redacted_value(value, current);
            }
        }
        _ => {}
    }
}

fn transform_url(value: &str, local: Option<&str>, snapshot: bool) -> String {
    let value = transform_userinfo_password(value, local, snapshot);
    let Some((prefix, query, fragment)) = split_url_query(&value) else {
        return value;
    };
    let local_query = local.and_then(split_url_query).map(|(_, query, _)| query);
    let query = transform_pair_components(query, local_query, snapshot);
    format!("{prefix}?{query}{fragment}")
}

/// Scrub the password inside a URL's authority userinfo (`user:pass@host`).
///
/// Works on the raw string so template URLs such as `{{base_url}}` survive;
/// parsing the whole URL with `reqwest::Url` would reject or rewrite them.
/// On snapshot a non-empty password becomes [`REDACTED_VALUE`]; on restore a
/// redacted password is recovered from the local URL's userinfo and any other
/// remote-supplied password is dropped (never accepted from remote).
fn transform_userinfo_password(value: &str, local: Option<&str>, snapshot: bool) -> String {
    let Some((start, end)) = userinfo_password_span(value) else {
        return value.to_string();
    };
    let password = &value[start..end];
    if password.is_empty() {
        return value.to_string();
    }
    let replacement = if snapshot {
        REDACTED_VALUE.to_string()
    } else if password == REDACTED_VALUE || password.eq_ignore_ascii_case(ENCODED_REDACTED_VALUE) {
        local
            .and_then(|local| {
                let (start, end) = userinfo_password_span(local)?;
                Some(local[start..end].to_string())
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    format!("{}{}{}", &value[..start], replacement, &value[end..])
}

/// Locate the byte span of the userinfo password: the authority section is
/// the text between `://` and the first `/`, `?` or `#`, and the password is
/// the part between the last `:` of the userinfo and the terminating `@`.
fn userinfo_password_span(value: &str) -> Option<(usize, usize)> {
    let scheme_end = value.find("://")?;
    if value[..scheme_end].contains(['/', '?', '#']) {
        return None;
    }
    let authority_start = scheme_end + 3;
    let authority_end = value[authority_start..]
        .find(['/', '?', '#'])
        .map_or(value.len(), |offset| authority_start + offset);
    let authority = &value[authority_start..authority_end];
    let at_index = authority.rfind('@')?;
    let colon_index = authority[..at_index].rfind(':')?;
    Some((
        authority_start + colon_index + 1,
        authority_start + at_index,
    ))
}

/// Transform `key=value&...` components, redacting values of sensitive keys
/// on snapshot and restoring redacted values from the matching local
/// component (by key occurrence order) on restore. Shared by URL query
/// strings and raw form-urlencoded bodies.
fn transform_pair_components(pairs: &str, local: Option<&str>, snapshot: bool) -> String {
    let mut local_values = local.map(sensitive_query_values).unwrap_or_default();
    pairs
        .split('&')
        .map(|component| transform_pair_component(component, &mut local_values, snapshot))
        .collect::<Vec<_>>()
        .join("&")
}

fn transform_pair_component(
    component: &str,
    local_values: &mut HashMap<String, VecDeque<String>>,
    snapshot: bool,
) -> String {
    let Some((key, decoded_value)) = decode_query_component(component) else {
        return component.to_string();
    };
    if !is_sensitive_key(&key) {
        return component.to_string();
    }
    let raw_key = component
        .split_once('=')
        .map(|(key, _)| key)
        .unwrap_or(component);
    let value = if snapshot {
        ENCODED_REDACTED_VALUE.to_string()
    } else if decoded_value == REDACTED_VALUE {
        local_values
            .get_mut(&key.to_ascii_lowercase())
            .and_then(VecDeque::pop_front)
            .unwrap_or_default()
    } else {
        String::new()
    };
    format!("{raw_key}={value}")
}

fn split_url_query(value: &str) -> Option<(&str, &str, &str)> {
    let (without_fragment, fragment) = value
        .split_once('#')
        .map(|(url, _)| (url, &value[url.len()..]))
        .unwrap_or((value, ""));
    let (prefix, query) = without_fragment.split_once('?')?;
    Some((prefix, query, fragment))
}

fn sensitive_query_values(query: &str) -> HashMap<String, VecDeque<String>> {
    let mut values = HashMap::<String, VecDeque<String>>::new();
    for component in query.split('&') {
        let Some((key, _)) = decode_query_component(component) else {
            continue;
        };
        if !is_sensitive_key(&key) {
            continue;
        }
        let raw_value = component
            .split_once('=')
            .map(|(_, value)| value)
            .unwrap_or_default();
        values
            .entry(key.to_ascii_lowercase())
            .or_default()
            .push_back(raw_value.to_string());
    }
    values
}

fn decode_query_component(component: &str) -> Option<(String, String)> {
    let url = Url::parse(&format!("https://sync.invalid/?{component}")).ok()?;
    url.query_pairs()
        .next()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_secret_helpers_redact_and_restore_local_material() {
        let auth = r#"{"type":"bearer","token":"device-token"}"#;
        let snapshot = snapshot_auth_json(auth);
        assert!(!snapshot.contains("device-token"));
        assert!(snapshot.contains(REDACTED_VALUE));
        assert_eq!(
            serde_json::from_str::<Value>(&restore_auth_json(&snapshot, Some(auth))).unwrap(),
            serde_json::from_str::<Value>(auth).unwrap()
        );

        let url = "https://example.test/users?access_token=secret&page=1";
        let redacted = snapshot_url(url);
        assert!(!redacted.contains("secret"));
        assert_eq!(restore_url(&redacted, Some(url)), url);
    }

    #[test]
    fn api_key_auth_value_is_redacted_and_restored_shape_aware() {
        let auth = r#"{"type":"api-key","key":"X-API-Key","value":"sk-live-123","addTo":"header"}"#;
        let snapshot = snapshot_auth_json(auth);
        assert!(!snapshot.contains("sk-live-123"));
        let parsed: Value = serde_json::from_str(&snapshot).unwrap();
        assert_eq!(parsed["type"], "api-key");
        assert_eq!(parsed["key"], "X-API-Key");
        assert_eq!(parsed["addTo"], "header");
        assert_eq!(parsed["value"], REDACTED_VALUE);

        assert_eq!(
            serde_json::from_str::<Value>(&restore_auth_json(&snapshot, Some(auth))).unwrap(),
            serde_json::from_str::<Value>(auth).unwrap()
        );

        let injected =
            r#"{"type":"api-key","key":"X-API-Key","value":"attacker-supplied","addTo":"header"}"#;
        let restored: Value =
            serde_json::from_str(&restore_auth_json(injected, Some(auth))).unwrap();
        assert_eq!(
            restored["value"], "",
            "remote plaintext secret material must never be accepted"
        );
    }

    #[test]
    fn unknown_auth_types_redact_every_string_field_by_default() {
        let auth = r#"{"type":"hawk","hawkId":"public-id","hawkKey":"top-secret","meta":{"note":"internal"},"attempts":2,"enabled":true,"empty":""}"#;
        let snapshot = snapshot_auth_json(auth);
        let parsed: Value = serde_json::from_str(&snapshot).unwrap();
        assert_eq!(parsed["type"], "hawk");
        assert_eq!(parsed["hawkId"], REDACTED_VALUE);
        assert_eq!(parsed["hawkKey"], REDACTED_VALUE);
        assert_eq!(parsed["meta"]["note"], REDACTED_VALUE);
        assert_eq!(parsed["attempts"], 2);
        assert_eq!(parsed["enabled"], true);
        assert_eq!(parsed["empty"], "");
        assert_eq!(
            serde_json::from_str::<Value>(&restore_auth_json(&snapshot, Some(auth))).unwrap(),
            serde_json::from_str::<Value>(auth).unwrap()
        );
    }

    #[test]
    fn bearer_and_basic_auth_round_trips_keep_identity_fields() {
        let basic = r#"{"type":"basic","username":"alice","password":"hunter2"}"#;
        let snapshot = snapshot_auth_json(basic);
        let parsed: Value = serde_json::from_str(&snapshot).unwrap();
        assert_eq!(parsed["username"], "alice");
        assert_eq!(parsed["password"], REDACTED_VALUE);
        assert_eq!(
            serde_json::from_str::<Value>(&restore_auth_json(&snapshot, Some(basic))).unwrap(),
            serde_json::from_str::<Value>(basic).unwrap()
        );

        assert_eq!(snapshot_auth_json("bearer raw-token"), REDACTED_VALUE);
        assert_eq!(snapshot_auth_json(r#""raw-token""#), REDACTED_VALUE);
        assert_eq!(restore_auth_json(REDACTED_VALUE, Some(basic)), basic);
        assert_eq!(restore_auth_json(REDACTED_VALUE, None), DEFAULT_AUTH_JSON);
    }

    #[test]
    fn form_body_key_value_list_redacts_and_restores_by_field_key() {
        let body = r#"[{"key":"password","value":"hunter2","enabled":true},{"key":"page","value":"1","enabled":true}]"#;
        let snapshot = snapshot_body(Some(body), "form-urlencoded").unwrap();
        assert!(!snapshot.contains("hunter2"));
        let items: Vec<KeyValue> = serde_json::from_str(&snapshot).unwrap();
        assert_eq!(items[0].value, REDACTED_VALUE);
        assert_eq!(items[1].value, "1");

        let restored = restore_body(Some(snapshot), Some(body), "form-urlencoded").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&restored).unwrap(),
            serde_json::from_str::<Value>(body).unwrap()
        );

        let injected = r#"[{"key":"password","value":"attacker","enabled":true}]"#;
        let restored =
            restore_body(Some(injected.to_string()), Some(body), "form-urlencoded").unwrap();
        let items: Vec<KeyValue> = serde_json::from_str(&restored).unwrap();
        assert_eq!(items[0].value, "");
    }

    #[test]
    fn raw_form_body_redacts_and_restores_sensitive_pairs() {
        let body = "password=hunter2&x=1";
        let snapshot = snapshot_body(Some(body), "form-urlencoded").unwrap();
        assert_eq!(snapshot, "password=%3Credacted%3E&x=1");
        assert_eq!(
            restore_body(Some(snapshot), Some(body), "form-urlencoded").unwrap(),
            body
        );

        let json_body = r#"{"token":"body-secret","name":"Ada"}"#;
        let snapshot = snapshot_body(Some(json_body), "json").unwrap();
        assert!(!snapshot.contains("body-secret"));
        assert_eq!(
            serde_json::from_str::<Value>(
                &restore_body(Some(snapshot), Some(json_body), "json").unwrap()
            )
            .unwrap(),
            serde_json::from_str::<Value>(json_body).unwrap()
        );
    }

    #[test]
    fn url_userinfo_passwords_redact_and_restore() {
        let url = "https://alice:s3cret@host.test/path?x=1";
        let snapshot = snapshot_url(url);
        assert_eq!(snapshot, "https://alice:<redacted>@host.test/path?x=1");
        assert_eq!(restore_url(&snapshot, Some(url)), url);

        let bare = "https://alice:s3cret@host.test/path";
        let snapshot = snapshot_url(bare);
        assert_eq!(snapshot, "https://alice:<redacted>@host.test/path");
        assert_eq!(restore_url(&snapshot, Some(bare)), bare);

        assert_eq!(
            snapshot_url("{{scheme}}://bob:pw@{{host}}/x"),
            "{{scheme}}://bob:<redacted>@{{host}}/x"
        );
        assert_eq!(
            restore_url("https://alice:injected@host.test/path", Some(bare)),
            "https://alice:@host.test/path",
            "remote-supplied userinfo password must never be accepted"
        );
        assert_eq!(
            snapshot_url("https://host.test/a?b=c"),
            "https://host.test/a?b=c"
        );
    }

    #[test]
    fn url_secret_helpers_preserve_templates_encoding_order_and_fragments() {
        let url = "{{base_url}}/users/%7Bid%7D?access_token=device%2Bsecret&page=1&access_token=second#result";
        let redacted = snapshot_url(url);
        assert_eq!(
            redacted,
            "{{base_url}}/users/%7Bid%7D?access_token=%3Credacted%3E&page=1&access_token=%3Credacted%3E#result"
        );
        assert_eq!(restore_url(&redacted, Some(url)), url);
        assert_eq!(
            snapshot_url("{{base_url}}/users/%7Bid%7D"),
            "{{base_url}}/users/%7Bid%7D"
        );
    }
}
