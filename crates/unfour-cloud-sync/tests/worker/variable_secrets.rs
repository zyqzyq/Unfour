use super::support::*;

#[tokio::test]
async fn unsafe_legacy_variable_retries_are_parked_without_transmitting_or_rewriting_them() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let created = bus
        .workspace_variable_create(
            workspace.clone(),
            variable(None, "DB_PASSWORD", "legacy-canary", false),
        )
        .await
        .unwrap();
    let unsafe_payload = serde_json::json!({"key":"DB_PASSWORD","value":"legacy-canary","isSecret":false,"isEnabled":true,"sortOrder":0,"createdAt":"2026-10-09T00:00:00Z","updatedAt":"2026-10-09T00:00:00Z"}).to_string();
    sqlx::query("UPDATE cloud_sync_outbox SET canonical_payload_json=?,status='uncertain' WHERE entity_id=?")
        .bind(&unsafe_payload).bind(&created.id).execute(db.pool()).await.unwrap();
    let operation: String =
        sqlx::query_scalar("SELECT operation_id FROM cloud_sync_outbox WHERE entity_id=?")
            .bind(&created.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(matches!(
        service.sync_workspace(&workspace).await,
        Err(SyncError::DeadLetterBlocked)
    ));
    assert!(transport.pushes.lock().unwrap().is_empty());
    let status = service.status(&workspace).await.unwrap();
    assert_eq!(status.dead_count, 1);
    assert_eq!(
        status.dead_letters[0].error_code,
        "sensitive_variable_payload"
    );
    let retained: (String, String) = sqlx::query_as(
        "SELECT operation_id,canonical_payload_json FROM cloud_sync_outbox WHERE entity_id=?",
    )
    .bind(&created.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(retained, (operation, unsafe_payload));
    let repaired = service
        .retry_dead_letter_current_local(&workspace, &retained.0)
        .await
        .unwrap();
    assert_ne!(repaired, retained.0);
    let pushes = transport.pushes.lock().unwrap();
    assert_eq!(pushes.len(), 1);
    let payload = pushes[0].operations[0].payload.as_ref().unwrap();
    assert_eq!(payload["isSecret"], true);
    assert!(payload.get("value").is_none());
    assert!(!payload.to_string().contains("legacy-canary"));
}
