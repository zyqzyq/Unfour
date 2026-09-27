use super::*;

#[tokio::test]
async fn flow_legacy_unsafe_definition_loads_but_is_rejected_before_execution() {
    let bus = test_bus().await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut value = definition(
        &ws,
        json!([
            {"id":"split","name":"Split","kind":"condition","timeoutMs":1000,"predicate":{"left":true,"op":"eq","right":true},"ifTrue":"branch","ifFalse":"branch"},
            {"id":"branch","name":"Branch","kind":"wait","timeoutMs":1000,"durationMs":0},
            {"id":"join","name":"Join","kind":"condition","timeoutMs":1000,"predicate":{"left":{"$ref":"/steps/branch/waitedMs"},"op":"eq","right":0},"ifTrue":"$end","ifFalse":"$end"}
        ]),
    );
    value = bus.save_flow(value).await.unwrap();
    let mut legacy = serde_json::to_value(&value).unwrap();
    legacy["steps"][0]["ifFalse"] = json!("join");
    // Simulate a definition saved before reference availability was validated.
    sqlx::query("UPDATE flow_definitions SET definition_json = ? WHERE id = ?")
        .bind(legacy.to_string())
        .bind(&value.id)
        .execute(bus.db.pool())
        .await
        .unwrap();
    let loaded = bus.get_flow(ws.clone(), value.id.clone()).await.unwrap();
    assert_eq!(serde_json::to_value(&loaded).unwrap(), legacy);
    assert!(bus
        .save_flow(loaded)
        .await
        .unwrap_err()
        .to_string()
        .contains("FLOW_UNSAFE_REFERENCE"));
    assert!(bus
        .run_flow(request(&ws, &value.id))
        .await
        .unwrap_err()
        .to_string()
        .contains("FLOW_UNSAFE_REFERENCE"));
    assert!(bus
        .list_flow_runs(ws.clone(), value.id.clone())
        .await
        .unwrap()
        .is_empty());
    // Restoring the common predecessor keeps the same persisted shape and can run.
    let repaired = bus.save_flow(value).await.unwrap();
    let run = bus.run_flow(request(&ws, &repaired.id)).await.unwrap();
    assert_eq!(finished(&bus, &run).await.status, FlowRunStatus::Succeeded);
}

#[tokio::test]
async fn flow_malformed_references_fail_save_and_legacy_run_before_creating_history() {
    let bus = test_bus().await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let safe = definition(
        &ws,
        json!([{"id":"check","name":"Check","kind":"condition","timeoutMs":1000,"predicate":{"left":true,"op":"eq","right":true},"ifTrue":"$end","ifFalse":"$end"}]),
    );
    let saved = bus.save_flow(safe).await.unwrap();
    for operand in [
        json!({"$ref":false}),
        json!({"$ref":"/inputs/a","extra":1}),
        json!("${inputs/a}"),
        json!("${/inputs/a} ${/bad/x}"),
        json!("${/inputs/a"),
    ] {
        let mut legacy = serde_json::to_value(&saved).unwrap();
        legacy["steps"][0]["predicate"]["left"] = operand;
        let error = bus
            .save_flow(serde_json::from_value(legacy.clone()).unwrap())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("FLOW_INVALID_REFERENCE"));
        sqlx::query("UPDATE flow_definitions SET definition_json = ? WHERE id = ?")
            .bind(legacy.to_string())
            .bind(&saved.id)
            .execute(bus.db.pool())
            .await
            .unwrap();
        assert!(bus
            .run_flow(request(&ws, &saved.id))
            .await
            .unwrap_err()
            .to_string()
            .contains("FLOW_INVALID_REFERENCE"));
        assert!(bus
            .list_flow_runs(ws.clone(), saved.id.clone())
            .await
            .unwrap()
            .is_empty());
    }
}
