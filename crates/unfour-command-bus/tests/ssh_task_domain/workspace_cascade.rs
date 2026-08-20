use super::*;
use unfour_core::domain::MutationOperation;

#[tokio::test]
async fn deleting_workspace_tombstones_ssh_tasks_and_steps_children_first() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let _keep = bus.list_workspaces().await.unwrap().active_workspace_id;
    let target = bus
        .create_workspace("SSH Scratch".to_string())
        .await
        .unwrap()
        .id;
    let detail = bus
        .save_ssh_task(task_input(
            &target,
            "Deploy",
            &["echo one", "echo two", "echo three"],
        ))
        .await
        .unwrap();
    clear_effects(&db).await;

    bus.delete_workspace(target.clone())
        .await
        .expect("delete workspace with SSH tasks");

    let delete_effects = effects(&db, "workspace.delete").await;
    let entity_types: Vec<&str> = delete_effects
        .iter()
        .map(|effect| effect.entity_type.as_str())
        .collect();
    assert_eq!(
        entity_types,
        [
            "SshTaskStep",
            "SshTaskStep",
            "SshTaskStep",
            "SshTask",
            "Workspace"
        ],
        "workspace delete mutations must be Step then Task then Workspace"
    );
    assert!(delete_effects
        .iter()
        .all(|effect| effect.operation == "Delete"));
    for effect in &delete_effects[..3] {
        assert_eq!(
            effect.parent_entity_id.as_deref(),
            Some(detail.task.id.as_str())
        );
    }
    assert!(delete_effects[3].parent_entity_id.is_none());
    assert_eq!(delete_effects[4].entity_id, target);

    let timestamps = tombstone_timestamps(&db, &target, &detail).await;
    assert!(timestamps.iter().all(Option::is_some));
    assert!(
        timestamps.windows(2).all(|pair| pair[0] == pair[1]),
        "workspace delete must reuse one deleted_at across SSH steps, task, and workspace"
    );
    assert_eq!(live_ssh_counts(&db, &target).await, (0, 0));
}

#[tokio::test]
async fn external_workspace_delete_tombstones_ssh_tasks_without_local_echo() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let _keep = bus.list_workspaces().await.unwrap().active_workspace_id;
    let target = bus
        .create_workspace("SSH Remote".to_string())
        .await
        .unwrap()
        .id;
    let detail = bus
        .save_ssh_task(task_input(
            &target,
            "Remote deploy",
            &["echo one", "echo two"],
        ))
        .await
        .unwrap();
    clear_effects(&db).await;

    let deleted_at = "2026-08-20T00:00:00Z".to_string();
    let report = bus
        .apply_external_workspaces(vec![ExternalWorkspaceApply::Delete(ExternalDelete {
            entity: DomainEntityKey::new(DomainEntityType::Workspace, &target, &target),
            deleted_at: deleted_at.clone(),
        })])
        .await
        .unwrap();

    let entity_types: Vec<_> = report
        .mutations
        .iter()
        .map(|mutation| mutation.entity.entity_type)
        .collect();
    assert_eq!(
        entity_types,
        [
            DomainEntityType::SshTaskStep,
            DomainEntityType::SshTaskStep,
            DomainEntityType::SshTask,
            DomainEntityType::Workspace,
        ]
    );
    assert!(report
        .mutations
        .iter()
        .all(|mutation| mutation.origin == MutationOrigin::External
            && mutation.operation == MutationOperation::Delete));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ssh_task_hook_effects")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "remote workspace delete must not produce a local sync echo"
    );

    let timestamps = tombstone_timestamps(&db, &target, &detail).await;
    assert!(timestamps
        .iter()
        .all(|value| value.as_deref() == Some(deleted_at.as_str())));
    assert_eq!(live_ssh_counts(&db, &target).await, (0, 0));

    let repeated = bus
        .apply_external_workspaces(vec![ExternalWorkspaceApply::Delete(ExternalDelete {
            entity: DomainEntityKey::new(DomainEntityType::Workspace, &target, &target),
            deleted_at,
        })])
        .await
        .unwrap();
    assert_eq!(repeated.applied_count, 0);
    assert_eq!(live_ssh_counts(&db, &target).await, (0, 0));
}

#[tokio::test]
async fn external_workspace_delete_heals_leftover_live_ssh_children() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let _keep = bus.list_workspaces().await.unwrap().active_workspace_id;
    let target = bus
        .create_workspace("SSH Leftover".to_string())
        .await
        .unwrap()
        .id;
    let detail = bus
        .save_ssh_task(task_input(&target, "Orphaned", &["echo leftover"]))
        .await
        .unwrap();
    sqlx::query("UPDATE workspaces SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2")
        .bind("2026-08-19T00:00:00Z")
        .bind(&target)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(live_ssh_counts(&db, &target).await, (1, 1));

    let deleted_at = "2026-08-20T12:00:00Z".to_string();
    let report = bus
        .apply_external_workspaces(vec![ExternalWorkspaceApply::Delete(ExternalDelete {
            entity: DomainEntityKey::new(DomainEntityType::Workspace, &target, &target),
            deleted_at: deleted_at.clone(),
        })])
        .await
        .unwrap();
    assert_eq!(report.applied_count, 2, "leftover Step then Task");
    assert_eq!(
        report.mutations[0].entity.entity_type,
        DomainEntityType::SshTaskStep
    );
    assert_eq!(
        report.mutations[1].entity.entity_type,
        DomainEntityType::SshTask
    );
    assert_eq!(
        report.mutations[0].entity.parent_entity_id.as_deref(),
        Some(detail.task.id.as_str())
    );

    let timestamps = tombstone_timestamps(&db, &target, &detail).await;
    assert_eq!(timestamps[0].as_deref(), Some(deleted_at.as_str()));
    assert_eq!(timestamps[1].as_deref(), Some(deleted_at.as_str()));
    assert_eq!(timestamps[2].as_deref(), Some("2026-08-19T00:00:00Z"));
    assert_eq!(live_ssh_counts(&db, &target).await, (0, 0));
}

#[tokio::test]
async fn deleting_workspace_removes_device_local_ssh_task_state() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let _keep = bus.list_workspaces().await.unwrap().active_workspace_id;
    let target = bus
        .create_workspace("SSH Local State".to_string())
        .await
        .unwrap()
        .id;
    let detail = bus
        .save_ssh_task(task_input(&target, "Bound", &["echo local"]))
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO ssh_task_local_binding (
          task_id, workspace_id, default_connection_id, last_used_connection_id,
          created_at, updated_at
        ) VALUES (?1, ?2, NULL, NULL, 'local-created', 'local-updated')
        "#,
    )
    .bind(&detail.task.id)
    .bind(&target)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO ssh_task_run (
          id, workspace_id, task_id, connection_id, status, started_at,
          finished_at, error_message, log_path
        ) VALUES ('run-workspace-delete', ?1, ?2, NULL, 'success', 'started', 'finished', NULL, 'device-only.log')
        "#,
    )
    .bind(&target)
    .bind(&detail.task.id)
    .execute(db.pool())
    .await
    .unwrap();

    bus.delete_workspace(target.clone())
        .await
        .expect("delete workspace with SSH local state");

    let bindings: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ssh_task_local_binding WHERE workspace_id = ?1 AND task_id = ?2",
    )
    .bind(&target)
    .bind(&detail.task.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let runs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2",
    )
    .bind(&target)
    .bind(&detail.task.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(bindings, 0);
    assert_eq!(runs, 0);
    assert_eq!(live_ssh_counts(&db, &target).await, (0, 0));
}

#[tokio::test]
async fn workspace_cascade_tombstones_orphan_live_ssh_steps() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    let _keep = bus.list_workspaces().await.unwrap().active_workspace_id;
    let target = bus
        .create_workspace("SSH Orphan Step".to_string())
        .await
        .unwrap()
        .id;
    let detail = bus
        .save_ssh_task(task_input(&target, "Parent gone", &["echo leftover"]))
        .await
        .unwrap();
    sqlx::query("UPDATE ssh_task SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2")
        .bind("2026-08-19T00:00:00Z")
        .bind(&detail.task.id)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(live_ssh_counts(&db, &target).await, (0, 1));
    clear_effects(&db).await;

    bus.delete_workspace(target.clone())
        .await
        .expect("delete workspace with orphan SSH steps");

    let delete_effects = effects(&db, "workspace.delete").await;
    let entity_types: Vec<&str> = delete_effects
        .iter()
        .map(|effect| effect.entity_type.as_str())
        .collect();
    assert_eq!(entity_types, ["SshTaskStep", "Workspace"]);
    assert_eq!(
        delete_effects[0].parent_entity_id.as_deref(),
        Some(detail.task.id.as_str())
    );
    assert_eq!(live_ssh_counts(&db, &target).await, (0, 0));
}

async fn live_ssh_counts(db: &LocalDb, workspace_id: &str) -> (i64, i64) {
    sqlx::query_as(
        r#"
        SELECT
          (SELECT COUNT(*) FROM ssh_task
           WHERE workspace_id = ?1 AND deleted_at IS NULL),
          (SELECT COUNT(*) FROM ssh_task_step
           WHERE workspace_id = ?1 AND deleted_at IS NULL)
        "#,
    )
    .bind(workspace_id)
    .fetch_one(db.pool())
    .await
    .expect("count live SSH task entities")
}

async fn tombstone_timestamps(
    db: &LocalDb,
    workspace_id: &str,
    detail: &SshTaskDetail,
) -> Vec<Option<String>> {
    let mut timestamps = Vec::new();
    for step in &detail.steps {
        timestamps.push(
            sqlx::query_scalar("SELECT deleted_at FROM ssh_task_step WHERE id = ?1")
                .bind(&step.id)
                .fetch_one(db.pool())
                .await
                .unwrap(),
        );
    }
    timestamps.push(
        sqlx::query_scalar("SELECT deleted_at FROM ssh_task WHERE id = ?1")
            .bind(&detail.task.id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
    );
    timestamps.push(
        sqlx::query_scalar("SELECT deleted_at FROM workspaces WHERE id = ?1")
            .bind(workspace_id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
    );
    timestamps
}
