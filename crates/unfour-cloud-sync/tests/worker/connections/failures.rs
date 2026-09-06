//! Connection operation-level failures and conservative batch fallback.

use super::*;

#[tokio::test]
async fn permanent_connection_failure_isolated_from_batch_peers() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let connection = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Dead Connection",
            "private-key",
            Some(r"C:\device\dead-key"),
        ))
        .await
        .unwrap();
    let variable = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "SURVIVOR", "value", false),
        )
        .await
        .unwrap();
    transport.fail_operation_once(&connection.id, "invalid_sync_entity");
    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Permanent
    );
    let failed = service.status(&workspace_id).await.unwrap();
    assert_eq!(failed.dead_count, 1);
    assert_eq!(failed.pending_count, 1);
    assert_eq!(failed.dead_letters[0].entity_type, "connection");
    assert_eq!(failed.dead_letters[0].entity_id, connection.id);

    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::DeadLetterBlocked
    );
    let remaining = service.status(&workspace_id).await.unwrap();
    assert_eq!(remaining.dead_count, 1);
    assert_eq!(remaining.pending_count, 0);
    assert!(pushed_operations(&transport)
        .iter()
        .any(|operation| operation.entity_id == variable.id));
}

#[tokio::test]
async fn unknown_operation_id_preserves_the_atomic_batch_for_attention() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let bus = CommandBus::from_db_with_extensions(db, CommandBusExtensions::new(vec![hook]))
        .await
        .unwrap();
    bus.save_ssh_connection(ssh_input(
        &workspace_id,
        None,
        "Unknown Operation Connection",
        "private-key",
        Some(r"C:\device\unknown-operation-key"),
    ))
    .await
    .unwrap();
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "UNKNOWN_OPERATION_PEER", "value", false),
    )
    .await
    .unwrap();
    transport.fail_unknown_operation_once("not-in-this-batch", "invalid_sync_entity");

    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Permanent
    );
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.dead_count, 0);
    assert_eq!(status.pending_count, 2);
    assert_eq!(status.binding.unwrap().state, "error");
}

#[tokio::test]
async fn compatibility_errors_preserve_operations_and_allow_atomic_peers_to_continue() {
    for code in [
        "protocol_version_unsupported",
        "feature_unsupported",
        "entity_type_unsupported",
        "payload_schema_version_unsupported",
        "field_unsupported",
    ] {
        let db = database().await;
        let seed = CommandBus::from_db(db.clone()).await.unwrap();
        let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
        let transport = Arc::new(MockTransport::new());
        let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
        service.enable(&workspace_id).await.unwrap();
        clear_pushes(&transport);
        let bus =
            CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
                .await
                .unwrap();
        let connection = bus
            .save_ssh_connection(ssh_input(
                &workspace_id,
                None,
                "Compatibility Connection",
                "private-key",
                Some(r"C:\device\compatibility-key"),
            ))
            .await
            .unwrap();
        let variable = bus
            .workspace_variable_create(
                workspace_id.clone(),
                variable(None, "COMPATIBILITY_PEER", "value", false),
            )
            .await
            .unwrap();
        transport.defer_operation_once(&connection.id, code);
        assert_eq!(
            service.sync_workspace(&workspace_id).await.unwrap_err(),
            SyncError::CompatibilityWaiting,
            "{code}"
        );
        let queued: (
            String,
            Option<String>,
            i64,
            String,
            Option<String>,
            Option<String>,
        ) = sqlx::query_as(
            r#"SELECT operation_id, canonical_payload_json, payload_schema_version,
                          status, last_error, next_attempt_at
                   FROM cloud_sync_outbox WHERE entity_id = ?1"#,
        )
        .bind(&connection.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        let sent = transport
            .pushes
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .operations
            .iter()
            .find(|operation| operation.entity_id == connection.id)
            .unwrap()
            .clone();
        assert_eq!(queued.0, sent.operation_id);
        assert_eq!(queued.2, sent.payload_schema_version);
        assert_eq!(
            queued
                .1
                .as_deref()
                .map(serde_json::from_str::<serde_json::Value>)
                .transpose()
                .unwrap(),
            sent.payload
        );
        assert_eq!(queued.3, "pending");
        assert_eq!(queued.4.as_deref(), Some(code));
        assert!(queued.5.is_some());
        let peer: (String, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT status, last_error, next_attempt_at FROM cloud_sync_outbox WHERE entity_id = ?1",
        )
        .bind(&variable.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(peer, ("pending".into(), None, None));
        assert_eq!(service.status(&workspace_id).await.unwrap().dead_count, 0);

        service.sync_workspace(&workspace_id).await.unwrap();
        assert!(!transport
            .pushes
            .lock()
            .unwrap()
            .iter()
            .skip(1)
            .flat_map(|push| &push.operations)
            .any(|operation| operation.entity_id == connection.id));
        assert!(transport
            .pushes
            .lock()
            .unwrap()
            .iter()
            .flat_map(|push| &push.operations)
            .any(|operation| operation.entity_id == variable.id));

        sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at = NULL WHERE entity_id = ?1")
            .bind(&connection.id)
            .execute(db.pool())
            .await
            .unwrap();
        service.sync_workspace(&workspace_id).await.unwrap();
        assert_eq!(
            service.status(&workspace_id).await.unwrap().pending_count,
            0
        );
    }
}

#[tokio::test]
async fn missing_common_protocol_stops_before_push_without_losing_local_intent() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let connection = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Protocol retained",
            "private-key",
            Some(r"C:\device\protocol-key"),
        ))
        .await
        .unwrap();
    let before: (String, Option<String>, i64) = sqlx::query_as(
        "SELECT operation_id, canonical_payload_json, payload_schema_version FROM cloud_sync_outbox WHERE entity_id = ?1",
    )
    .bind(&connection.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    *transport.protocol_versions.lock().unwrap() = vec![4];

    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::ProtocolIncompatible
    );
    assert!(transport.pushes.lock().unwrap().is_empty());
    let after: (String, Option<String>, i64) = sqlx::query_as(
        "SELECT operation_id, canonical_payload_json, payload_schema_version FROM cloud_sync_outbox WHERE entity_id = ?1",
    )
    .bind(&connection.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(after, before);
    assert_eq!(transport.generation.load(Ordering::SeqCst), 0);
}
