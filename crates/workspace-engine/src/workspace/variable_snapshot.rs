use super::variables::{get_environment_on, list_variables_on};
use super::*;
use unfour_core::models::{WorkspaceEnvironment, WorkspaceVariable};

/// Ephemeral metadata and values from one SQLite read transaction. Never persisted.
pub struct WorkspaceVariableSnapshot {
    pub variables: Vec<WorkspaceVariable>,
    pub environment: Option<WorkspaceEnvironment>,
}

impl WorkspaceVariableSnapshot {
    pub fn resolver(&self, include_environment: bool) -> WorkspaceVariableResolver {
        let mut values = std::collections::HashMap::new();
        for variable in self.variables.iter().filter(|v| v.is_enabled) {
            values.insert(
                variable.key.clone(),
                (variable.value.clone(), variable.is_secret),
            );
        }
        if include_environment {
            for variable in self
                .environment
                .iter()
                .flat_map(|env| &env.variables)
                .filter(|v| v.is_enabled)
            {
                values.insert(
                    variable.key.clone(),
                    (variable.value.clone(), variable.is_secret),
                );
            }
        }
        WorkspaceVariableResolver::from_values(values)
    }
}

impl WorkspaceService {
    /// Version checks and execution resolution must consume this same immutable snapshot.
    pub async fn variable_snapshot(
        &self,
        workspace_id: &str,
        environment_id: Option<&str>,
    ) -> AppResult<WorkspaceVariableSnapshot> {
        let mut transaction = self.db.pool().begin().await?;
        get_workspace_on(&mut transaction, workspace_id, false).await?;
        let variables = list_variables_on(&mut transaction, workspace_id, false).await?;
        let environment = match environment_id {
            Some(id) => Some(get_environment_on(&mut transaction, workspace_id, id, false).await?),
            None => None,
        };
        transaction.rollback().await?;
        Ok(WorkspaceVariableSnapshot {
            variables,
            environment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn variable_snapshot_keeps_metadata_and_both_sources_consistent_during_writes() {
        let path = std::env::temp_dir().join(format!(
            "unfour-variable-snapshot-{}.sqlite",
            unfour_core::id::new_id()
        ));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true)
                    .foreign_keys(true)
                    .journal_mode(SqliteJournalMode::Wal),
            )
            .await
            .unwrap();
        let db = LocalDb::from_pool(pool.clone());
        db.migrate().await.unwrap();
        let service = WorkspaceService::new(db);
        let context = CommandContext::local("test.variable.snapshot");
        let mut tx = pool.begin().await.unwrap();
        service
            .ensure_default_workspace_on(&mut tx, &context)
            .await
            .unwrap();
        let workspace: String = sqlx::query_scalar("SELECT id FROM workspaces LIMIT 1")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        let variable = unfour_core::models::WorkspaceVariableInput {
            id: None,
            key: "REGION".into(),
            value: "0".into(),
            is_secret: true,
            is_enabled: true,
            description: None,
            sort_order: 0,
        };
        service
            .create_variable_on(&mut tx, &context, workspace.clone(), variable.clone())
            .await
            .unwrap();
        let environment = service
            .create_environment_on(&mut tx, &context, workspace.clone(), "Test".into())
            .await
            .unwrap()
            .value;
        service
            .create_environment_variable_on(
                &mut tx,
                &context,
                workspace.clone(),
                environment.id.clone(),
                variable,
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let writes = async {
            for n in 1..=32 {
                let mut tx = pool.begin().await.unwrap();
                sqlx::query("UPDATE workspace_variables SET value = ?, revision = revision + 1 WHERE workspace_id = ?")
                    .bind(n.to_string()).bind(&workspace).execute(&mut *tx).await.unwrap();
                tokio::task::yield_now().await;
                sqlx::query("UPDATE workspace_environment_variables SET value = ?, revision = revision + 1 WHERE workspace_id = ?")
                    .bind(n.to_string()).bind(&workspace).execute(&mut *tx).await.unwrap();
                tx.commit().await.unwrap();
            }
        };
        let reads = async {
            for _ in 0..32 {
                let snapshot = service
                    .variable_snapshot(&workspace, Some(&environment.id))
                    .await
                    .unwrap();
                // Force time for a writer after the read transaction has ended: resolvers must
                // keep using the captured values, even when the database has advanced.
                tokio::task::yield_now().await;
                let workspace_var = &snapshot.variables[0];
                let environment_var = &snapshot.environment.as_ref().unwrap().variables[0];
                assert_eq!(workspace_var.value, environment_var.value);
                assert_eq!(workspace_var.revision, environment_var.revision);
                assert_eq!(
                    workspace_var.revision.to_string(),
                    (workspace_var.value.parse::<i64>().unwrap() + 1).to_string()
                );
                for resolver in [snapshot.resolver(false), snapshot.resolver(true)] {
                    assert_eq!(resolver.resolve("{{REGION}}").unwrap(), workspace_var.value);
                    assert_eq!(resolver.secret_values(), &[workspace_var.value.clone()]);
                }
            }
        };
        tokio::join!(writes, reads);
        pool.close().await;
        std::fs::remove_file(path).unwrap();
    }
}
