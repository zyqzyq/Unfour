use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqliteConnection;
use unfour_command_bus::{CommandBus, CommandBusExtensions, TransactionalCommandHook};
use unfour_core::domain::{
    CommandContext, DomainEntityKey, DomainEntityType, DomainMutation, DomainSnapshot,
    ExternalApplyPage, ExternalDelete, ExternalVariableValue, ExternalWorkspaceApply,
    ExternalWorkspaceEnvironmentApply, ExternalWorkspaceUpsert, ExternalWorkspaceVariableApply,
    ExternalWorkspaceVariableUpsert, MutationOrigin, SnapshotVariableValue,
};
use unfour_core::models::WorkspaceVariableInput;
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;

#[path = "workspace_domain/environments.rs"]
mod environments;
#[path = "workspace_domain/external_apply.rs"]
mod external_apply;
#[path = "workspace_domain/hooks.rs"]
mod hooks;
#[path = "workspace_domain/variables.rs"]
mod variables;
#[path = "workspace_domain/workspace_usage.rs"]
mod workspace_usage;

#[derive(Clone)]
struct SqlHook {
    name: &'static str,
    fail_on: Option<&'static str>,
    local_only: bool,
}

impl TransactionalCommandHook for SqlHook {
    fn on_mutations<'a>(
        &'a self,
        connection: &'a mut SqliteConnection,
        context: &'a CommandContext,
        mutations: &'a [DomainMutation],
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            if mutations.is_empty() || (self.local_only && context.origin != MutationOrigin::Local)
            {
                return Ok(());
            }
            for mutation in mutations {
                sqlx::query(
                    r#"
                    INSERT INTO hook_effects (
                      hook_name, command_name, origin, entity_type,
                      entity_id, operation, revision
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                )
                .bind(self.name)
                .bind(&context.command_name)
                .bind(format!("{:?}", context.origin))
                .bind(format!("{:?}", mutation.entity.entity_type))
                .bind(&mutation.entity.entity_id)
                .bind(format!("{:?}", mutation.operation))
                .bind(mutation.revision)
                .execute(&mut *connection)
                .await?;
            }
            if self.fail_on == Some(context.command_name.as_str()) {
                return Err(AppError::Config(format!(
                    "{} rejected {}",
                    self.name, context.command_name
                )));
            }
            Ok(())
        })
    }
}

async fn database() -> LocalDb {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect sqlite");
    let db = LocalDb::from_pool(pool);
    db.migrate().await.expect("migrate sqlite");
    db
}

async fn bus_with_hooks(hooks: Vec<Arc<dyn TransactionalCommandHook>>) -> (CommandBus, LocalDb) {
    let db = database().await;
    CommandBus::from_db(db.clone())
        .await
        .expect("seed default workspace");
    sqlx::query(
        r#"
        CREATE TABLE hook_effects (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          hook_name TEXT NOT NULL,
          command_name TEXT NOT NULL,
          origin TEXT NOT NULL,
          entity_type TEXT NOT NULL,
          entity_id TEXT NOT NULL,
          operation TEXT NOT NULL,
          revision INTEGER NOT NULL
        )
        "#,
    )
    .execute(db.pool())
    .await
    .expect("create hook table");
    let bus = CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(hooks))
        .await
        .expect("build hooked bus");
    (bus, db)
}

fn input(id: Option<String>, key: &str, value: &str, secret: bool) -> WorkspaceVariableInput {
    WorkspaceVariableInput {
        id,
        key: key.to_string(),
        value: value.to_string(),
        is_secret: secret,
        is_enabled: true,
        description: None,
        sort_order: 0,
    }
}
