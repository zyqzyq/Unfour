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
        allow_writes: None,
        ssh_connection_id: None,
        expected_ssh_revision: None,
        username: None,
        expected_revision: None,
    }
}

#[tokio::test]
async fn redis_route_retarget_revokes_write_access_and_rechecks_stored_password() {
    let bus = test_bus().await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let jump = bus
        .save_ssh_connection(unfour_core::models::SshConnectionInput {
            id: None,
            workspace_id: ws.clone(),
            name: "Jump".into(),
            host: "original.invalid".into(),
            port: Some(22),
            username: "reader".into(),
            auth_kind: "none".into(),
            key_path: None,
            credential_ref: None,
            secret: None,
        })
        .await
        .unwrap();
    let other = bus.create_workspace("Other".into()).await.unwrap().id;
    let mut edit = draft(other);
    edit.ssh_connection_id = Some(Some(jump.id.clone()));
    assert_eq!(
        bus.save_redis_connection(edit.clone(), None)
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_MISSING"
    );
    edit.workspace_id = ws.clone();
    edit.expected_ssh_revision = Some(jump.revision + 1);
    assert_eq!(
        bus.test_redis_connection_input(edit.clone(), Some("new-password".into()))
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
    assert_eq!(
        bus.save_redis_connection(edit.clone(), Some("new-password".into()))
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
    assert!(bus.list_redis_connections(&ws).await.unwrap().is_empty());
    edit.expected_ssh_revision = None;
    edit.allow_writes = Some(true);
    let saved = bus
        .save_redis_connection(edit.clone(), Some("test-password".into()))
        .await
        .unwrap();
    for host in ["second.invalid", "third.invalid"] {
        sqlx::query("UPDATE connections SET host=? WHERE workspace_id=? AND id=?")
            .bind(host)
            .bind(&ws)
            .bind(&jump.id)
            .execute(bus.db.pool())
            .await
            .unwrap();
    }
    let changed = bus.list_redis_connections(&ws).await.unwrap().remove(0);
    assert_eq!(changed.revision, saved.revision + 2);
    assert!(changed.has_password && changed.route_needs_review && !changed.allow_writes);
    assert_eq!(
        bus.inspect_redis_key(&ws, &saved.id, "aw==", None, Some(changed.revision))
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
    edit.id = Some(saved.id.clone());
    edit.expected_revision = Some(changed.revision);
    edit.allow_writes = None;
    let legacy = RedisConnectionInput {
        ssh_connection_id: None,
        ..edit.clone()
    };
    assert_eq!(
        bus.save_redis_connection(legacy, None)
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
    assert_eq!(
        bus.save_redis_connection(edit.clone(), None)
            .await
            .unwrap_err()
            .code(),
        "REDIS_ENDPOINT_REQUIRES_PASSWORD"
    );
    let approved = bus
        .save_redis_connection(edit.clone(), Some("new-password".into()))
        .await
        .unwrap();
    assert!(!approved.route_needs_review && !approved.allow_writes);
    sqlx::query("INSERT INTO ssh_host_keys(workspace_id,host,port,fingerprint,created_at) VALUES(?, 'third.invalid',22,'test-fingerprint','now')")
        .bind(&ws).execute(bus.db.pool()).await.unwrap();
    sqlx::query(
        "DELETE FROM ssh_host_keys WHERE workspace_id=? AND host='third.invalid' AND port=22",
    )
    .bind(&ws)
    .execute(bus.db.pool())
    .await
    .unwrap();
    assert!(bus.list_redis_connections(&ws).await.unwrap()[0].route_needs_review);
}

#[tokio::test]
async fn redis_external_retargeting_revokes_local_write_access() {
    use unfour_core::domain::{
        ConnectionSnapshotConfig, ExternalConnectionApply, ExternalConnectionUpsert,
    };
    let bus = test_bus().await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = draft(ws.clone());
    input.allow_writes = Some(true);
    let saved = bus.save_redis_connection(input, None).await.unwrap();
    let remote = |db_index| {
        ExternalConnectionApply::Upsert(ExternalConnectionUpsert {
            id: saved.id.clone(),
            workspace_id: ws.clone(),
            connection_type: "redis".into(),
            name: "Renamed".into(),
            host: Some(saved.host.clone()),
            port: Some(saved.port),
            config: ConnectionSnapshotConfig::Redis {
                username: None,
                db_index,
                tls: false,
            },
            created_at: "2026-11-13T00:00:00Z".into(),
            updated_at: "2026-11-13T00:01:00Z".into(),
        })
    };
    bus.apply_external_connections(vec![remote(0)])
        .await
        .unwrap();
    assert!(bus.list_redis_connections(&ws).await.unwrap()[0].allow_writes);
    bus.apply_external_connections(vec![remote(1)])
        .await
        .unwrap();
    assert!(!bus.list_redis_connections(&ws).await.unwrap()[0].allow_writes);
}

#[tokio::test]
async fn redis_write_requires_local_permission_confirmation_and_matching_target() {
    let bus = test_bus().await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = draft(ws.clone());
    let readonly = bus
        .save_redis_connection(input.clone(), None)
        .await
        .unwrap();
    let op = || {
        serde_json::from_value::<RedisMutation>(serde_json::json!({
            "kind": "deleteKey", "expectedType": "string"
        }))
        .unwrap()
    };
    assert!(!readonly.allow_writes);
    let denied = bus
        .mutate_redis_key(&ws, &readonly.id, "YQ==", op(), readonly.revision, true)
        .await
        .unwrap_err();
    assert_eq!(denied.code(), "REDIS_WRITE_READ_ONLY");
    input.id = Some(readonly.id.clone());
    input.expected_revision = Some(readonly.revision);
    input.allow_writes = Some(true);
    let writable = bus
        .save_redis_connection(input.clone(), None)
        .await
        .unwrap();
    assert!(writable.allow_writes && writable.revision > readonly.revision);
    assert_eq!(
        bus.mutate_redis_key(&ws, &writable.id, "YQ==", op(), writable.revision, false)
            .await
            .unwrap_err()
            .code(),
        "REDIS_WRITE_CONFIRMATION_REQUIRED"
    );
    assert!(matches!(
        bus.mutate_redis_key(&ws, &writable.id, "YQ==", op(), readonly.revision, true)
            .await,
        Err(AppError::RowConflict(_))
    ));
    let other = bus
        .create_workspace("Other Redis workspace".into())
        .await
        .unwrap()
        .id;
    assert!(matches!(
        bus.mutate_redis_key(&other, &writable.id, "YQ==", op(), writable.revision, true)
            .await,
        Err(AppError::NotFound(_))
    ));

    // An older client can rename a connection without losing the local grant.
    // Pointing it at a new logical database must not carry that grant with it.
    input.expected_revision = Some(writable.revision);
    input.name = "Renamed".into();
    input.allow_writes = None;
    let renamed = bus
        .save_redis_connection(input.clone(), None)
        .await
        .unwrap();
    assert!(renamed.allow_writes);
    input.expected_revision = Some(renamed.revision);
    input.db_index = 1;
    assert!(
        !bus.save_redis_connection(input, None)
            .await
            .unwrap()
            .allow_writes
    );
}

#[tokio::test]
async fn redis_activity_keeps_only_the_operation_and_outcome_when_commit_is_uncertain() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut input = draft(workspace.clone());
    input.port = listener.local_addr().unwrap().port();
    input.allow_writes = Some(true);
    let record = bus.save_redis_connection(input, None).await.unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let info = "redis_mode:standalone\r\nredis_version:8.0.0\r\n";
        let info_reply = format!("${}\r\n{}\r\n", info.len(), info);
        for (args, response) in [
            (vec!["INFO", "SERVER"], info_reply.as_str()),
            (vec!["WATCH", "private:key"], "+OK\r\n"),
            (vec!["TYPE", "private:key"], "+none\r\n"),
            (vec!["MULTI"], "+OK\r\n"),
            (
                vec!["SET", "private:key", "private value", "NX"],
                "+QUEUED\r\n",
            ),
            (vec!["EXEC"], ""),
        ] {
            let mut expected = format!("*{}\r\n", args.len());
            for arg in args {
                expected.push_str(&format!("${}\r\n{arg}\r\n", arg.len()));
            }
            let mut actual = vec![0; expected.len()];
            stream.read_exact(&mut actual).await.unwrap();
            assert_eq!(actual, expected.as_bytes());
            stream.write_all(response.as_bytes()).await.unwrap();
        }
        // Drop the socket after reading EXEC. Its outcome cannot be inferred.
    });
    let failure = bus
        .mutate_redis_key(
            &workspace,
            &record.id,
            &STANDARD.encode("private:key"),
            RedisMutation::CreateString {
                value_base64: STANDARD.encode("private value"),
                ttl_ms: None,
            },
            record.revision,
            true,
        )
        .await
        .unwrap_err();
    server.await.unwrap();
    assert_eq!(failure.code(), "REDIS_WRITE_OUTCOME_UNKNOWN");
    let (target, details): (String, String) = sqlx::query_as(
        "SELECT target, details_json FROM activity_events WHERE workspace_id=? AND action='redis.key.mutate'"
    ).bind(&workspace).fetch_one(bus.db.pool()).await.unwrap();
    assert_eq!(target, record.id);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&details).unwrap(),
        serde_json::json!({"operation":"createString", "outcome":"unknown"})
    );
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
