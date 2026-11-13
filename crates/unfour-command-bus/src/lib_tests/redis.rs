use super::*;

fn draft(workspace_id: String) -> RedisConnectionInput {
    RedisConnectionInput {
        id: None,
        workspace_id,
        name: "Cache".into(),
        host: "127.0.0.1".into(),
        port: 6379,
        db_index: 0,
        tls: false,
        username: None,
        expected_revision: None,
    }
}

#[tokio::test]
async fn redis_credentials_follow_checked_edits_and_never_travel_to_another_endpoint() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut edit = draft(workspace.clone());
    let saved = bus
        .save_redis_connection(edit.clone(), Some("cached-password".into()))
        .await
        .unwrap();
    assert!(saved.has_password);
    let ReadCommandResult::Connections(discovered) = bus
        .execute_read(ReadCommand::ListConnectionsInWorkspace {
            workspace_id: workspace.clone(),
            connection_type: ConnectionType::Redis,
        })
        .await
        .unwrap()
    else {
        panic!("expected connection discovery");
    };
    assert_eq!(discovered.count, 1);
    assert_eq!(discovered.connections[0].id, saved.id);
    assert_eq!(discovered.connections[0].connection_type, "redis");
    let reference: String = sqlx::query_scalar("SELECT credential_ref FROM connections WHERE id=?")
        .bind(&saved.id)
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    assert_eq!(
        bus.secret_store
            .read_secret(workspace.clone(), reference.clone())
            .await
            .unwrap(),
        "cached-password"
    );
    assert!(!serde_json::to_string(&saved).unwrap().contains(&reference));
    edit.id = Some(saved.id.clone());
    edit.expected_revision = Some(saved.revision);
    edit.host = "192.0.2.1".into();
    assert!(
        matches!(bus.test_redis_connection_input(edit.clone(),None).await,Err(AppError::Validation(code)) if code=="REDIS_ENDPOINT_REQUIRES_PASSWORD")
    );
    assert!(bus.save_redis_connection(edit.clone(), None).await.is_err());
    edit.host = saved.host;
    edit.name = "New name".into();
    let renamed = bus.save_redis_connection(edit.clone(), None).await.unwrap();
    assert!(renamed.has_password);
    assert!(bus
        .save_redis_connection(edit.clone(), Some("ignored".into()))
        .await
        .is_err());
    edit.expected_revision = Some(renamed.revision);
    let cleared = bus
        .save_redis_connection(edit, Some(String::new()))
        .await
        .unwrap();
    assert!(!cleared.has_password);
    assert!(bus
        .secret_store
        .read_secret(workspace.clone(), reference)
        .await
        .is_err());
    assert!(bus
        .delete_redis_connection(workspace.clone(), saved.id.clone(), saved.revision)
        .await
        .is_err());
    assert!(bus
        .delete_redis_connection(workspace, saved.id, cleared.revision)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn redis_rejects_foreign_connection_before_any_network_access() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let saved = bus
        .save_redis_connection(draft(workspace), None)
        .await
        .unwrap();
    let other = bus.create_workspace("Other".into()).await.unwrap().id;
    assert!(bus.list_redis_connections(&other).await.unwrap().is_empty());
    assert!(matches!(
        bus.inspect_redis_key(&other, &saved.id, "YQ==", None, Some(saved.revision))
            .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        bus.scan_redis_keys(&other, &saved.id, "0", "*", Some(saved.revision))
            .await,
        Err(AppError::NotFound(_))
    ));
    assert!(bus
        .delete_redis_connection(other, saved.id, saved.revision)
        .await
        .is_err());
}

#[tokio::test]
async fn redis_browser_rejects_changed_connection_before_contacting_another_host() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = draft(workspace.clone());
    let first = bus
        .save_redis_connection(input.clone(), None)
        .await
        .unwrap();
    input.id = Some(first.id.clone());
    input.expected_revision = Some(first.revision);
    input.host = "192.0.2.9".into();
    let latest = bus.save_redis_connection(input, None).await.unwrap();
    assert_ne!(first.revision, latest.revision);
    assert!(matches!(
        bus.inspect_redis_key(&workspace, &first.id, "YQ==", None, Some(first.revision))
            .await,
        Err(AppError::RowConflict(_))
    ));
    assert!(matches!(
        bus.scan_redis_keys(&workspace, &first.id, "0", "*", Some(first.revision))
            .await,
        Err(AppError::RowConflict(_))
    ));
}
