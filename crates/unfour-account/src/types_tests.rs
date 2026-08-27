use super::*;

const STATE: &str = "abcdefghijklmnopqrstuvwxyz0123456789_-ABCDE";
const AUTHORIZATION_CODE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde";

#[test]
fn billing_urls_require_safe_https_navigation_destinations() {
    let session_token = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde";
    let valid = BillingUrl::from_api_response(
        "https://checkout.example.test/session/checkout-1?provider=creem",
        session_token,
    )
    .expect("valid billing URL");
    assert_eq!(
        valid.as_str(),
        "https://checkout.example.test/session/checkout-1?provider=creem"
    );
    assert_eq!(format!("{valid:?}"), "BillingUrl([REDACTED])");

    for invalid in [
        "http://checkout.example.test/session/checkout-1",
        "javascript:alert(1)",
        "https://user:password@checkout.example.test/session/checkout-1",
        "https:///missing-host",
        " https://checkout.example.test/session/checkout-1",
        "https://checkout.example.test/session/checkout-1\n",
        "https://checkout.example.test/?desktopSession=ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde",
    ] {
        assert!(
            matches!(
                BillingUrl::from_api_response(invalid, session_token),
                Err(AccountError::InvalidBillingUrl)
            ),
            "accepted invalid billing URL: {invalid}"
        );
    }
}

#[test]
fn parses_the_web_callback_shape() {
    let callback = AuthCallback::parse(&format!(
        "{AUTH_CALLBACK_URI}?authorizationCode={AUTHORIZATION_CODE}&state={STATE}"
    ))
    .expect("valid callback");
    match callback {
        AuthCallback::Code {
            authorization_code,
            state,
        } => {
            assert_eq!(authorization_code, AUTHORIZATION_CODE);
            assert_eq!(state, STATE);
        }
        AuthCallback::Denied { .. } => panic!("expected authorization code"),
    }
}

#[test]
fn accepts_a_valid_denial_without_an_authorization_code() {
    let callback = AuthCallback::parse(&format!(
        "{AUTH_CALLBACK_URI}?error=access_denied&state={STATE}"
    ))
    .expect("valid denial callback");
    assert!(matches!(callback, AuthCallback::Denied { .. }));
}

#[test]
fn rejects_partial_ambiguous_or_legacy_callbacks() {
    for raw in [
            format!(
                "unfour://other/callback?authorizationCode={AUTHORIZATION_CODE}&state={STATE}"
            ),
            format!(
                "unfour://auth/callback/?authorizationCode={AUTHORIZATION_CODE}&state={STATE}"
            ),
            format!(
                "unfour://auth/callback?authorizationCode={AUTHORIZATION_CODE}&state={STATE}#fragment"
            ),
            format!(
                "unfour://auth/callback?authorizationCode={AUTHORIZATION_CODE}&state={STATE}&extra=1"
            ),
            format!(
                "unfour://auth/callback?authorizationCode={AUTHORIZATION_CODE}&authorizationCode={AUTHORIZATION_CODE}&state={STATE}"
            ),
            format!(
                "unfour://auth/callback?authorizationCode={AUTHORIZATION_CODE}&error=denied&state={STATE}"
            ),
            format!("unfour://auth/callback?code={AUTHORIZATION_CODE}&state={STATE}"),
            "unfour://auth/callback?authorizationCode=short&state=short".to_string(),
        ] {
            assert!(AuthCallback::parse(&raw).is_err(), "accepted {raw}");
        }
}

#[test]
fn deserializes_the_openapi_account_shape() {
    let account: AccountSummary = serde_json::from_str(
        r#"{
                "id":"550e8400-e29b-41d4-a716-446655440000",
                "email":"alex@example.test",
                "username":"alexchen",
                "displayName":"Alex Chen",
                "avatarUrl":"https://avatars.example.test/alexchen.png",
                "entitlements":[{"code":"cloud_sync","status":"active","validUntil":null}],
                "devices":[{
                    "id":"550e8400-e29b-41d4-a716-446655440001",
                    "name":"Unfour Desktop",
                    "platform":"windows",
                    "lastSeenAt":null,
                    "revoked":false
                }]
            }"#,
    )
    .expect("OpenAPI account response");
    account.validate().expect("valid account response");
    assert_eq!(account.username.as_deref(), Some("alexchen"));
    assert_eq!(account.display_name.as_deref(), Some("Alex Chen"));
    assert_eq!(
        account.avatar_url.as_deref(),
        Some("https://avatars.example.test/alexchen.png")
    );
    assert_eq!(account.entitlements[0].status, EntitlementStatus::Active);
    assert!(AccountState::SignedIn {
        profile: account.clone()
    }
    .has_active_entitlement("cloud_sync"));
    assert!(!AccountState::SignedIn {
        profile: account.clone()
    }
    .has_active_entitlement("unrelated"));

    let ipc_state = serde_json::to_value(AccountState::SignedIn { profile: account })
        .expect("frontend-visible account state");
    assert_eq!(ipc_state["profile"]["username"], "alexchen");
    assert_eq!(ipc_state["profile"]["displayName"], "Alex Chen");
    assert_eq!(
        ipc_state["profile"]["avatarUrl"],
        "https://avatars.example.test/alexchen.png"
    );
    assert!(ipc_state["profile"].get("avatar_url").is_none());
}

#[test]
fn deserializes_suspended_entitlement_without_granting_access() {
    let mut value = account_value(
        serde_json::Value::Null,
        serde_json::Value::Null,
        serde_json::Value::Null,
    );
    value["entitlements"][0]["status"] = serde_json::json!("suspended");
    let account: AccountSummary =
        serde_json::from_value(value).expect("account with suspended entitlement");

    account.validate().expect("valid account response");
    assert_eq!(account.entitlements[0].status, EntitlementStatus::Suspended);
    let state = AccountState::SignedIn { profile: account };
    assert!(!state.has_active_entitlement("cloud_sync"));

    let ipc_state = serde_json::to_value(state).expect("frontend-visible account state");
    assert_eq!(
        ipc_state["profile"]["entitlements"][0]["status"],
        "suspended"
    );
}

#[test]
fn accepts_null_optional_profile_fields() {
    let account: AccountSummary = serde_json::from_value(account_value(
        serde_json::Value::Null,
        serde_json::Value::Null,
        serde_json::Value::Null,
    ))
    .expect("account with null profile fields");

    account.validate().expect("valid account response");
    assert_eq!(account.username, None);
    assert_eq!(account.display_name, None);
    assert_eq!(account.avatar_url, None);
}

#[test]
fn entitlement_check_uses_code_status_and_expiry() {
    let now = OffsetDateTime::now_utc();
    let mut account: AccountSummary = serde_json::from_value(account_value(
        serde_json::Value::Null,
        serde_json::Value::Null,
        serde_json::Value::Null,
    ))
    .expect("account");
    account.entitlements = vec![EntitlementSummary {
        code: "cloud_sync".into(),
        status: EntitlementStatus::Active,
        valid_until: Some(now + time::Duration::hours(1)),
    }];
    assert!(account.has_active_entitlement("cloud_sync", now));
    assert!(!account.has_active_entitlement("unrelated", now));
    for status in [
        EntitlementStatus::Expired,
        EntitlementStatus::Revoked,
        EntitlementStatus::Suspended,
    ] {
        account.entitlements[0].status = status;
        assert!(!account.has_active_entitlement("cloud_sync", now));
    }
    account.entitlements[0].status = EntitlementStatus::Active;
    account.entitlements[0].valid_until = Some(now - time::Duration::seconds(1));
    assert!(!account.has_active_entitlement("cloud_sync", now));
}

#[test]
fn accepts_a_github_username_without_a_display_name() {
    let account: AccountSummary = serde_json::from_value(account_value(
        serde_json::json!("octocat"),
        serde_json::Value::Null,
        serde_json::Value::Null,
    ))
    .expect("GitHub account response");

    account.validate().expect("valid account response");
    assert_eq!(account.username.as_deref(), Some("octocat"));
    assert_eq!(account.display_name, None);
}

#[test]
fn rejects_invalid_avatar_urls_without_exposing_them() {
    for avatar_url in [
        "http://avatars.example.test/alex.png",
        "/avatars/alex.png",
        "https://user:secret@avatars.example.test/alex.png",
    ] {
        let account: AccountSummary = serde_json::from_value(account_value(
            serde_json::json!("alexchen"),
            serde_json::json!("Alex Chen"),
            serde_json::json!(avatar_url),
        ))
        .expect("account response");
        let error = account.validate().expect_err("invalid avatar URL");

        assert!(matches!(error, AccountError::InvalidApiResponse));
        assert!(!error.to_string().contains(avatar_url));
    }
}

#[test]
fn rejects_blank_or_oversized_profile_fields() {
    let oversized_avatar = format!(
        "https://avatars.example.test/{}",
        "a".repeat(MAX_AVATAR_URL_LENGTH)
    );
    for (username, display_name, avatar_url) in [
        (
            serde_json::json!("   "),
            serde_json::Value::Null,
            serde_json::Value::Null,
        ),
        (
            serde_json::Value::Null,
            serde_json::json!("\t\n"),
            serde_json::Value::Null,
        ),
        (
            serde_json::json!("u".repeat(MAX_USERNAME_LENGTH + 1)),
            serde_json::Value::Null,
            serde_json::Value::Null,
        ),
        (
            serde_json::Value::Null,
            serde_json::json!("d".repeat(MAX_DISPLAY_NAME_LENGTH + 1)),
            serde_json::Value::Null,
        ),
        (
            serde_json::Value::Null,
            serde_json::Value::Null,
            serde_json::json!(oversized_avatar),
        ),
    ] {
        let account: AccountSummary =
            serde_json::from_value(account_value(username, display_name, avatar_url))
                .expect("account response");
        assert!(matches!(
            account.validate(),
            Err(AccountError::InvalidApiResponse)
        ));
    }
}

fn account_value(
    username: serde_json::Value,
    display_name: serde_json::Value,
    avatar_url: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "email": "alex@example.test",
        "username": username,
        "displayName": display_name,
        "avatarUrl": avatar_url,
        "entitlements": [{"code": "cloud_sync", "status": "active", "validUntil": null}],
        "devices": [{
            "id": "550e8400-e29b-41d4-a716-446655440001",
            "name": "Unfour Desktop",
            "platform": "windows",
            "lastSeenAt": null,
            "revoked": false
        }]
    })
}
