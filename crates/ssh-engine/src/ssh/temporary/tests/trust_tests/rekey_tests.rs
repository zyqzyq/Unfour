use super::*;

async fn connected(service: &SshService, workspace: &str) -> (TrustPrompt, HostKeyStore) {
    let store = HostKeyStore::new(service.db.pool().clone());
    let mut prompt = connecting(service, workspace).await;
    assert!(
        prompt
            .verify(&store, workspace, "fixture", 22, "first")
            .await
    );
    prompt.finish_key_exchange(workspace).unwrap();
    service
        .temporary
        .0
        .lock()
        .unwrap()
        .owned(workspace, &prompt.handle)
        .unwrap()
        .summary
        .status = "connected".into();
    (prompt, store)
}

#[tokio::test]
async fn rekey_accepts_only_fresh_verification_of_initial_pin() {
    let (service, workspace) = fixture().await;
    let (mut prompt, store) = connected(&service, &workspace).await;
    for _ in 0..3 {
        assert!(
            prompt
                .verify(&store, &workspace, "fixture", 22, "first")
                .await
        );
        prompt.finish_key_exchange(&workspace).unwrap();
    }
    assert!(prompt.finish_key_exchange(&workspace).is_err());
    assert_eq!(
        service.list_temporary(&workspace).unwrap()[0]
            .error_code
            .as_deref(),
        Some("TEMPORARY_REKEY_REQUIRES_RELAUNCH")
    );
    assert!(store
        .get_fingerprint(&workspace, "fixture", 22)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn rekey_rejects_changed_key_even_when_other_pins_are_changed_to_match() {
    let (service, workspace) = fixture().await;
    let (mut prompt, store) = connected(&service, &workspace).await;
    {
        let mut queue = service.temporary.0.lock().unwrap();
        queue
            .owned(&workspace, &prompt.handle)
            .unwrap()
            .trusted_fingerprint = Some("changed".into());
        queue
            .trusted_hosts
            .insert((workspace.clone(), "fixture".into(), 22), "changed".into());
    }
    assert!(
        !prompt
            .verify(&store, &workspace, "fixture", 22, "changed")
            .await
    );
    assert!(prompt.finish_key_exchange(&workspace).is_err());
    assert_eq!(
        service.list_temporary(&workspace).unwrap()[0]
            .error_code
            .as_deref(),
        Some("TEMPORARY_HOST_KEY_MISMATCH")
    );
}

#[tokio::test]
async fn rekey_obeys_new_durable_conflicts_and_never_reuses_previous_evidence() {
    let (service, workspace) = fixture().await;
    let (mut prompt, store) = connected(&service, &workspace).await;
    assert!(
        prompt
            .verify(&store, &workspace, "fixture", 22, "first")
            .await
    );
    store
        .record_fingerprint(&workspace, "fixture", 22, "different")
        .await
        .unwrap();
    assert!(
        !prompt
            .verify(&store, &workspace, "fixture", 22, "first")
            .await
    );
    assert!(prompt.finish_key_exchange(&workspace).is_err());
}

#[tokio::test]
async fn rekey_cannot_accept_after_cancel_or_workspace_close_during_lookup() {
    use std::{future::Future, task::Poll};
    for close_workspace in [false, true] {
        let (service, workspace) = fixture().await;
        let (mut prompt, store) = connected(&service, &workspace).await;
        let handle = prompt.handle.clone();
        let connection = service.db.pool().acquire().await.unwrap();
        let verify = prompt.verify(&store, &workspace, "fixture", 22, "first");
        tokio::pin!(verify);
        assert!(matches!(
            std::future::poll_fn(|cx| Poll::Ready(verify.as_mut().poll(cx))).await,
            Poll::Pending
        ));
        if close_workspace {
            service.close_temporary_workspace(&workspace).unwrap();
        } else {
            service.cancel_temporary(&workspace, &handle).unwrap();
        }
        drop(connection);
        assert!(!verify.await);
    }
}

#[tokio::test]
async fn rekey_finish_rechecks_cancel_and_requires_initial_callback() {
    let (service, workspace) = fixture().await;
    let mut initial = connecting(&service, &workspace).await;
    assert!(initial.finish_key_exchange(&workspace).is_err());
    let (mut prompt, store) = connected(&service, &workspace).await;
    assert!(
        prompt
            .verify(&store, &workspace, "fixture", 22, "first")
            .await
    );
    let (cancel, _) = tokio::sync::watch::channel(true);
    service
        .temporary
        .0
        .lock()
        .unwrap()
        .owned(&workspace, &prompt.handle)
        .unwrap()
        .cancel = Some(cancel);
    assert!(prompt.finish_key_exchange(&workspace).is_err());
}
