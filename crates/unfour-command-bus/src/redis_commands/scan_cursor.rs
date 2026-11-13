use std::hash::{DefaultHasher, Hash, Hasher};
use unfour_core::{AppError, AppResult};

use crate::RedisConnection;

// Consistency tag for ephemeral paging within a single client version.
// Authorization and connection lookup are performed on every call, independently.
pub(super) fn scope(connection: &RedisConnection, pattern: &str) -> String {
    let mut tag = DefaultHasher::new();
    (
        &connection.workspace_id,
        &connection.id,
        connection.revision,
        connection.db_index,
        pattern,
    )
        .hash(&mut tag);
    format!("r1.{:016x}.", tag.finish())
}

pub(super) fn pack(scope: &str, offset: &str) -> String {
    if offset == "0" {
        "0".into()
    } else {
        format!("{scope}{offset}")
    }
}

pub(super) fn unpack<'a>(scope: &str, cursor: &'a str) -> AppResult<&'a str> {
    if cursor == "0" {
        return Ok(cursor);
    }
    cursor
        .strip_prefix(scope)
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 20
                && !s.starts_with('0')
                && s.bytes().all(|c| c.is_ascii_digit())
                && s.parse::<u64>().is_ok()
        })
        .ok_or_else(|| AppError::Validation("REDIS_INVALID_INPUT".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_scan_cursor_is_bound_to_workspace_resource_revision_db_and_pattern() {
        let mut record = RedisConnection {
            id: "conn".into(),
            workspace_id: "a".into(),
            name: "name".into(),
            host: "localhost".into(),
            port: 6379,
            db_index: 0,
            username: None,
            tls: false,
            has_password: false,
            revision: 2,
        };
        let original = scope(&record, "price:*");
        let cursor = pack(&original, "18446744073709551615");
        assert!(cursor.len() <= 40);
        assert_eq!(unpack(&original, &cursor).unwrap(), "18446744073709551615");
        assert_eq!(pack(&original, "0"), "0");
        assert!(unpack(&original, "244").is_err());
        assert!(unpack(&original, &format!("{original}18446744073709551616")).is_err());
        assert!(unpack(&scope(&record, "another:*"), &cursor).is_err());
        record.workspace_id = "b".into();
        assert!(unpack(&scope(&record, "price:*"), &cursor).is_err());
        record.workspace_id = "a".into();
        record.db_index = 1;
        assert!(unpack(&scope(&record, "price:*"), &cursor).is_err());
        record.db_index = 0;
        record.revision += 1;
        assert!(unpack(&scope(&record, "price:*"), &cursor).is_err());
        record.revision -= 1;
        record.id = "another".into();
        assert!(unpack(&scope(&record, "price:*"), &cursor).is_err());
    }
}
