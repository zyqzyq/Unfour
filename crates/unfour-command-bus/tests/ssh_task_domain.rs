use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqliteConnection;
use unfour_command_bus::{CommandBus, CommandBusExtensions, TransactionalCommandHook};
use unfour_core::domain::{
    CommandContext, DomainEntityKey, DomainEntityType, DomainMutation, DomainSnapshot,
    ExternalApplyPage, ExternalDelete, ExternalSshTaskApply, ExternalSshTaskStepApply,
    ExternalSshTaskStepUpsert, ExternalSshTaskUpsert, ExternalWorkspaceApply,
    ExternalWorkspaceUpsert, MutationOrigin, SshTaskSnapshot, SshTaskStepSnapshot,
};
use unfour_core::models::{
    SshTaskDetail, SshTaskSaveInput, SshTaskStepInput, SshTasksReorderInput,
};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;

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
                    INSERT INTO ssh_task_hook_effects (
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

#[derive(Debug, sqlx::FromRow)]
struct HookEffect {
    entity_type: String,
    entity_id: String,
    parent_entity_id: Option<String>,
    operation: String,
    revision: i64,
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
        CREATE TABLE ssh_task_hook_effects (
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
    .unwrap();
    let bus = CommandBus::from_db_with_extensions(
        db.clone(),
        CommandBusExtensions::new(vec![Arc::new(hook)]),
    )
    .await
    .unwrap();
    (bus, db)
}

async fn effects(db: &LocalDb, command_name: &str) -> Vec<HookEffect> {
    sqlx::query_as(
        r#"
        SELECT entity_type, entity_id, parent_entity_id, operation, revision
        FROM ssh_task_hook_effects
        WHERE command_name = ?1 ORDER BY id
        "#,
    )
    .bind(command_name)
    .fetch_all(db.pool())
    .await
    .unwrap()
}

async fn clear_effects(db: &LocalDb) {
    sqlx::query("DELETE FROM ssh_task_hook_effects")
        .execute(db.pool())
        .await
        .unwrap();
}

fn command_step(id: Option<String>, position: i64, command: &str) -> SshTaskStepInput {
    SshTaskStepInput {
        id,
        name: format!("Command {}", position + 1),
        step_type: "command".to_string(),
        position,
        enabled: true,
        config_version: Some(1),
        config_json: serde_json::json!({
            "command": command,
            "workingDirectory": "",
            "timeoutSeconds": 30,
            "continueOnError": false
        }),
    }
}

fn task_input(workspace_id: &str, name: &str, commands: &[&str]) -> SshTaskSaveInput {
    SshTaskSaveInput {
        id: None,
        workspace_id: workspace_id.to_string(),
        name: name.to_string(),
        description: "syncable task".to_string(),
        default_connection_id: None,
        steps: commands
            .iter()
            .enumerate()
            .map(|(position, command)| command_step(None, position as i64, command))
            .collect(),
    }
}

fn edit_input(detail: &SshTaskDetail) -> SshTaskSaveInput {
    SshTaskSaveInput {
        id: Some(detail.task.id.clone()),
        workspace_id: detail.task.workspace_id.clone(),
        name: detail.task.name.clone(),
        description: detail.task.description.clone(),
        default_connection_id: detail
            .local_binding
            .as_ref()
            .and_then(|binding| binding.default_connection_id.clone()),
        steps: detail
            .steps
            .iter()
            .map(|step| SshTaskStepInput {
                id: Some(step.id.clone()),
                name: step.name.clone(),
                step_type: step.step_type.clone(),
                position: step.position,
                enabled: step.enabled,
                config_version: Some(step.config_version),
                config_json: step.config_json.clone(),
            })
            .collect(),
    }
}

#[tokio::test]
async fn local_task_and_step_lifecycle_emits_transactional_mutations() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let created = bus
        .save_ssh_task(task_input(
            &workspace_id,
            "Deploy",
            &["echo one", "echo two"],
        ))
        .await
        .unwrap();
    let created_effects = effects(&db, "ssh.task.save").await;
    assert_eq!(created_effects.len(), 3);
    assert_eq!(created_effects[0].entity_type, "SshTask");
    assert_eq!(created_effects[0].entity_id, created.task.id);
    assert_eq!(created_effects[0].operation, "Upsert");
    assert!(created_effects[0].parent_entity_id.is_none());
    for effect in &created_effects[1..] {
        assert_eq!(effect.entity_type, "SshTaskStep");
        assert_eq!(effect.operation, "Upsert");
        assert_eq!(
            effect.parent_entity_id.as_deref(),
            Some(created.task.id.as_str())
        );
        assert_eq!(effect.revision, 1);
    }
    clear_effects(&db).await;
    let mut update = edit_input(&created);
    update.name = "Deploy safely".to_string();
    update.steps[0].config_json["command"] = serde_json::json!("echo updated");
    let updated = bus.save_ssh_task(update).await.unwrap();
    let update_effects = effects(&db, "ssh.task.save").await;
    assert_eq!(update_effects.len(), 2);
    assert_eq!(update_effects[0].entity_type, "SshTask");
    assert_eq!(update_effects[0].operation, "Upsert");
    assert_eq!(update_effects[1].entity_type, "SshTaskStep");
    assert_eq!(update_effects[1].operation, "Upsert");
    clear_effects(&db).await;
    let mut reorder = edit_input(&updated);
    reorder.steps[0].position = 1;
    reorder.steps[1].position = 0;
    let reordered = bus.save_ssh_task(reorder).await.unwrap();
    let reorder_effects = effects(&db, "ssh.task.save").await;
    assert_eq!(reorder_effects.len(), 2);
    assert!(reorder_effects
        .iter()
        .all(|effect| effect.entity_type == "SshTaskStep" && effect.operation == "Upsert"));
    clear_effects(&db).await;
    let removed_step_id = reordered.steps[1].id.clone();
    let mut remove_step = edit_input(&reordered);
    remove_step.steps.pop();
    let remaining = bus.save_ssh_task(remove_step).await.unwrap();
    let remove_effects = effects(&db, "ssh.task.save").await;
    assert_eq!(remove_effects.len(), 1);
    assert_eq!(remove_effects[0].entity_id, removed_step_id);
    assert_eq!(remove_effects[0].operation, "Delete");
    clear_effects(&db).await;
    bus.delete_ssh_task(workspace_id.clone(), remaining.task.id.clone())
        .await
        .unwrap();
    let delete_effects = effects(&db, "ssh.task.delete").await;
    assert_eq!(delete_effects.len(), 2);
    assert_eq!(delete_effects[0].entity_type, "SshTaskStep");
    assert_eq!(delete_effects[0].operation, "Delete");
    assert_eq!(delete_effects[1].entity_type, "SshTask");
    assert_eq!(delete_effects[1].operation, "Delete");
    assert!(matches!(
        bus.read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::SshTaskStep,
            &workspace_id,
            &remaining.steps[0].id,
        ))
        .await
        .unwrap(),
        DomainSnapshot::Tombstone(_)
    ));
}

#[tokio::test]
async fn task_reorder_emits_upserts_for_every_changed_task() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let first = bus
        .save_ssh_task(task_input(&workspace_id, "First", &[]))
        .await
        .unwrap();
    let second = bus
        .save_ssh_task(task_input(&workspace_id, "Second", &[]))
        .await
        .unwrap();
    clear_effects(&db).await;
    let reordered = bus
        .reorder_ssh_tasks(SshTasksReorderInput {
            workspace_id: workspace_id.clone(),
            task_ids: vec![second.task.id.clone(), first.task.id.clone()],
        })
        .await
        .unwrap();
    assert_eq!(reordered[0].id, second.task.id);
    assert_eq!(reordered[1].id, first.task.id);
    let reorder_effects = effects(&db, "ssh.task.reorder").await;
    assert_eq!(reorder_effects.len(), 2);
    assert!(reorder_effects
        .iter()
        .all(|effect| effect.entity_type == "SshTask" && effect.operation == "Upsert"));
}

#[tokio::test]
async fn snapshots_and_enumeration_are_canonical_and_device_local_safe() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = task_input(&workspace_id, "Download", &[]);
    input.steps = vec![SshTaskStepInput {
        id: None,
        name: "Download artifact".to_string(),
        step_type: "download".to_string(),
        position: 0,
        enabled: true,
        config_version: Some(1),
        config_json: serde_json::json!({
            "remotePath": "/tmp/artifact.tar",
            "localPath": r"C:\Users\alice\artifact.tar",
            "overwrite": true
        }),
    }];
    let detail = bus.save_ssh_task(input).await.unwrap();
    sqlx::query(
        r#"
        INSERT INTO ssh_task_local_binding (
          task_id, workspace_id, default_connection_id, last_used_connection_id,
          created_at, updated_at
        ) VALUES (?1, ?2, NULL, NULL, 'local-created', 'local-updated')
        "#,
    )
    .bind(&detail.task.id)
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO ssh_task_run (
          id, workspace_id, task_id, connection_id, status, started_at,
          finished_at, error_message, log_path
        ) VALUES ('run-local', ?1, ?2, NULL, 'success', 'started', 'finished', NULL, 'device-only.log')
        "#,
    )
    .bind(&workspace_id)
    .bind(&detail.task.id)
    .execute(db.pool())
    .await
    .unwrap();
    let keys = bus
        .list_ssh_task_domain_entities(workspace_id.clone())
        .await
        .unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].entity_type, DomainEntityType::SshTask);
    assert_eq!(keys[1].entity_type, DomainEntityType::SshTaskStep);
    assert_eq!(
        keys[1].parent_entity_id.as_deref(),
        Some(detail.task.id.as_str())
    );
    let DomainSnapshot::SshTask(task_snapshot) = bus.read_domain_snapshot(&keys[0]).await.unwrap()
    else {
        panic!("expected SSH task snapshot");
    };
    assert_eq!(task_snapshot.name, "Download");
    assert_eq!(task_snapshot.description, "syncable task");
    assert_eq!(task_snapshot.sort_order, 0);
    assert!(task_snapshot.revision > 0);
    let DomainSnapshot::SshTaskStep(step_snapshot) =
        bus.read_domain_snapshot(&keys[1]).await.unwrap()
    else {
        panic!("expected SSH task step snapshot");
    };
    assert_eq!(step_snapshot.task_id, detail.task.id);
    assert_eq!(step_snapshot.config_version, 1);
    let local_path_placeholder = step_snapshot.config_json["localPath"].as_str().unwrap();
    assert!(local_path_placeholder.starts_with("{{local_path_"));
    assert!(local_path_placeholder.ends_with("}}"));
    let serialized = serde_json::to_string(&(task_snapshot, step_snapshot)).unwrap();
    for excluded in [
        r"C:\Users\alice\artifact.tar",
        "defaultConnectionId",
        "lastUsedConnectionId",
        "run-local",
        "device-only.log",
        "runtimeInputValue",
        "transferProgress",
        "executionResult",
    ] {
        assert!(!serialized.contains(excluded), "snapshot leaked {excluded}");
    }
    let stored_config: String =
        sqlx::query_scalar("SELECT config_json FROM ssh_task_step WHERE id = ?1")
            .bind(&detail.steps[0].id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(stored_config.contains(r"C:\\Users\\alice\\artifact.tar"));

    let mut unsafe_input = task_input(&workspace_id, "Unsafe", &["echo ok"]);
    unsafe_input.steps[0].config_json["credentialRef"] = serde_json::json!("unfour:secret");
    let error = bus.save_ssh_task(unsafe_input).await.unwrap_err();
    assert!(error.to_string().contains("unsupported"));
}

#[tokio::test]
async fn serialized_snapshots_omit_all_literal_transfer_local_paths() {
    let (bus, _) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let local_paths = [
        r"C:\Users\alice\artifact.tar",
        "/Users/alice/artifact.tar",
        "relative/artifact.tar",
        "./output/artifact.tar",
        "../output/artifact.tar",
    ];
    let mut input = task_input(&workspace_id, "Literal paths", &[]);
    input.steps = local_paths
        .iter()
        .enumerate()
        .map(|(position, local_path)| SshTaskStepInput {
            id: None,
            name: format!("Transfer {position}"),
            step_type: if position % 2 == 0 {
                "upload".to_string()
            } else {
                "download".to_string()
            },
            position: position as i64,
            enabled: true,
            config_version: Some(1),
            config_json: serde_json::json!({
                "remotePath": "/tmp/artifact.tar",
                "localPath": local_path,
                "overwrite": true
            }),
        })
        .collect();
    let detail = bus.save_ssh_task(input).await.unwrap();
    let snapshots = futures_for_steps(&bus, &workspace_id, &detail).await;

    assert!(snapshots.iter().all(|snapshot| {
        snapshot.config_json["localPath"]
            .as_str()
            .is_some_and(|value| value.starts_with("{{local_path_") && value.ends_with("}}"))
    }));
    let serialized = serde_json::to_string(&snapshots).unwrap();
    for local_path in local_paths {
        let encoded = serde_json::to_string(local_path).unwrap();
        let encoded = encoded.trim_matches('"');
        assert!(
            !serialized.contains(encoded),
            "snapshot leaked literal localPath {local_path}"
        );
    }
}

#[tokio::test]
async fn external_apply_preserves_the_current_device_transfer_path() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let device_path = r"C:\Users\alice\artifact.tar";
    let mut input = task_input(&workspace_id, "Download", &[]);
    input.steps = vec![SshTaskStepInput {
        id: None,
        name: "Download artifact".to_string(),
        step_type: "download".to_string(),
        position: 0,
        enabled: true,
        config_version: Some(1),
        config_json: serde_json::json!({
            "remotePath": "/tmp/artifact.tar",
            "localPath": device_path,
            "overwrite": true
        }),
    }];
    let detail = bus.save_ssh_task(input).await.unwrap();
    let mut snapshot = futures_for_steps(&bus, &workspace_id, &detail)
        .await
        .remove(0);
    assert_ne!(snapshot.config_json["localPath"], device_path);
    snapshot.name = "Download renamed elsewhere".to_string();
    snapshot.updated_at = "2026-08-17T03:00:00Z".to_string();
    let page = ExternalApplyPage {
        ssh_task_steps: vec![step_apply(&snapshot)],
        ..ExternalApplyPage::default()
    };
    assert_eq!(
        bus.apply_external_page(page.clone())
            .await
            .unwrap()
            .applied_count,
        1
    );
    let stored: String = sqlx::query_scalar("SELECT config_json FROM ssh_task_step WHERE id = ?1")
        .bind(&detail.steps[0].id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["localPath"], device_path);
    assert_eq!(
        bus.apply_external_page(page).await.unwrap().applied_count,
        0
    );
}

#[tokio::test]
async fn external_step_reorder_can_arrive_across_pages() {
    let (bus, _) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let detail = bus
        .save_ssh_task(task_input(
            &workspace_id,
            "Paged reorder",
            &["echo one", "echo two"],
        ))
        .await
        .unwrap();
    let mut snapshots = futures_for_steps(&bus, &workspace_id, &detail).await;
    snapshots[0].position = 1;
    snapshots[0].updated_at = "2026-08-17T04:00:00Z".to_string();
    snapshots[1].position = 0;
    snapshots[1].updated_at = "2026-08-17T04:00:00Z".to_string();

    for snapshot in &snapshots {
        let report = bus
            .apply_external_page(ExternalApplyPage {
                ssh_task_steps: vec![step_apply(snapshot)],
                ..ExternalApplyPage::default()
            })
            .await
            .unwrap();
        assert_eq!(report.applied_count, 1);
    }
    let reordered = bus
        .get_ssh_task(workspace_id, detail.task.id)
        .await
        .unwrap();
    assert_eq!(reordered.steps[0].id, detail.steps[1].id);
    assert_eq!(reordered.steps[1].id, detail.steps[0].id);
}

#[tokio::test]
async fn legacy_unknown_step_fields_remain_readable_and_deletable() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let detail = bus
        .save_ssh_task(task_input(&workspace_id, "Legacy", &["echo legacy"]))
        .await
        .unwrap();
    let legacy_config = serde_json::json!({
        "command": "echo legacy",
        "workingDirectory": "",
        "timeoutSeconds": 30,
        "continueOnError": false,
        "legacyExtension": "old-value"
    });
    sqlx::query("UPDATE ssh_task_step SET config_json = ?1 WHERE id = ?2")
        .bind(serde_json::to_string(&legacy_config).unwrap())
        .bind(&detail.steps[0].id)
        .execute(db.pool())
        .await
        .unwrap();
    let readable = bus
        .get_ssh_task(workspace_id.clone(), detail.task.id.clone())
        .await
        .unwrap();
    assert!(readable.steps[0]
        .config_json
        .get("legacyExtension")
        .is_none());
    bus.delete_ssh_task(workspace_id, detail.task.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn external_apply_is_idempotent_cascades_and_preserves_local_runtime_state() {
    let source_db = database().await;
    let source = CommandBus::from_db(source_db).await.unwrap();
    let workspace_id = source.list_workspaces().await.unwrap().active_workspace_id;
    source
        .rename_workspace(workspace_id.clone(), "SSH Source Workspace".to_string())
        .await
        .unwrap();
    let workspace = source
        .list_workspaces()
        .await
        .unwrap()
        .workspaces
        .into_iter()
        .find(|workspace| workspace.id == workspace_id)
        .unwrap();
    let detail = source
        .save_ssh_task(task_input(
            &workspace_id,
            "Remote deploy",
            &["echo one", "echo two"],
        ))
        .await
        .unwrap();
    let task_snapshot = task_snapshot(&source, &workspace_id, &detail.task.id).await;
    let step_snapshots = futures_for_steps(&source, &workspace_id, &detail).await;

    let (target, target_db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let page = ExternalApplyPage {
        workspaces: vec![ExternalWorkspaceApply::Upsert(ExternalWorkspaceUpsert {
            id: workspace.id,
            name: workspace.name,
            environment_type: workspace.environment_type,
            mcp_policy: workspace.mcp_policy,
            created_at: workspace.created_at,
            updated_at: workspace.updated_at,
        })],
        ssh_tasks: vec![task_apply(&task_snapshot)],
        ssh_task_steps: step_snapshots.iter().map(step_apply).collect(),
        ..ExternalApplyPage::default()
    };
    let first = target.apply_external_page(page.clone()).await.unwrap();
    assert_eq!(first.applied_count, 4);
    assert!(first
        .mutations
        .iter()
        .all(|mutation| mutation.origin == MutationOrigin::External));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ssh_task_hook_effects")
            .fetch_one(target_db.pool())
            .await
            .unwrap(),
        0
    );
    for table in ["ssh_task_local_binding", "ssh_task_run"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(target_db.pool())
                .await
                .unwrap(),
            0,
            "external upsert must not create {table} rows"
        );
    }
    assert_eq!(
        target
            .apply_external_page(page)
            .await
            .unwrap()
            .applied_count,
        0
    );

    sqlx::query(
        r#"
        INSERT INTO ssh_task_local_binding (
          task_id, workspace_id, default_connection_id, last_used_connection_id,
          created_at, updated_at
        ) VALUES (?1, ?2, NULL, NULL, 'local-created', 'local-updated')
        "#,
    )
    .bind(&detail.task.id)
    .bind(&workspace_id)
    .execute(target_db.pool())
    .await
    .unwrap();

    let mut updated_task = task_snapshot.clone();
    updated_task.name = "Remote deploy updated".to_string();
    updated_task.updated_at = "2026-08-17T00:30:00Z".to_string();
    let mut updated_step = step_snapshots[0].clone();
    updated_step.config_json["command"] = serde_json::json!("echo remotely updated");
    updated_step.updated_at = "2026-08-17T00:30:00Z".to_string();
    let updated = target
        .apply_external_page(ExternalApplyPage {
            ssh_tasks: vec![task_apply(&updated_task)],
            ssh_task_steps: vec![step_apply(&updated_step)],
            ..ExternalApplyPage::default()
        })
        .await
        .unwrap();
    assert_eq!(updated.applied_count, 2);
    assert_eq!(
        target
            .get_ssh_task(workspace_id.clone(), detail.task.id.clone())
            .await
            .unwrap()
            .task
            .name,
        "Remote deploy updated"
    );
    sqlx::query(
        r#"
        INSERT INTO ssh_task_run (
          id, workspace_id, task_id, connection_id, status, started_at,
          finished_at, error_message, log_path
        ) VALUES ('remote-local-run', ?1, ?2, NULL, 'success', 'started', 'finished', NULL, 'local.log')
        "#,
    )
    .bind(&workspace_id)
    .bind(&detail.task.id)
    .execute(target_db.pool())
    .await
    .unwrap();

    let step_delete = ExternalDelete {
        entity: DomainEntityKey::new(
            DomainEntityType::SshTaskStep,
            &workspace_id,
            &detail.steps[0].id,
        )
        .with_parent_entity_id(&detail.task.id),
        deleted_at: "2026-08-17T01:00:00Z".to_string(),
    };
    let deleted_step = target
        .apply_external_page(ExternalApplyPage {
            ssh_task_steps: vec![ExternalSshTaskStepApply::Delete(step_delete)],
            ..ExternalApplyPage::default()
        })
        .await
        .unwrap();
    assert_eq!(deleted_step.applied_count, 1);

    let task_delete = ExternalDelete {
        entity: DomainEntityKey::new(DomainEntityType::SshTask, &workspace_id, &detail.task.id),
        deleted_at: "2026-08-17T02:00:00Z".to_string(),
    };
    let deleted_task = target
        .apply_external_page(ExternalApplyPage {
            ssh_tasks: vec![ExternalSshTaskApply::Delete(task_delete)],
            ..ExternalApplyPage::default()
        })
        .await
        .unwrap();
    assert_eq!(deleted_task.applied_count, 2, "remaining Step then Task");
    assert_eq!(
        deleted_task.mutations[0].entity.entity_type,
        DomainEntityType::SshTaskStep
    );
    assert_eq!(
        deleted_task.mutations[1].entity.entity_type,
        DomainEntityType::SshTask
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM ssh_task_local_binding WHERE task_id = ?1",
        )
        .bind(&detail.task.id)
        .fetch_one(target_db.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ssh_task_run WHERE task_id = ?1")
            .bind(&detail.task.id)
            .fetch_one(target_db.pool())
            .await
            .unwrap(),
        1
    );
    assert!(matches!(
        target
            .read_domain_snapshot(&DomainEntityKey::new(
                DomainEntityType::SshTaskStep,
                &workspace_id,
                &detail.steps[1].id,
            ))
            .await
            .unwrap(),
        DomainSnapshot::Tombstone(_)
    ));
}

#[tokio::test]
async fn external_apply_rejects_cross_workspace_parents_and_skips_missing_parents() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let other_workspace = bus.create_workspace("Other".to_string()).await.unwrap();
    let task = bus
        .save_ssh_task(task_input(&workspace_id, "Parent", &[]))
        .await
        .unwrap();
    clear_effects(&db).await;

    let missing = target_step_record(&workspace_id, "missing-task", "missing-step");
    let report = bus
        .apply_external_page(ExternalApplyPage {
            ssh_task_steps: vec![ExternalSshTaskStepApply::Upsert(missing)],
            ..ExternalApplyPage::default()
        })
        .await
        .unwrap();
    assert_eq!(report.applied_count, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM ssh_task_step WHERE id = 'missing-step'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );

    let cross_workspace =
        target_step_record(&other_workspace.id, &task.task.id, "cross-workspace-step");
    let error = bus
        .apply_external_page(ExternalApplyPage {
            ssh_task_steps: vec![ExternalSshTaskStepApply::Upsert(cross_workspace)],
            ..ExternalApplyPage::default()
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("parent workspace mismatch"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM ssh_task_step WHERE id = 'cross-workspace-step'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn hook_failure_rolls_back_task_steps_activity_and_hook_rows() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: false,
        fail_on: Some("ssh.task.save"),
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let error = bus
        .save_ssh_task(task_input(&workspace_id, "Rollback", &["echo rollback"]))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("hook rejected"));
    for table in ["ssh_task", "ssh_task_step", "ssh_task_hook_effects"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} must roll back");
    }
    let activity_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM activity_events WHERE action = 'ssh.task.save'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(activity_count, 0);
}

async fn task_snapshot(bus: &CommandBus, workspace_id: &str, id: &str) -> SshTaskSnapshot {
    let DomainSnapshot::SshTask(snapshot) = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::SshTask,
            workspace_id,
            id,
        ))
        .await
        .unwrap()
    else {
        panic!("expected SSH task snapshot");
    };
    snapshot
}

async fn futures_for_steps(
    bus: &CommandBus,
    workspace_id: &str,
    detail: &SshTaskDetail,
) -> Vec<SshTaskStepSnapshot> {
    let mut snapshots = Vec::new();
    for step in &detail.steps {
        let DomainSnapshot::SshTaskStep(snapshot) = bus
            .read_domain_snapshot(&DomainEntityKey::new(
                DomainEntityType::SshTaskStep,
                workspace_id,
                &step.id,
            ))
            .await
            .unwrap()
        else {
            panic!("expected SSH task step snapshot");
        };
        snapshots.push(snapshot);
    }
    snapshots
}

fn task_apply(snapshot: &SshTaskSnapshot) -> ExternalSshTaskApply {
    ExternalSshTaskApply::Upsert(ExternalSshTaskUpsert {
        id: snapshot.id.clone(),
        workspace_id: snapshot.workspace_id.clone(),
        name: snapshot.name.clone(),
        description: snapshot.description.clone(),
        sort_order: snapshot.sort_order,
        created_at: snapshot.created_at.clone(),
        updated_at: snapshot.updated_at.clone(),
    })
}

fn step_apply(snapshot: &SshTaskStepSnapshot) -> ExternalSshTaskStepApply {
    ExternalSshTaskStepApply::Upsert(ExternalSshTaskStepUpsert {
        id: snapshot.id.clone(),
        workspace_id: snapshot.workspace_id.clone(),
        task_id: snapshot.task_id.clone(),
        name: snapshot.name.clone(),
        step_type: snapshot.step_type.clone(),
        position: snapshot.position,
        enabled: snapshot.enabled,
        config_version: snapshot.config_version,
        config_json: snapshot.config_json.clone(),
        created_at: snapshot.created_at.clone(),
        updated_at: snapshot.updated_at.clone(),
    })
}

fn target_step_record(workspace_id: &str, task_id: &str, id: &str) -> ExternalSshTaskStepUpsert {
    ExternalSshTaskStepUpsert {
        id: id.to_string(),
        workspace_id: workspace_id.to_string(),
        task_id: task_id.to_string(),
        name: "Remote step".to_string(),
        step_type: "command".to_string(),
        position: 0,
        enabled: true,
        config_version: 1,
        config_json: serde_json::json!({
            "command": "echo remote",
            "workingDirectory": "",
            "timeoutSeconds": 30,
            "continueOnError": false
        }),
        created_at: "2026-08-17T00:00:00Z".to_string(),
        updated_at: "2026-08-17T00:00:00Z".to_string(),
    }
}

#[test]
fn entity_contract_serializes_to_protocol_names() {
    assert_eq!(
        serde_json::to_value(DomainEntityType::SshTask).unwrap(),
        "sshTask"
    );
    assert_eq!(
        serde_json::to_value(DomainEntityType::SshTaskStep).unwrap(),
        "sshTaskStep"
    );
}
