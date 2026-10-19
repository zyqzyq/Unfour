use super::*;

impl DatabaseService {
    /// Validate without opening a database connection or executing SQL.
    pub fn validate_single_statement(sql: &str, driver: &str) -> AppResult<()> {
        if super::script_parser::split_script(sql, DatabaseDialect::for_driver(driver))?.len() != 1
        {
            return Err(AppError::Validation(
                "FLOW_SQL_SINGLE_STATEMENT_REQUIRED".into(),
            ));
        }
        Ok(())
    }

    pub async fn execute_query(&self, input: DatabaseQueryInput) -> AppResult<DatabaseQueryResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        let dialect = DatabaseDialect::for_driver(&connection.driver);
        let statements = super::script_parser::split_script(&input.sql, dialect)?;
        if statements.len() != 1 {
            return Err(AppError::Validation(
                "exactly one SQL statement is required".into(),
            ));
        }
        let mut safety = classify_query_for_dialect(&input.sql, dialect);
        // The saved flag and an explicit MCP request share one gate. Neither
        // one is enough on its own: a later native constraint covers reads
        // that classification still treats as read-only.
        let enforce_read_only = input.enforce_read_only || connection.read_only;
        if enforce_read_only && safety.classification != "read" {
            let scope = if connection.read_only {
                "this connection is read-only"
            } else {
                "read-only execution"
            };
            return Err(AppError::ReadOnly(format!(
                "{scope}; {} statements are not allowed",
                safety.classification
            )));
        }
        if safety.requires_confirmation && input.confirm_mutation != Some(true) {
            return Err(AppError::ConfirmationRequired {
                message: safety
                    .message
                    .clone()
                    .unwrap_or_else(|| "SQL statement requires confirmation".into()),
                details: serde_json::json!({ "classification": safety.classification, "requiresConfirmation": true, "confirmed": false }),
            });
        }
        safety.confirmed = !safety.requires_confirmation || input.confirm_mutation == Some(true);
        let timeout = resolve_timeout(input.timeout_ms);
        let run = async {
            let mut conn = self.script_connection(&connection, &input).await?;
            if enforce_read_only {
                conn.establish_read_only().await?;
            }
            let executed = conn
                .execute(
                    &input.sql,
                    input.limit.unwrap_or(100).clamp(1, 1_000),
                    safety,
                )
                .await
                .map(|(result, _)| result);
            if !enforce_read_only {
                return executed;
            }
            // Always leave the physical connection before it can return to a
            // pool. A failed statement still rolls back; its error wins.
            let released = conn.release_read_only().await;
            match executed {
                Ok(result) => released.map(|()| result),
                Err(error) => {
                    let _ = released;
                    Err(error)
                }
            }
        };
        let started = Instant::now();
        let result = tokio::time::timeout(timeout, run)
            .await
            .unwrap_or_else(|_| {
                Err(AppError::Timeout(format!(
                    "query exceeded the {} ms timeout; completion may be uncertain",
                    timeout.as_millis()
                )))
            });
        super::script_connection::log_query_outcome(&connection.driver, started, &result);
        result
    }
}
