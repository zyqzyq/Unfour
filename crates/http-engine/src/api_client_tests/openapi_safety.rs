use super::super::*;
use super::openapi_history::{input, insert_history};
use super::support::service;
use unfour_core::models::ApiCollectionExportFormat;

async fn export(service: &ApiClientService, collection: &str, yaml: bool) -> serde_json::Value {
    let artifact = service
        .export_collection_openapi(
            "workspace-a".into(),
            collection.into(),
            if yaml {
                ApiCollectionExportFormat::Yaml
            } else {
                ApiCollectionExportFormat::Json
            },
            vec![],
        )
        .await
        .expect("collection export must succeed");
    if yaml {
        serde_yaml_ng::from_str(&artifact.content).unwrap()
    } else {
        serde_json::from_str(&artifact.content).unwrap()
    }
}

fn auth_cases(value: &str) -> [serde_json::Value; 4] {
    [
        serde_json::json!({"type":"bearer","token":value}),
        serde_json::json!({"type":"basic","username":"fixture-user","password":value}),
        serde_json::json!({"type":"api-key","key":"X-Credential","value":value,"addTo":"header"}),
        serde_json::json!({"type":"api-key","key":"credential","value":value,"addTo":"query"}),
    ]
}

#[tokio::test]
async fn openapi_mixed_collection_isolates_invalid_auth_without_matching_fallback() {
    let service = service().await;
    let collection = service
        .create_collection("workspace-a".into(), "Mixed auth".into())
        .await
        .unwrap();
    let invalid_auth = [
        r#"{"type":"bearer"}"#,
        r#"{"type":"bearer","token":42}"#,
        r#"{"type":"basic","username":"fixture-user"}"#,
        r#"{"type":"basic","username":false,"password":"invalid-auth-canary"}"#,
        r#"{"type":"api-key","key":"X-Credential"}"#,
        r#"{"type":"api-key","key":42,"value":"invalid-auth-canary"}"#,
        r#"{"type":"api-key","key":"X-Credential","value":"invalid-auth-canary","addTo":"body"}"#,
        r#"{"type":"api-key","key":"X-Credential","value":"invalid-auth-canary","addTo":42}"#,
        r#"{"type":"unsupported","token":"invalid-auth-canary"}"#,
        r#"{"type":42,"token":"invalid-auth-canary"}"#,
        r#"{"token":"invalid-auth-canary"}"#,
        "null",
        "[]",
        r#""invalid-auth-canary""#,
        "{invalid-auth-canary",
    ];
    for (index, auth) in invalid_auth.iter().enumerate() {
        let mut request = input(&collection.id, false);
        request.url = format!("https://example.test/invalid/{index}");
        // Both a pre-auth snapshot and an explicit override are tempting matches.
        // Neither may be associated after Auth can no longer be materialized.
        if index % 2 == 0 {
            request.headers.push(KeyValue {
                key: "Authorization".into(),
                value: "Bearer override-auth-canary".into(),
                enabled: true,
            });
        }
        let saved = service.save_request(request.clone()).await.unwrap();
        insert_history(&service, &request, &format!("invalid-{index}"), true).await;
        // Model an imported/old malformed definition without weakening save validation.
        sqlx::query("UPDATE api_requests SET auth_json=? WHERE id=?")
            .bind(auth)
            .bind(saved.id)
            .execute(service.db.pool())
            .await
            .unwrap();
    }
    for (index, auth) in auth_cases("valid-auth-canary").iter().enumerate() {
        let mut request = input(&collection.id, false);
        request.url = format!("https://example.test/valid/{index}");
        request.auth_json = Some(auth.to_string());
        service.save_request(request.clone()).await.unwrap();
        let sent = service.materialize_auth(request).unwrap();
        insert_history(&service, &sent, &format!("valid-{index}"), false).await;
    }
    for yaml in [false, true] {
        let document = export(&service, &collection.id, yaml).await;
        assert_eq!(
            document["paths"].as_object().unwrap().len(),
            invalid_auth.len() + 4
        );
        for index in 0..invalid_auth.len() {
            let operation = &document["paths"][format!("/invalid/{index}")]["post"];
            assert_eq!(operation["summary"], "Ordinary request");
            assert_eq!(
                operation["requestBody"]["content"]["application/json"]["example"]["note"],
                "keep"
            );
            let responses = operation["responses"].as_object().unwrap();
            assert_eq!(responses.len(), 1);
            assert!(
                responses.contains_key("default"),
                "invalid auth must skip history"
            );
        }
        for index in 0..4 {
            assert_eq!(
                document["paths"][format!("/valid/{index}")]["post"]["responses"]["201"]
                    ["x-unfour-history-id"],
                format!("valid-{index}")
            );
        }
        let text = document.to_string();
        for canary in [
            "invalid-auth-canary",
            "override-auth-canary",
            "valid-auth-canary",
        ] {
            assert!(
                !text.contains(canary),
                "export must not contain credentials"
            );
        }
    }
}

#[tokio::test]
async fn openapi_rotated_auth_omits_examples_when_historical_secrets_are_lost() {
    for (index, old_auth) in auth_cases("historical-echo-canary").iter().enumerate() {
        for plaintext in [false, true] {
            for version in [0, 1] {
                let service = service().await;
                let collection = service
                    .create_collection("workspace-a".into(), "Rotated auth".into())
                    .await
                    .unwrap();
                let mut old = input(&collection.id, false);
                old.auth_json = Some(old_auth.to_string());
                let sent = service.materialize_auth(old.clone()).unwrap();
                insert_history(&service, &sent, "rotated", plaintext).await;
                if !plaintext && version == 1 {
                    // Equivalent imported JSON may escape the marker's brackets.
                    let rows: (String, String) = sqlx::query_as("SELECT request_headers_json,request_query_json FROM api_history WHERE id='rotated'")
                        .fetch_one(service.db.pool()).await.unwrap();
                    sqlx::query("UPDATE api_history SET request_headers_json=?,request_query_json=? WHERE id='rotated'")
                        .bind(rows.0.replace("<redacted>", r"\u003credacted\u003e"))
                        .bind(rows.1.replace("<redacted>", r"\u003credacted\u003e"))
                        .execute(service.db.pool()).await.unwrap();
                }
                // Old redaction only handled sensitive field names. A prior Token
                // survived in an ordinary body/header even when the request was redacted.
                sqlx::query("UPDATE api_history SET response_body_preview=?,response_headers_json=?,redaction_version=? WHERE id='rotated'")
                    .bind(r#"{"echo":"historical-echo-canary","result":"ok"}"#)
                    .bind(r#"[{"key":"Content-Type","value":"application/json","enabled":true},{"key":"X-Echo","value":"historical-echo-canary","enabled":true}]"#)
                    .bind(version)
                    .execute(service.db.pool()).await.unwrap();
                let mut current = old;
                current.auth_json = Some(auth_cases("rotated-current-canary")[index].to_string());
                let saved = service.save_request(current).await.unwrap();
                for repair in [false, true] {
                    if repair {
                        service.redact_legacy_history().await.unwrap();
                        service.redact_legacy_history().await.unwrap();
                    }
                    let before: (String, String, String) = sqlx::query_as("SELECT request_headers_json,response_headers_json,response_body_preview FROM api_history WHERE id='rotated'")
                        .fetch_one(service.db.pool()).await.unwrap();
                    for yaml in [false, true] {
                        let document = export(&service, &collection.id, yaml).await;
                        let response = &document["paths"]["/items"]["post"]["responses"]["201"];
                        assert_eq!(
                            response["x-unfour-history-id"], "rotated",
                            "matching must survive rotation"
                        );
                        assert!(
                            !document.to_string().contains("historical-echo-canary"),
                            "lost historical credentials must not leak"
                        );
                        assert!(!document.to_string().contains("rotated-current-canary"));
                        if !plaintext || (repair && version == 0 && index == 0) || index == 1 {
                            assert!(
                                response["content"].is_null(),
                                "uncertain response body must be omitted"
                            );
                            assert!(
                                response["headers"].is_null(),
                                "uncertain response headers must be omitted"
                            );
                        } else {
                            assert_eq!(
                                response["content"]["application/json"]["example"]["result"],
                                "ok"
                            );
                            assert_eq!(
                                response["content"]["application/json"]["example"]["echo"],
                                "<redacted>"
                            );
                        }
                    }
                    let after: (String, String, String) = sqlx::query_as("SELECT request_headers_json,response_headers_json,response_body_preview FROM api_history WHERE id='rotated'")
                        .fetch_one(service.db.pool()).await.unwrap();
                    assert!(before == after, "export must remain read-only");
                }
                let after = service
                    .list_saved_requests("workspace-a".into())
                    .await
                    .unwrap();
                assert!(
                    after[0].auth_json == saved.auth_json,
                    "rotation definition must be preserved"
                );
            }
        }
    }
}
