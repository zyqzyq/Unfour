use super::super::*;
use super::support::*;

#[tokio::test]
async fn task_runs_remain_local_and_can_be_physically_cleared() {
    let (service, workspace_id) = service().await;
    let saved = service
        .save_task(docker_export_input(workspace_id.clone()))
        .await
        .unwrap();
    let run_id = unfour_core::id::new_id();
    sqlx::query(
        r#"
            INSERT INTO ssh_task_run (
              id, workspace_id, task_id, status, started_at, log_path
            ) VALUES (?1, ?2, ?3, 'success', ?4, ?5)
            "#,
    )
    .bind(&run_id)
    .bind(&workspace_id)
    .bind(&saved.task.id)
    .bind(Utc::now().to_rfc3339())
    .bind(format!("{run_id}.log"))
    .execute(service.db.pool())
    .await
    .unwrap();
    assert_eq!(uuid::Uuid::parse_str(&run_id).unwrap().get_version_num(), 7);

    let result = service
        .clear_task_runs(SshTaskCleanupInput {
            workspace_id: workspace_id.clone(),
            task_id: Some(saved.task.id),
        })
        .await
        .unwrap();
    assert_eq!(result.deleted_runs, 1);
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_run WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(service.db.pool())
            .await
            .unwrap();
    assert_eq!(remaining, 0);
}
