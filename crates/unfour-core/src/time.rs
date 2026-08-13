use chrono::{DateTime, Utc};

/// Parse an RFC 3339 timestamp (any UTC offset, `Z` or `+hh:mm`) into a UTC
/// instant. Returns `None` for values that are not valid RFC 3339.
pub fn parse_rfc3339_utc(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}

/// Normalize an RFC 3339 timestamp to the canonical `+00:00` UTC form that
/// persisted timestamp columns use (`Utc::now().to_rfc3339()`), so SQLite TEXT
/// comparison stays chronologically correct across `Z`, `+00:00`, and
/// non-UTC-offset inputs.
pub fn normalize_rfc3339_utc(value: &str) -> Option<String> {
    parse_rfc3339_utc(value).map(|instant| instant.to_rfc3339())
}

/// Epoch microseconds for an RFC 3339 timestamp. Lets crates that do not
/// depend on chrono compare instants without string ordering pitfalls.
pub fn parse_rfc3339_epoch_micros(value: &str) -> Option<i64> {
    parse_rfc3339_utc(value).map(|instant| instant.timestamp_micros())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_z_and_offset_forms_to_the_same_instant() {
        let zulu = parse_rfc3339_utc("2026-08-13T06:00:00Z").expect("parse zulu");
        let offset = parse_rfc3339_utc("2026-08-13T14:00:00+08:00").expect("parse offset");
        assert_eq!(zulu, offset);
    }

    #[test]
    fn normalizes_to_utc_offset_form() {
        assert_eq!(
            normalize_rfc3339_utc("2026-08-13T14:00:00+08:00").as_deref(),
            Some("2026-08-13T06:00:00+00:00")
        );
        assert_eq!(
            normalize_rfc3339_utc("2026-08-13T06:00:00.250Z").as_deref(),
            Some("2026-08-13T06:00:00.250+00:00")
        );
    }

    #[test]
    fn rejects_invalid_timestamps() {
        assert!(parse_rfc3339_utc("not a timestamp").is_none());
        assert!(parse_rfc3339_utc("2026-08-13").is_none());
        assert!(parse_rfc3339_utc("").is_none());
    }
}
