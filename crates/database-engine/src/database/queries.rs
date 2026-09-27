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
        if connection.read_only && safety.classification != "read" {
            return Err(AppError::ReadOnly(format!(
                "this connection is read-only; {} statements are not allowed",
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
            conn.execute(
                &input.sql,
                input.limit.unwrap_or(100).clamp(1, 1_000),
                safety,
            )
            .await
            .map(|(result, _)| result)
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
