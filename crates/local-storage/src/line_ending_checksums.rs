use sha2::{Digest, Sha384};
use sqlx::migrate::Migrator;
use sqlx::SqlitePool;
use unfour_core::AppResult;

/// Rewrite `_sqlx_migrations.checksum` when it matches the same SQL with the
/// opposite newline convention.
///
/// sqlx hashes raw file bytes. A Windows working tree with `core.autocrlf=true`
/// can embed CRLF into a newly added migration even when Git stores LF.
/// Opening that database from a CI/LF binary then fails with `VersionMismatch`.
pub async fn reconcile_sqlx_line_ending_checksums(
    pool: &SqlitePool,
    migrator: &Migrator,
) -> AppResult<()> {
    let table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !table_exists {
        return Ok(());
    }

    for migration in migrator.iter() {
        let stored: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version = ?1 AND success = 1",
        )
        .bind(migration.version)
        .fetch_optional(pool)
        .await?;
        let Some(stored) = stored else {
            continue;
        };
        if stored.as_slice() == migration.checksum.as_ref() {
            continue;
        }
        let Some(alternate) = alternate_line_ending_checksum(migration.sql.as_ref()) else {
            continue;
        };
        if stored.as_slice() != alternate.as_slice() {
            continue;
        }
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2")
            .bind(migration.checksum.as_ref())
            .bind(migration.version)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub(crate) fn alternate_line_ending_checksum(sql: &str) -> Option<Vec<u8>> {
    let alternate = if sql.contains("\r\n") {
        sql.replace("\r\n", "\n")
    } else if sql.contains('\n') {
        sql.replace('\n', "\r\n")
    } else {
        return None;
    };
    Some(Sha384::digest(alternate.as_bytes()).to_vec())
}

#[cfg(test)]
mod tests {
    use super::alternate_line_ending_checksum;
    use sha2::{Digest, Sha384};

    #[test]
    fn lf_and_crlf_checksums_are_swapped_equivalents() {
        let lf = "SELECT 1;\n";
        let crlf = "SELECT 1;\r\n";
        assert_eq!(
            alternate_line_ending_checksum(lf).as_deref(),
            Some(Sha384::digest(crlf.as_bytes()).as_slice())
        );
        assert_eq!(
            alternate_line_ending_checksum(crlf).as_deref(),
            Some(Sha384::digest(lf.as_bytes()).as_slice())
        );
    }
}
