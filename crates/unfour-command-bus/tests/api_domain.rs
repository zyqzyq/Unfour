use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqliteConnection;
use unfour_command_bus::{CommandBus, CommandBusExtensions, TransactionalCommandHook};
use unfour_core::domain::{
    ApiCollectionSnapshot, ApiFolderSnapshot, ApiRequestSnapshot, CommandContext, DomainEntityKey,
    DomainEntityType, DomainMutation, DomainSnapshot, ExternalApiCollectionApply,
    ExternalApiCollectionUpsert, ExternalApiFolderApply, ExternalApiFolderUpsert,
    ExternalApiRequestApply, ExternalApiRequestUpsert, ExternalApplyPage, ExternalDelete,
    ExternalWorkspaceApply, ExternalWorkspaceUpsert, MutationOrigin,
};
use unfour_core::models::{ApiRequestInput, KeyValue};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;

#[path = "api_domain/activity.rs"]
mod activity;
#[path = "api_domain/hook_rollback.rs"]
mod hook_rollback;
#[path = "api_domain/mutations.rs"]
mod mutations;
#[path = "api_domain/snapshot_apply.rs"]
mod snapshot_apply;

#[derive(Clone)]
struct RecordingHook {
    local_only: bool,
    fail_on: Option<&'static str>,
}

impl TransactionalCommandHook for RecordingHook {
    fn on_mutations<'a>(
        &'a self,
        connection: &'a mut SqliteConnection,
        context: &'a CommandContext,
        mutations: &'a [DomainMutation],
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            if self.local_only && context.origin != MutationOrigin::Local {
                return Ok(());
            }
            for mutation in mutations {
                sqlx::query(
                    r#"
                    INSERT INTO api_hook_effects (
                      command_name, origin, entity_type, entity_id,
                      parent_entity_id, operation, revision
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                )
                .bind(&context.command_name)
                .bind(format!("{:?}", context.origin))
                .bind(format!("{:?}", mutation.entity.entity_type))
                .bind(&mutation.entity.entity_id)
                .bind(&mutation.entity.parent_entity_id)
                .bind(format!("{:?}", mutation.operation))
                .bind(mutation.revision)
                .execute(&mut *connection)
                .await?;
            }
            if self.fail_on == Some(context.command_name.as_str()) {
                return Err(AppError::Config(format!(
                    "hook rejected {}",
                    context.command_name
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

async fn bus_with_hook(hook: RecordingHook) -> (CommandBus, LocalDb) {
    let db = database().await;
    CommandBus::from_db(db.clone())
        .await
        .expect("seed default workspace");
    sqlx::query(
        r#"
        CREATE TABLE api_hook_effects (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          command_name TEXT NOT NULL,
          origin TEXT NOT NULL,
          entity_type TEXT NOT NULL,
          entity_id TEXT NOT NULL,
          parent_entity_id TEXT,
          operation TEXT NOT NULL,
          revision INTEGER NOT NULL
        )
        "#,
    )
    .execute(db.pool())
    .await
    .expect("create hook table");
    let bus = CommandBus::from_db_with_extensions(
        db.clone(),
        CommandBusExtensions::new(vec![Arc::new(hook)]),
    )
    .await
    .expect("build hooked bus");
    (bus, db)
}

fn request_input(
    workspace_id: &str,
    collection_id: &str,
    parent_folder_id: Option<String>,
) -> ApiRequestInput {
    ApiRequestInput {
        workspace_id: workspace_id.to_string(),
        name: Some("Create user".to_string()),
        parent_folder_id,
        collection_id: Some(collection_id.to_string()),
        auth_json: Some(
            serde_json::json!({
                "type": "bearer",
                "token": "auth-device-secret",
                "prefix": "Bearer",
            })
            .to_string(),
        ),
        method: "post".to_string(),
        url: "https://api.example.test/users?access_token=url-device-secret&page=1".to_string(),
        headers: vec![
            KeyValue {
                key: "Authorization".to_string(),
                value: "Bearer header-device-secret".to_string(),
                enabled: true,
            },
            KeyValue {
                key: "Accept".to_string(),
                value: "application/json".to_string(),
                enabled: true,
            },
        ],
        query: vec![KeyValue {
            key: "api_key".to_string(),
            value: "query-device-secret".to_string(),
            enabled: true,
        }],
        body: Some(r#"{"name":"Ada","token":"body-device-secret"}"#.to_string()),
        body_kind: "json".to_string(),
        timeout_ms: Some(9_999),
        pre_request_script: Some("pm.variables.set('trace', '1');".to_string()),
        post_response_script: Some("pm.test('ok', () => true);".to_string()),
        script_schema_version: 1,
        temporary_variables: vec![KeyValue {
            key: "runtime_only".to_string(),
            value: "not-synced".to_string(),
            enabled: true,
        }],
    }
}
