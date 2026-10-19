use super::*;
use serde_json::json;

async fn fail_activity(bus: &CommandBus) {
    sqlx::query("CREATE TRIGGER fail_audit_activity BEFORE INSERT ON activity_events BEGIN SELECT RAISE(FAIL, 'audit sink unavailable'); END;")
        .execute(bus.db.pool()).await.unwrap();
}

#[tokio::test]
async fn committed_database_effects_survive_activity_failure() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let path = std::env::temp_dir().join(format!(
        "unfour-activity-{}.sqlite",
        unfour_core::id::new_id()
    ));
    let target = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE effects (id INTEGER PRIMARY KEY, n INTEGER)")
        .execute(&target)
        .await
        .unwrap();
    let connection = bus.save_database_connection(serde_json::from_value(json!({
        "workspaceId":workspace,"name":"isolated target","driver":"sqlite","sqlitePath":path.to_str().unwrap(),"readOnly":false
    })).unwrap()).await.unwrap();
    let flow = bus
        .save_flow(super::flow::definition(
            &workspace,
            json!([super::flow::action(
                "write",
                "database",
                &connection.id,
                json!({"sql":"INSERT INTO effects(n) VALUES (3)"})
            )]),
        ))
        .await
        .unwrap();
    fail_activity(&bus).await;
    let input = |sql: &str, confirmed| DatabaseQueryInput {
        workspace_id: workspace.clone(),
        connection_id: connection.id.clone(),
        sql: sql.into(),
        limit: None,
        confirm_mutation: Some(confirmed),
        catalog: None,
        schema: None,
        timeout_ms: None,
    };
    assert!(bus
        .execute_database_query(input("INSERT INTO effects(n) VALUES (0)", false))
        .await
        .is_err());
    let result = bus
        .execute_database_query(input("INSERT INTO effects(n) VALUES (1)", true))
        .await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM effects")
        .fetch_one(&target)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!(
        result.is_ok(),
        "committed write reported as failed: {result:?}"
    );
    let mut serialized = serde_json::to_value(result.unwrap()).unwrap();
    assert_eq!(serialized["warnings"], json!(["ACTIVITY_RECORD_FAILED"]));
    serialized.as_object_mut().unwrap().remove("warnings");
    let legacy: DatabaseQueryResult = serde_json::from_value(serialized).unwrap();
    assert!(
        legacy.warnings.is_empty(),
        "old query results remain readable"
    );
    let result = bus.mutate_database_row(serde_json::from_value(json!({
        "workspaceId":workspace,"connectionId":connection.id,"schema":null,"tableName":"effects",
        "operation":"insert","values":[{"column":"n","value":"2"}],"confirmMutation":true
    })).unwrap()).await.unwrap();
    assert_eq!(result.affected_rows, 1);
    let mut serialized = serde_json::to_value(result).unwrap();
    assert_eq!(serialized["warnings"], json!(["ACTIVITY_RECORD_FAILED"]));
    serialized.as_object_mut().unwrap().remove("warnings");
    let legacy: DatabaseRowMutationResult = serde_json::from_value(serialized).unwrap();
    assert!(legacy.warnings.is_empty());
    assert!(bus
        .execute_database_query(input("INSERT INTO missing_table VALUES (1)", true))
        .await
        .is_err());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM effects")
        .fetch_one(&target)
        .await
        .unwrap();
    assert_eq!(count, 2, "no replay after an activity failure");
    let run = bus
        .run_flow(super::flow::request(&workspace, &flow.id))
        .await
        .unwrap();
    let finished = super::flow::finished(&bus, &run).await;
    assert_eq!(
        finished.status,
        unfour_core::models::FlowRunStatus::Succeeded,
        "{finished:?}"
    );
    assert_eq!(finished.steps[0].attempts.len(), 1);
    assert_eq!(
        finished.steps[0].attempts[0].output.as_ref().unwrap()["warnings"],
        json!(["ACTIVITY_RECORD_FAILED"])
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM effects")
        .fetch_one(&target)
        .await
        .unwrap();
    assert_eq!(count, 3);
    target.close().await;
    std::fs::remove_file(path).unwrap();
}
