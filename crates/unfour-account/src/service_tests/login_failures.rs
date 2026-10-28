use super::*;
use std::future::Future;
use std::task::Poll;

#[tokio::test]
async fn ambiguous_exchange_failures_never_replay_a_single_use_code() {
    for (response, expected) in [
        (
            TestResponse::Json(502, json!("gateway HTML substitute")),
            "api_unavailable",
        ),
        (
            TestResponse::Json(
                503,
                json!({"error":{"code":"not_ready","message":"secret","requestId":"r"}}),
            ),
            "not_ready",
        ),
        (TestResponse::Json(504, Value::Null), "api_unavailable"),
        (TestResponse::Disconnect, "api_unavailable"),
        (TestResponse::Truncated, "invalid_api_response"),
        (
            TestResponse::Delay(Duration::from_millis(500)),
            "api_timeout",
        ),
    ] {
        let api = TestApi::start_with(vec![response], None).await;
        let mut service = api.service();
        service.client.set_test_timeout(Duration::from_millis(250));
        save_session(&service).await;
        let callback = callback(&service).await;
        let error = tokio::time::timeout(
            Duration::from_millis(900),
            service.handle_deep_link(&callback),
        )
        .await
        .expect("exchange must finish after one attempt, without retry backoff")
        .unwrap_err();
        assert_eq!(error.code(), expected);
        assert_eq!(error.stage(), Some(AccountFailureStage::CodeExchange));
        assert!(service.sessions.load().await.unwrap().is_some());
        assert_eq!(service.generation(), 0);
        assert_eq!(
            service
                .handle_deep_link(&callback)
                .await
                .unwrap_err()
                .code(),
            "no_pending_authorization"
        );
        assert_eq!(api.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn credential_save_failure_is_distinct_and_requires_fresh_authorization() {
    let mut response = session_response();
    response["sessionToken"] = json!("B".repeat(43));
    let api = TestApi::start(vec![(200, response)], None).await;
    let service = api.service();
    save_session(&service).await;
    service.sessions.fail_save.store(true, Ordering::SeqCst);
    let callback = callback(&service).await;
    let error = service.handle_deep_link(&callback).await.unwrap_err();
    assert_eq!(error.code(), "keychain_unavailable");
    assert_eq!(error.stage(), Some(AccountFailureStage::CredentialSave));
    assert_eq!(service.generation(), 0);
    assert_eq!(
        service
            .sessions
            .load()
            .await
            .unwrap()
            .unwrap()
            .session_token,
        TOKEN
    );
    assert_eq!(
        service
            .handle_deep_link(&callback)
            .await
            .unwrap_err()
            .code(),
        "no_pending_authorization"
    );
    assert_eq!(api.finish().await.len(), 1);
}

#[tokio::test]
async fn expired_and_used_codes_keep_the_original_rejection_without_replay() {
    for code in ["authorization_code_expired", "authorization_code_used"] {
        let api = TestApi::start(
            vec![(
                400,
                json!({"error":{"code":code,"message":"private-error","requestId":"r"}}),
            )],
            None,
        )
        .await;
        let service = api.service();
        let callback = callback(&service).await;
        let error = service.handle_deep_link(&callback).await.unwrap_err();
        assert_eq!(error.code(), code);
        assert_eq!(error.stage(), Some(AccountFailureStage::CodeExchange));
        assert!(!error.to_string().contains("private-error"));
        assert!(service.sessions.load().await.unwrap().is_none());
        assert_eq!(
            service
                .handle_deep_link(&callback)
                .await
                .unwrap_err()
                .code(),
            "no_pending_authorization"
        );
        assert_eq!(api.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn corrupt_session_is_reported_as_a_local_read_failure_without_exposing_it() {
    let api = TestApi::start(vec![], None).await;
    let secrets = unfour_secret_store::SecretStore::in_memory("corrupt-session-test");
    secrets
        .put_named_secret("pro-auth", "login-key", "sensitive-corrupt-session")
        .await
        .unwrap();
    let service =
        AccountService::with_secret_store(&api.url, "https://web.example.test", true, secrets)
            .unwrap();
    let (state, failure) = service.state_with_diagnostics().await.unwrap();
    assert!(matches!(state, AccountState::SignedOut));
    let failure = failure.unwrap();
    assert_eq!(failure.code(), "corrupt_stored_session");
    assert_eq!(failure.stage(), Some(AccountFailureStage::CredentialRead));
    assert!(!failure.to_string().contains("sensitive-corrupt-session"));
    assert!(service.sessions.load().await.unwrap().is_none());
    assert!(api.finish().await.is_empty());
}

#[tokio::test]
async fn stale_corrupt_session_cleanup_cannot_delete_new_sign_in_credentials() {
    let api = TestApi::start(vec![(200, profile("active"))], None).await;
    let secrets = unfour_secret_store::SecretStore::in_memory("corrupt-session-race-test");
    secrets
        .put_named_secret("pro-auth", "login-key", "corrupt")
        .await
        .unwrap();
    let service =
        AccountService::with_secret_store(&api.url, "https://web.example.test", true, secrets)
            .unwrap();
    let transition = service.session_transition.lock().await;
    let refresh = service.state_with_diagnostics();
    tokio::pin!(refresh);
    // Poll deterministically until cleanup waits for the credential transition.
    std::future::poll_fn(|context| match refresh.as_mut().poll(context) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(_) => panic!("corrupt-session cleanup must serialize with sign-in"),
    })
    .await;
    save_session(&service).await;
    service.advance_generation();
    drop(transition);
    let (state, failure) = refresh.await.unwrap();
    assert!(matches!(state, AccountState::SignedIn { .. }));
    assert!(failure.is_none());
    assert_eq!(
        service
            .sessions
            .load()
            .await
            .unwrap()
            .unwrap()
            .session_token,
        TOKEN
    );
    assert_eq!(api.finish().await.len(), 1);
}

#[tokio::test]
async fn newer_sign_in_invalidates_in_flight_exchange_without_losing_pending_pkce() {
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let api = TestApi::start(vec![(200, session_response())], Some(gate.clone())).await;
    let service = api.service();
    let old_callback = callback(&service).await;
    let exchanging = {
        let service = service.clone();
        tokio::spawn(async move { service.handle_deep_link(&old_callback).await })
    };
    tokio::time::timeout(Duration::from_secs(5), gate.wait())
        .await
        .unwrap();
    assert!(matches!(
        service.state().await.unwrap(),
        AccountState::SigningIn
    ));
    let new_callback = callback(&service).await;
    gate.wait().await;
    assert_eq!(
        exchanging.await.unwrap().unwrap_err().code(),
        "no_pending_authorization"
    );
    assert!(service.sessions.load().await.unwrap().is_none());
    let pending = service.pending.lock().unwrap();
    assert!(new_callback.contains(&pending.as_ref().unwrap().state));
    drop(pending);
    assert_eq!(api.finish().await.len(), 1);
}

#[tokio::test]
async fn late_exchange_failure_reports_superseded_attempt_instead_of_current_failure() {
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let api = TestApi::start(vec![(400, json!({"error":{"code":"authorization_code_expired","message":"expired","requestId":"r"}}))], Some(gate.clone())).await;
    let service = api.service();
    let old_callback = callback(&service).await;
    let exchanging = {
        let service = service.clone();
        tokio::spawn(async move { service.handle_deep_link(&old_callback).await })
    };
    tokio::time::timeout(Duration::from_secs(5), gate.wait())
        .await
        .unwrap();
    callback(&service).await;
    gate.wait().await;
    assert_eq!(
        exchanging.await.unwrap().unwrap_err().code(),
        "no_pending_authorization"
    );
    assert!(matches!(
        service.state().await.unwrap(),
        AccountState::SigningIn
    ));
    assert_eq!(api.finish().await.len(), 1);
}

#[tokio::test]
async fn restart_loses_pending_pkce_and_never_exchanges_cold_callback() {
    let api = TestApi::start(vec![], None).await;
    let callback = callback(&api.service()).await;
    let restarted = api.service();
    assert_eq!(
        restarted
            .handle_deep_link(&callback)
            .await
            .unwrap_err()
            .code(),
        "no_pending_authorization"
    );
    assert!(api.finish().await.is_empty());
}

#[tokio::test]
async fn expired_session_diagnostics_keep_remote_code_and_authoritative_signed_out_state() {
    let api = TestApi::start(vec![(401, json!({"error":{"code":"desktop_session_expired","message":"sensitive","requestId":"r"}}))], None).await;
    let service = api.service();
    save_session(&service).await;
    let (state, error) = service.state_with_diagnostics().await.unwrap();
    assert!(matches!(state, AccountState::SignedOut));
    let error = error.unwrap();
    assert_eq!(error.code(), "desktop_session_expired");
    assert_eq!(error.stage(), Some(AccountFailureStage::AccountRefresh));
    assert!(!error.to_string().contains("sensitive"));
    assert!(service.sessions.load().await.unwrap().is_none());
    assert_eq!(api.finish().await.len(), 1);
}
