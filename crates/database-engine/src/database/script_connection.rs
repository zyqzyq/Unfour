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
enum ScriptDriverConnection {
    Sqlite(sqlx::pool::PoolConnection<sqlx::Sqlite>),
    Postgres(sqlx::pool::PoolConnection<sqlx::Postgres>),
    Mysql(sqlx::pool::PoolConnection<sqlx::MySql>),
}

pub(super) struct ScriptConnection {
    driver: ScriptDriverConnection,
    pub profile: RuntimeDatabaseProfile,
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
                let pool = sqlite_pool(&effective).await?;
                let mut conn = pool.acquire().await?;
                conn.close_on_drop();
                Ok(ScriptConnection {
                    driver: ScriptDriverConnection::Sqlite(conn),
                    profile: pool.profile,
                })
            }
            "postgres" => {
                let pool = self.postgres_pool(&effective).await?;
                let mut conn = pool.acquire().await.map_err(sanitize_pg_error)?;
                conn.close_on_drop();
                if let Some(schema) = schema {
                    require_capability(pool.profile.capabilities.schemas, "schemas")?;
                    conn.as_mut()
                        .execute(
                            format!(
                                "SET search_path TO {}",
                                pool.profile.dialect.quote_identifier(schema)
                            )
                            .as_str(),
                        )
                        .await
                        .map_err(sanitize_pg_error)?;
                }
                Ok(ScriptConnection {
                    driver: ScriptDriverConnection::Postgres(conn),
                    profile: pool.profile,
                })
            }
            "mysql" => {
                let pool = self.mysql_pool(&effective).await?;
                let mut conn = pool.acquire().await.map_err(sanitize_mysql_error)?;
                conn.close_on_drop();
                Ok(ScriptConnection {
                    driver: ScriptDriverConnection::Mysql(conn),
                    profile: pool.profile,
                })
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
        let runtime_safety = classify_query_for_dialect(sql, self.profile.dialect);
        if runtime_safety.classification != safety.classification {
            return Err(AppError::Unsupported(
                "runtime SQL dialect differs from preflight".into(),
            ));
        }
        let started = Instant::now();
        // Callers clamp too. Keep the cap here so a retained row is never past
        // the command-bus maximum, and so `row_limit` matches what was applied.
        let limit = limit.clamp(1, 1_000);
        let mut result = DatabaseQueryResult {
            warnings: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows: 0,
            duration_ms: 0,
            safety,
            truncated: false,
            row_limit: limit,
        };
        let mut budget = ResultRowBudget::new(limit);
        // Use the simple protocol and inspect actual driver results, not a
        // leading keyword guess. Retain at most `limit` rows from the first
        // result set and record truncation only when another row of that set
        // arrives. Do not append LIMIT to the caller's SQL.
        //
        // The stream is still drained after the cap. Stopping early would
        // change execution: SQLite applies RETURNING rows as it steps, SELECT
        // can call volatile functions per row, and PostgreSQL/openGauss/MySQL
        // simple-query streams must reach the end of every result set before
        // the next statement on this connection. A later read-only portal
        // would be a separate, explicit mode.
        macro_rules! drain {
            ($conn:expr, $columns:ident, $values:ident, $error:expr) => {{
                let mut stream = $conn.as_mut().fetch_many(sql);
                while let Some(item) = stream.try_next().await.map_err($error)? {
                    match item {
                        sqlx::Either::Left(done) => {
                            if budget.finish_rowset() {
                                result.affected_rows += done.rows_affected();
                            }
                        }
                        sqlx::Either::Right(row) => match budget.observe_row() {
                            RowBudgetDecision::Keep => {
                                if result.columns.is_empty() {
                                    result.columns = $columns(&row);
                                }
                                result.rows.push($values(&row)?);
                            }
                            // The driver row is dropped here, but the stream
                            // continues so the statement and later result sets
                            // still run to completion.
                            RowBudgetDecision::Omit | RowBudgetDecision::Additional => {}
                        },
                    }
                }
            }};
        }
        match &mut self.driver {
            ScriptDriverConnection::Sqlite(conn) => drain!(
                conn,
                sqlite_result_columns,
                sqlite_row_values,
                AppError::from
            ),
            ScriptDriverConnection::Postgres(conn) => drain!(
                conn,
                postgres_result_columns,
                postgres_row_values,
                sanitize_pg_error
            ),
            ScriptDriverConnection::Mysql(conn) => drain!(
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
        result.truncated = budget.truncated;
        result.duration_ms = started.elapsed().as_millis();
        Ok((result, budget.additional_rowsets))
    }
}

/// Decides which streamed rows are kept. The driver stream itself is not stopped.
pub(super) struct ResultRowBudget {
    limit: usize,
    kept: usize,
    pub truncated: bool,
    rowset_finished: bool,
    pub additional_rowsets: bool,
}

pub(super) enum RowBudgetDecision {
    Keep,
    Omit,
    Additional,
}

impl ResultRowBudget {
    pub(super) fn new(limit: u32) -> Self {
        Self {
            limit: limit as usize,
            kept: 0,
            truncated: false,
            rowset_finished: false,
            additional_rowsets: false,
        }
    }

    /// Returns whether this completion belongs to the first result set.
    /// Later completions must not be added into `affected_rows`.
    pub(super) fn finish_rowset(&mut self) -> bool {
        let first = !self.rowset_finished;
        self.rowset_finished = true;
        first
    }

    pub(super) fn observe_row(&mut self) -> RowBudgetDecision {
        if self.rowset_finished {
            self.additional_rowsets = true;
            return RowBudgetDecision::Additional;
        }
        if self.kept < self.limit {
            self.kept += 1;
            RowBudgetDecision::Keep
        } else {
            self.truncated = true;
            RowBudgetDecision::Omit
        }
    }
}

#[cfg(test)]
mod result_row_budget_tests {
    use super::{ResultRowBudget, RowBudgetDecision};

    #[test]
    fn exact_limit_is_not_truncated() {
        let mut budget = ResultRowBudget::new(2);
        assert!(matches!(budget.observe_row(), RowBudgetDecision::Keep));
        assert!(matches!(budget.observe_row(), RowBudgetDecision::Keep));
        assert!(budget.finish_rowset());
        assert!(!budget.truncated);
        assert!(!budget.additional_rowsets);
    }

    #[test]
    fn one_row_past_the_limit_is_truncated() {
        let mut budget = ResultRowBudget::new(2);
        assert!(matches!(budget.observe_row(), RowBudgetDecision::Keep));
        assert!(matches!(budget.observe_row(), RowBudgetDecision::Keep));
        assert!(matches!(budget.observe_row(), RowBudgetDecision::Omit));
        assert!(budget.truncated);
        assert!(budget.finish_rowset());
        assert!(!budget.finish_rowset());
    }

    #[test]
    fn later_result_sets_are_not_merged_or_counted_as_the_first_cap() {
        let mut budget = ResultRowBudget::new(1);
        assert!(matches!(budget.observe_row(), RowBudgetDecision::Keep));
        assert!(budget.finish_rowset());
        assert!(matches!(
            budget.observe_row(),
            RowBudgetDecision::Additional
        ));
        assert!(!budget.truncated);
        assert!(budget.additional_rowsets);
        assert!(!budget.finish_rowset());
    }
}
