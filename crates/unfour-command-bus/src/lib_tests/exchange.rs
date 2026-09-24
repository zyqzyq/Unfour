use super::*;
use serde_json::{json, Value};

#[tokio::test]
async fn exchange_collection_names_validate_and_import_as_copies() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let first = bus
        .api_collection_create(workspace.clone(), "Example".into())
        .await
        .unwrap();
    assert!(matches!(
        bus.api_collection_create(workspace.clone(), "EXAMPLE".into())
            .await,
        Err(AppError::Validation(_))
    ));
    let other = bus
        .api_collection_create(workspace.clone(), "Other".into())
        .await
        .unwrap();
    assert!(matches!(
        bus.api_collection_rename(workspace.clone(), other.id, "example".into())
            .await,
        Err(AppError::Validation(_))
    ));
    let doc = json!({"format":"unfour.collection","version":1,"collection":{"name":"example","description":null,"folders":[],"requests":[]}}).to_string();
    let preview = bus
        .api_collection_import_preview(workspace.clone(), &doc)
        .await
        .unwrap();
    assert!(preview.conflict);
    assert_eq!(preview.target_name, "example (Copy 1)");
    let imported = bus
        .api_collection_import(workspace, doc)
        .await
        .unwrap()
        .collection
        .unwrap();
    assert_eq!(imported.name, preview.target_name);
    assert_ne!(imported.id, first.id);
}

#[tokio::test]
async fn exchange_environment_copy_preserves_metadata_and_redacts_secrets() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let doc = json!({"format":"unfour.environment","version":1,"name":"Development","variables":[
        {"id":"old-id","key":"ordinary_name","value":"private-environment-value","isSecret":true,"isEnabled":false,"description":"Description"},
        {"key":"base_url","value":"https://example.test","isSecret":false,"isEnabled":true}
    ]}).to_string();
    let first = bus
        .workspace_environment_import(workspace.clone(), doc.clone())
        .await
        .unwrap();
    assert_eq!(first.name, "Development");
    assert!(first.variables[0].is_secret);
    assert!(!first.variables[0].is_enabled);
    assert_ne!(first.variables[0].id, "old-id");
    let preview = bus
        .workspace_environment_import_preview(workspace.clone(), &doc)
        .await
        .unwrap();
    assert_eq!(preview["conflict"], true);
    assert!(!preview.to_string().contains("private-environment-value"));
    let second = bus
        .workspace_environment_import(workspace.clone(), doc)
        .await
        .unwrap();
    assert_eq!(second.name, "Development (Copy 1)");
    assert_ne!(first.id, second.id);
    for format in ["unfour", "postman"] {
        let exported = bus
            .workspace_environment_export(workspace.clone(), first.id.clone(), format.into())
            .await
            .unwrap();
        assert!(!exported.content.contains("private-environment-value"));
        assert!(!exported.content.contains(&first.id));
        let imported = bus
            .workspace_environment_import(workspace.clone(), exported.content)
            .await
            .unwrap();
        let variable = imported
            .variables
            .iter()
            .find(|v| v.key == "ordinary_name")
            .unwrap();
        assert!(variable.is_secret);
        assert!(!variable.is_enabled);
        assert_eq!(variable.value, "");
        assert_eq!(variable.description.as_deref(), Some("Description"));
    }
    let current = bus.workspace_environments_list(workspace).await.unwrap();
    assert_eq!(
        current.iter().find(|e| e.id == first.id).unwrap().variables[0].value,
        "private-environment-value"
    );
}

#[tokio::test]
async fn exchange_environment_copy_shortens_a_max_length_name() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let name = "N".repeat(80);
    let doc =
        json!({"format":"unfour.environment","version":1,"name":name,"variables":[]}).to_string();
    let first = bus
        .workspace_environment_import(workspace.clone(), doc.clone())
        .await
        .unwrap();
    assert_eq!(first.name, name);
    let second = bus
        .workspace_environment_import(workspace, doc)
        .await
        .unwrap();
    assert_eq!(second.name.chars().count(), 80);
    assert!(second.name.ends_with(" (Copy 1)"));
    assert!(second.name.starts_with(&"N".repeat(71)));
    assert_ne!(first.id, second.id);
}

#[tokio::test]
async fn exchange_openapi_uses_secret_metadata_for_non_sensitive_names() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    bus.workspace_environment_import(workspace.clone(),json!({"_postman_variable_scope":"environment","name":"Secret","values":[{"key":"base_url","value":"https://private-location.example","type":"secret","enabled":true}]}).to_string()).await.unwrap();
    let collection = bus
        .api_collection_create(workspace.clone(), "Secret metadata".into())
        .await
        .unwrap();
    let mut request = api_script_test_input(workspace.clone(), "{{base_url}}/users".into());
    request.collection_id = Some(collection.id.clone());
    bus.save_api_request(request).await.unwrap();
    for format in [
        ApiCollectionExportFormat::Json,
        ApiCollectionExportFormat::Yaml,
    ] {
        let exported = bus
            .api_collection_export(workspace.clone(), collection.id.clone(), format)
            .await
            .unwrap();
        assert!(!exported.content.contains("private-location"));
        assert!(exported.content.contains("base_url"));
    }
}

#[tokio::test]
async fn exchange_environment_failed_write_has_no_partial_environment() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    // Fail after the environment row is inserted, while variables are written.
    sqlx::query("CREATE TRIGGER fail_environment BEFORE INSERT ON workspace_environment_variables BEGIN SELECT RAISE(ABORT, 'injected failure'); END").execute(bus.db.pool()).await.unwrap();
    let document = json!({"_postman_variable_scope":"environment","name":"Rollback","values":[{"key":"base","value":"ok","enabled":true}]}).to_string();
    assert!(bus
        .workspace_environment_import(workspace.clone(), document)
        .await
        .is_err());
    assert!(bus
        .workspace_environments_list(workspace)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn exchange_environment_heuristic_is_additional_and_invalid_variables_fail() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let doc = json!({"_postman_variable_scope":"environment","name":"Keys","values":[{"key":"api_token","value":"sensitive-value","enabled":false}]}).to_string();
    let imported = bus
        .workspace_environment_import(workspace.clone(), doc)
        .await
        .unwrap();
    assert!(imported.variables[0].is_secret);
    let exported = bus
        .workspace_environment_export(workspace.clone(), imported.id, "postman".into())
        .await
        .unwrap();
    let value: Value = serde_json::from_str(&exported.content).unwrap();
    assert_eq!(value["values"][0]["value"], "");
    assert_eq!(value["values"][0]["type"], "secret");
    let duplicate = json!({"format":"unfour.environment","version":1,"name":"Invalid","variables":[{"key":"x","value":"a"},{"key":"x","value":"b"}]}).to_string();
    assert!(bus
        .workspace_environment_import(workspace, duplicate)
        .await
        .is_err());
}
