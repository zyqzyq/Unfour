//! Account orchestration through real loopback HTTP and the in-memory keychain.
use super::*;
use serde_json::{json, Value};

mod support;
use support::*;

#[tokio::test]
async fn callback_exchange_persists_only_session_and_replay_cannot_exchange_twice() {
    let api = TestApi::start(vec![(200, session_response())], None).await;
    let service = api.service();
    let callback = callback(&service).await;
    let verifier = service
        .pending
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .code_verifier
        .clone();
    let generation = service.generation();
    let state = service.handle_deep_link(&callback).await.unwrap();
    assert!(matches!(state, AccountState::SignedIn { .. }));
    assert!(!serde_json::to_string(&state).unwrap().contains(TOKEN));
    assert!(!format!("{state:?}").contains(TOKEN));
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
    assert_eq!(service.generation(), generation + 1);
    assert!(matches!(
        service.handle_deep_link(&callback).await,
        Err(AccountError::NoPendingAuthorization)
    ));
    let requests = api.finish().await;
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /v1/desktop/token HTTP/1.1"));
    let body: Value = serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["codeVerifier"], verifier);
    assert_eq!(body["authorizationCode"], "C".repeat(43));
    assert_eq!(body.as_object().unwrap().len(), 3);
}

#[tokio::test]
async fn stale_and_malformed_callbacks_preserve_new_attempt_but_denial_consumes_it() {
    let api = TestApi::start(vec![], None).await;
    let service = api.service();
    let old = callback(&service).await;
    let current = callback(&service).await;
    assert!(matches!(
        service.handle_deep_link(&old).await,
        Err(AccountError::StateMismatch)
    ));
    assert!(matches!(
        service
            .handle_deep_link(&(current.clone() + "&state=duplicate"))
            .await,
        Err(AccountError::InvalidDeepLink(_))
    ));
    assert!(matches!(
        service.state().await.unwrap(),
        AccountState::SigningIn
    ));
    let denied = current.replace(
        &format!("authorizationCode={}", "C".repeat(43)),
        "error=access_denied",
    );
    assert!(matches!(
        service.handle_deep_link(&denied).await,
        Err(AccountError::AuthorizationDenied)
    ));
    assert!(matches!(
        service.handle_deep_link(&current).await,
        Err(AccountError::NoPendingAuthorization)
    ));
    assert!(service.sessions.load().await.unwrap().is_none());
    assert!(api.finish().await.is_empty());
}

#[tokio::test]
async fn failed_exchange_preserves_previous_session_and_consumes_authorization() {
    for (status, body, code) in [
        (
            400,
            json!({"error":{"code":"pkce_mismatch","message":"private-server-detail","requestId":"test-request"}}),
            "pkce_mismatch",
        ),
        (
            200,
            json!({"sessionToken":"invalid-sensitive-token"}),
            "invalid_api_response",
        ),
    ] {
        let api = TestApi::start(vec![(status, body)], None).await;
        let service = api.service();
        save_session(&service).await;
        let callback = callback(&service).await;
        let generation = service.generation();
        let error = service.handle_deep_link(&callback).await.unwrap_err();
        assert_eq!(error.code(), code);
        assert!(!error.to_string().contains("private-server-detail"));
        assert!(!error.to_string().contains("invalid-sensitive-token"));
        assert_eq!(service.generation(), generation);
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
        assert!(matches!(
            service.handle_deep_link(&callback).await,
            Err(AccountError::NoPendingAuthorization)
        ));
        assert_eq!(
            api.finish().await.len(),
            1,
            "permanent failures must not retry"
        );
    }
}

#[tokio::test]
async fn sign_out_during_exchange_cannot_restore_the_deleted_session() {
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let api = TestApi::start(vec![(200, session_response())], Some(gate.clone())).await;
    let service = api.service();
    let callback = callback(&service).await;
    let pending = {
        let service = service.clone();
        tokio::spawn(async move { service.handle_deep_link(&callback).await })
    };
    tokio::time::timeout(Duration::from_secs(5), gate.wait())
        .await
        .unwrap();
    assert!(matches!(
        service.sign_out().await.unwrap(),
        AccountState::SignedOut
    ));
    gate.wait().await;
    let result = pending.await.unwrap();
    assert!(
        matches!(result, Err(AccountError::NoPendingAuthorization)),
        "stale exchange must be discarded: {result:?}"
    );
    assert!(service.sessions.load().await.unwrap().is_none());
    assert_eq!(api.finish().await.len(), 1);
}

#[tokio::test]
async fn stale_state_response_cannot_sign_out_a_newer_session() {
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let api = TestApi::start(
        vec![
            (
                401,
                json!({"error":{"code":"desktop_session_expired","message":"expired","requestId":"stale-state"}}),
            ),
            (200, profile("active")),
        ],
        Some(gate.clone()),
    )
    .await;
    let service = api.service();
    save_session(&service).await;
    let task = {
        let service = service.clone();
        tokio::spawn(async move { service.state().await })
    };
    tokio::time::timeout(Duration::from_secs(5), gate.wait())
        .await
        .unwrap();

    service
        .sessions
        .save(StoredSession {
            session_token: "B".repeat(43),
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::days(1),
        })
        .await
        .unwrap();
    service.advance_generation();
    gate.wait().await;

    assert!(matches!(
        task.await.unwrap().unwrap(),
        AccountState::SignedIn { .. }
    ));
    assert_eq!(
        service
            .sessions
            .load()
            .await
            .unwrap()
            .unwrap()
            .session_token,
        "B".repeat(43)
    );
    assert_eq!(api.finish().await.len(), 2);
}

#[tokio::test]
async fn stale_entitlement_response_cannot_sign_out_a_newer_session() {
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let api = TestApi::start(
        vec![
            (
                401,
                json!({"error":{"code":"desktop_session_expired","message":"expired","requestId":"stale-entitlement"}}),
            ),
            (200, profile("active")),
        ],
        Some(gate.clone()),
    )
    .await;
    let service = api.service();
    save_session(&service).await;
    let task = {
        let service = service.clone();
        tokio::spawn(async move { service.require_entitlement("cloud_sync").await })
    };
    tokio::time::timeout(Duration::from_secs(5), gate.wait())
        .await
        .unwrap();

    service
        .sessions
        .save(StoredSession {
            session_token: "B".repeat(43),
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::days(1),
        })
        .await
        .unwrap();
    service.advance_generation();
    gate.wait().await;

    let authorized = task.await.unwrap().unwrap();
    assert_eq!(authorized.desktop_session_token(), "B".repeat(43));
    assert_eq!(authorized.generation(), 1);
    assert_eq!(api.finish().await.len(), 2);
}

#[tokio::test]
async fn entitlement_cache_is_capability_scoped_and_revocation_is_rechecked() {
    let api = TestApi::start(
        vec![
            (200, profile("active")),
            (200, profile("active")),
            (200, profile("revoked")),
        ],
        None,
    )
    .await;
    let service = api.service();
    save_session(&service).await;
    let session = service.require_entitlement("cloud_sync").await.unwrap();
    assert_eq!(session.account_id(), "account-a");
    assert!(!format!("{session:?}").contains(TOKEN));
    service.require_entitlement("cloud_sync").await.unwrap(); // cached, no second request
    assert!(matches!(
        service.require_entitlement("team_workspace").await,
        Err(AccountError::EntitlementUnavailable)
    ));
    service.invalidate_entitlement_cache();
    assert!(matches!(
        service.require_entitlement("cloud_sync").await,
        Err(AccountError::EntitlementUnavailable)
    ));
    assert!(service.entitlement_cache.lock().unwrap().is_none());
    assert!(
        service.sessions.load().await.unwrap().is_some(),
        "revocation must not sign the user out"
    );
    let requests = api.finish().await;
    assert_eq!(requests.len(), 3);
    assert!(requests
        .iter()
        .all(|request| request.starts_with("GET /v1/me HTTP/1.1")));
}

#[tokio::test]
async fn expired_entitlement_cache_cannot_authorize_after_suspension() {
    let api = TestApi::start(
        vec![(200, profile("active")), (200, profile("suspended"))],
        None,
    )
    .await;
    let service = api.service();
    save_session(&service).await;
    service.require_entitlement("cloud_sync").await.unwrap();
    service
        .entitlement_cache
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .expires_at = Instant::now() - Duration::from_secs(1);
    assert!(matches!(
        service.require_entitlement("cloud_sync").await,
        Err(AccountError::EntitlementUnavailable)
    ));
    assert_eq!(api.finish().await.len(), 2);
}

#[tokio::test]
async fn invalid_remote_session_clears_credentials_but_invalid_response_preserves_them() {
    for (status, body, signed_out) in [
        (
            401,
            json!({"error":{"code":"desktop_session_expired","message":"expired","requestId":"test-request"}}),
            true,
        ),
        (200, json!({"id":"invalid-profile"}), false),
    ] {
        let api = TestApi::start(vec![(status, body)], None).await;
        let service = api.service();
        save_session(&service).await;
        let generation = service.generation();
        let result = service.require_entitlement("cloud_sync").await;
        if signed_out {
            assert!(matches!(result, Err(AccountError::SignedOut)));
            assert!(service.sessions.load().await.unwrap().is_none());
            assert_eq!(service.generation(), generation + 1);
        } else {
            assert!(matches!(result, Err(AccountError::InvalidApiResponse)));
            assert!(service.sessions.load().await.unwrap().is_some());
            assert_eq!(service.generation(), generation);
        }
        assert!(service.entitlement_cache.lock().unwrap().is_none());
        assert_eq!(api.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn sign_out_keeps_local_credentials_deleted_even_if_revocation_fails() {
    let api = TestApi::start(
        vec![
            (200, profile("active")),
            (
                500,
                json!({"error":{"code":"internal_error","message":"failed"}}),
            ),
        ],
        None,
    )
    .await;
    let service = api.service();
    save_session(&service).await;
    let authorized = service.require_entitlement("cloud_sync").await.unwrap();
    assert!(matches!(
        service.sign_out().await.unwrap(),
        AccountState::SignedOut
    ));
    assert!(service.generation() > authorized.generation());
    assert!(service.sessions.load().await.unwrap().is_none());
    assert!(service.entitlement_cache.lock().unwrap().is_none());
    assert!(matches!(
        service.require_entitlement("cloud_sync").await,
        Err(AccountError::SignedOut)
    ));
    let requests = api.finish().await;
    assert_eq!(requests.len(), 2);
    assert!(requests[1].starts_with("DELETE /v1/desktop/session HTTP/1.1"));
}
