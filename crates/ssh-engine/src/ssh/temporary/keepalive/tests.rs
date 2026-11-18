use super::*;
use std::sync::atomic::Ordering;
mod fixture;
mod rekey;
use fixture::{advance, create, until, State};

#[tokio::test]
async fn native_idle_heartbeats_preserve_one_auth_and_do_not_write_to_busy_pty() {
    let f = create(State::default()).await;
    f.connected().await;
    assert_eq!(
        client_config().keepalive_interval,
        Some(Duration::from_secs(60))
    );
    assert_eq!(client_config().keepalive_max, 3);
    let initial = f.count();
    advance(59).await;
    assert_eq!(
        f.count(),
        initial,
        "heartbeat must respect the 60s idle interval"
    );
    // The transport heartbeat must remain independent of the terminal's owner.
    let operation = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap();
    advance(2).await;
    until(|| f.count() > initial).await;
    for _ in 0..4 {
        let before = f.count();
        advance(61).await;
        until(|| f.count() > before).await;
        assert_eq!(f.summary().status, "connected");
    }
    drop(operation);
    assert_eq!(f.state.auths.load(Ordering::SeqCst), 1);
    assert_eq!(f.state.ptys.load(Ordering::SeqCst), 1);
    assert_eq!(f.state.data_bytes.load(Ordering::SeqCst), 0);
    f.service.cancel_temporary(&f.workspace, &f.handle).unwrap();
    until(|| f.state.closed.load(Ordering::SeqCst)).await;
    let closed = f.count();
    advance(240).await;
    assert_eq!(
        f.count(),
        closed,
        "closed sessions must stop protocol traffic"
    );
    assert!(f.service.list_temporary(&f.workspace).unwrap().is_empty());
}

#[tokio::test]
async fn native_silent_peer_gets_keepalive_timeout_without_reauthentication() {
    let f = create(State::default()).await;
    f.connected().await;
    f.state.silent.store(true, Ordering::SeqCst);
    for _ in 0..3 {
        advance(61).await;
        assert_eq!(f.summary().status, "connected");
    }
    advance(61).await;
    until(|| f.summary().status == "disconnected").await;
    assert_eq!(
        f.summary().error_code.as_deref(),
        Some("TEMPORARY_KEEPALIVE_TIMEOUT")
    );
    assert_eq!(f.state.auths.load(Ordering::SeqCst), 1);
    assert_eq!(f.state.data_bytes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn native_slow_authentication_and_pty_keep_their_stage_deadlines() {
    let auth = Arc::new(tokio::sync::Notify::new());
    let pty = Arc::new(tokio::sync::Notify::new());
    let f = create(State {
        auth_hold: Some(auth.clone()),
        pty_hold: Some(pty.clone()),
        ..Default::default()
    })
    .await;
    until(|| f.state.auths.load(Ordering::SeqCst) == 1).await;
    advance(25).await;
    assert_eq!(f.summary().status, "connecting");
    auth.notify_one();
    until(|| f.state.ptys.load(Ordering::SeqCst) == 1).await;
    advance(12).await; // Beyond the ordinary connection's ~9s failure budget.
    assert_eq!(f.summary().status, "connecting");
    pty.notify_one();
    f.connected().await;
    assert_eq!(f.state.auths.load(Ordering::SeqCst), 1);
    assert_eq!(f.state.data_bytes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn native_remote_disconnect_and_channel_eof_have_fixed_distinct_reasons() {
    for shell_only in [true, false] {
        let f = create(State::default()).await;
        f.connected().await;
        let server = f.state.server.lock().unwrap().clone().unwrap();
        if shell_only {
            let channel = f.state.channel.lock().unwrap().unwrap();
            server.eof(channel).await.unwrap();
        } else {
            server
                .disconnect(
                    russh::Disconnect::ByApplication,
                    "SECRET_REMOTE_DIAGNOSTIC".into(),
                    "en".into(),
                )
                .await
                .unwrap();
        }
        until(|| f.summary().status == "disconnected").await;
        assert_eq!(
            f.summary().error_code.as_deref(),
            Some(if shell_only {
                "TEMPORARY_REMOTE_SHELL_CLOSED"
            } else {
                "TEMPORARY_REMOTE_DISCONNECTED"
            })
        );
        assert!(!serde_json::to_string(&f.summary())
            .unwrap()
            .contains("SECRET_REMOTE_DIAGNOSTIC"));
    }
}

#[tokio::test]
async fn disconnect_reason_precedence_survives_late_callback_and_keeps_security_codes() {
    let f = create(State::default()).await;
    f.connected().await;
    record_channel_close(
        &f.service,
        &f.workspace,
        &f.handle,
        "TEMPORARY_TRANSPORT_CLOSED",
    );
    {
        let mut queue = f.service.temporary.0.lock().unwrap();
        let entry = queue.owned(&f.workspace, &f.handle).unwrap();
        entry.summary.status = "disconnected".into();
        entry.cancel.as_ref().unwrap().send_replace(true);
    }
    record_channel_close(
        &f.service,
        &f.workspace,
        &f.handle,
        "TEMPORARY_KEEPALIVE_TIMEOUT",
    );
    assert_eq!(
        f.summary().error_code.as_deref(),
        Some("TEMPORARY_KEEPALIVE_TIMEOUT")
    );
    for security in [
        "TEMPORARY_HOST_KEY_MISMATCH",
        "TEMPORARY_REKEY_REQUIRES_RELAUNCH",
    ] {
        f.service
            .temporary
            .0
            .lock()
            .unwrap()
            .owned(&f.workspace, &f.handle)
            .unwrap()
            .summary
            .error_code = Some(security.into());
        record_channel_close(
            &f.service,
            &f.workspace,
            &f.handle,
            "TEMPORARY_REMOTE_DISCONNECTED",
        );
        assert_eq!(f.summary().error_code.as_deref(), Some(security));
    }
    f.service.cancel_temporary(&f.workspace, &f.handle).unwrap();
    record_channel_close(
        &f.service,
        &f.workspace,
        &f.handle,
        "TEMPORARY_KEEPALIVE_TIMEOUT",
    );
    assert!(f.service.list_temporary(&f.workspace).unwrap().is_empty());
}
