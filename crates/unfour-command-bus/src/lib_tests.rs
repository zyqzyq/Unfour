use super::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use unfour_core::models::{
    ApiCollectionExportFormat, ApiRequestInput, DatabaseConnectionInput, SshConnectionInput,
};
use unfour_local_storage::LocalDb;

async fn test_bus() -> CommandBus {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect in-memory sqlite");
    let db = LocalDb::from_pool(pool);
    db.migrate().await.expect("run migrations");
    CommandBus::from_db(db).await.expect("build command bus")
}

#[tokio::test]
async fn workspace_create_and_list() {
    let bus = test_bus().await;

    // from_db seeds a default workspace, so list should have at least one
    let initial_state = bus.list_workspaces().await.expect("list workspaces");
    let initial_count = initial_state.workspaces.len();
    assert!(
        initial_count >= 1,
        "should have the default workspace seeded"
    );

    // Create a new workspace
    let created = bus
        .create_workspace("Integration Test WS".to_string())
        .await
        .expect("create workspace");
    assert_eq!(created.name, "Integration Test WS");
    assert!(!created.id.is_empty());
    assert!(!created.is_default);

    // List should now include the new workspace
    let state = bus.list_workspaces().await.expect("list workspaces");
    assert_eq!(state.workspaces.len(), initial_count + 1);
    assert!(
        state.workspaces.iter().any(|w| w.id == created.id),
        "newly created workspace should appear in the list"
    );

    // The new workspace should be active (create sets it active)
    assert_eq!(state.active_workspace_id, created.id);
}

#[tokio::test]
async fn save_and_list_api_requests() {
    let bus = test_bus().await;

    // Get the default workspace
    let state = bus.list_workspaces().await.expect("list workspaces");
    let workspace_id = state.active_workspace_id.clone();

    // Initially no saved requests
    let initial = bus
        .list_saved_api_requests(workspace_id.clone())
        .await
        .expect("list saved requests");
    assert!(initial.is_empty(), "no saved requests initially");

    // Save a request
    let input = ApiRequestInput {
        workspace_id: workspace_id.clone(),
        name: Some("Test GET request".to_string()),
        parent_folder_id: None,
        collection_id: None,
        auth_json: None,
        method: "GET".to_string(),
        url: "https://httpbin.org/get".to_string(),
        headers: vec![],
        query: vec![],
        body: None,
        body_kind: "none".to_string(),
        timeout_ms: None,
    };

    let saved = bus.save_api_request(input).await.expect("save api request");
    assert_eq!(saved.name, "Test GET request");
    assert_eq!(saved.method, "GET");
    assert_eq!(saved.workspace_id, workspace_id);

    // List should now have one request
    let listed = bus
        .list_saved_api_requests(workspace_id.clone())
        .await
        .expect("list saved requests");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, saved.id);
    assert_eq!(listed[0].name, "Test GET request");

    // Save a second request
    let input2 = ApiRequestInput {
        workspace_id: workspace_id.clone(),
        name: Some("Test POST request".to_string()),
        parent_folder_id: None,
        collection_id: None,
        auth_json: None,
        method: "POST".to_string(),
        url: "https://httpbin.org/post".to_string(),
        headers: vec![],
        query: vec![],
        body: Some(r#"{"key":"value"}"#.to_string()),
        body_kind: "json".to_string(),
        timeout_ms: None,
    };

    let saved2 = bus
        .save_api_request(input2)
        .await
        .expect("save second api request");
    assert_eq!(saved2.parent_folder_id, None);

    let listed2 = bus
        .list_saved_api_requests(workspace_id)
        .await
        .expect("list saved requests after second save");
    assert_eq!(listed2.len(), 2);
}

#[tokio::test]
async fn collection_openapi_export_uses_command_bus_and_persisted_requests() {
    let bus = test_bus().await;
    let state = bus.list_workspaces().await.expect("list workspaces");
    let workspace_id = state.active_workspace_id;
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Users API".to_string())
        .await
        .expect("create collection");
    bus.save_api_request(ApiRequestInput {
        workspace_id: workspace_id.clone(),
        name: Some("List users".to_string()),
        parent_folder_id: None,
        collection_id: Some(collection.id.clone()),
        auth_json: Some(r#"{"type":"bearer","token":"secret"}"#.to_string()),
        method: "GET".to_string(),
        url: "https://api.example.test/users".to_string(),
        headers: vec![],
        query: vec![],
        body: None,
        body_kind: "none".to_string(),
        timeout_ms: None,
    })
    .await
    .expect("save request");

    let artifact = bus
        .api_collection_export(workspace_id, collection.id, ApiCollectionExportFormat::Yaml)
        .await
        .expect("export collection");

    assert_eq!(artifact.suggested_file_name, "Users-API.openapi.yaml");
    assert_eq!(artifact.media_type, "application/yaml");
    assert!(artifact.content.contains("openapi: 3.1.0"));
    assert!(artifact.content.contains("/users:"));
    assert!(artifact.content.contains("x-unfour-request-id"));
    assert!(!artifact.content.contains("secret"));
}

#[tokio::test]
async fn execute_saved_api_request_rejects_mismatched_workspace() {
    let bus = test_bus().await;
    let state = bus.list_workspaces().await.expect("list workspaces");
    let workspace_id = state.active_workspace_id.clone();
    let saved = bus
        .save_api_request(ApiRequestInput {
            workspace_id: workspace_id.clone(),
            name: Some("Saved GET".to_string()),
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.invalid/get".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "none".to_string(),
            timeout_ms: None,
        })
        .await
        .expect("save request");
    let other_workspace = bus
        .create_workspace("Other Workspace".to_string())
        .await
        .expect("create other workspace");

    let error = bus
        .execute_saved_api_request_in_workspace(Some(other_workspace.id), &saved.id, None)
        .await
        .expect_err("workspace mismatch should be rejected before sending");

    assert_eq!(error.code(), "NOT_FOUND");
}

#[tokio::test]
async fn workspace_rename_updates_state() {
    let bus = test_bus().await;

    let created = bus
        .create_workspace("Rename Me".to_string())
        .await
        .expect("create workspace");

    let renamed = bus
        .rename_workspace(created.id.clone(), "Renamed Workspace".to_string())
        .await
        .expect("rename workspace");
    assert_eq!(renamed.name, "Renamed Workspace");
    assert_eq!(renamed.id, created.id);

    let state = bus.list_workspaces().await.expect("list workspaces");
    let ws = state
        .workspaces
        .iter()
        .find(|w| w.id == created.id)
        .expect("workspace should still exist");
    assert_eq!(ws.name, "Renamed Workspace");
}

#[tokio::test]
async fn read_commands_return_current_workspace_and_safe_connections() {
    let bus = test_bus().await;
    let workspace = bus
        .execute_read(ReadCommand::CurrentWorkspace)
        .await
        .expect("read current workspace");
    let ReadCommandResult::CurrentWorkspace(workspace) = workspace else {
        panic!("expected current workspace result");
    };
    assert_eq!(workspace.source, "command-bus");
    assert_eq!(workspace.workspace_root, None);

    bus.save_database_connection(DatabaseConnectionInput {
        id: None,
        workspace_id: workspace.workspace_id.clone(),
        name: "Database".to_string(),
        driver: "postgres".to_string(),
        host: Some("db.internal".to_string()),
        port: Some(5432),
        database: Some("app".to_string()),
        username: Some("developer".to_string()),
        ssl_mode: None,
        sqlite_path: None,
        credential_ref: Some("database-secret".to_string()),
        read_only: false,
    })
    .await
    .expect("save database connection");
    bus.save_ssh_connection(SshConnectionInput {
        id: None,
        workspace_id: workspace.workspace_id,
        name: "SSH".to_string(),
        host: "ssh.internal".to_string(),
        port: Some(22),
        username: "developer".to_string(),
        auth_kind: "private-key".to_string(),
        key_path: Some("C:\\sensitive\\id_ed25519".to_string()),
        credential_ref: Some("ssh-secret".to_string()),
        secret: None,
    })
    .await
    .expect("save ssh connection");

    let result = bus
        .execute_read(ReadCommand::ListConnections {
            connection_type: ConnectionType::All,
        })
        .await
        .expect("list safe connections");
    let ReadCommandResult::Connections(result) = result else {
        panic!("expected connection list result");
    };
    assert_eq!(result.count, 2);
    assert_eq!(result.source, "command-bus");

    let json = serde_json::to_string(&result).expect("serialize safe result");
    assert!(!json.contains("credential"));
    assert!(!json.contains("developer"));
    assert!(!json.contains("id_ed25519"));
    assert!(json.contains("db.internal"));
    assert!(json.contains("ssh.internal"));
}

#[tokio::test]
async fn api_connection_filter_is_empty_until_an_api_connection_model_exists() {
    let bus = test_bus().await;
    let result = bus
        .execute_read(ReadCommand::ListConnections {
            connection_type: ConnectionType::Api,
        })
        .await
        .expect("list api connections");
    let ReadCommandResult::Connections(result) = result else {
        panic!("expected connection list result");
    };

    assert!(result.connections.is_empty());
    assert_eq!(result.count, 0);
}

#[tokio::test]
async fn api_read_commands_use_real_collection_ids() {
    let bus = test_bus().await;
    let state = bus.list_workspaces().await.expect("list workspaces");
    let workspace_id = state.active_workspace_id;
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Public APIs".to_string())
        .await
        .expect("create collection");
    let empty_collection = bus
        .api_collection_create(workspace_id.clone(), "Empty APIs".to_string())
        .await
        .expect("create empty collection");
    let saved = bus
        .save_api_request(ApiRequestInput {
            workspace_id: workspace_id.clone(),
            name: Some("List users".to_string()),
            parent_folder_id: None,
            collection_id: Some(collection.id.clone()),
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.test/users".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "json".to_string(),
            timeout_ms: None,
        })
        .await
        .expect("save request");

    let collections = bus
        .execute_read(ReadCommand::ApiListCollections {
            workspace_id: Some(workspace_id.clone()),
        })
        .await
        .expect("list api collections");
    let ReadCommandResult::ApiCollections(collections) = collections else {
        panic!("expected api collections");
    };
    let public = collections
        .collections
        .iter()
        .find(|item| item.id == collection.id)
        .expect("public collection summary");
    assert_eq!(public.request_count, 1);
    assert!(collections
        .collections
        .iter()
        .any(|item| item.id == empty_collection.id && item.request_count == 0));

    let requests = bus
        .execute_read(ReadCommand::ApiListRequests {
            workspace_id: Some(workspace_id),
            collection_id: Some(collection.id.clone()),
        })
        .await
        .expect("list api requests");
    let ReadCommandResult::ApiRequests(requests) = requests else {
        panic!("expected api requests");
    };
    assert_eq!(requests.count, 1);
    assert_eq!(requests.requests[0].id, saved.id);
    assert_eq!(requests.requests[0].collection_id, collection.id);
}
