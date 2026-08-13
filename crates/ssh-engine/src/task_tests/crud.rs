use super::super::*;
use super::support::*;

#[tokio::test]
async fn task_crud_persists_ordered_steps_without_parameter_records() {
    let (service, workspace_id) = service().await;
    let saved = service
        .save_task(docker_export_input(workspace_id.clone()))
        .await
        .unwrap();
    for id in [&workspace_id, &saved.task.id] {
        let parsed = uuid::Uuid::parse_str(id).unwrap();
        assert_eq!(parsed.get_version_num(), 7);
    }
    assert_eq!(saved.steps.len(), 5);
    assert_eq!(saved.steps[3].step_type, "download");
    assert!(saved.steps.iter().all(|step| {
        uuid::Uuid::parse_str(&step.id).is_ok_and(|id| id.get_version_num() == 7)
            && step.config_version == 1
            && step.config_json.get("version").is_none()
    }));
    assert_eq!(
        detected_inputs(&saved.steps).unwrap(),
        vec![
            "source_image",
            "target_image",
            "archive_name",
            "local_output_dir"
        ]
    );

    let copy = service
        .duplicate_task(workspace_id.clone(), saved.task.id.clone())
        .await
        .unwrap();
    assert_ne!(copy.task.id, saved.task.id);
    assert_eq!(
        service
            .list_tasks(workspace_id.clone())
            .await
            .unwrap()
            .len(),
        2
    );

    service
        .delete_task(workspace_id.clone(), saved.task.id)
        .await
        .unwrap();
    assert_eq!(service.list_tasks(workspace_id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn task_manual_order_is_stable_and_reorder_requires_the_complete_workspace_set() {
    let (service, workspace_id) = service().await;
    let mut first_input = docker_export_input(workspace_id.clone());
    first_input.name = "First".to_string();
    let first = service.save_task(first_input).await.unwrap();
    let mut second_input = docker_export_input(workspace_id.clone());
    second_input.name = "Second".to_string();
    let second = service.save_task(second_input).await.unwrap();
    assert_eq!(first.task.sort_order, 0);
    assert_eq!(second.task.sort_order, 1);

    let mut first_update = edit_input(&first);
    first_update.description = "Updated without moving".to_string();
    let updated = service.save_task(first_update).await.unwrap();
    assert_eq!(updated.task.sort_order, 0);
    assert_eq!(
        service
            .list_tasks(workspace_id.clone())
            .await
            .unwrap()
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        vec![first.task.id.as_str(), second.task.id.as_str()]
    );

    let reordered = service
        .reorder_tasks(SshTasksReorderInput {
            workspace_id: workspace_id.clone(),
            task_ids: vec![second.task.id.clone(), first.task.id.clone()],
        })
        .await
        .unwrap();
    assert_eq!(
        reordered
            .iter()
            .map(|task| (task.id.as_str(), task.sort_order))
            .collect::<Vec<_>>(),
        vec![(second.task.id.as_str(), 0), (first.task.id.as_str(), 1)]
    );
    assert_eq!(reordered[0].updated_at, second.task.updated_at);
    assert_eq!(reordered[1].updated_at, updated.task.updated_at);

    let missing = service
        .reorder_tasks(SshTasksReorderInput {
            workspace_id: workspace_id.clone(),
            task_ids: vec![first.task.id.clone()],
        })
        .await
        .unwrap_err();
    assert!(missing.to_string().contains("every active task"));
    let duplicate = service
        .reorder_tasks(SshTasksReorderInput {
            workspace_id: workspace_id.clone(),
            task_ids: vec![first.task.id.clone(), first.task.id.clone()],
        })
        .await
        .unwrap_err();
    assert!(duplicate.to_string().contains("duplicate"));

    let other_workspace_id = unfour_core::id::new_id();
    sqlx::query(
        r#"
            INSERT INTO workspaces (
              id, name, is_default, created_at, updated_at, revision, sync_status
            ) VALUES (?1, 'Other Tasks', 0, ?2, ?2, 1, 'local')
            "#,
    )
    .bind(&other_workspace_id)
    .bind(Utc::now().to_rfc3339())
    .execute(service.db.pool())
    .await
    .unwrap();
    let mut other_input = docker_export_input(other_workspace_id);
    other_input.name = "Other".to_string();
    let other = service.save_task(other_input).await.unwrap();
    let cross_workspace = service
        .reorder_tasks(SshTasksReorderInput {
            workspace_id,
            task_ids: vec![second.task.id, other.task.id],
        })
        .await
        .unwrap_err();
    assert!(cross_workspace.to_string().contains("every active task"));
}

#[tokio::test]
async fn ordinary_step_updates_preserve_config_version_and_unknown_versions_fail() {
    let (service, workspace_id) = service().await;
    let saved = service
        .save_task(docker_export_input(workspace_id.clone()))
        .await
        .unwrap();
    let mut update = edit_input(&saved);
    update.steps[0].config_version = None;
    update.steps[0].config_json["command"] =
        serde_json::json!("docker pull --quiet {{source_image}}");
    let updated = service.save_task(update).await.unwrap();
    assert_eq!(updated.steps[0].config_version, 1);

    let mut invalid = edit_input(&updated);
    invalid.steps.push(SshTaskStepInput {
        id: None,
        name: "Future command".to_string(),
        step_type: "command".to_string(),
        position: invalid.steps.len() as i64,
        enabled: true,
        config_version: Some(99),
        config_json: serde_json::json!({
            "command": "true",
            "workingDirectory": "",
            "timeoutSeconds": 30,
            "continueOnError": false
        }),
    });
    let error = service.save_task(invalid).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("unsupported SSH task command config version: 99"));
}
