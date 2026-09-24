use super::support::service;
use serde_json::{json, Value};
use unfour_core::models::ApiCollectionExportFormat;

fn native() -> Value {
    json!({"format":"unfour.collection","version":1,"collection":{
        "name":"Portable", "description":"A collection", "folders":[
            {"sourceId":"old-parent","parentSourceId":null,"name":"Parent","sortOrder":0},
            {"sourceId":"old-child","parentSourceId":"old-parent","name":"Child","sortOrder":1}
        ], "requests":[{
            "parentSourceId":"old-child","name":"Create","method":"POST","url":"https://example.test/items",
            "headers":[{"key":"X-Mode","value":"draft","enabled":false}],
            "query":[{"key":"page","value":"2","enabled":false}],
            "body":"[{\"key\":\"name\",\"value\":\"Ada\",\"enabled\":false}]","bodyKind":"form-urlencoded",
            "authJson":"{\"type\":\"bearer\",\"token\":\"{{credential}}\"}",
            "settingsJson":"{\"timeoutMs\":5000}","preRequestScript":"pm.variables.set('x', '1');",
            "postResponseScript":"pm.test('ok', () => pm.expect(1).to.equal(1));","scriptSchemaVersion":1,"sortOrder":3
        }]
    }})
}

#[tokio::test]
async fn native_and_postman_round_trip_hierarchy_scripts_auth_disabled_values() {
    let service = service().await;
    let content = native().to_string();
    let preview = service.preview_collection_import(&content).unwrap();
    assert_eq!(
        (
            preview.folder_count,
            preview.request_count,
            preview.script_count
        ),
        (2, 1, 2)
    );
    let imported = service
        .import_collection_openapi("workspace-a".into(), content)
        .await
        .unwrap();
    let id = imported.collection.unwrap().id;
    for format in [
        ApiCollectionExportFormat::Unfour,
        ApiCollectionExportFormat::Postman,
    ] {
        let exported = service
            .export_collection_exchange("workspace-a".into(), id.clone(), format)
            .await
            .unwrap();
        for internal in [
            "workspaceId",
            "history",
            "sync_status",
            "revision",
            "deletedAt",
            "remoteId",
        ] {
            assert!(!exported.content.contains(internal));
        }
        let result = service
            .import_collection_openapi("workspace-b".into(), exported.content)
            .await
            .unwrap();
        assert_eq!((result.folder_count, result.request_count), (2, 1));
        let id = result.collection.unwrap().id;
        let folders = service
            .list_collection_folders("workspace-b".into(), Some(id.clone()))
            .await
            .unwrap();
        assert!(folders.iter().all(|f| !f.id.starts_with("old-")));
        let child = folders.iter().find(|f| f.name == "Child").unwrap();
        let parent = folders.iter().find(|f| f.name == "Parent").unwrap();
        assert_eq!(child.parent_folder_id.as_ref(), Some(&parent.id));
        let request = service
            .list_saved_requests("workspace-b".into())
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.collection_id == id)
            .unwrap();
        assert_eq!(request.parent_folder_id.as_ref(), Some(&child.id));
        assert_eq!(request.method, "POST");
        assert_eq!(request.body_kind, "form-urlencoded");
        assert_eq!(
            serde_json::from_str::<Value>(request.body.as_ref().unwrap()).unwrap()[0]["enabled"],
            false
        );
        assert_eq!(
            serde_json::from_str::<Value>(&request.headers_json).unwrap()[0]["enabled"],
            false
        );
        assert_eq!(
            serde_json::from_str::<Value>(&request.query_json).unwrap()[0]["enabled"],
            false
        );
        assert_eq!(
            serde_json::from_str::<Value>(&request.auth_json).unwrap()["token"],
            "{{credential}}"
        );
        assert!(request.pre_request_script.unwrap().contains("pm.variables"));
        assert!(request.post_response_script.unwrap().contains("pm.test"));
        assert_eq!(
            serde_json::from_str::<Value>(&request.settings_json).unwrap()["timeoutMs"],
            5000
        );
    }
}

#[tokio::test]
async fn export_redacts_credentials_from_auth_headers_url_and_body() {
    let service = service().await;
    let mut value = native();
    let r = &mut value["collection"]["requests"][0];
    r["authJson"] = json!("{\"type\":\"bearer\",\"token\":\"auth-private-value\"}");
    r["headers"] = json!([{"key":"Cookie","value":"cookie-private-value","enabled":false}]);
    r["url"] = json!("https://user:url-private-value@example.test/items?token=query-private-value");
    r["bodyKind"] = json!("json");
    r["body"] = json!("{\"password\":\"body-private-value\",\"name\":\"Ada\"}");
    let id = service
        .import_collection_openapi("workspace-a".into(), value.to_string())
        .await
        .unwrap()
        .collection
        .unwrap()
        .id;
    for format in [
        ApiCollectionExportFormat::Unfour,
        ApiCollectionExportFormat::Postman,
    ] {
        let artifact = service
            .export_collection_exchange("workspace-a".into(), id.clone(), format)
            .await
            .unwrap();
        assert!(
            !artifact.content.contains("private-value"),
            "{}",
            artifact.content
        );
        assert!(artifact.content.contains("Ada"));
    }
}

#[tokio::test]
async fn transaction_failure_rolls_back_collection_folders_and_requests() {
    let service = service().await;
    sqlx::query("CREATE TRIGGER fail_import BEFORE INSERT ON api_requests BEGIN SELECT RAISE(ABORT, 'injected failure'); END").execute(service.db.pool()).await.unwrap();
    assert!(service
        .import_collection_openapi("workspace-a".into(), native().to_string())
        .await
        .is_err());
    for table in ["api_collections", "api_collection_folders", "api_requests"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(service.db.pool())
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[tokio::test]
async fn postman_preview_warns_and_file_paths_are_not_imported() {
    let service = service().await;
    let document = json!({"info":{"name":"Files","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"variable":[{"key":"base","value":"https://example.test"}],"item":[{"name":"Upload","event":[{"listen":"prerequest","script":{"exec":["pm.sendRequest('https://example.test');"]}}],"request":{"method":"POST","url":"{{base}}/upload","body":{"mode":"formdata","formdata":[{"key":"file","type":"file","src":"C:\\private\\document.pdf"}]}}}]});
    let preview = service
        .preview_collection_import(&document.to_string())
        .unwrap();
    for warning in [
        "reselectFiles",
        "variablesNotImported",
        "unsupportedScriptApi",
    ] {
        assert!(preview.warnings.contains(&warning.into()));
    }
    assert_eq!(preview.variables, vec!["base"]);
    let imported = service
        .import_collection_openapi("workspace-a".into(), document.to_string())
        .await
        .unwrap();
    let exported = service
        .export_collection_exchange(
            "workspace-a".into(),
            imported.collection.unwrap().id,
            ApiCollectionExportFormat::Unfour,
        )
        .await
        .unwrap();
    assert!(!exported.content.contains("private"));
    assert!(exported.content.contains("pm.sendRequest"));
}

#[tokio::test]
async fn preview_rejects_cycles_and_unknown_versions_before_writing() {
    let service = service().await;
    let mut value = native();
    value["collection"]["folders"][0]["parentSourceId"] = json!("old-child");
    assert!(service
        .preview_collection_import(&value.to_string())
        .is_err());
    value["version"] = json!(2);
    assert!(service
        .preview_collection_import(&value.to_string())
        .is_err());
    assert!(service
        .list_collections("workspace-a".into())
        .await
        .unwrap()
        .is_empty());
}
