use super::*;

async fn fixture() -> (SshService, String) {
    let db = LocalDb::connect_ephemeral().await.unwrap();
    db.migrate().await.unwrap();
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO workspaces (id,name,is_default,created_at,updated_at,revision,sync_status) VALUES (?,'fixture',0,'now','now',1,'local')")
        .bind(&id).execute(db.pool()).await.unwrap();
    (
        SshService::new(db, SecretStore::in_memory("temporary-test")),
        id,
    )
}
fn request() -> TemporaryRequest {
    TemporaryRequest::password(
        "synthetic.invalid".into(),
        2222,
        "SECRET_USER_SENTINEL".into(),
        "SECRET_PASSWORD_SENTINEL".into(),
    )
    .unwrap()
}

#[cfg(feature = "ssh-native")]
#[tokio::test]
async fn stalled_key_exchange_closes_socket_on_cancel_and_transport_timeout() {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    let (service, workspace) = fixture().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    for cancel in [true, false] {
        let request = TemporaryRequest::password(
            "127.0.0.1".into(),
            listener.local_addr().unwrap().port(),
            "user".into(),
            "grant".into(),
        )
        .unwrap();
        let handle = service
            .receive_temporary(&LaunchReceipt::default(), request)
            .unwrap();
        service.claim_temporary(&workspace, &handle).await.unwrap();
        service.start_temporary(&workspace, &handle).unwrap();
        let mut peer = tokio::time::timeout(Duration::from_secs(5), async {
            let (socket, _) = listener.accept().await.unwrap();
            let mut peer = BufReader::new(socket);
            let mut banner = String::new();
            peer.read_line(&mut banner).await.unwrap();
            assert!(banner.starts_with("SSH-2.0-"));
            peer.get_mut()
                .write_all(b"SSH-2.0-stalled\r\n")
                .await
                .unwrap();
            peer.read_exact(&mut [0; 5]).await.unwrap();
            peer
        })
        .await
        .unwrap();
        if cancel {
            service.cancel_temporary(&workspace, &handle).unwrap();
        } else {
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(31)).await;
            tokio::time::resume();
        }
        let closed =
            tokio::time::timeout(Duration::from_secs(3), peer.read_to_end(&mut Vec::new()))
                .await
                .expect("stalled key exchange kept the client's TCP socket open");
        if let Err(error) = closed {
            assert!(matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
            ));
        }
        let entries = service.list_temporary(&workspace).unwrap();
        if cancel {
            assert!(entries.is_empty());
        } else {
            assert_eq!(
                entries[0].error_code.as_deref(),
                Some("TEMPORARY_TRANSPORT_TIMEOUT")
            );
        }
    }
}

#[cfg(feature = "ssh-native")]
#[test]
fn sync_adapter_without_runtime_rejects_before_consuming() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (service, workspace, handle) = runtime.block_on(async {
        let (service, workspace) = fixture().await;
        let handle = service
            .receive_temporary(&LaunchReceipt::default(), request())
            .unwrap();
        service.claim_temporary(&workspace, &handle).await.unwrap();
        (service, workspace, handle)
    });
    assert!(service.start_temporary(&workspace, &handle).is_err());
    assert_eq!(
        service.list_temporary(&workspace).unwrap()[0].status,
        "claimed"
    );
}

#[tokio::test]
async fn cold_warm_delayed_readiness_redelivery_and_new_launch() {
    let (service, workspace) = fixture().await;
    let receipt = LaunchReceipt::default();
    let first = service.receive_temporary(&receipt, request()).unwrap();
    // Cold delivery is retained before any subscriber. A cloned service is
    // the already-running instance adapter, not a fresh queue.
    let warm = service.clone();
    assert_eq!(warm.receive_temporary(&receipt, request()).unwrap(), first);
    for _ in 0..3 {
        assert_eq!(warm.list_temporary(&workspace).unwrap().len(), 1);
    }
    let again = warm
        .receive_temporary(&LaunchReceipt::default(), request())
        .unwrap();
    assert_ne!(first, again);
    let (a, b) = tokio::join!(
        service.claim_temporary(&workspace, &first),
        warm.claim_temporary(&workspace, &first)
    );
    assert_eq!(a.unwrap().handle, b.unwrap().handle);
    service.cancel_temporary(&workspace, &first).unwrap();
    assert_eq!(
        service.receive_temporary(&receipt, request()).unwrap(),
        first
    );
    assert!(service.claim_temporary(&workspace, &first).await.is_err());
    assert_eq!(service.list_temporary(&workspace).unwrap().len(), 1);
}

#[tokio::test]
async fn isolation_safe_summary_and_zero_persistence() {
    let (service, workspace) = fixture().await;
    let before: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(service.db.pool())
        .await
        .unwrap();
    let handle = service
        .receive_temporary(&LaunchReceipt::default(), request())
        .unwrap();
    assert!(service.claim_temporary("missing", &handle).await.is_err());
    service.claim_temporary(&workspace, &handle).await.unwrap();
    assert!(service.list_temporary("other").unwrap().is_empty());
    assert!(service.cancel_temporary("other", &handle).is_err());
    assert!(service.temporary_output("other", &handle).is_err());
    let summary = serde_json::to_string(&service.list_temporary(&workspace).unwrap()).unwrap();
    let debug = format!("{:?}", service.list_temporary(&workspace).unwrap());
    for secret in ["SECRET_USER_SENTINEL", "SECRET_PASSWORD_SENTINEL"] {
        assert!(!summary.contains(secret));
        assert!(!debug.contains(secret));
    }
    assert!(summary.contains("synthetic.invalid:2222"));
    service.close_temporary_workspace(&workspace).unwrap();
    assert!(service.list_temporary(&workspace).unwrap().is_empty());
    assert!(service
        .list_connections(workspace.clone())
        .await
        .unwrap()
        .is_empty());
    assert!(service.list_sessions(workspace).await.unwrap().is_empty());
    let after: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(service.db.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn display_metadata_is_bounded_and_never_uses_login_material() {
    let (service, workspace) = fixture().await;
    for (label, expected) in [
        ("  生产节点  ".to_owned(), "生产节点".to_owned()),
        (
            "node\u{202e}\u{200b}\nname".to_owned(),
            "nodename".to_owned(),
        ),
        ("节点".repeat(100), format!("{}…", "节点".repeat(80))),
        (
            "SECRET_USER_SENTINEL@node".to_owned(),
            "synthetic.invalid:2222".to_owned(),
        ),
        (
            "SECRET_PASSWORD_SENTINEL".to_owned(),
            "synthetic.invalid:2222".to_owned(),
        ),
        (
            "SECRET_PASS\u{200b}WORD_SENTINEL".to_owned(),
            "synthetic.invalid:2222".to_owned(),
        ),
        (
            "ssh://some:credentials@node".to_owned(),
            "synthetic.invalid:2222".to_owned(),
        ),
        ("   ".to_owned(), "synthetic.invalid:2222".to_owned()),
    ] {
        let handle = service
            .receive_temporary(
                &LaunchReceipt::default(),
                request().with_display_name(&label),
            )
            .unwrap();
        let summary = service
            .list_temporary(&workspace)
            .unwrap()
            .into_iter()
            .find(|entry| entry.handle == handle)
            .unwrap();
        assert_eq!(summary.display_name, expected);
        assert_eq!(summary.endpoint, "synthetic.invalid:2222");
        service.cancel_temporary(&workspace, &handle).unwrap();
    }
}

#[tokio::test]
async fn bounds_expiry_and_late_receipt_rejection() {
    let (service, workspace) = fixture().await;
    for _ in 0..MAX_REQUESTS {
        service
            .receive_temporary(&LaunchReceipt::default(), request())
            .unwrap();
    }
    assert!(service
        .receive_temporary(&LaunchReceipt::default(), request())
        .is_err());
    service
        .temporary
        .0
        .lock()
        .unwrap()
        .sweep(Instant::now() + RETENTION + Duration::from_secs(1));
    assert!(service.list_temporary(&workspace).unwrap().is_empty());
    let mut expired = LaunchReceipt::default();
    expired.deadline = Instant::now() - Duration::from_secs(1);
    assert!(service.receive_temporary(&expired, request()).is_err());
    assert!(TemporaryRequest::password("x".into(), 22, "u".into(), "x".repeat(65536)).is_err());
    for _ in 0..MAX_RECEIPTS {
        let handle = service
            .receive_temporary(&LaunchReceipt::default(), request())
            .unwrap();
        service.cancel_temporary(&workspace, &handle).unwrap();
    }
    assert!(service
        .receive_temporary(&LaunchReceipt::default(), request())
        .is_err());
}

#[tokio::test]
async fn expiry_runs_without_ui_and_releases_credentials() {
    let (service, workspace) = fixture().await;
    tokio::time::pause();
    service
        .receive_temporary(&LaunchReceipt::default(), request())
        .unwrap();
    tokio::task::yield_now().await;
    tokio::time::advance(RETENTION + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(service.temporary.0.lock().unwrap().entries.is_empty());
    assert!(service.list_temporary(&workspace).unwrap().is_empty());
}

#[tokio::test]
async fn workspace_close_invalidates_claim_waiting_for_database() {
    use std::{future::Future, task::Poll};
    let (service, workspace) = fixture().await;
    let handle = service
        .receive_temporary(&LaunchReceipt::default(), request())
        .unwrap();
    let connection = service.db.pool().acquire().await.unwrap();
    let claim = service.claim_temporary(&workspace, &handle);
    tokio::pin!(claim);
    let first = std::future::poll_fn(|cx| Poll::Ready(claim.as_mut().poll(cx))).await;
    assert!(matches!(first, Poll::Pending));
    service.close_temporary_workspace(&workspace).unwrap();
    drop(connection);
    assert!(claim.await.is_err());
    assert_eq!(
        service.list_temporary(&workspace).unwrap()[0].workspace_id,
        None
    );
}

#[cfg(feature = "ssh-native")]
#[tokio::test]
async fn cancel_before_setup_is_polled_does_not_resurrect() {
    let (service, workspace) = fixture().await;
    let handle = service
        .receive_temporary(&LaunchReceipt::default(), request())
        .unwrap();
    service.claim_temporary(&workspace, &handle).await.unwrap();
    service.start_temporary(&workspace, &handle).unwrap();
    assert!(service.start_temporary(&workspace, &handle).is_err());
    service.cancel_temporary(&workspace, &handle).unwrap();
    tokio::task::yield_now().await;
    assert!(service.list_temporary(&workspace).unwrap().is_empty());
}

#[cfg(feature = "ssh-native")]
mod trust_tests;
