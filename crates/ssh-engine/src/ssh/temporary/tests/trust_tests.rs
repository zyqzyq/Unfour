use super::*;

async fn connecting(service: &SshService, workspace: &str) -> TrustPrompt {
    let handle = service
        .receive_temporary(&LaunchReceipt::default(), request())
        .unwrap();
    service.claim_temporary(workspace, &handle).await.unwrap();
    let mut queue = service.temporary.0.lock().unwrap();
    let entry = queue.owned(workspace, &handle).unwrap();
    entry.request = None;
    entry.summary.status = "connecting".into();
    TrustPrompt {
        queue: Arc::downgrade(&service.temporary.0),
        handle,
        initial_exchange_seen: false,
    }
}

#[tokio::test]
async fn first_use_connects_without_approval_and_does_not_persist() {
    let (service, workspace) = fixture().await;
    let store = HostKeyStore::new(service.db.pool().clone());
    let before: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(service.db.pool())
        .await
        .unwrap();
    let prompt = connecting(&service, &workspace).await;
    assert!(
        prompt
            .verify(&store, &workspace, "fixture", 22, "first")
            .await
    );
    let summary = service.list_temporary(&workspace).unwrap().remove(0);
    assert_eq!(summary.status, "connecting");
    assert_eq!(summary.fingerprint, None);
    assert!(store
        .get_fingerprint(&workspace, "fixture", 22)
        .await
        .unwrap()
        .is_none());
    let after: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(service.db.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    assert!(service
        .trust_temporary(&workspace, &prompt.handle, "changed")
        .is_err());
    assert!(
        !prompt
            .verify(&store, &workspace, "fixture", 22, "changed")
            .await
    );
    assert_eq!(
        service.list_temporary(&workspace).unwrap()[0]
            .error_code
            .as_deref(),
        Some("TEMPORARY_HOST_KEY_MISMATCH")
    );
}

#[tokio::test]
async fn repeated_launches_share_pin_until_workspace_closes() {
    let (service, workspace) = fixture().await;
    let store = HostKeyStore::new(service.db.pool().clone());
    let first = connecting(&service, &workspace).await;
    assert!(
        first
            .verify(&store, &workspace, "Fixture", 22, "first")
            .await
    );
    service.cancel_temporary(&workspace, &first.handle).unwrap();
    let next = connecting(&service, &workspace).await;
    assert!(
        next.verify(&store, &workspace, "fixture", 22, "first")
            .await
    );
    let changed = connecting(&service, &workspace).await;
    assert!(
        !changed
            .verify(&store, &workspace, "fixture", 22, "changed")
            .await
    );
    let other_port = connecting(&service, &workspace).await;
    assert!(
        other_port
            .verify(&store, &workspace, "fixture", 23, "changed")
            .await
    );
    service.close_temporary_workspace(&workspace).unwrap();
    assert!(service.temporary.0.lock().unwrap().trusted_hosts.is_empty());
    let reopened = connecting(&service, &workspace).await;
    assert!(
        reopened
            .verify(&store, &workspace, "fixture", 22, "changed")
            .await
    );
}

#[tokio::test]
async fn durable_pin_mismatch_and_cancelled_known_host_fail_closed() {
    let (service, workspace) = fixture().await;
    let store = HostKeyStore::new(service.db.pool().clone());
    store
        .record_fingerprint(&workspace, "fixture", 22, "expected")
        .await
        .unwrap();
    let mismatch = connecting(&service, &workspace).await;
    assert!(
        !mismatch
            .verify(&store, &workspace, "fixture", 22, "changed")
            .await
    );
    assert!(service.temporary.0.lock().unwrap().trusted_hosts.is_empty());
    let known = connecting(&service, &workspace).await;
    assert!(
        known
            .verify(&store, &workspace, "fixture", 22, "expected")
            .await
    );
    service.cancel_temporary(&workspace, &known.handle).unwrap();
    assert!(
        !known
            .verify(&store, &workspace, "fixture", 22, "expected")
            .await
    );
    assert!(
        !known
            .verify(&store, "elsewhere", "fixture", 22, "expected")
            .await
    );
}

#[tokio::test]
async fn concurrent_first_uses_cannot_replace_each_others_pin() {
    let (service, workspace) = fixture().await;
    let store = HostKeyStore::new(service.db.pool().clone());
    let a = connecting(&service, &workspace).await;
    let b = connecting(&service, &workspace).await;
    let (accepted_a, accepted_b) = tokio::join!(
        a.verify(&store, &workspace, "fixture", 22, "a"),
        b.verify(&store, &workspace, "fixture", 22, "b"),
    );
    assert_ne!(accepted_a, accepted_b);
    assert_eq!(service.temporary.0.lock().unwrap().trusted_hosts.len(), 1);
}

#[tokio::test]
async fn known_dns_names_are_case_insensitive_and_conflicting_pins_fail_closed() {
    let (service, workspace) = fixture().await;
    let store = HostKeyStore::new(service.db.pool().clone());
    store
        .record_fingerprint(&workspace, "FIXTURE", 22, "expected")
        .await
        .unwrap();
    let known = connecting(&service, &workspace).await;
    assert!(
        known
            .verify(&store, &workspace, "fixture", 22, "expected")
            .await
    );
    let changed = connecting(&service, &workspace).await;
    assert!(
        !changed
            .verify(&store, &workspace, "fixture", 22, "changed")
            .await
    );
    store
        .record_fingerprint(&workspace, "fixture", 22, "different")
        .await
        .unwrap();
    let conflict = connecting(&service, &workspace).await;
    assert!(
        !conflict
            .verify(&store, &workspace, "FIXTURE", 22, "expected")
            .await
    );
}

#[tokio::test]
async fn cancellation_during_known_pin_lookup_cannot_accept_the_host() {
    use std::{future::Future, task::Poll};
    let (service, workspace) = fixture().await;
    let store = HostKeyStore::new(service.db.pool().clone());
    store
        .record_fingerprint(&workspace, "fixture", 22, "expected")
        .await
        .unwrap();
    let prompt = connecting(&service, &workspace).await;
    let connection = service.db.pool().acquire().await.unwrap();
    let verify = prompt.verify(&store, &workspace, "fixture", 22, "expected");
    tokio::pin!(verify);
    assert!(matches!(
        std::future::poll_fn(|cx| Poll::Ready(verify.as_mut().poll(cx))).await,
        Poll::Pending
    ));
    service
        .cancel_temporary(&workspace, &prompt.handle)
        .unwrap();
    drop(connection);
    assert!(!verify.await);
    assert!(service.temporary.0.lock().unwrap().trusted_hosts.is_empty());
}

#[tokio::test]
async fn full_pin_cache_rejects_new_hosts_without_evicting_known_pins() {
    let (service, workspace) = fixture().await;
    let store = HostKeyStore::new(service.db.pool().clone());
    let known = connecting(&service, &workspace).await;
    let new = connecting(&service, &workspace).await;
    {
        let mut queue = service.temporary.0.lock().unwrap();
        for i in 0..MAX_TRUSTED_HOSTS {
            queue.trusted_hosts.insert(
                (workspace.clone(), format!("fixture-{i}"), 22),
                "expected".into(),
            );
        }
    }
    assert!(
        known
            .verify(&store, &workspace, "fixture-0", 22, "expected")
            .await
    );
    assert!(!new.verify(&store, &workspace, "new", 22, "expected").await);
    assert_eq!(
        service.temporary.0.lock().unwrap().trusted_hosts.len(),
        MAX_TRUSTED_HOSTS
    );
    let summary = service
        .list_temporary(&workspace)
        .unwrap()
        .into_iter()
        .find(|s| s.handle == new.handle)
        .unwrap();
    assert_eq!(
        summary.error_code.as_deref(),
        Some("TEMPORARY_TRUST_CAPACITY")
    );
}
