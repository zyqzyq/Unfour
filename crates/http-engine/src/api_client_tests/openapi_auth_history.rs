use super::super::*;
use super::openapi_history::{exported, input, insert_history};
use super::support::service;

fn auth_cases() -> [serde_json::Value; 4] {
    [
        serde_json::json!({"type":"bearer","token":"auth-canary-value"}),
        serde_json::json!({"type":"basic","username":"auth-canary-user","password":"auth-canary-value"}),
        serde_json::json!({"type":"api-key","addTo":"header","key":" X-Custom-Credential ","value":"auth-canary-value"}),
        serde_json::json!({"type":"api-key","addTo":"query","key":" credential ","value":"auth-canary-value"}),
    ]
}

fn assert_safe(document: &serde_json::Value) {
    let text = document.to_string();
    for secret in [
        "auth-canary-value",
        "auth-canary-user",
        "manual-canary-value",
        "YXV0aC1jYW5hcnktdXNlcjphdXRoLWNhbmFyeS12YWx1ZQ==",
    ] {
        assert!(
            !text.contains(secret),
            "export must not contain a credential"
        );
    }
}

#[tokio::test]
async fn openapi_matches_materialized_auth_in_current_and_legacy_history() {
    for auth in auth_cases() {
        for legacy in [false, true] {
            let service = service().await;
            let collection = service
                .create_collection("workspace-a".into(), "Auth examples".into())
                .await
                .unwrap();
            let mut input = input(&collection.id, false);
            input.auth_json = Some(auth.to_string());
            let saved = service.save_request(input.clone()).await.unwrap();
            let sent = service.materialize_auth(input).unwrap();
            let sent = service.materialize_auth(sent).unwrap();
            insert_history(&service, &sent, "auth-history", legacy).await;

            // Old rows can echo custom auth values under an ordinary JSON key.
            if legacy {
                sqlx::query("UPDATE api_history SET response_body_preview=? WHERE id=?")
                    .bind(r#"{"echo":"auth-canary-value","result":"ok"}"#)
                    .bind("auth-history")
                    .execute(service.db.pool())
                    .await
                    .unwrap();
            }
            for repair in [false, true] {
                if repair {
                    service.redact_legacy_history().await.unwrap();
                }
                let document = exported(&service, &collection.id).await;
                assert_eq!(
                    document["paths"]["/items"]["post"]["responses"]["201"]["x-unfour-history-id"],
                    "auth-history",
                    "materialized auth history must match (legacy={legacy}, repair={repair})"
                );
                assert_safe(&document);
            }
            let after = service
                .list_saved_requests("workspace-a".into())
                .await
                .unwrap();
            assert!(after[0].auth_json == saved.auth_json);
            assert!(after[0].headers_json == saved.headers_json);
            assert!(after[0].query_json == saved.query_json);
        }
    }
}

#[tokio::test]
async fn openapi_auth_matching_preserves_explicit_overrides_and_disabled_entries() {
    for auth in auth_cases() {
        for enabled in [false, true] {
            let service = service().await;
            let collection = service
                .create_collection("workspace-a".into(), "Auth conflicts".into())
                .await
                .unwrap();
            let mut input = input(&collection.id, false);
            input.auth_json = Some(auth.to_string());
            let query = auth["addTo"] == "query";
            let key = if query {
                "credential"
            } else if auth["type"] == "api-key" {
                " x-custom-credential "
            } else {
                " aUtHoRiZaTiOn "
            };
            let entries = if query {
                &mut input.query
            } else {
                &mut input.headers
            };
            let before = entries.len();
            entries.push(KeyValue {
                key: key.into(),
                value: "manual-canary-value".into(),
                enabled,
            });
            service.save_request(input.clone()).await.unwrap();
            let sent = service.materialize_auth(input).unwrap();
            let entries = if query { &sent.query } else { &sent.headers };
            assert_eq!(entries.len(), before + if enabled { 1 } else { 2 });
            if enabled {
                assert!(entries.last().unwrap().value == "manual-canary-value");
            }
            insert_history(&service, &sent, "override-history", false).await;
            if enabled {
                let row: (String, String) = sqlx::query_as(
                    "SELECT request_headers_json,request_query_json FROM api_history WHERE id=?",
                )
                .bind("override-history")
                .fetch_one(service.db.pool())
                .await
                .unwrap();
                assert!(!format!("{}{}", row.0, row.1).contains("manual-canary-value"));
            }
            let document = exported(&service, &collection.id).await;
            assert_eq!(
                document["paths"]["/items"]["post"]["responses"]["201"]["x-unfour-history-id"],
                "override-history"
            );
            assert_safe(&document);
        }
    }
}

#[tokio::test]
async fn openapi_query_auth_keys_keep_case_sensitive_override_rules() {
    let service = service().await;
    let collection = service
        .create_collection("workspace-a".into(), "Query case".into())
        .await
        .unwrap();
    let mut input = input(&collection.id, false);
    input.auth_json = Some(auth_cases()[3].to_string());
    input.query.push(KeyValue {
        key: "Credential".into(),
        value: "public-value".into(),
        enabled: true,
    });
    service.save_request(input.clone()).await.unwrap();
    let sent = service.materialize_auth(input).unwrap();
    assert_eq!(sent.query.len(), 3);
    assert_eq!(sent.query[1].key, "Credential");
    assert_eq!(sent.query[2].key, "credential");
    let repeated = service.materialize_auth(sent.clone()).unwrap();
    assert_eq!(repeated.query.len(), 3);
    insert_history(&service, &sent, "query-case-history", false).await;
    let document = exported(&service, &collection.id).await;
    assert_eq!(
        document["paths"]["/items"]["post"]["responses"]["201"]["x-unfour-history-id"],
        "query-case-history"
    );
    assert_safe(&document);
}

#[tokio::test]
async fn openapi_auth_matching_rejects_missing_or_different_auth_structure() {
    for auth in auth_cases() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".into(), "Auth mismatch".into())
            .await
            .unwrap();
        let mut input = input(&collection.id, false);
        input.auth_json = Some(auth.to_string());
        service.save_request(input.clone()).await.unwrap();
        // No fallback to the pre-auth definition, even for legacy rows.
        insert_history(&service, &input, "missing-auth", true).await;
        let mut sent = service.materialize_auth(input).unwrap();
        let entries = if auth["addTo"] == "query" {
            &mut sent.query
        } else {
            &mut sent.headers
        };
        entries.last_mut().unwrap().key = "Different-Credential".into();
        insert_history(&service, &sent, "wrong-auth-key", false).await;
        let document = exported(&service, &collection.id).await;
        assert!(document["paths"]["/items"]["post"]["responses"]["default"].is_object());
        assert_safe(&document);
    }
}
