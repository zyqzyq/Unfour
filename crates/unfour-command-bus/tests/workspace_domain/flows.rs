use super::*;
use serde_json::json;
use unfour_core::models::{FlowDefinition, FlowRunInput};

fn definition(workspace: &str) -> FlowDefinition {
    serde_json::from_value(json!({"id":"", "workspaceId":workspace, "name":"Flow",
        "revision":0,"inputs":[],"steps":[{"id":"wait","name":"Wait","kind":"wait",
        "durationMs":0,"timeoutMs":1000}]}))
    .unwrap()
}

async fn saved_run(bus: &CommandBus, flow: &FlowDefinition) -> String {
    let input: FlowRunInput = serde_json::from_value(json!({"workspaceId":flow.workspace_id,
        "flowId":flow.id,"inputs":{},"initiator":"human","confirmEffects":true}))
    .unwrap();
    let run = bus.run_flow(input).await.unwrap();
    // Wait for the lease to finish before testing historical retention.
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while bus
            .get_flow_run(flow.workspace_id.clone(), run.id.clone())
            .await
            .unwrap()
            .status
            .as_str()
            == "running"
        {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    run.id
}

async fn tombstone(db: &LocalDb, id: &str) -> (i64, i64, Option<String>, String) {
    sqlx::query_as("SELECT revision, json_extract(definition_json, '$.revision'), deleted_at, updated_at FROM flow_definitions WHERE id = ?")
        .bind(id).fetch_one(db.pool()).await.unwrap()
}

#[tokio::test]
async fn flow_revision_conflicts_and_tombstone_preserve_local_history() {
    let (bus, db) = bus_with_hooks(vec![Arc::new(SqlHook {
        name: "capture",
        fail_on: None,
        local_only: false,
    })])
    .await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let saved = bus.save_flow(definition(&workspace)).await.unwrap();
    let run = saved_run(&bus, &saved).await;
    let revised = bus.save_flow(saved.clone()).await.unwrap();
    for expected in [0, -1, saved.revision, i64::MAX] {
        assert!(bus
            .delete_flow(workspace.clone(), saved.id.clone(), expected)
            .await
            .is_err());
    }
    assert!(bus.save_flow(saved.clone()).await.is_err());
    assert!(bus
        .delete_flow("foreign".into(), saved.id.clone(), revised.revision)
        .await
        .is_err());
    bus.delete_flow(workspace.clone(), saved.id.clone(), revised.revision)
        .await
        .unwrap();
    let dead = tombstone(&db, &saved.id).await;
    assert_eq!(dead.0, revised.revision + 1);
    assert_eq!(dead.0, dead.1);
    assert_eq!(dead.2.as_ref(), Some(&dead.3));
    assert!(bus
        .get_flow(workspace.clone(), saved.id.clone())
        .await
        .is_err());
    assert!(bus.list_flows(workspace.clone()).await.unwrap().is_empty());
    assert!(bus
        .list_flow_summaries(workspace.clone())
        .await
        .unwrap()
        .is_empty());
    assert!(bus.save_flow(revised.clone()).await.is_err());
    assert!(bus
        .delete_flow(workspace.clone(), saved.id.clone(), dead.0)
        .await
        .is_err());
    assert_eq!(tombstone(&db, &saved.id).await, dead);
    assert!(bus.get_flow_run(workspace.clone(), run).await.is_ok());
    assert_eq!(
        bus.list_flow_runs(workspace.clone(), saved.id.clone())
            .await
            .unwrap()
            .len(),
        1
    );
    let input = serde_json::from_value(json!({"workspaceId":workspace,"flowId":saved.id,
        "inputs":{},"initiator":"human","confirmEffects":true}))
    .unwrap();
    assert!(bus.run_flow(input).await.is_err());
    let effects: Vec<(String, i64)> = sqlx::query_as(
        "SELECT operation, revision FROM hook_effects WHERE entity_type='FlowDefinition' ORDER BY id",
    ).fetch_all(db.pool()).await.unwrap();
    assert_eq!(
        effects,
        vec![
            ("Upsert".into(), 1),
            ("Upsert".into(), 2),
            ("Delete".into(), 3)
        ]
    );
    let mut tx = db.pool().begin().await.unwrap();
    assert!(unfour_flow_engine::FlowService::new(db.clone())
        .export_definitions_on(&mut tx, &workspace)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn flow_workspace_cascade_local_and_external_purges_history_and_blocks_access() {
    for external in [false, true] {
        let (bus, db) = bus_with_hooks(vec![]).await;
        let ws = bus.create_workspace("Delete me".into()).await.unwrap().id;
        let flow = bus.save_flow(definition(&ws)).await.unwrap();
        let run = saved_run(&bus, &flow).await;
        let already_deleted = bus.save_flow(definition(&ws)).await.unwrap();
        bus.delete_flow(ws.clone(), already_deleted.id.clone(), 1)
            .await
            .unwrap();
        let prior = tombstone(&db, &already_deleted.id).await;
        if external {
            bus.apply_external_workspaces(vec![ExternalWorkspaceApply::Delete(ExternalDelete {
                entity: DomainEntityKey::new(DomainEntityType::Workspace, &ws, &ws),
                deleted_at: "2026-10-20T00:00:00Z".into(),
            })])
            .await
            .unwrap();
        } else {
            bus.delete_workspace(ws.clone()).await.unwrap();
        }
        let workspace_deleted_at: String =
            sqlx::query_scalar("SELECT deleted_at FROM workspaces WHERE id=?")
                .bind(&ws)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(
            tombstone(&db, &flow.id).await.2.as_deref(),
            Some(workspace_deleted_at.as_str())
        );
        assert_eq!(tombstone(&db, &already_deleted.id).await, prior);
        let history: i64 =
            sqlx::query_scalar("SELECT count(*) FROM flow_runs WHERE workspace_id=?")
                .bind(&ws)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(history, 0);
        assert!(bus.get_flow_run(ws.clone(), run).await.is_err());
        assert!(bus
            .list_flow_runs(ws.clone(), flow.id.clone())
            .await
            .is_err());
        assert!(bus.list_flows(ws.clone()).await.is_err());
        assert!(bus.list_flow_summaries(ws.clone()).await.is_err());
        assert!(bus.get_flow(ws.clone(), flow.id.clone()).await.is_err());
        assert!(bus.save_flow(flow.clone()).await.is_err());
        assert!(bus.save_flow(definition(&ws)).await.is_err());
        assert!(bus
            .delete_flow(ws.clone(), flow.id.clone(), 2)
            .await
            .is_err());
        let input = serde_json::from_value(json!({"workspaceId":ws,"flowId":flow.id,
            "inputs":{},"initiator":"human","confirmEffects":true}))
        .unwrap();
        assert!(bus.run_flow(input).await.is_err());
    }
}

#[tokio::test]
async fn flow_hook_failure_rolls_back_save_delete_and_workspace_history_purge() {
    for command in ["flow.save", "flow.delete", "workspace.delete"] {
        let (seed, db) = bus_with_hooks(vec![]).await;
        let ws = seed.create_workspace("Rollback".into()).await.unwrap().id;
        let flow = seed.save_flow(definition(&ws)).await.unwrap();
        let run = saved_run(&seed, &flow).await;
        let prior = tombstone(&db, &flow.id).await;
        let activity: i64 = sqlx::query_scalar("SELECT count(*) FROM activity_events")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let bus = CommandBus::from_db_with_extensions(
            db.clone(),
            CommandBusExtensions::new(vec![Arc::new(SqlHook {
                name: "reject",
                fail_on: Some(command),
                local_only: false,
            })]),
        )
        .await
        .unwrap();
        let result = match command {
            "flow.save" => bus.save_flow(flow.clone()).await.map(|_| ()),
            "flow.delete" => bus.delete_flow(ws.clone(), flow.id.clone(), 1).await,
            _ => bus.delete_workspace(ws.clone()).await.map(|_| ()),
        };
        assert!(result.is_err(), "{command}");
        assert_eq!(tombstone(&db, &flow.id).await, prior);
        assert!(bus.get_flow_run(ws.clone(), run).await.is_ok());
        assert!(bus.get_flow(ws.clone(), flow.id.clone()).await.is_ok());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM hook_effects")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM activity_events")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            activity
        );
        if command == "flow.save" {
            assert!(bus.save_flow(definition(&ws)).await.is_err());
            assert_eq!(bus.list_flows(ws).await.unwrap().len(), 1);
        }
    }
}

#[tokio::test]
async fn flow_prepared_before_definition_or_workspace_delete_cannot_start_or_recreate_history() {
    use serde_json::Value;
    use tokio::sync::Notify;
    use unfour_core::models::FlowAction;
    use unfour_flow_engine::{FlowExecutor, FlowFuture, FlowService};

    #[derive(Default)]
    struct PausedPrepare {
        entered: Notify,
        resume: Notify,
    }
    impl FlowExecutor for PausedPrepare {
        fn prepare<'a>(
            &'a self,
            _: &'a FlowAction,
            _: &'a FlowRunInput,
            _: bool,
        ) -> FlowFuture<'a, Value> {
            Box::pin(async move {
                self.entered.notify_one();
                self.resume.notified().await;
                Ok(json!({}))
            })
        }
        fn execute<'a>(
            &'a self,
            _: &'a FlowAction,
            _: &'a Value,
            _: &'a FlowRunInput,
            _: tokio_util::sync::CancellationToken,
            _: &'a unfour_flow_engine::FlowPersistenceContext,
        ) -> FlowFuture<'a, Value> {
            Box::pin(async { panic!("deleted workspace must never execute") })
        }
    }
    for delete_workspace in [false, true] {
        let (bus, db) = bus_with_hooks(vec![]).await;
        let ws = bus.create_workspace("Preparing".into()).await.unwrap().id;
        let service = FlowService::new(db.clone());
        let flow = service.save(serde_json::from_value(json!({"id":"", "workspaceId":ws,
        "name":"Prepare race", "revision":0,"inputs":[],"steps":[{"id":"api","name":"API",
        "kind":"action","timeoutMs":1000,"action":{"capability":"api","resourceId":"mock","arguments":{}}}]})).unwrap()).await.unwrap();
        let driver = Arc::new(PausedPrepare::default());
        let executor = driver.clone();
        let input = serde_json::from_value(json!({"workspaceId":ws,"flowId":flow.id,
        "inputs":{},"initiator":"human","confirmEffects":true}))
        .unwrap();
        let task = tokio::spawn(async move { service.run(input, executor).await });
        tokio::time::timeout(std::time::Duration::from_secs(3), driver.entered.notified())
            .await
            .unwrap();
        if delete_workspace {
            bus.delete_workspace(ws.clone()).await.unwrap();
        } else {
            bus.delete_flow(ws.clone(), flow.id.clone(), flow.revision)
                .await
                .unwrap();
        }
        driver.resume.notify_one();
        assert!(task.await.unwrap().is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM flow_runs WHERE workspace_id=?")
                .bind(&ws)
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
    }
}
