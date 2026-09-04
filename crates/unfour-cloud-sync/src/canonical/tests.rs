use super::*;
use unfour_core::domain::{
    ApiCollectionSnapshot, ApiFolderSnapshot, ApiRequestSnapshot, ConnectionSnapshot,
    ConnectionSnapshotConfig, DomainSnapshot, ExternalApiRequestApply, ExternalConnectionApply,
    SshTaskSnapshot, SshTaskStepSnapshot, TombstoneSnapshot, WorkspaceVariableSnapshot,
};
use unfour_core::models::{ApiRequestSettings, MAX_API_TIMEOUT_MS};

#[test]
fn secret_payload_has_no_identity_or_secret_value() {
    let payload = canonical_payload(DomainSnapshot::WorkspaceVariable(
        WorkspaceVariableSnapshot {
            id: "variable-1".into(),
            workspace_id: "workspace-1".into(),
            key: "TOKEN".into(),
            value: SnapshotVariableValue::SecretRedacted,
            is_secret: true,
            is_enabled: true,
            description: None,
            sort_order: 0,
            created_at: "2026-07-27T00:00:00Z".into(),
            updated_at: "2026-07-27T00:00:00Z".into(),
            revision: 1,
        },
    ))
    .expect("canonical")
    .expect("payload");
    let encoded = serde_json::to_string(&payload).expect("serialize");
    assert_eq!(payload["isSecret"], true);
    for forbidden in ["value", "secretValue", "id", "workspaceId", "revision"] {
        assert!(payload.get(forbidden).is_none(), "unexpected {forbidden}");
    }
    assert!(!encoded.contains("variable-1"));
}

#[test]
fn protocol_parent_rules_are_strict() {
    assert!(validate_parent("w", SyncEntityType::Workspace, "w", None).is_ok());
    assert!(validate_parent("w", SyncEntityType::Connection, "c", None).is_ok());
    assert!(validate_parent("w", SyncEntityType::Connection, "c", Some("w")).is_err());
    assert!(validate_parent("w", SyncEntityType::WorkspaceVariable, "v", Some("w")).is_ok());
    assert!(validate_parent("w", SyncEntityType::WorkspaceEnvironment, "e", Some("w")).is_ok());
    assert!(validate_parent(
        "w",
        SyncEntityType::WorkspaceEnvironmentVariable,
        "ev",
        Some("e")
    )
    .is_ok());
    assert!(validate_parent("w", SyncEntityType::WorkspaceVariable, "v", None).is_err());
}

#[test]
fn connection_snapshots_use_a_strict_device_local_safe_allowlist() {
    let ssh_payload = canonical_payload(DomainSnapshot::Connection(ConnectionSnapshot {
        id: "ssh-1".into(),
        workspace_id: "workspace-1".into(),
        connection_type: "ssh".into(),
        name: "Production SSH".into(),
        host: Some("ssh.example.test".into()),
        port: Some(22),
        config: ConnectionSnapshotConfig::Ssh {
            username: "deploy".into(),
            auth_method: "private-key".into(),
        },
        created_at: "2026-08-21T00:00:00Z".into(),
        updated_at: "2026-08-21T01:00:00Z".into(),
        revision: 7,
    }))
    .unwrap()
    .unwrap();
    assert_eq!(
        ssh_payload,
        serde_json::json!({
            "id": "ssh-1",
            "workspaceId": "workspace-1",
            "connectionType": "ssh",
            "name": "Production SSH",
            "host": "ssh.example.test",
            "port": 22,
            "config": {
                "kind": "ssh",
                "username": "deploy",
                "authMethod": "private-key"
            },
            "createdAt": "2026-08-21T00:00:00Z",
            "updatedAt": "2026-08-21T01:00:00Z"
        })
    );

    let database_payload = canonical_payload(DomainSnapshot::Connection(ConnectionSnapshot {
        id: "database-1".into(),
        workspace_id: "workspace-1".into(),
        connection_type: "database".into(),
        name: "Local SQLite".into(),
        host: None,
        port: None,
        config: ConnectionSnapshotConfig::Database {
            driver: "sqlite".into(),
            database_name: None,
            username: None,
            ssl_mode: None,
            read_only: true,
        },
        created_at: "2026-08-21T00:00:00Z".into(),
        updated_at: "2026-08-21T01:00:00Z".into(),
        revision: 3,
    }))
    .unwrap()
    .unwrap();
    assert_eq!(database_payload["config"]["kind"], "database");
    assert_eq!(database_payload["config"]["driver"], "sqlite");
    let serialized = format!("{ssh_payload}{database_payload}");
    for forbidden in [
        "credentialRef",
        "password",
        "passphrase",
        "secret",
        "privateKey",
        "keyPath",
        "sqlitePath",
        "lastConnectedAt",
        "syncStatus",
        "remoteId",
        "revision",
    ] {
        assert!(!serialized.contains(forbidden), "leaked {forbidden}");
    }

    let change = RemoteChange {
        cursor: 1,
        operation_id: "remote-connection".into(),
        entity_type: SyncEntityType::Connection,
        entity_id: "ssh-1".into(),
        parent_entity_id: None,
        operation: SyncOperation::Upsert,
        server_version: 1,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: Some(ssh_payload),
        deleted_at: None,
    };
    let page = parse_remote_change("workspace-1", &change).unwrap();
    assert!(matches!(
        &page.connections[0],
        ExternalConnectionApply::Upsert(record)
            if record.id == "ssh-1" && record.workspace_id == "workspace-1"
    ));

    let mut mismatched = change;
    mismatched.payload.as_mut().unwrap()["workspaceId"] = serde_json::json!("other");
    assert_eq!(
        parse_remote_change("workspace-1", &mismatched).unwrap_err(),
        SyncError::InvalidData
    );
}

#[test]
fn api_snapshots_map_to_strict_canonical_payloads_and_parents() {
    let collection =
        canonical_snapshot_intent(DomainSnapshot::ApiCollection(ApiCollectionSnapshot {
            id: "collection-1".into(),
            workspace_id: "workspace-1".into(),
            name: "Accounts".into(),
            description: Some("Account API".into()),
            created_at: "2026-08-13T00:00:00Z".into(),
            updated_at: "2026-08-13T01:00:00Z".into(),
            revision: 4,
        }))
        .expect("collection intent");
    assert_eq!(collection.intent.entity_type.as_str(), "apiCollection");
    assert_eq!(
        collection.intent.parent_entity_id.as_deref(),
        Some("workspace-1")
    );
    let collection_payload: Value =
        serde_json::from_str(collection.intent.payload_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        collection_payload,
        serde_json::json!({
            "name": "Accounts",
            "description": "Account API",
            "createdAt": "2026-08-13T00:00:00Z",
            "updatedAt": "2026-08-13T01:00:00Z"
        })
    );

    let folder = canonical_snapshot_intent(DomainSnapshot::ApiFolder(ApiFolderSnapshot {
        id: "folder-1".into(),
        workspace_id: "workspace-1".into(),
        collection_id: "collection-1".into(),
        parent_folder_id: None,
        name: "Root".into(),
        sort_order: 2,
        created_at: "2026-08-13T00:00:00Z".into(),
        updated_at: "2026-08-13T01:00:00Z".into(),
        revision: 3,
    }))
    .expect("folder intent");
    assert_eq!(folder.intent.entity_type.as_str(), "apiFolder");
    assert_eq!(
        folder.intent.parent_entity_id.as_deref(),
        Some("collection-1")
    );

    let request = canonical_snapshot_intent(DomainSnapshot::ApiRequest(ApiRequestSnapshot {
        id: "request-1".into(),
        workspace_id: "workspace-1".into(),
        collection_id: "collection-1".into(),
        parent_folder_id: Some("folder-1".into()),
        name: "List accounts".into(),
        sort_order: 7,
        auth_json: r#"{"type":"bearer","token":"<redacted>"}"#.into(),
        method: "GET".into(),
        url: "https://example.test/accounts?token=%3Credacted%3E".into(),
        headers: vec![KeyValue {
            key: "Authorization".into(),
            value: "<redacted>".into(),
            enabled: true,
        }],
        query: vec![KeyValue {
            key: "api_key".into(),
            value: "<redacted>".into(),
            enabled: true,
        }],
        body: Some(r#"{"token":"<redacted>"}"#.into()),
        body_kind: "json".into(),
        pre_request_script: Some("console.log('pre')".into()),
        post_response_script: Some("console.log('post')".into()),
        script_schema_version: 1,
        settings_json: r#"{"timeoutMs":null}"#.into(),
        created_at: "2026-08-13T00:00:00Z".into(),
        updated_at: "2026-08-13T01:00:00Z".into(),
        revision: 9,
    }))
    .expect("request intent");
    assert_eq!(request.intent.entity_type.as_str(), "apiRequest");
    assert_eq!(request.intent.parent_entity_id.as_deref(), Some("folder-1"));
    let payload: Value =
        serde_json::from_str(request.intent.payload_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        payload["settingsJson"],
        serde_json::json!(r#"{"timeoutMs":null}"#)
    );
    let expected = [
        "collectionId",
        "parentFolderId",
        "name",
        "sortOrder",
        "authJson",
        "method",
        "url",
        "headers",
        "query",
        "body",
        "bodyKind",
        "preRequestScript",
        "postResponseScript",
        "scriptSchemaVersion",
        "settingsJson",
        "createdAt",
        "updatedAt",
    ];
    for expected in expected.iter() {
        assert!(payload.get(*expected).is_some(), "missing {expected}");
    }
    assert_eq!(payload.as_object().unwrap().len(), expected.len());
    for forbidden in [
        "id",
        "workspaceId",
        "revision",
        "syncStatus",
        "remoteId",
        "timeoutMs",
        "temporaryVariables",
    ] {
        assert!(payload.get(forbidden).is_none(), "unexpected {forbidden}");
    }
}

#[test]
fn api_request_canonical_payload_preserves_all_core_timeout_states() {
    for (timeout_ms, expected_timeout) in [
        (None, serde_json::json!(null)),
        (Some(0), serde_json::json!(0)),
        (Some(30_000), serde_json::json!(30_000)),
        (
            Some(MAX_API_TIMEOUT_MS),
            serde_json::json!(MAX_API_TIMEOUT_MS),
        ),
    ] {
        let settings_json = serde_json::to_string(&ApiRequestSettings { timeout_ms }).unwrap();
        let payload = canonical_payload(DomainSnapshot::ApiRequest(ApiRequestSnapshot {
            id: "request-timeout".into(),
            workspace_id: "workspace-1".into(),
            collection_id: "collection-1".into(),
            parent_folder_id: None,
            name: "Timeout request".into(),
            sort_order: 0,
            auth_json: "{}".into(),
            method: "GET".into(),
            url: "https://example.test/timeout".into(),
            headers: Vec::new(),
            query: Vec::new(),
            body: None,
            body_kind: "none".into(),
            settings_json,
            pre_request_script: None,
            post_response_script: None,
            script_schema_version: 1,
            created_at: "2026-08-13T00:00:00Z".into(),
            updated_at: "2026-08-13T00:00:00Z".into(),
            revision: 1,
        }))
        .unwrap()
        .unwrap();
        let settings: Value =
            serde_json::from_str(payload["settingsJson"].as_str().unwrap()).unwrap();
        assert_eq!(settings["timeoutMs"], expected_timeout);
    }
}

#[test]
fn api_remote_changes_decode_to_external_apply_pages_and_tombstones() {
    let folder = RemoteChange {
        cursor: 1,
        operation_id: "remote-folder".into(),
        entity_type: SyncEntityType::ApiFolder,
        entity_id: "folder-1".into(),
        parent_entity_id: Some("collection-1".into()),
        operation: SyncOperation::Upsert,
        server_version: 1,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: Some(serde_json::json!({
            "collectionId": "collection-1",
            "parentFolderId": null,
            "name": "Root",
            "sortOrder": 0,
            "createdAt": "2026-08-13T00:00:00Z",
            "updatedAt": "2026-08-13T00:00:00Z"
        })),
        deleted_at: None,
    };
    let page = parse_remote_change("workspace-1", &folder).expect("folder page");
    assert_eq!(page.api_folders.len(), 1);

    for (index, settings_json) in [r#"{"timeoutMs":null}"#, r#"{"timeoutMs":30000}"#]
        .iter()
        .enumerate()
    {
        let request = RemoteChange {
            cursor: 2,
            operation_id: format!("remote-request-{index}"),
            entity_type: SyncEntityType::ApiRequest,
            entity_id: "request-1".into(),
            parent_entity_id: Some("folder-1".into()),
            operation: SyncOperation::Upsert,
            server_version: 1,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload: Some(serde_json::json!({
                "collectionId": "collection-1",
                "parentFolderId": "folder-1",
                "name": "List accounts",
                "sortOrder": 0,
                "authJson": "{}",
                "method": "GET",
                "url": "https://example.test/accounts",
                "headers": [],
                "query": [],
                "body": null,
                "bodyKind": "none",
                "settingsJson": settings_json,
                "preRequestScript": null,
                "postResponseScript": null,
                "scriptSchemaVersion": 1,
                "createdAt": "2026-08-13T00:00:00Z",
                "updatedAt": "2026-08-13T00:00:00Z"
            })),
            deleted_at: None,
        };
        let page = parse_remote_change("workspace-1", &request).expect("request page");
        let ExternalApiRequestApply::Upsert(record) = &page.api_requests[0] else {
            panic!("expected API request upsert");
        };
        assert_eq!(record.settings_json, *settings_json);
    }

    let legacy_request = RemoteChange {
        cursor: 3,
        operation_id: "legacy-request".into(),
        entity_type: SyncEntityType::ApiRequest,
        entity_id: "request-legacy".into(),
        parent_entity_id: Some("folder-1".into()),
        operation: SyncOperation::Upsert,
        server_version: 1,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: Some(serde_json::json!({
            "collectionId": "collection-1",
            "parentFolderId": "folder-1",
            "name": "Legacy request",
            "sortOrder": 0,
            "authJson": "{}",
            "method": "GET",
            "url": "https://example.test/legacy",
            "headers": [],
            "query": [],
            "body": null,
            "bodyKind": "none",
            "preRequestScript": null,
            "postResponseScript": null,
            "scriptSchemaVersion": 1,
            "createdAt": "2026-08-13T00:00:00Z",
            "updatedAt": "2026-08-13T00:00:00Z"
        })),
        deleted_at: None,
    };
    let legacy_page = parse_remote_change("workspace-1", &legacy_request).unwrap();
    let ExternalApiRequestApply::Upsert(record) = &legacy_page.api_requests[0] else {
        panic!("expected legacy API request upsert");
    };
    assert_eq!(record.settings_json, r#"{"timeoutMs":null}"#);

    let delete = canonical_snapshot_intent(DomainSnapshot::Tombstone(TombstoneSnapshot {
        entity: DomainEntityKey::new(DomainEntityType::ApiRequest, "workspace-1", "request-1")
            .with_parent_entity_id("folder-1"),
        deleted_at: "2026-08-13T02:00:00Z".into(),
        revision: 10,
    }))
    .expect("delete intent");
    assert_eq!(delete.intent.operation, SyncOperation::Delete);
    assert!(delete.intent.payload_json.is_none());
    assert_eq!(
        delete.intent.deleted_at.as_deref(),
        Some("2026-08-13T02:00:00Z")
    );
}

#[test]
fn ssh_task_snapshots_are_intrinsic_and_remote_pages_enforce_the_task_parent() {
    let task = canonical_snapshot_intent(DomainSnapshot::SshTask(SshTaskSnapshot {
        id: "task-1".into(),
        workspace_id: "workspace-1".into(),
        name: "Deploy".into(),
        description: "Deploy service".into(),
        sort_order: 4,
        created_at: "2026-08-17T00:00:00Z".into(),
        updated_at: "2026-08-17T01:00:00Z".into(),
        revision: 7,
    }))
    .expect("task intent");
    assert_eq!(task.intent.entity_type, SyncEntityType::SshTask);
    assert!(task.intent.parent_entity_id.is_none());
    let task_payload: Value =
        serde_json::from_str(task.intent.payload_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        task_payload,
        serde_json::json!({
            "name": "Deploy",
            "description": "Deploy service",
            "sortOrder": 4,
            "createdAt": "2026-08-17T00:00:00Z",
            "updatedAt": "2026-08-17T01:00:00Z"
        })
    );

    let step = canonical_snapshot_intent(DomainSnapshot::SshTaskStep(SshTaskStepSnapshot {
        id: "step-1".into(),
        workspace_id: "workspace-1".into(),
        task_id: "task-1".into(),
        name: "Upload".into(),
        step_type: "upload".into(),
        position: 0,
        enabled: true,
        config_version: 1,
        config_json: serde_json::json!({
            "localPath": "{{local_path_step-1}}",
            "remotePath": "/tmp/app.tar",
            "overwrite": true
        }),
        created_at: "2026-08-17T00:00:00Z".into(),
        updated_at: "2026-08-17T01:00:00Z".into(),
        revision: 8,
    }))
    .expect("step intent");
    assert_eq!(step.intent.entity_type, SyncEntityType::SshTaskStep);
    assert_eq!(step.intent.parent_entity_id.as_deref(), Some("task-1"));
    let step_payload: Value =
        serde_json::from_str(step.intent.payload_json.as_deref().unwrap()).unwrap();
    for field in [
        "taskId",
        "name",
        "stepType",
        "position",
        "enabled",
        "configVersion",
        "configJson",
        "createdAt",
        "updatedAt",
    ] {
        assert!(step_payload.get(field).is_some(), "missing {field}");
    }
    for forbidden in [
        "id",
        "workspaceId",
        "revision",
        "connectionId",
        "credentialRef",
        "localBinding",
        "run",
        "log",
    ] {
        assert!(
            step_payload.get(forbidden).is_none(),
            "unexpected {forbidden}"
        );
    }

    let remote_step = RemoteChange {
        cursor: 1,
        operation_id: "remote-step".into(),
        entity_type: SyncEntityType::SshTaskStep,
        entity_id: "step-1".into(),
        parent_entity_id: Some("task-1".into()),
        operation: SyncOperation::Upsert,
        server_version: 1,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: Some(step_payload.clone()),
        deleted_at: None,
    };
    let page = parse_remote_change("workspace-1", &remote_step).expect("SSH step page");
    assert_eq!(page.ssh_task_steps.len(), 1);

    let mut mismatched = remote_step;
    mismatched.parent_entity_id = Some("other-task".into());
    assert_eq!(
        parse_remote_change("workspace-1", &mismatched).unwrap_err(),
        SyncError::InvalidData
    );

    let task_delete = RemoteChange {
        cursor: 2,
        operation_id: "delete-task".into(),
        entity_type: SyncEntityType::SshTask,
        entity_id: "task-1".into(),
        parent_entity_id: None,
        operation: SyncOperation::Delete,
        server_version: 2,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: None,
        deleted_at: Some("2026-08-17T02:00:00Z".into()),
    };
    assert_eq!(
        parse_remote_change("workspace-1", &task_delete)
            .unwrap()
            .ssh_tasks
            .len(),
        1
    );
}
