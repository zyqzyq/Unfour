use super::*;
use futures_util::TryStreamExt;

pub(super) fn log_query_outcome(
    driver: &str,
    started: Instant,
    result: &AppResult<DatabaseQueryResult>,
) {
    let (event, status, error, fields) = match result {
        Ok(result) => (
            "query_completed",
            "ok",
            None,
            serde_json::json!({
                "driver": driver, "sql_operation": result.safety.classification,
                "row_count": result.rows.len(), "affected_rows": result.affected_rows,
            }),
        ),
        Err(error) => (
            "query_failed",
            "error",
            Some(unfour_diag::app_error_kind(error)),
            serde_json::json!({ "driver": driver }),
        ),
    };
    unfour_diag::log_operation_event(
        event,
        "database",
        "execute_query",
        status,
        Some(started.elapsed().as_millis()),
        error,
        fields,
    );
}

/// One checked-out physical connection for the entire script. Never return
/// arbitrary editor session state or an open user transaction to a pool.
pub(super) enum ScriptConnection {
    Sqlite(sqlx::pool::PoolConnection<sqlx::Sqlite>),
    Postgres(sqlx::pool::PoolConnection<sqlx::Postgres>),
    Mysql(sqlx::pool::PoolConnection<sqlx::MySql>),
}

impl DatabaseService {
    pub(super) async fn script_connection(
        &self,
        connection: &DatabaseConnection,
        input: &DatabaseQueryInput,
    ) -> AppResult<ScriptConnection> {
        let effective =
            Self::effective_connection(connection, clean_identifier(input.catalog.as_deref())?);
        let schema = clean_identifier(input.schema.as_deref())?;
        match connection.driver.as_str() {
            "sqlite" => {
                let mut conn = sqlite_pool(&effective).await?.acquire().await?;
                conn.close_on_drop();
                Ok(ScriptConnection::Sqlite(conn))
            }
            "postgres" => {
                let mut conn = self
                    .postgres_pool(&effective)
                    .await?
                    .acquire()
                    .await
                    .map_err(sanitize_pg_error)?;
                conn.close_on_drop();
                if let Some(schema) = schema {
                    conn.as_mut()
                        .execute(
                            format!("SET search_path TO {}", quote_identifier(schema)).as_str(),
                        )
                        .await
                        .map_err(sanitize_pg_error)?;
                }
                Ok(ScriptConnection::Postgres(conn))
            }
            "mysql" => {
                let mut conn = self
                    .mysql_pool(&effective)
                    .await?
                    .acquire()
                    .await
                    .map_err(sanitize_mysql_error)?;
                conn.close_on_drop();
                Ok(ScriptConnection::Mysql(conn))
            }
            _ => Err(AppError::Unsupported("unsupported database driver".into())),
        }
    }
}

impl ScriptConnection {
    pub(super) async fn execute(
        &mut self,
        sql: &str,
        limit: u32,
        safety: DatabaseQuerySafety,
    ) -> AppResult<(DatabaseQueryResult, bool)> {
        let started = Instant::now();
        let mut result = DatabaseQueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows: 0,
            duration_ms: 0,
            safety,
        };
        let mut rowset_finished = false;
        let mut additional_rowsets = false;
        // Use the simple protocol and inspect actual driver results, not a
        // leading keyword guess. Drain every row so RETURNING writes complete,
        // while bounding retained rows without adding SQL LIMIT clauses.
        macro_rules! drain {
            ($conn:expr, $columns:ident, $values:ident, $error:expr) => {{
                let mut stream = $conn.as_mut().fetch_many(sql);
                while let Some(item) = stream.try_next().await.map_err($error)? {
                    match item {
                        sqlx::Either::Left(done) => {
                            if !rowset_finished {
                                result.affected_rows += done.rows_affected();
                            }
                            rowset_finished = true;
                        }
                        sqlx::Either::Right(row) => {
                            // Procedures may return different row shapes. Never
                            // merge separate result sets into one result table.
                            if rowset_finished {
                                additional_rowsets = true;
                                continue;
                            }
                            if result.columns.is_empty() {
                                result.columns = $columns(&row);
                            }
                            if result.rows.len() < limit as usize {
                                result.rows.push($values(&row)?);
                            }
                        }
                    }
                }
            }};
        }
        match self {
            Self::Sqlite(conn) => drain!(
                conn,
                sqlite_result_columns,
                sqlite_row_values,
                AppError::from
            ),
            Self::Postgres(conn) => drain!(
                conn,
                postgres_result_columns,
                postgres_row_values,
                sanitize_pg_error
            ),
            Self::Mysql(conn) => drain!(
                conn,
                mysql_result_columns,
                mysql_row_values,
                sanitize_mysql_error
            ),
        }
        // Some engines report SELECT row counts as "affected"; reads don't mutate.
        if result.safety.classification == "read" {
            result.affected_rows = 0;
        }
        result.duration_ms = started.elapsed().as_millis();
        Ok((result, additional_rowsets))
    }
}
