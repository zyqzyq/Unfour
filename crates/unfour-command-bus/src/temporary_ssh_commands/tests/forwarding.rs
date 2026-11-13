use super::*;
use tokio::net::TcpListener;

async fn fixture_server(
    mut server: Server,
) -> (std::net::SocketAddr, String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let key =
        russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let fingerprint = key
        .public_key()
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let task = tokio::spawn(async move { server.run_on_socket(config, &listener).await.unwrap() });
    (addr, fingerprint, task)
}

#[tokio::test]
async fn temporary_forwarding_uses_one_parent_login_and_authenticates_the_ssh_target() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let target = Server {
        downstream: true,
        ..Default::default()
    };
    let target_calls = target.calls.clone();
    let (target_addr, target_key, target_task) = fixture_server(target).await;
    let parent = Server {
        forward_target: Some(target_addr),
        ..Default::default()
    };
    let parent_calls = parent.calls.clone();
    let resume = parent.pending_open.clone();
    let (parent_addr, parent_key, parent_task) = fixture_server(parent).await;
    let request = TemporaryRequest::password(
        "127.0.0.1".into(),
        parent_addr.port(),
        "synthetic-ticket-user".into(),
        "synthetic-password".into(),
    )
    .unwrap();
    let parent = bus
        .receive_temporary_ssh(&LaunchReceipt::default(), request)
        .unwrap();
    assert!(bus.ssh.temporary_tunnel(&workspace, &parent).is_err());
    bus.claim_temporary_ssh(&workspace, &parent).await.unwrap();
    bus.start_temporary_ssh(&workspace, &parent).unwrap();
    wait_for(&bus, &workspace, "awaiting-trust").await;
    assert!(bus.ssh.temporary_tunnel(&workspace, &parent).is_err());
    bus.trust_temporary_ssh(&workspace, &parent, &parent_key)
        .unwrap();
    wait_for(&bus, &workspace, "connected").await;
    assert!(bus.ssh.temporary_tunnel("elsewhere", &parent).is_err());
    let tunnel = bus.ssh.temporary_tunnel(&workspace, &parent).unwrap();
    let proxy = tunnel.http_proxy().await.unwrap();
    assert_eq!(
        tunnel
            .connect("denied.invalid", 2222)
            .await
            .err()
            .unwrap()
            .code(),
        "SSH_TUNNEL_TARGET_UNREACHABLE"
    );
    let definition = SshConnectionInput {
        id: None,
        workspace_id: workspace.clone(),
        name: "ssh target".into(),
        host: "ssh-target.invalid".into(),
        port: Some(2222),
        username: "target-user".into(),
        auth_kind: "password".into(),
        key_path: None,
        credential_ref: None,
        secret: Some("target-password".into()),
    };
    let host = unfour_ssh_engine::HostKeyStore::new(bus.db.pool().clone());
    host.record_fingerprint(&workspace, &definition.host, 2222, "different-target-key")
        .await
        .unwrap();
    let rejected = bus
        .test_ssh_connection_via(definition.clone(), Some(&parent))
        .await
        .unwrap();
    assert!(!rejected.ok);
    assert!(!target_calls.lock().unwrap().contains(&"auth"));
    host.record_fingerprint(&workspace, &definition.host, 2222, &target_key)
        .await
        .unwrap();
    let accepted = bus
        .test_ssh_connection_via(definition.clone(), Some(&parent))
        .await
        .unwrap();
    assert!(
        accepted.ok,
        "{}; parent {:?}; target {:?}",
        accepted.message,
        parent_calls.lock().unwrap(),
        target_calls.lock().unwrap()
    );
    let saved = bus.save_ssh_connection(definition).await.unwrap();
    let input = SshConnectInput {
        workspace_id: workspace.clone(),
        connection_id: saved.id,
        cols: None,
        rows: None,
        secret: None,
    };
    let child = bus
        .connect_ssh_session_via(input.clone(), Some(&parent))
        .await
        .unwrap();
    bus.send_ssh_input(SshSessionInput {
        workspace_id: workspace.clone(),
        session_id: child.session_id.clone(),
        data: "forward-test\r".into(),
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !target_calls.lock().unwrap().contains(&"input") {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        parent_calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| **c == "auth")
            .count(),
        1
    );
    assert_eq!(
        target_calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| **c == "auth")
            .count(),
        2
    );

    // Another window can supply a stale selection. Copying a terminal uses
    // the parent of its source, which the backend remembers while it is live.
    let copy = bus
        .connect_ssh_session_from(
            input.clone(),
            Some("not-its-parent"),
            Some(&child.session_id),
        )
        .await
        .unwrap();
    assert_ne!(copy.session_id, child.session_id);
    let mut wrong = input.clone();
    wrong.connection_id = "other".into();
    assert!(bus
        .connect_ssh_session_from(wrong, None, Some(&child.session_id))
        .await
        .is_err());

    let task = bus
        .save_ssh_task(
            serde_json::from_value(serde_json::json!({
                "workspaceId": workspace, "name": "forwarded task", "steps": [{
                    "name":"command", "stepType":"command", "position":0, "enabled":true,
                    "configJson":{"command":"fixture-task","timeoutSeconds":3}
                }]
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let run = bus
        .run_ssh_task_via(
            SshTaskRunInput {
                workspace_id: workspace.clone(),
                task_id: task.task.id.clone(),
                connection_id: Some(input.connection_id.clone()),
                inputs: Default::default(),
                secret_input_names: vec![],
                context: None,
            },
            Some(&parent),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let runs = bus
                .list_ssh_task_runs(workspace.clone(), task.task.id.clone())
                .await
                .unwrap();
            if let Some(done) = runs
                .iter()
                .find(|r| r.id == run.id && r.status != "running")
            {
                assert_eq!(done.status, "success");
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(target_calls.lock().unwrap().contains(&"task"));

    // The mock server deliberately withholds a reply. All accepted attempts,
    // including cancelled callers awaiting a late response, share the parent budget.
    let mut pending = tokio::task::JoinSet::new();
    for _ in 0..33 {
        let source = tunnel.clone();
        pending.spawn(async move {
            source
                .connect("slow.fixture.invalid", 2222)
                .await
                .err()
                .unwrap()
                .code()
        });
    }
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), pending.join_next())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        "SSH_TUNNEL_FORWARD_BUSY"
    );
    pending.abort_all();
    while pending.join_next().await.is_some() {}
    // The fixture serves SSH requests serially. Release outstanding opens and
    // confirm capacity is reclaimed before closing the parent.
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            resume.notify_one();
            if let Ok(Err(error)) = tokio::time::timeout(
                Duration::from_millis(10),
                tunnel.connect("denied.invalid", 2222),
            )
            .await
            {
                if error.code() == "SSH_TUNNEL_TARGET_UNREACHABLE" {
                    break;
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();

    bus.cancel_temporary_ssh(&workspace, &parent).unwrap();
    assert_eq!(
        bus.connect_ssh_session_from(input.clone(), None, Some(&child.session_id))
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_TEMPORARY_UNAVAILABLE"
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let disconnected = bus
                .list_ssh_sessions(workspace.clone())
                .await
                .unwrap()
                .iter()
                .any(|s| {
                    s.session_id == child.session_id
                        && s.status == "disconnected"
                        && s.reconnect_attempt == 0
                });
            if disconnected
                && tokio::net::TcpStream::connect(proxy.address())
                    .await
                    .is_err()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        tunnel
            .connect("ssh-target.invalid", 2222)
            .await
            .err()
            .unwrap()
            .code(),
        "SSH_TUNNEL_TEMPORARY_UNAVAILABLE"
    );
    assert_eq!(
        bus.connect_ssh_session_via(input, Some(&parent))
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_TEMPORARY_UNAVAILABLE"
    );
    parent_task.abort();
    target_task.abort();
}
