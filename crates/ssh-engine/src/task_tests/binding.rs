use super::super::*;
use super::support::*;

#[tokio::test]
async fn task_delete_soft_deletes_templates_and_removes_local_state() {
    let (service, workspace_id) = service().await;
    let connection = connection(&service, &workspace_id).await;
    let mut input = docker_export_input(workspace_id.clone());
    input.default_connection_id = Some(connection.id.clone());
    let saved = service.save_task(input).await.unwrap();
    assert_eq!(
        saved
            .local_binding
            .as_ref()
            .and_then(|binding| binding.default_connection_id.as_deref()),
        Some(connection.id.as_str())
    );

    let removed_step_id = saved.steps[0].id.clone();
    let mut update = edit_input(&saved);
    update.steps.remove(0);
    let updated = service.save_task(update).await.unwrap();
    assert_eq!(updated.steps.len(), 4);
    let deleted_at: Option<String> = sqlx::query_scalar(
        "SELECT deleted_at FROM ssh_task_step WHERE workspace_id = ?1 AND id = ?2",
    )
    .bind(&workspace_id)
    .bind(&removed_step_id)
    .fetch_one(service.db.pool())
    .await
    .unwrap();
    assert!(deleted_at.is_some());

    let run_id = unfour_core::id::new_id();
    std::fs::create_dir_all(&*service.task_log_dir).unwrap();
    let log_path = service.task_log_dir.join(format!("{run_id}.log"));
    std::fs::write(&log_path, "local task output").unwrap();
    sqlx::query(
        r#"
            INSERT INTO ssh_task_run (
              id, workspace_id, task_id, status, started_at, finished_at, log_path
            ) VALUES (?1, ?2, ?3, 'success', ?4, ?4, ?5)
            "#,
    )
    .bind(&run_id)
    .bind(&workspace_id)
    .bind(&saved.task.id)
    .bind(Utc::now().to_rfc3339())
    .bind(log_path.to_string_lossy().to_string())
    .execute(service.db.pool())
    .await
    .unwrap();

    service
        .delete_task(workspace_id.clone(), saved.task.id.clone())
        .await
        .unwrap();
    assert!(service
        .get_task(&workspace_id, &saved.task.id)
        .await
        .is_err());
    let active_steps: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ssh_task_step WHERE task_id = ?1 AND deleted_at IS NULL",
    )
    .bind(&saved.task.id)
    .fetch_one(service.db.pool())
    .await
    .unwrap();
    let bindings: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_local_binding WHERE task_id = ?1")
            .bind(&saved.task.id)
            .fetch_one(service.db.pool())
            .await
            .unwrap();
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_run WHERE task_id = ?1")
        .bind(&saved.task.id)
        .fetch_one(service.db.pool())
        .await
        .unwrap();
    assert_eq!(active_steps, 0);
    assert_eq!(bindings, 0);
    assert_eq!(runs, 0);
    assert!(!log_path.exists());

    let insert_error = sqlx::query(
        r#"
            INSERT INTO ssh_task_run (
              id, workspace_id, task_id, status, started_at, finished_at, log_path
            ) VALUES (?1, ?2, ?3, 'success', ?4, ?4, ?5)
            "#,
    )
    .bind(unfour_core::id::new_id())
    .bind(&workspace_id)
    .bind(&saved.task.id)
    .bind(Utc::now().to_rfc3339())
    .bind(log_path.to_string_lossy().to_string())
    .execute(service.db.pool())
    .await
    .unwrap_err();
    assert!(insert_error
        .to_string()
        .contains("must reference an active task"));
    let _ = std::fs::remove_dir(&*service.task_log_dir);
}

#[tokio::test]
async fn local_binding_is_optional_and_tracks_default_and_last_used_connections() {
    let (service, workspace_id) = service().await;
    let saved = service
        .save_task(docker_export_input(workspace_id.clone()))
        .await
        .unwrap();
    assert!(saved.local_binding.is_none());
    let error = service
        .run_task(SshTaskRunInput {
            workspace_id: workspace_id.clone(),
            task_id: saved.task.id.clone(),
            connection_id: None,
            inputs: std::collections::BTreeMap::new(),
            secret_input_names: Vec::new(),
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("requires a connection"));

    let default = connection(&service, &workspace_id).await;
    let mut update = edit_input(&saved);
    update.default_connection_id = Some(default.id.clone());
    let updated = service.save_task(update).await.unwrap();
    let binding = updated.local_binding.unwrap();
    assert_eq!(
        binding.default_connection_id.as_deref(),
        Some(default.id.as_str())
    );
    assert!(binding.last_used_connection_id.is_none());
    let task_json = serde_json::to_value(&updated.task).unwrap();
    assert!(task_json.get("defaultConnectionId").is_none());
    let task_columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('ssh_task') ORDER BY cid")
            .fetch_all(service.db.pool())
            .await
            .unwrap();
    assert!(!task_columns
        .iter()
        .any(|name| name == "default_connection_id"));

    let last_used = connection(&service, &workspace_id).await;
    service
        .record_task_connection_use(&workspace_id, &saved.task.id, &last_used.id)
        .await
        .unwrap();
    let binding = service
        .get_task(&workspace_id, &saved.task.id)
        .await
        .unwrap()
        .local_binding
        .unwrap();
    assert_eq!(
        binding.default_connection_id.as_deref(),
        Some(default.id.as_str())
    );
    assert_eq!(
        binding.last_used_connection_id.as_deref(),
        Some(last_used.id.as_str())
    );
}
