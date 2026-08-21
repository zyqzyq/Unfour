use super::*;
use crate::transaction::CommandActivity;
use unfour_core::domain::CommandContext;

impl CommandBus {
    pub async fn list_database_connections(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<DatabaseConnection>> {
        self.database.list_connections(workspace_id).await
    }

    pub async fn save_database_connection(
        &self,
        input: DatabaseConnectionInput,
    ) -> AppResult<DatabaseConnection> {
        let context = CommandContext::local("database.connection.save");
        let executor_context = context.clone();
        let service = self.database.clone();
        self.execute_domain_command_with_activity(
            context,
            |connection: &DatabaseConnection| CommandActivity {
                workspace_id: Some(connection.workspace_id.clone()),
                action: "database.connection.save",
                target: Some(connection.id.clone()),
                details: serde_json::json!({
                    "name": connection.name,
                    "driver": connection.driver,
                    "credentialRef": connection.credential_ref.is_some()
                }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .save_connection_on(connection, &executor_context, input)
                        .await
                })
            },
        )
        .await
    }

    pub async fn delete_database_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<Vec<DatabaseConnection>> {
        let context = CommandContext::local("database.connection.delete");
        let executor_context = context.clone();
        let service = self.database.clone();
        let executor_service = service.clone();
        let (connections, cleanup) = self
            .execute_domain_command(
                context,
                Some(CommandActivity {
                    workspace_id: Some(workspace_id.clone()),
                    action: "database.connection.delete",
                    target: Some(connection_id.clone()),
                    details: serde_json::json!({ "softDelete": true }),
                }),
                move |connection| {
                    Box::pin(async move {
                        executor_service
                            .delete_connection_on(
                                connection,
                                &executor_context,
                                workspace_id,
                                connection_id,
                            )
                            .await
                    })
                },
            )
            .await?;
        service.cleanup_connection_changes(vec![cleanup]).await;
        Ok(connections)
    }

    pub async fn test_database_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<DatabaseTestResult> {
        self.database
            .test_connection(workspace_id, connection_id)
            .await
    }

    pub async fn test_database_connection_input(
        &self,
        input: DatabaseConnectionInput,
        secret: Option<String>,
    ) -> AppResult<DatabaseTestResult> {
        self.database.test_connection_input(input, secret).await
    }

    pub async fn database_schema(
        &self,
        workspace_id: String,
        connection_id: String,
        catalog: Option<String>,
    ) -> AppResult<DatabaseSchema> {
        self.database
            .schema(workspace_id, connection_id, catalog)
            .await
    }

    pub async fn database_catalogs(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<Vec<String>> {
        self.database
            .list_catalogs(workspace_id, connection_id)
            .await
    }

    pub async fn execute_database_query(
        &self,
        input: DatabaseQueryInput,
    ) -> AppResult<DatabaseQueryResult> {
        let result = self.database.execute_query(input.clone()).await?;
        if result.safety.classification != "read" {
            let classification = result.safety.classification.clone();
            self.activity_log
                .record(
                    Some(&input.workspace_id),
                    "database.query.execute",
                    Some(&input.connection_id),
                    serde_json::json!({
                        "classification": classification,
                        "confirmed": result.safety.confirmed,
                        "columns": result.columns.len(),
                        "rows": result.rows.len(),
                        "affectedRows": result.affected_rows
                    }),
                )
                .await?;
        }
        Ok(result)
    }

    pub async fn browse_database_table(
        &self,
        input: DatabaseBrowseInput,
    ) -> AppResult<DatabaseBrowseResult> {
        self.database.browse_table(input).await
    }

    pub async fn database_table_structure(
        &self,
        input: DatabaseTableStructureInput,
    ) -> AppResult<DatabaseTableStructure> {
        self.database.table_structure(input).await
    }

    pub async fn mutate_database_row(
        &self,
        input: DatabaseRowMutationInput,
    ) -> AppResult<DatabaseRowMutationResult> {
        let workspace_id = input.workspace_id.clone();
        let connection_id = input.connection_id.clone();
        let table_name = input.table_name.clone();
        let operation = input.operation.clone();
        let result = self.database.mutate_table_row(input).await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "database.row.mutate",
                Some(&connection_id),
                serde_json::json!({
                    "operation": operation,
                    "tableName": table_name,
                    "affectedRows": result.affected_rows,
                    "confirmed": true
                }),
            )
            .await?;
        Ok(result)
    }

    pub async fn record_database_query_history(
        &self,
        input: DbQueryHistoryRecordInput,
    ) -> AppResult<()> {
        self.database.record_query_history(input).await
    }

    pub async fn list_database_query_history(
        &self,
        workspace_id: String,
        limit: Option<i64>,
    ) -> AppResult<Vec<DbQueryHistoryEntry>> {
        self.database.list_query_history(workspace_id, limit).await
    }

    pub async fn clear_database_query_history(&self, workspace_id: String) -> AppResult<()> {
        self.database.clear_query_history(workspace_id).await
    }

    pub async fn list_saved_sql(&self, workspace_id: String) -> AppResult<Vec<SavedSql>> {
        self.database.list_saved_sql(workspace_id).await
    }

    pub async fn save_saved_sql(&self, input: SavedSqlInput) -> AppResult<SavedSql> {
        let saved = self.database.save_sql(input).await?;
        self.activity_log
            .record(
                Some(&saved.workspace_id),
                "database.saved_sql.save",
                Some(&saved.id),
                serde_json::json!({ "name": saved.name }),
            )
            .await?;
        Ok(saved)
    }

    pub async fn delete_saved_sql(
        &self,
        workspace_id: String,
        id: String,
    ) -> AppResult<Vec<SavedSql>> {
        let remaining = self
            .database
            .delete_saved_sql(workspace_id.clone(), id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "database.saved_sql.delete",
                Some(&id),
                serde_json::json!({}),
            )
            .await?;
        Ok(remaining)
    }
}
