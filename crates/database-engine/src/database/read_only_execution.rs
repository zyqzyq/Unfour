//! Database-enforced read-only execution on the checked-out connection.
//!
//! SQL classification still rejects wrapped writes before connect. This layer
//! stops a statement that classification still treats as a read, such as
//! `SELECT nextval(...)`, from changing durable database state.
//!
//! PostgreSQL and openGauss use `BEGIN READ ONLY` and continue only when
//! `transaction_read_only` reads back as on. MySQL uses
//! `START TRANSACTION READ ONLY` and continues only when
//! `@@transaction_read_only` is on. SQLite sets `PRAGMA query_only` and
//! continues only when that pragma reads back as on. The constraint is
//! transaction- or connection-scoped and is released on the same connection.
//! Session defaults are not changed, so a later checkout is not left read-only.
//!
//! A verified constraint does not stop every effect outside the database
//! transaction. Temporary tables, file or network functions, `dblink` and
//! foreign-data writes that use another credential, and openGauss autonomous
//! transactions can still have effects. A database role without write
//! privilege is the additional control for those cases. If the constraint
//! cannot be started or verified, the caller's statement is not sent.

use super::script_connection::{ScriptConnection, ScriptDriverConnection};
use super::*;
use sqlx::{Executor, Row};

const NOT_ESTABLISHED: &str =
    "read-only execution could not be established and the statement was not sent";

impl ScriptConnection {
    /// Start and verify a native read-only constraint. On failure the caller's
    /// statement must not be sent.
    pub(super) async fn establish_read_only(&mut self) -> AppResult<()> {
        match &mut self.driver {
            ScriptDriverConnection::Sqlite(conn) => {
                if let Err(error) = set_sqlite_query_only(conn.as_mut(), true).await {
                    let _ = set_sqlite_query_only(conn.as_mut(), false).await;
                    return Err(error);
                }
                Ok(())
            }
            ScriptDriverConnection::Postgres(conn) => {
                conn.as_mut()
                    .execute("BEGIN READ ONLY")
                    .await
                    .map_err(sanitize_pg_error)?;
                match fetch_postgres_flag(conn.as_mut(), "SHOW transaction_read_only").await {
                    Ok(true) => Ok(()),
                    Ok(false) => {
                        let _ = conn.as_mut().execute("ROLLBACK").await;
                        Err(AppError::ReadOnly(NOT_ESTABLISHED.into()))
                    }
                    Err(error) => {
                        let _ = conn.as_mut().execute("ROLLBACK").await;
                        Err(error)
                    }
                }
            }
            ScriptDriverConnection::Mysql(conn) => {
                conn.as_mut()
                    .execute("START TRANSACTION READ ONLY")
                    .await
                    .map_err(sanitize_mysql_error)?;
                match fetch_mysql_flag(conn.as_mut(), "SELECT @@transaction_read_only").await {
                    Ok(true) => Ok(()),
                    Ok(false) => {
                        let _ = conn.as_mut().execute("ROLLBACK").await;
                        Err(AppError::ReadOnly(NOT_ESTABLISHED.into()))
                    }
                    Err(error) => {
                        let _ = conn.as_mut().execute("ROLLBACK").await;
                        Err(error)
                    }
                }
            }
        }
    }

    /// End the constraint on this same connection. PostgreSQL, openGauss, and
    /// MySQL roll the read-only transaction back. SQLite clears `query_only`.
    pub(super) async fn release_read_only(&mut self) -> AppResult<()> {
        match &mut self.driver {
            ScriptDriverConnection::Sqlite(conn) => {
                set_sqlite_query_only(conn.as_mut(), false).await
            }
            ScriptDriverConnection::Postgres(conn) => {
                conn.as_mut()
                    .execute("ROLLBACK")
                    .await
                    .map_err(sanitize_pg_error)?;
                Ok(())
            }
            ScriptDriverConnection::Mysql(conn) => {
                conn.as_mut()
                    .execute("ROLLBACK")
                    .await
                    .map_err(sanitize_mysql_error)?;
                Ok(())
            }
        }
    }
}

async fn set_sqlite_query_only(conn: &mut sqlx::SqliteConnection, enabled: bool) -> AppResult<()> {
    let statement = if enabled {
        "PRAGMA query_only = ON"
    } else {
        "PRAGMA query_only = OFF"
    };
    // The assignment may or may not yield a row. The following read is the check.
    let _ = conn.fetch_optional(statement).await?;
    let value: i64 = sqlx::query_scalar("PRAGMA query_only")
        .fetch_one(&mut *conn)
        .await?;
    if (value == 1) != enabled {
        return Err(AppError::ReadOnly(NOT_ESTABLISHED.into()));
    }
    Ok(())
}

async fn fetch_postgres_flag(conn: &mut sqlx::PgConnection, sql: &str) -> AppResult<bool> {
    let row: sqlx::postgres::PgRow = conn.fetch_one(sql).await.map_err(sanitize_pg_error)?;
    Ok(row.try_get::<String, _>(0).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "on" | "1" | "true"
        )
    }))
}

async fn fetch_mysql_flag(conn: &mut sqlx::MySqlConnection, sql: &str) -> AppResult<bool> {
    let row: sqlx::mysql::MySqlRow = conn.fetch_one(sql).await.map_err(sanitize_mysql_error)?;
    if row.try_get::<i64, _>(0).ok() == Some(1)
        || row.try_get::<i8, _>(0).ok() == Some(1)
        || row.try_get::<u64, _>(0).ok() == Some(1)
        || row.try_get::<bool, _>(0).ok() == Some(true)
    {
        return Ok(true);
    }
    let text = row.try_get::<String, _>(0).ok().or_else(|| {
        row.try_get::<Vec<u8>, _>(0)
            .ok()
            .map(|value| String::from_utf8_lossy(&value).into_owned())
    });
    Ok(text.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "on" | "true"
        )
    }))
}
