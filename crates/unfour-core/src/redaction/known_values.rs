use super::REDACTED_VALUE;

/// Scrub known credentials in raw, JSON-escaped and form/URL-encoded text.
/// This is a persistence/diagnostic view; never apply it to execution payloads.
pub fn scrub_secret_values(value: &str, secrets: &[String]) -> String {
    let mut variants = Vec::new();
    for secret in secrets
        .iter()
        .filter(|secret| !secret.is_empty() && secret.as_str() != REDACTED_VALUE)
    {
        // Serializing a String always succeeds and includes two enclosing quotes.
        let escaped = serde_json::to_string(secret).expect("serialize credential string");
        let encoded = reqwest::Url::parse_with_params("http://localhost/", &[("v", secret)])
            .expect("static URL")
            .query()
            .expect("query parameter")
            .trim_start_matches("v=")
            .to_owned();
        variants.extend([
            secret.clone(),
            escaped[1..escaped.len() - 1].to_owned(),
            encoded.clone(),
            encoded.replace('+', "%20"),
        ]);
    }
    variants.retain(|value| !value.is_empty());
    variants.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    variants.dedup();
    let mut value = value.to_owned();
    for secret in variants {
        // A shorter credential must not corrupt an earlier redaction marker.
        value = value
            .split(REDACTED_VALUE)
            .map(|part| part.replace(&secret, REDACTED_VALUE))
            .collect::<Vec<_>>()
            .join(REDACTED_VALUE);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_encoded_and_json_escaped_credentials_without_erasing_safe_context() {
        let secret = "opaque+/ value=\"end\\";
        let escaped = serde_json::to_string(secret).unwrap();
        for representation in [
            secret.to_owned(),
            escaped[1..escaped.len() - 1].to_owned(),
            "opaque%2B%2F+value%3D%22end%5C".into(),
            "opaque%2B%2F%20value%3D%22end%5C".into(),
        ] {
            assert_eq!(
                scrub_secret_values(&format!("before {representation} after"), &[secret.into()]),
                "before <redacted> after"
            );
        }
    }

    #[test]
    fn overlapping_values_and_one_character_secrets_preserve_markers() {
        assert_eq!(
            scrub_secret_values(
                "abc abcdef <redacted>",
                &["a".into(), "abc".into(), "abcdef".into()]
            ),
            "<redacted> <redacted> <redacted>"
        );
        assert_eq!(
            scrub_secret_values("keep", &[String::new(), REDACTED_VALUE.into()]),
            "keep"
        );
    }
}
