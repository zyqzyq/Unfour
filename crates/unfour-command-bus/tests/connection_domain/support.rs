use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqliteConnection;
use unfour_command_bus::{CommandBus, CommandBusExtensions, TransactionalCommandHook};
use unfour_core::domain::{
    CommandContext, ConnectionSnapshotConfig, DomainEntityType, DomainMutation,
    ExternalConnectionApply, ExternalConnectionUpsert, MutationOrigin,
};
use unfour_core::models::{DatabaseConnectionInput, SshConnectionInput};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;

#[derive(Clone)]
struct RecordingHook {
    effects: Arc<Mutex<Vec<(String, Vec<DomainMutation>)>>>,
    fail_on: Option<&'static str>,
    local_only: bool,
}

impl TransactionalCommandHook for RecordingHook {
    fn on_mutations<'a>(
        &'a self,
        _connection: &'a mut SqliteConnection,
        context: &'a CommandContext,
        mutations: &'a [DomainMutation],
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            if self.local_only && context.origin != MutationOrigin::Local {
                return Ok(());
            }
            self.effects
                .lock()
                .expect("recording hook lock")
                .push((context.command_name.clone(), mutations.to_vec()));
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

#[derive(Clone)]
pub(super) struct CaptureCredentialAndRejectHook {
    pub(super) credential_ref: Arc<Mutex<Option<String>>>,
}

impl TransactionalCommandHook for CaptureCredentialAndRejectHook {
    fn on_mutations<'a>(
        &'a self,
        connection: &'a mut SqliteConnection,
        context: &'a CommandContext,
        mutations: &'a [DomainMutation],
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            if context.command_name != "ssh.connection.save" {
                return Ok(());
            }
            let connection_id = mutations
                .iter()
                .find(|mutation| mutation.entity.entity_type == DomainEntityType::Connection)
                .map(|mutation| mutation.entity.entity_id.clone())
                .ok_or_else(|| AppError::Config("missing connection mutation".to_string()))?;
            let credential_ref: Option<String> =
                sqlx::query_scalar("SELECT credential_ref FROM connections WHERE id = ?1")
                    .bind(connection_id)
                    .fetch_one(&mut *connection)
                    .await?;
            *self.credential_ref.lock().expect("credential capture lock") = credential_ref;
            Err(AppError::Config(
                "hook rejected ssh.connection.save".to_string(),
            ))
        })
    }
}

pub(super) async fn database() -> LocalDb {
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

pub(super) async fn bus_with_hook(
    fail_on: Option<&'static str>,
    local_only: bool,
) -> (
    CommandBus,
    LocalDb,
    Arc<Mutex<Vec<(String, Vec<DomainMutation>)>>>,
) {
    let db = database().await;
    CommandBus::from_db(db.clone())
        .await
        .expect("seed default workspace");
    let effects = Arc::new(Mutex::new(Vec::new()));
    let hook = RecordingHook {
        effects: effects.clone(),
        fail_on,
        local_only,
    };
    let bus = CommandBus::from_db_with_extensions(
        db.clone(),
        CommandBusExtensions::new(vec![Arc::new(hook)]),
    )
    .await
    .expect("construct bus with hook");
    (bus, db, effects)
}

pub(super) fn ssh_input(
    workspace_id: &str,
    id: Option<String>,
    name: &str,
    auth_kind: &str,
    key_path: Option<&str>,
    credential_ref: Option<&str>,
) -> SshConnectionInput {
    SshConnectionInput {
        id,
        workspace_id: workspace_id.to_string(),
        name: name.to_string(),
        host: "ssh.example.test".to_string(),
        port: Some(22),
        username: "alice".to_string(),
        auth_kind: auth_kind.to_string(),
        key_path: key_path.map(str::to_string),
        credential_ref: credential_ref.map(str::to_string),
        secret: None,
    }
}

pub(super) fn database_input(
    workspace_id: &str,
    id: Option<String>,
    name: &str,
    driver: &str,
    credential_ref: Option<&str>,
) -> DatabaseConnectionInput {
    let sqlite = driver == "sqlite";
    DatabaseConnectionInput {
        id,
        workspace_id: workspace_id.to_string(),
        name: name.to_string(),
        driver: driver.to_string(),
        host: (!sqlite).then(|| "db.example.test".to_string()),
        port: (!sqlite).then_some(if driver == "mysql" { 3306 } else { 5432 }),
        database: (!sqlite).then(|| "app".to_string()),
        username: (!sqlite).then(|| "app_user".to_string()),
        ssl_mode: (!sqlite).then(|| "require".to_string()),
        sqlite_path: sqlite.then(|| "C:\\data\\device-only.sqlite".to_string()),
        credential_ref: credential_ref.map(str::to_string),
        read_only: false,
    }
}

pub(super) fn external_ssh(
    id: &str,
    workspace_id: &str,
    name: &str,
    auth_method: &str,
    created_at: &str,
    updated_at: &str,
) -> ExternalConnectionApply {
    ExternalConnectionApply::Upsert(ExternalConnectionUpsert {
        id: id.to_string(),
        workspace_id: workspace_id.to_string(),
        connection_type: "ssh".to_string(),
        name: name.to_string(),
        host: Some("remote-ssh.example.test".to_string()),
        port: Some(2222),
        config: ConnectionSnapshotConfig::Ssh {
            username: "remote-user".to_string(),
            auth_method: auth_method.to_string(),
        },
        created_at: created_at.to_string(),
        updated_at: updated_at.to_string(),
    })
}

pub(super) fn external_database(
    id: &str,
    workspace_id: &str,
    name: &str,
    driver: &str,
    created_at: &str,
    updated_at: &str,
) -> ExternalConnectionApply {
    external_database_with_read_only(id, workspace_id, name, driver, created_at, updated_at, true)
}

pub(super) fn external_database_with_read_only(
    id: &str,
    workspace_id: &str,
    name: &str,
    driver: &str,
    created_at: &str,
    updated_at: &str,
    read_only: bool,
) -> ExternalConnectionApply {
    let sqlite = driver == "sqlite";
    ExternalConnectionApply::Upsert(ExternalConnectionUpsert {
        id: id.to_string(),
        workspace_id: workspace_id.to_string(),
        connection_type: "database".to_string(),
        name: name.to_string(),
        host: (!sqlite).then(|| "remote-db.example.test".to_string()),
        port: (!sqlite).then_some(if driver == "mysql" { 3306 } else { 5432 }),
        config: ConnectionSnapshotConfig::Database {
            driver: driver.to_string(),
            database_name: (!sqlite).then(|| "remote_app".to_string()),
            username: (!sqlite).then(|| "remote_user".to_string()),
            ssl_mode: (!sqlite).then(|| "require".to_string()),
            read_only,
        },
        created_at: created_at.to_string(),
        updated_at: updated_at.to_string(),
    })
}

pub(super) fn mutations_for(
    effects: &Arc<Mutex<Vec<(String, Vec<DomainMutation>)>>>,
    command_name: &str,
) -> Vec<DomainMutation> {
    effects
        .lock()
        .expect("recording hook lock")
        .iter()
        .filter(|(name, _)| name == command_name)
        .flat_map(|(_, mutations)| mutations.clone())
        .collect()
}
