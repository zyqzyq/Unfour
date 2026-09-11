use super::script_parser::{split_script, ScriptStatement};
use super::*;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use unfour_core::models::{DatabaseScriptInput, DatabaseScriptResult, DatabaseStatementResult};

type RunKey = (String, String);
type Runs = Mutex<HashMap<RunKey, Arc<AtomicBool>>>;
static RUNS: OnceLock<Runs> = OnceLock::new();

struct RunGuard(RunKey);
impl Drop for RunGuard {
    fn drop(&mut self) {
        if let Ok(mut runs) = RUNS.get_or_init(Default::default).lock() {
            runs.remove(&self.0);
        }
    }
}

fn select_statements(
    source: &str,
    driver: &str,
    cursor: Option<usize>,
) -> AppResult<Vec<ScriptStatement>> {
    let mut statements = split_script(source, driver)?;
    if let Some(cursor) = cursor {
        let index = statements
            .iter()
            .position(|s| cursor < s.end)
            .unwrap_or(statements.len().saturating_sub(1));
        if !statements.is_empty() {
            statements = vec![statements.remove(index)];
        }
    }
    if statements.is_empty() {
        return Err(AppError::Validation("SQL cannot be empty".into()));
    }
    Ok(statements)
}

impl DatabaseService {
    /// Stop schedules no more statements. The current statement is allowed to
    /// finish (or timeout); its actual outcome is preserved, never called rolled back.
    pub fn stop_script(&self, workspace_id: String, run_id: String) -> AppResult<bool> {
        validate_workspace_id(&workspace_id)?;
        let runs = RUNS
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| AppError::Config("SQL run registry unavailable".into()))?;
        if let Some(flag) = runs.get(&(workspace_id, run_id)) {
            flag.store(true, Ordering::SeqCst);
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn execute_script(
        &self,
        input: DatabaseScriptInput,
    ) -> AppResult<DatabaseScriptResult> {
        validate_workspace_id(&input.query.workspace_id)?;
        validate_connection_id(&input.query.connection_id)?;
        if input.run_id.trim().is_empty() || input.run_id.len() > 128 {
            return Err(AppError::Validation("invalid SQL run id".into()));
        }
        let key = (input.query.workspace_id.clone(), input.run_id.clone());
        let stopped = Arc::new(AtomicBool::new(false));
        {
            let mut runs = RUNS
                .get_or_init(Default::default)
                .lock()
                .map_err(|_| AppError::Config("SQL run registry unavailable".into()))?;
            if runs.contains_key(&key) {
                return Err(AppError::Validation("SQL run is already active".into()));
            }
            runs.insert(key.clone(), stopped.clone());
        }
        let _guard = RunGuard(key);
        let connection = self
            .get_connection(&input.query.workspace_id, &input.query.connection_id)
            .await?;
        let mut statements =
            select_statements(&input.query.sql, &connection.driver, input.cursor_offset)?;
        if input.explain {
            if statements.len() != 1 {
                return Err(AppError::Validation(
                    "EXPLAIN requires one statement".into(),
                ));
            }
            statements[0].sql = format!("EXPLAIN {}", statements[0].sql);
        }
        let safeties = statements
            .iter()
            .map(|s| classify_query_for_driver(&s.sql, &connection.driver))
            .collect::<Vec<_>>();
        let mut output = DatabaseScriptResult {
            statements: statements
                .iter()
                .map(|s| DatabaseStatementResult {
                    index: s.index,
                    start: s.start,
                    end: s.end,
                    sql: s.sql.clone(),
                    status: "skipped".into(),
                    result: None,
                    error: None,
                })
                .collect(),
            stopped: false,
            warnings: Vec::new(),
        };
        if stopped.load(Ordering::SeqCst) {
            output.stopped = true;
            return Ok(output);
        }
        // Whole-script preflight: no earlier read, DDL or session command runs
        // before a later blocked statement or confirmation is resolved.
        if connection.read_only {
            if let Some(index) = safeties.iter().position(|s| s.classification != "read") {
                return Err(AppError::ReadOnly(format!(
                    "statement {} is not allowed on this read-only connection",
                    statements[index].index
                )));
            }
        }
        if input.query.confirm_mutation != Some(true) {
            let indices: Vec<_> = safeties
                .iter()
                .enumerate()
                .filter(|(_, s)| s.requires_confirmation)
                .map(|(i, _)| statements[i].index)
                .collect();
            if !indices.is_empty() {
                return Err(AppError::ConfirmationRequired {
                    message: "This script may modify data, schema or session state. Confirm the entire script before execution.".into(),
                    details: serde_json::json!({
                        "statementIndices": indices, "statementCount": statements.len(), "confirmed": false,
                        "statementRanges": statements.iter().map(|s| serde_json::json!({ "start": s.start, "end": s.end })).collect::<Vec<_>>(),
                    }),
                });
            }
        }
        let timeout = resolve_timeout(input.query.timeout_ms);
        let mut conn =
            tokio::time::timeout(timeout, self.script_connection(&connection, &input.query))
                .await
                .map_err(|_| AppError::Timeout("database connection setup timed out".into()))??;
        for (entry, mut safety) in output.statements.iter_mut().zip(safeties) {
            if stopped.load(Ordering::SeqCst) {
                break;
            }
            safety.confirmed =
                !safety.requires_confirmation || input.query.confirm_mutation == Some(true);
            let started = Instant::now();
            let result = tokio::time::timeout(
                timeout,
                conn.execute(
                    &entry.sql,
                    input.query.limit.unwrap_or(100).clamp(1, 1_000),
                    safety,
                ),
            )
            .await
            .unwrap_or_else(|_| {
                Err(AppError::Timeout(format!(
                    "statement {} exceeded {} ms; completion may be uncertain",
                    entry.index,
                    timeout.as_millis()
                )))
            });
            let result = result.map(|(result, additional_rowsets)| {
                if additional_rowsets {
                    output
                        .warnings
                        .push("database.batch.additionalResults".into());
                }
                result
            });
            super::script_connection::log_query_outcome(&connection.driver, started, &result);
            match result {
                Ok(result) => {
                    entry.status = "success".into();
                    entry.result = Some(result);
                }
                Err(error) => {
                    entry.status = "failed".into();
                    entry.error = Some(serde_json::to_value(&error)?);
                    break;
                }
            }
        }
        output.stopped = stopped.load(Ordering::SeqCst);
        // close_on_drop also discards failed/timed-out connections and rolls
        // back open transactions. No transaction/session survives this batch.
        Ok(output)
    }
}
