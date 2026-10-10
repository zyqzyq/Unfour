use super::super::*;
use super::support::service;
use unfour_core::models::ApiCollectionExportFormat;

pub(super) fn input(collection: &str, sensitive: bool) -> ApiRequestInput {
    let rows = |key: &str, value: &str| KeyValue {
        key: key.into(),
        value: value.into(),
        enabled: true,
    };
    ApiRequestInput {
        workspace_id: "workspace-a".into(),
        name: Some(
            if sensitive {
                "Secret request"
            } else {
                "Ordinary request"
            }
            .into(),
        ),
        method: "POST".into(),
        url: if sensitive {
            "https://example.test/items?token=url-secret"
        } else {
            "https://example.test/items"
        }
        .into(),
        headers: if sensitive {
            [
                "Authorization",
                "Cookie",
                "Proxy-Authorization",
                "X-Api-Key",
                "X-Auth-Token",
            ]
            .into_iter()
            .map(|key| rows(key, "header-secret"))
            .collect()
        } else {
            vec![rows("Accept", "application/json")]
        },
        query: vec![rows(
            if sensitive { "password" } else { "page" },
            if sensitive { "query-secret" } else { "1" },
        )],
        body: Some(
            if sensitive {
                r#"{"password":"body-secret","note":"keep"}"#
            } else {
                r#"{"note":"keep"}"#
            }
            .into(),
        ),
        body_kind: "json".into(),
        auth_json: None,
        collection_id: Some(collection.into()),
        parent_folder_id: None,
        timeout_ms: None,
        pre_request_script: None,
        post_response_script: None,
        script_schema_version: 1,
        multipart_parts: vec![],
        temporary_variables: vec![],
    }
}

pub(super) async fn insert_history(
    service: &ApiClientService,
    input: &ApiRequestInput,
    id: &str,
    legacy: bool,
) {
    let headers = vec![KeyValue {
        key: "Content-Type".into(),
        value: "application/json".into(),
        enabled: true,
    }];
    let body = r#"{"result":"ok","token":"response-secret"}"#;
    let safe = history_redaction::sanitize(input, &headers, body).unwrap();
    sqlx::query("INSERT INTO api_history (id,workspace_id,name,method,url,request_headers_json,request_query_json,request_body,request_body_kind,status,response_headers_json,response_body_preview,created_at,updated_at,redaction_version) VALUES (?,?,?,?,?,?,?,?,?,201,?,?, '2026-10-09', '2026-10-09',?)")
        .bind(id).bind(&input.workspace_id).bind(if legacy { &input.name } else { &safe.name })
        .bind("post").bind(if legacy { &input.url } else { &safe.url })
        .bind(if legacy { serde_json::to_string_pretty(&input.headers).unwrap() } else { safe.headers })
        .bind(if legacy { serde_json::to_string_pretty(&input.query).unwrap() } else { safe.query })
        .bind(if legacy { input.body.clone() } else { safe.body })
        .bind(&input.body_kind).bind(serde_json::to_string(&headers).unwrap())
        .bind(if legacy { body.to_owned() } else { safe.response_body }).bind(if legacy { 0 } else { 1 })
        .execute(service.db.pool()).await.unwrap();
}

pub(super) async fn exported(service: &ApiClientService, collection: &str) -> serde_json::Value {
    let artifact = service
        .export_collection_openapi(
            "workspace-a".into(),
            collection.into(),
            ApiCollectionExportFormat::Json,
            vec![],
        )
        .await
        .unwrap();
    assert!(!artifact.content.contains("response-secret"));
    serde_json::from_str(&artifact.content).unwrap()
}

#[tokio::test]
async fn openapi_associates_redacted_plain_and_legacy_history_through_service() {
    for (sensitive, legacy, form) in [
        (true, false, false),
        (false, false, false),
        (true, true, false),
        (false, true, false),
        (true, false, true),
        (false, false, true),
        (true, true, true),
        (false, true, true),
    ] {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".into(), "Examples".into())
            .await
            .unwrap();
        let mut input = input(&collection.id, sensitive);
        if form {
            input.body_kind = "form-urlencoded".into();
            input.body = Some(
                if sensitive {
                    "password=body-secret&note=keep"
                } else {
                    "note=keep"
                }
                .into(),
            );
        }
        let request = service.save_request(input.clone()).await.unwrap();
        insert_history(&service, &input, "matching", legacy).await;

        for repair in [false, true] {
            if repair {
                service.redact_legacy_history().await.unwrap();
            }
            let document = exported(&service, &collection.id).await;
            let response = &document["paths"]["/items"]["post"]["responses"]["201"];
            assert_eq!(
                response["x-unfour-history-id"], "matching",
                "sensitive={sensitive}, legacy={legacy}, repair={repair}"
            );
            assert_eq!(
                response["content"]["application/json"]["example"]["result"],
                "ok"
            );
            let text = document.to_string();
            for secret in ["url-secret", "header-secret", "query-secret", "body-secret"] {
                assert!(!text.contains(secret));
            }
        }
        // Export/repair must never modify the local request definition.
        let saved = service
            .list_saved_requests("workspace-a".into())
            .await
            .unwrap()
            .remove(0);
        assert_eq!(saved.id, request.id);
        assert_eq!(saved.url, input.url);
        assert_eq!(saved.body, input.body);
    }
}

#[tokio::test]
async fn openapi_history_matching_keeps_non_secret_and_workspace_discriminators() {
    let service = service().await;
    let collection = service
        .create_collection("workspace-a".into(), "Examples".into())
        .await
        .unwrap();
    let input = input(&collection.id, true);
    service.save_request(input.clone()).await.unwrap();
    let mut changed = input.clone();
    changed.body = Some(r#"{"password":"body-secret","note":"different"}"#.into());
    insert_history(&service, &changed, "wrong-body", false).await;
    changed = input.clone();
    changed.query.push(KeyValue {
        key: "page".into(),
        value: "2".into(),
        enabled: true,
    });
    insert_history(&service, &changed, "wrong-query", false).await;
    changed = input.clone();
    changed.url = "https://example.test/other?token=url-secret".into();
    insert_history(&service, &changed, "wrong-url", false).await;
    changed = input.clone();
    changed.workspace_id = "workspace-b".into();
    insert_history(&service, &changed, "other-workspace", false).await;
    assert!(
        exported(&service, &collection.id).await["paths"]["/items"]["post"]["responses"]["default"]
            .is_object()
    );
}
