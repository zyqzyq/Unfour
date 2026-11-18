use super::*;

#[tokio::test]
async fn native_repeated_rekeys_preserve_pty_and_authenticate_only_once() {
    let f = create(State::default()).await;
    f.connected().await;
    let (transport, _, _) = f
        .service
        .temporary_forward_source(&f.workspace, &f.handle)
        .unwrap();
    let channel = f
        .service
        .temporary
        .0
        .lock()
        .unwrap()
        .owned(&f.workspace, &f.handle)
        .unwrap()
        .native
        .as_ref()
        .unwrap()
        .writer
        .id();
    for round in 0..3 {
        transport.rekey_soon().await.unwrap();
        // Handle::data shares the FIFO queue used by rekey_soon. That queue
        // stops being polled while KEX is active. Its echoed marker therefore
        // proves this exchange finished, before we test the normal input path.
        let barrier = format!("rekey-complete-{round}\n");
        transport
            .data(channel, barrier.clone().into_bytes())
            .await
            .unwrap();
        expect_echo(&f, &barrier).await;
        let text = format!("after-rekey-{round}\n");
        f.service
            .temporary_input(SshSessionInput {
                workspace_id: f.workspace.clone(),
                session_id: f.handle.clone(),
                data: text.clone(),
            })
            .await
            .unwrap();
        expect_echo(&f, &text).await;
        assert!(f.summary().error_code.is_none());
    }
    assert_eq!(f.state.auths.load(Ordering::SeqCst), 1);
    assert_eq!(f.state.ptys.load(Ordering::SeqCst), 1);
    assert!(!transport.is_closed());
}

async fn expect_echo(f: &fixture::Fixture, text: &str) {
    let mut received = String::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !received.contains(text) {
            for event in f.service.temporary_output(&f.workspace, &f.handle).unwrap() {
                received.push_str(&event.data);
            }
            assert_eq!(f.summary().status, "connected");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("PTY echo must survive a completed rekey");
}

#[tokio::test]
async fn native_rekey_rechecks_pin_and_rejects_a_conflict_without_reauth() {
    let f = create(State::default()).await;
    f.connected().await;
    let (transport, _, _) = f
        .service
        .temporary_forward_source(&f.workspace, &f.handle)
        .unwrap();
    {
        let mut queue = f.service.temporary.0.lock().unwrap();
        assert_eq!(queue.trusted_hosts.len(), 1);
        for pin in queue.trusted_hosts.values_mut() {
            *pin = "conflicting-host-key".into();
        }
    }
    transport.rekey_soon().await.unwrap();
    until(|| f.summary().status == "disconnected").await;
    assert_eq!(
        f.summary().error_code.as_deref(),
        Some("TEMPORARY_HOST_KEY_MISMATCH")
    );
    assert_eq!(f.state.auths.load(Ordering::SeqCst), 1);
    assert_eq!(f.state.data_bytes.load(Ordering::SeqCst), 0);
}
