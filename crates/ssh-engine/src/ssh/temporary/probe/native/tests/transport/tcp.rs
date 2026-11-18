use super::*;
use tokio::io::AsyncReadExt;

const HOST: &str = "tcp.fixture.invalid";
const PORT: u16 = 32123;

async fn wait_closed(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while f.state.lock().unwrap().closed == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn tcp_probe_opens_only_explicit_target_sends_no_payload_and_keeps_pty() {
    let f = fixture().await;
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    f.state.lock().unwrap().forward_target = Some(target.local_addr().unwrap());
    let target_read = tokio::spawn(async move {
        let (mut socket, _) = target.accept().await.unwrap();
        let mut payload = Vec::new();
        tokio::time::timeout(Duration::from_secs(3), socket.read_to_end(&mut payload))
            .await
            .unwrap()
            .unwrap();
        payload
    });
    let before: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    assert!(f
        .service
        .start_temporary_tcp_probe(&f.workspace, &f.handle, HOST, PORT, false)
        .is_err());
    assert!(f
        .service
        .start_temporary_tcp_probe("wrong", &f.handle, HOST, PORT, true)
        .is_err());
    assert!(f
        .service
        .start_temporary_tcp_probe(&f.workspace, &f.handle, "http://not-a-host", PORT, true)
        .is_err());
    assert!(f
        .service
        .start_temporary_tcp_probe(&f.workspace, &f.handle, HOST, 0, true)
        .is_err());
    assert!(f
        .service
        .start_temporary_probe(&f.workspace, &f.handle, TemporaryProbeKind::Tcp, true)
        .is_err());
    assert_eq!(f.state.lock().unwrap().direct_requests, 0);
    f.service
        .start_temporary_tcp_probe(&f.workspace, &f.handle, HOST, PORT, true)
        .unwrap();
    let report = finished(&f).await;
    assert_eq!(report.kind, TemporaryProbeKind::Tcp);
    assert_eq!(report.checks.len(), 1);
    assert_eq!(
        (
            &*report.checks[0].id,
            &*report.checks[0].code,
            &*report.checks[0].status
        ),
        ("tcp-forward", "OK", "pass")
    );
    assert!(!serde_json::to_string(&report).unwrap().contains(HOST));
    assert!(
        target_read.await.unwrap().is_empty(),
        "not even a HTTP/SSH/exec byte reaches the selected target"
    );
    wait_closed(&f).await;
    let state = f.state.lock().unwrap();
    assert_eq!(state.auths, 1);
    assert_eq!(state.direct_requests, 1);
    assert_eq!(state.direct_data_bytes, 0);
    assert_eq!(state.live.len(), 1);
    drop(state);
    assert_eq!(
        f.service.list_temporary(&f.workspace).unwrap()[0].status,
        "connected"
    );
    let after: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    assert_eq!(before, after, "target and result are never persisted");
}

#[tokio::test]
async fn tcp_refusal_is_not_reported_as_target_network_failure() {
    let f = fixture().await;
    f.state.lock().unwrap().reject_direct = true;
    f.service
        .start_temporary_tcp_probe(&f.workspace, &f.handle, HOST, PORT, true)
        .unwrap();
    let report = finished(&f).await;
    assert_eq!(report.checks[0].code, "SSH_TUNNEL_FORWARD_PROHIBITED");
    assert_eq!(report.checks[0].status, "unsupported");
    assert_eq!(f.state.lock().unwrap().auths, 1);
    assert_eq!(
        f.service.list_temporary(&f.workspace).unwrap()[0].status,
        "connected"
    );
}

#[tokio::test]
async fn tcp_late_open_after_cancel_or_timeout_closes_only_the_probe_channel() {
    for cancel in [true, false] {
        let f = fixture().await;
        let hold = Arc::new(tokio::sync::Notify::new());
        let opened = Arc::new(tokio::sync::Notify::new());
        {
            let mut state = f.state.lock().unwrap();
            state.hold = Some(hold.clone());
            state.opened = Some(opened.clone());
        }
        let run = f
            .service
            .start_temporary_tcp_probe(&f.workspace, &f.handle, HOST, PORT, true)
            .unwrap();
        opened.notified().await;
        assert!(f
            .service
            .start_temporary_tcp_probe(&f.workspace, &f.handle, HOST, PORT, true)
            .is_err());
        if cancel {
            f.service
                .cancel_temporary_probe(&f.workspace, &f.handle, &run.run_id)
                .unwrap();
        } else {
            tokio::time::pause();
            tokio::time::advance(STEP_TIMEOUT + Duration::from_secs(1)).await;
            tokio::time::resume();
        }
        let report = finished(&f).await;
        assert_eq!(
            report.checks[0].code,
            if cancel {
                "CANCELLED"
            } else {
                "SSH_TUNNEL_TIMEOUT"
            }
        );
        hold.notify_one();
        wait_closed(&f).await;
        assert_eq!(f.state.lock().unwrap().live.len(), 1);
        assert_eq!(f.state.lock().unwrap().auths, 1);
        assert_eq!(f.state.lock().unwrap().direct_data_bytes, 0);
    }
}

#[tokio::test]
async fn tcp_parent_loss_never_reports_success_or_retries() {
    let f = fixture().await;
    f.state.lock().unwrap().disconnect_direct = true;
    f.service
        .start_temporary_tcp_probe(&f.workspace, &f.handle, HOST, PORT, true)
        .unwrap();
    let report = finished(&f).await;
    assert_ne!(report.checks[0].status, "pass");
    assert!(matches!(
        report.checks[0].code.as_str(),
        "SSH_TUNNEL_TRANSPORT_CLOSED" | "CANCELLED"
    ));
    assert_eq!(f.state.lock().unwrap().auths, 1);
    assert_eq!(f.state.lock().unwrap().direct_requests, 1);
}
