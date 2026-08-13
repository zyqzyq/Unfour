use super::*;

#[tokio::test]
async fn local_api_mutations_are_revisioned_noop_aware_and_hierarchical() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: false,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Users".to_string())
        .await
        .unwrap();
    let initial_revision: i64 =
        sqlx::query_scalar("SELECT revision FROM api_collections WHERE id = ?1")
            .bind(&collection.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(initial_revision, 1);

    bus.api_collection_rename(
        workspace_id.clone(),
        collection.id.clone(),
        "People".to_string(),
    )
    .await
    .unwrap();
    let hook_count_before_noop: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_hook_effects")
        .fetch_one(db.pool())
        .await
        .unwrap();
    bus.api_collection_rename(
        workspace_id.clone(),
        collection.id.clone(),
        "People".to_string(),
    )
    .await
    .unwrap();
    let revision_after_noop: i64 =
        sqlx::query_scalar("SELECT revision FROM api_collections WHERE id = ?1")
            .bind(&collection.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let hook_count_after_noop: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_hook_effects")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(revision_after_noop, 2);
    assert_eq!(hook_count_after_noop, hook_count_before_noop);

    let root = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Root".to_string(),
        )
        .await
        .unwrap();
    let child = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            Some(root.id.clone()),
            "Child".to_string(),
        )
        .await
        .unwrap();
    let child_revision = child.revision;
    let child = bus
        .api_collection_folder_rename(workspace_id.clone(), child.id, "Renamed Child".to_string())
        .await
        .unwrap();
    assert_eq!(child.revision, child_revision + 1);
    let child_noop = bus
        .api_collection_folder_rename(workspace_id.clone(), child.id.clone(), child.name.clone())
        .await
        .unwrap();
    assert_eq!(child_noop.revision, child.revision);
    let other_collection = bus
        .api_collection_create(workspace_id.clone(), "Other".to_string())
        .await
        .unwrap();
    let invalid_parent = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            other_collection.id,
            Some(root.id.clone()),
            "Invalid".to_string(),
        )
        .await;
    assert!(matches!(invalid_parent, Err(AppError::Validation(_))));

    let input = request_input(&workspace_id, &collection.id, Some(child.id.clone()));
    let request = bus.save_api_request(input.clone()).await.unwrap();
    let saved_revision = request.revision;
    let saved_again = bus
        .update_api_request(workspace_id.clone(), request.id.clone(), input.clone())
        .await
        .unwrap();
    assert_eq!(saved_again.revision, saved_revision);
    let mut changed = input;
    changed.method = "PUT".to_string();
    changed.url = "https://api.example.test/users/1".to_string();
    changed.body = Some(r#"{"name":"Grace","token":"body-device-secret"}"#.to_string());
    let changed_request = bus
        .update_api_request(workspace_id.clone(), request.id.clone(), changed.clone())
        .await
        .unwrap();
    assert_eq!(changed_request.revision, saved_revision + 1);
    let changed_again = bus
        .update_api_request(workspace_id.clone(), request.id.clone(), changed)
        .await
        .unwrap();
    assert_eq!(changed_again.revision, changed_request.revision);

    let snapshot = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::ApiRequest,
            &workspace_id,
            &request.id,
        ))
        .await
        .unwrap();
    let DomainSnapshot::ApiRequest(snapshot) = snapshot else {
        panic!("expected API request snapshot");
    };
    assert_eq!(snapshot.method, "PUT");
    let snapshot_body: serde_json::Value =
        serde_json::from_str(snapshot.body.as_deref().unwrap()).unwrap();
    assert_eq!(snapshot_body["name"], "Grace");
    assert_eq!(snapshot_body["token"], "<redacted>");
    assert_eq!(
        snapshot.pre_request_script.as_deref(),
        Some("pm.variables.set('trace', '1');")
    );
    assert_eq!(
        snapshot.post_response_script.as_deref(),
        Some("pm.test('ok', () => true);")
    );
    let serialized = serde_json::to_string(&snapshot).unwrap();
    for excluded in [
        "auth-device-secret",
        "header-device-secret",
        "query-device-secret",
        "url-device-secret",
        "body-device-secret",
        "runtime_only",
        "not-synced",
        "timeoutMs",
    ] {
        assert!(!serialized.contains(excluded), "snapshot leaked {excluded}");
    }

    bus.api_collection_folder_delete(workspace_id.clone(), root.id.clone())
        .await
        .unwrap();
    for (table, id) in [
        ("api_collection_folders", root.id.as_str()),
        ("api_collection_folders", child.id.as_str()),
        ("api_requests", request.id.as_str()),
    ] {
        let query = format!("SELECT deleted_at FROM {table} WHERE id = ?1");
        let deleted_at: Option<String> = sqlx::query_scalar(&query)
            .bind(id)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert!(deleted_at.is_some(), "{table}:{id} must be tombstoned");
    }
    let delete_types: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT entity_type FROM api_hook_effects
        WHERE command_name = 'api.collection.folder.delete'
          AND operation = 'Delete'
        ORDER BY id
        "#,
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        delete_types,
        vec![
            "ApiRequest".to_string(),
            "ApiFolder".to_string(),
            "ApiFolder".to_string(),
        ]
    );
    let request_tombstone = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::ApiRequest,
            &workspace_id,
            &request.id,
        ))
        .await
        .unwrap();
    let DomainSnapshot::Tombstone(tombstone) = request_tombstone else {
        panic!("expected request tombstone");
    };
    assert_eq!(
        tombstone.entity.parent_entity_id.as_deref(),
        Some(child.id.as_str())
    );

    let direct_request = bus
        .save_api_request(request_input(&workspace_id, &collection.id, None))
        .await
        .unwrap();
    bus.delete_api_request(workspace_id.clone(), direct_request.id.clone())
        .await
        .unwrap();
    assert!(matches!(
        bus.read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::ApiRequest,
            &workspace_id,
            &direct_request.id,
        ))
        .await
        .unwrap(),
        DomainSnapshot::Tombstone(_)
    ));

    let remaining_folder = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Remaining".to_string(),
        )
        .await
        .unwrap();
    let remaining_request = bus
        .save_api_request(request_input(
            &workspace_id,
            &collection.id,
            Some(remaining_folder.id.clone()),
        ))
        .await
        .unwrap();
    bus.api_collection_delete(workspace_id.clone(), collection.id.clone())
        .await
        .unwrap();
    let collection_delete_types: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT entity_type FROM api_hook_effects
        WHERE command_name = 'api.collection.delete' AND operation = 'Delete'
        ORDER BY id
        "#,
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        collection_delete_types,
        vec![
            "ApiRequest".to_string(),
            "ApiFolder".to_string(),
            "ApiCollection".to_string(),
        ]
    );
    for (entity_type, id) in [
        (DomainEntityType::ApiCollection, collection.id.as_str()),
        (DomainEntityType::ApiFolder, remaining_folder.id.as_str()),
        (DomainEntityType::ApiRequest, remaining_request.id.as_str()),
    ] {
        assert!(matches!(
            bus.read_domain_snapshot(&DomainEntityKey::new(entity_type, &workspace_id, id,))
                .await
                .unwrap(),
            DomainSnapshot::Tombstone(_)
        ));
    }
}
