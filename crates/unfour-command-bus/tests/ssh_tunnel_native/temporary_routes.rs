use super::*;
use unfour_command_bus::{LaunchReceipt, TemporaryRequest};
use unfour_core::models::DatabaseConnectionInput;

#[tokio::test]
async fn editing_a_sql_route_during_a_draft_test_stops_before_database_login() {
    let destination = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gate = Arc::new(ForwardGate::default());
    let f = fixture_with_gate(
        vec![("draft.invalid", 5432, destination.local_addr().unwrap())],
        Some(gate.clone()),
    )
    .await;
    let input: DatabaseConnectionInput = serde_json::from_value(serde_json::json!({
        "workspaceId":f.workspace, "name":"Reporting", "driver":"postgres", "host":"saved.invalid",
        "port":5432, "database":"app","username":"reader","sshConnectionId":f.jump
    }))
    .unwrap();
    let saved = f
        .bus
        .save_database_connection_with_secret(input.clone(), Some("test-password".into()))
        .await
        .unwrap();
    let mut draft = input;
    draft.id = Some(saved.id.clone());
    draft.host = Some("draft.invalid".into());
    draft.credential_ref = saved.credential_ref;
    let bus = f.bus.clone();
    let request =
        tokio::spawn(async move { bus.test_database_connection_input(draft, None).await });
    tokio::time::timeout(Duration::from_secs(5), gate.opened.notified())
        .await
        .unwrap();
    sqlx::query("UPDATE database_connections SET ssh_connection_id=NULL, ssh_revision=NULL WHERE connection_id=?")
        .bind(&saved.id).execute(f.db.pool()).await.unwrap();
    gate.release.notify_one();
    let err = request.await.unwrap().unwrap_err();
    assert_eq!(err.code(), "SSH_TUNNEL_ROUTE_CHANGED");
    let (mut stream, _) = destination.accept().await.unwrap();
    let mut byte = [0];
    let received = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut byte))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received, 0, "SQL login began after the saved route changed");
}

async fn start_temporary(f: &Fixture) -> String {
    let handle = f
        .bus
        .receive_temporary_ssh(
            &LaunchReceipt::default(),
            TemporaryRequest::password(
                "127.0.0.1".into(),
                f.port,
                "fixture".into(),
                "temporary".into(),
            )
            .unwrap(),
        )
        .unwrap();
    f.bus
        .claim_temporary_ssh(&f.workspace, &handle)
        .await
        .unwrap();
    f.bus.start_temporary_ssh(&f.workspace, &handle).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let entry = f.bus.list_temporary_ssh(&f.workspace).unwrap().remove(0);
            match entry.status.as_str() {
                "connected" => break,
                "connecting" => tokio::time::sleep(Duration::from_millis(10)).await,
                status => panic!("unexpected fixture state {status} {:?}", entry.error_code),
            }
        }
    })
    .await
    .unwrap();
    handle
}

#[tokio::test]
async fn temporary_http_and_redis_use_the_connected_parent_and_fail_closed_after_close() {
    let (http, observed) =
        http_origin("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK").await;
    let redis = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let f = fixture(vec![
        ("http.invalid", 80, http),
        ("redis.invalid", 6379, redis.local_addr().unwrap()),
    ])
    .await;
    let handle = start_temporary(&f).await;
    let api: ApiRequestInput = serde_json::from_value(serde_json::json!({
        "workspaceId":f.workspace, "method":"GET", "url":"http://http.invalid/path", "headers":[], "query":[], "bodyKind":"none", "timeoutMs":3000
    })).unwrap();
    let res = f
        .bus
        .send_api_request_via("live-http", api.clone(), None, None, None, Some(&handle))
        .await
        .unwrap();
    assert_eq!(res.response.unwrap().body, "OK");
    assert!(observed
        .await
        .unwrap()
        .to_ascii_lowercase()
        .contains("host: http.invalid"));
    let input: RedisConnectionInput = serde_json::from_value(serde_json::json!({
        "workspaceId": f.workspace, "host":"redis.invalid", "name":"Cache", "dbIndex":0, "port":6379, "tls":false
    })).unwrap();
    let saved = f
        .bus
        .save_redis_connection(input, Some("stored-secret".into()))
        .await
        .unwrap();
    let target = serde_json::from_value(serde_json::json!({
        "id":saved.id, "expectedRevision":saved.revision, "workspaceId":f.workspace,
        "host":"redis.invalid", "name":"Cache", "dbIndex":0, "port":6379, "tls":false
    }))
    .unwrap();
    let via = f
        .bus
        .with_redis_temporary_route(&f.workspace, Some(&handle), None)
        .unwrap();
    let err = via
        .test_redis_connection_input(target, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "REDIS_ENDPOINT_REQUIRES_PASSWORD");
    sqlx::query("UPDATE redis_connections SET route_review_generation=1 WHERE connection_id=?")
        .bind(&saved.id)
        .execute(f.db.pool())
        .await
        .unwrap();
    // The stale saved jump stays blocked; an explicitly selected live
    // session uses the newly supplied Redis target password.
    let peer = tokio::spawn(async move {
        let (mut s, _) = redis.accept().await.unwrap();
        for (req, reply) in [
            ("*2\r\n$4\r\nAUTH\r\n$6\r\ninline\r\n", "+OK\r\n"),
            ("*2\r\n$4\r\nTYPE\r\n$1\r\nk\r\n", "+none\r\n"),
        ] {
            let mut actual = vec![0; req.len()];
            s.read_exact(&mut actual).await.unwrap();
            assert_eq!(actual, req.as_bytes());
            s.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let via = f
        .bus
        .with_redis_temporary_route(&f.workspace, Some(&handle), Some("inline".into()))
        .unwrap();
    let result = via
        .check_redis_key(&f.workspace, &saved.id, b"k", None, None)
        .await
        .unwrap();
    assert!(!result.exists);
    peer.await.unwrap();
    assert!(f
        .bus
        .with_redis_temporary_route("another-workspace", Some(&handle), None)
        .is_err());
    f.bus.cancel_temporary_ssh(&f.workspace, &handle).unwrap();
    let err = f
        .bus
        .send_api_request_via("closed-http", api, None, None, None, Some(&handle))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "SSH_TUNNEL_TEMPORARY_UNAVAILABLE");
    assert_eq!(
        via.check_redis_key(&f.workspace, &saved.id, b"k", None, None)
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_TEMPORARY_UNAVAILABLE"
    );
}

#[tokio::test]
async fn sql_saved_and_temporary_jumps_carry_postgres_tls_negotiation() {
    let backend = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let f = fixture(vec![(
        "postgres.invalid",
        5432,
        backend.local_addr().unwrap(),
    )])
    .await;
    let handle = start_temporary(&f).await;
    let input: DatabaseConnectionInput = serde_json::from_value(serde_json::json!({
        "workspaceId":f.workspace,"name":"Reporting","driver":"postgres","host":"postgres.invalid",
        "port":5432,"database":"reports","username":"app","sslMode":"require","sshConnectionId":f.jump
    })).unwrap();
    let saved = f.bus.save_database_connection(input).await.unwrap();
    assert_eq!(saved.ssh_connection_id.as_deref(), Some(f.jump.as_str()));
    let peer = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut stream, _) = backend.accept().await.unwrap();
            assert_eq!(stream.read_u32().await.unwrap(), 8);
            assert_eq!(stream.read_u32().await.unwrap(), 80877103);
            stream.write_all(b"N").await.unwrap();
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte).await.unwrap(),
                0,
                "require must refuse plaintext"
            );
        }
    });
    for route in [None, Some(handle.as_str())] {
        let bus = f.bus.with_database_route(route, None);
        assert!(bus
            .test_database_connection(f.workspace.clone(), saved.id.clone())
            .await
            .is_err());
    }
    tokio::time::timeout(Duration::from_secs(4), peer)
        .await
        .unwrap()
        .unwrap();
    f.bus.cancel_temporary_ssh(&f.workspace, &handle).unwrap();
    let err = f
        .bus
        .with_database_route(Some(&handle), None)
        .test_database_connection(f.workspace.clone(), saved.id.clone())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "SSH_TUNNEL_TEMPORARY_UNAVAILABLE");
}

#[tokio::test]
async fn stale_window_cannot_confirm_sql_or_apply_table_edits_after_route_changes() {
    let f = fixture(vec![]).await;
    let input: DatabaseConnectionInput = serde_json::from_value(serde_json::json!({
        "workspaceId":f.workspace, "name":"Warehouse", "driver":"postgres",
        "host":"pg.invalid", "username":"tester", "database":"app"
    }))
    .unwrap();
    let direct = f.bus.save_database_connection(input.clone()).await.unwrap();
    let viewed = unfour_core::models::DatabaseConnectionVersion {
        revision: direct.revision,
        ssh_connection_id: direct.ssh_connection_id.clone(),
        ssh_revision: direct.ssh_revision,
    };
    let stale_window = f.bus.with_database_route(None, Some(viewed));
    let routed = f
        .bus
        .save_database_connection(DatabaseConnectionInput {
            id: Some(direct.id.clone()),
            ssh_connection_id: Some(Some(f.jump.clone())),
            ..input
        })
        .await
        .unwrap();
    assert_eq!(
        routed.revision, direct.revision,
        "the route edit is device-local"
    );
    let batch = serde_json::from_value(serde_json::json!({
        "workspaceId":f.workspace,"connectionId":direct.id,"sql":"UPDATE items SET value = 1 WHERE id = 1",
        "runId":"stale-confirmation","confirmMutation":true
    })).unwrap();
    assert_eq!(
        stale_window
            .execute_database_script(batch)
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
    let edit = serde_json::from_value(serde_json::json!({
        "workspaceId":f.workspace,"connectionId":direct.id,"tableName":"items","operation":"delete",
        "primaryKey":[{"column":"id","value":"1"}],"confirmMutation":true
    }))
    .unwrap();
    assert_eq!(
        stale_window
            .mutate_database_row(edit)
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
}
