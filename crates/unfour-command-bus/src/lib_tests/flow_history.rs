use super::*;

#[tokio::test]
async fn flow_history_reads_columns_without_decoding_snapshots_and_limits_scope() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for i in 0..102 {
        sqlx::query("INSERT INTO flow_runs (id, workspace_id, flow_id, status, run_json, started_at, updated_at, finished_at) VALUES (?, ?, 'deleted-flow', 'succeeded', 'invalid snapshot', ?, 'heartbeat', '2026-09-21T01:00:00Z')")
            .bind(format!("run-{i:03}")).bind(&workspace).bind(format!("2026-09-21T00:{:02}:{:02}Z", i / 60, i % 60)).execute(bus.db.pool()).await.unwrap();
    }
    let summaries = bus
        .list_flow_runs(workspace.clone(), "deleted-flow".into())
        .await
        .unwrap();
    assert_eq!(summaries.len(), 100);
    assert_eq!(summaries[0].id, "run-101");
    assert_eq!(summaries[99].id, "run-002");
    let value = serde_json::to_value(&summaries[0]).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 5);
    assert_eq!(value["status"], "succeeded");
    assert_eq!(value["finishedAt"], "2026-09-21T01:00:00Z");
    assert!(bus
        .get_flow_run(workspace.clone(), "run-101".into())
        .await
        .is_err());
    assert!(bus
        .list_flow_runs("other".into(), "deleted-flow".into())
        .await
        .unwrap()
        .is_empty());
    assert!(bus
        .list_flow_runs(workspace, "other".into())
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn flow_history_preserves_exact_snapshot_finish_time_after_completion_and_deletion() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    let flow = service
        .save(definition(
            &workspace,
            json!([{"id":"wait","name":"wait","kind":"wait","durationMs":20,"timeoutMs":1000}]),
        ))
        .await
        .unwrap();
    let run = service
        .run(request(&workspace, &flow.id), Arc::new(Driver::default()))
        .await
        .unwrap();
    let summary = bus
        .list_flow_runs(workspace.clone(), flow.id.clone())
        .await
        .unwrap();
    assert_eq!(summary[0].status, FlowRunStatus::Running);
    assert_eq!(summary[0].finished_at, None);
    let done = finished(&bus, &run).await;
    service.delete(&workspace, &flow.id).await.unwrap();
    let summary = bus
        .list_flow_runs(workspace.clone(), flow.id)
        .await
        .unwrap();
    assert_eq!(summary[0].status, done.status);
    assert_eq!(summary[0].finished_at, done.finished_at);
    assert_eq!(summary[0].started_at, done.started_at);
    assert_eq!(
        bus.get_flow_run(workspace, run.id)
            .await
            .unwrap()
            .definition
            .revision,
        1
    );
}

#[tokio::test]
async fn flow_history_migration_backfills_without_changing_snapshots() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::raw_sql(r#"CREATE TABLE flow_runs (id TEXT, run_json TEXT); INSERT INTO flow_runs VALUES ('done', '{"finishedAt":"exact-time","steps":[]}'), ('running', '{"finishedAt":null}');"#).execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../local-storage/migrations/20260921000000_core_flow_run_summary.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let value: (String, String) =
        sqlx::query_as("SELECT finished_at, run_json FROM flow_runs WHERE id = 'done'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(value.0, "exact-time");
    assert_eq!(value.1, r#"{"finishedAt":"exact-time","steps":[]}"#);
    let running: Option<String> =
        sqlx::query_scalar("SELECT finished_at FROM flow_runs WHERE id = 'running'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(running, None);
}

#[tokio::test]
async fn flow_history_validation_failure_has_exact_finish_time() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    let flow = service
        .save(definition(
            &workspace,
            json!([{"id":"wait","name":"wait","kind":"wait","durationMs":20,"timeoutMs":1000}]),
        ))
        .await
        .unwrap();
    let mut input = request(&workspace, &flow.id);
    input.inputs = json!([]);
    let run = service
        .run(input, Arc::new(Driver::default()))
        .await
        .unwrap();
    let summary = bus.list_flow_runs(workspace, flow.id).await.unwrap();
    assert_eq!(summary[0].status, FlowRunStatus::ValidationFailed);
    assert!(run.finished_at.is_some());
    assert_eq!(summary[0].finished_at, run.finished_at);
}
