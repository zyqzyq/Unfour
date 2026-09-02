use super::*;
#[path = "lib_tests/api_environment_override.rs"]
mod api_environment_override;
#[path = "lib_tests/script_rollback.rs"]
mod script_rollback;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use unfour_core::models::{
    ApiCollectionExportFormat, ApiRequestInput, DatabaseConnectionInput, ScriptExecutionStatus,
    SshConnectionInput, WorkspaceVariableInput,
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

fn api_script_test_input(workspace_id: String, url: String) -> ApiRequestInput {
    ApiRequestInput {
        workspace_id,
        name: None,
        parent_folder_id: None,
        collection_id: None,
        auth_json: None,
        method: "GET".to_string(),
        url,
        headers: vec![],
        query: vec![],
        body: None,
        body_kind: "none".to_string(),
        timeout_ms: Some(2_000),
        pre_request_script: None,
        post_response_script: None,
        script_schema_version: 1,
        temporary_variables: vec![],
    }
}

fn spawn_api_test_server() -> (String, std::sync::mpsc::Receiver<String>) {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("read test server address");
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept API request");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .expect("set read timeout");
        let mut request = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = stream.read(&mut chunk).expect("read API request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        sender
            .send(String::from_utf8_lossy(&request).into_owned())
            .expect("capture API request");
        let body = r#"{"ok":true}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("write API response");
    });
    (format!("http://{address}/echo"), receiver)
}

#[tokio::test]
async fn api_pre_request_script_mutates_the_outbound_request() {
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let (url, request) = spawn_api_test_server();
    let mut input = api_script_test_input(workspace_id, url);
    input.pre_request_script = Some(
        r#"
pm.request.headers.upsert({ key: "X-From-Script", value: "yes" });
pm.variables.set("source", "pre");
pm.request.url = pm.request.url + "?source={{source}}";
"#
        .to_string(),
    );

    let result = bus
        .send_api_request_with_scripts(input)
        .await
        .expect("execute scripted request");
    let outbound = request
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("capture outbound request");

    assert_eq!(result.pre_request.status, ScriptExecutionStatus::Success);
    assert_eq!(result.post_response.status, ScriptExecutionStatus::Skipped);
    assert_eq!(result.response.expect("HTTP response").status, 200);
    assert!(outbound.starts_with("GET /echo?source=pre HTTP/1.1"));
    assert!(outbound.to_ascii_lowercase().contains("x-from-script: yes"));
}

#[tokio::test]
async fn api_pre_request_failure_prevents_network_io() {
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind no-send server");
    listener
        .set_nonblocking(true)
        .expect("make no-send server nonblocking");
    let mut input = api_script_test_input(
        workspace_id,
        format!("http://{}/must-not-send", listener.local_addr().unwrap()),
    );
    input.pre_request_script = Some("throw new Error('stop before send')".to_string());

    let result = bus
        .send_api_request_with_scripts(input)
        .await
        .expect("return typed script failure");

    assert_eq!(result.pre_request.status, ScriptExecutionStatus::Failed);
    assert!(result.response.is_none());
    assert!(result.http_error.is_none());
    assert_eq!(result.post_response.status, ScriptExecutionStatus::Skipped);
    assert!(matches!(
        listener.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
}

#[tokio::test]
async fn api_post_response_failure_keeps_the_http_response() {
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let (url, request) = spawn_api_test_server();
    let mut input = api_script_test_input(workspace_id, url);
    input.post_response_script = Some("throw new Error('post failed')".to_string());

    let result = bus
        .send_api_request_with_scripts(input)
        .await
        .expect("return response and post-script failure");
    request
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("request reached server");

    assert_eq!(result.response.expect("HTTP response").status, 200);
    assert_eq!(result.post_response.status, ScriptExecutionStatus::Failed);
    assert!(result
        .post_response
        .error
        .expect("post-script error")
        .message
        .contains("post failed"));
}

#[tokio::test]
async fn api_environment_script_writes_commit_only_after_success() {
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .workspace_environment_create(workspace_id.clone(), "Scripts".to_string())
        .await
        .expect("create script environment");
    bus.workspace_environment_update(
        workspace_id.clone(),
        environment.id.clone(),
        environment.name,
        vec![WorkspaceVariableInput {
            id: None,
            key: "token".to_string(),
            value: "old".to_string(),
            is_secret: true,
            is_enabled: true,
            description: None,
            sort_order: 0,
        }],
    )
    .await
    .expect("seed script variable");
    bus.workspace_environment_set_active(workspace_id.clone(), Some(environment.id.clone()))
        .await
        .expect("activate script environment");

    let (url, request) = spawn_api_test_server();
    let mut success = api_script_test_input(workspace_id.clone(), url);
    success.pre_request_script = Some(r#"pm.environment.set("token", "committed")"#.to_string());
    bus.send_api_request_with_scripts(success)
        .await
        .expect("execute successful environment write");
    request
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("successful request reached server");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind rollback server");
    listener.set_nonblocking(true).unwrap();
    let mut failure = api_script_test_input(
        workspace_id.clone(),
        format!("http://{}/must-not-send", listener.local_addr().unwrap()),
    );
    failure.pre_request_script = Some(
        r#"pm.environment.set("token", "rolled-back"); throw new Error("rollback")"#.to_string(),
    );
    let result = bus
        .send_api_request_with_scripts(failure)
        .await
        .expect("return failed script result");

    let active = bus
        .workspace_environments_list(workspace_id)
        .await
        .expect("reload environments")
        .into_iter()
        .find(|item| item.id == environment.id)
        .expect("script environment");
    assert_eq!(result.pre_request.status, ScriptExecutionStatus::Failed);
    assert_eq!(active.variables[0].value, "committed");
    assert!(matches!(
        listener.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
}

#[tokio::test]
async fn api_request_without_scripts_uses_the_versioned_path_unchanged() {
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let (url, request) = spawn_api_test_server();

    let result = bus
        .send_api_request_with_scripts(api_script_test_input(workspace_id, url))
        .await
        .expect("execute request without scripts");
    request
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("request reached server");

    assert_eq!(result.response.expect("HTTP response").status, 200);
    assert_eq!(result.pre_request.status, ScriptExecutionStatus::Skipped);
    assert_eq!(result.post_response.status, ScriptExecutionStatus::Skipped);
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
        pre_request_script: Some("console.log('saved pre')".to_string()),
        post_response_script: Some("pm.test('saved post', () => {})".to_string()),
        script_schema_version: 1,
        temporary_variables: vec![],
    };

    let saved = bus.save_api_request(input).await.expect("save api request");
    assert_eq!(saved.name, "Test GET request");
    assert_eq!(saved.method, "GET");
    assert_eq!(saved.workspace_id, workspace_id);
    assert_eq!(
        saved.pre_request_script.as_deref(),
        Some("console.log('saved pre')")
    );
    assert_eq!(
        saved.post_response_script.as_deref(),
        Some("pm.test('saved post', () => {})")
    );
    assert_eq!(saved.script_schema_version, 1);

    // List should now have one request
    let listed = bus
        .list_saved_api_requests(workspace_id.clone())
        .await
        .expect("list saved requests");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, saved.id);
    assert_eq!(listed[0].name, "Test GET request");
    assert_eq!(listed[0].pre_request_script, saved.pre_request_script);
    assert_eq!(listed[0].post_response_script, saved.post_response_script);

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
        pre_request_script: None,
        post_response_script: None,
        script_schema_version: 1,
        temporary_variables: vec![],
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
        pre_request_script: None,
        post_response_script: None,
        script_schema_version: 1,
        temporary_variables: vec![],
    })
    .await
    .expect("save request");

    let artifact = bus
        .api_collection_export(
            workspace_id.clone(),
            collection.id,
            ApiCollectionExportFormat::Yaml,
        )
        .await
        .expect("export collection");

    assert_eq!(artifact.suggested_file_name, "Users-API.openapi.yaml");
    assert_eq!(artifact.media_type, "application/yaml");
    assert!(artifact.content.contains("openapi: 3.1.0"));
    assert!(artifact.content.contains("/users:"));
    assert!(artifact.content.contains("x-unfour-request-id"));
    assert!(!artifact.content.contains("secret"));

    let imported = bus
        .api_collection_import(workspace_id.clone(), artifact.content)
        .await
        .expect("import collection through command bus");
    assert!(imported.imported);
    assert_eq!(imported.request_count, 1);
    assert_eq!(
        imported.collection.expect("imported collection").name,
        "Users API"
    );
    assert_eq!(
        bus.api_collection_list(workspace_id)
            .await
            .expect("list collections after import")
            .len(),
        2
    );
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
            pre_request_script: None,
            post_response_script: None,
            script_schema_version: 1,
            temporary_variables: vec![],
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

    let workspace_id = workspace.workspace_id.clone();
    bus.save_database_connection(DatabaseConnectionInput {
        id: None,
        workspace_id: workspace_id.clone(),
        name: "Database".to_string(),
        driver: "postgres".to_string(),
        host: Some("db.internal".to_string()),
        port: Some(5432),
        database: Some("app".to_string()),
        username: Some("developer".to_string()),
        ssl_mode: None,
        sqlite_path: None,
        credential_ref: Some(format!(
            "unfour:{workspace_id}:database-password:database-secret"
        )),
        read_only: false,
    })
    .await
    .expect("save database connection");
    bus.save_ssh_connection(SshConnectionInput {
        id: None,
        workspace_id,
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
            pre_request_script: None,
            post_response_script: None,
            script_schema_version: 1,
            temporary_variables: vec![],
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

#[tokio::test]
async fn api_request_resolution_uses_current_workspace_environment_then_workspace_values() {
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    bus.workspace_variables_replace(
        workspace_id.clone(),
        vec![WorkspaceVariableInput {
            id: None,
            key: "HOST".to_string(),
            value: "workspace.example".to_string(),
            is_secret: false,
            is_enabled: true,
            description: None,
            sort_order: 0,
        }],
    )
    .await
    .expect("save workspace variable");
    let environment = bus
        .workspace_environment_create(workspace_id.clone(), "Development".to_string())
        .await
        .expect("create environment");
    bus.workspace_environment_update(
        workspace_id.clone(),
        environment.id.clone(),
        environment.name,
        vec![WorkspaceVariableInput {
            id: None,
            key: "HOST".to_string(),
            value: "environment.example".to_string(),
            is_secret: false,
            is_enabled: true,
            description: None,
            sort_order: 0,
        }],
    )
    .await
    .expect("save environment variable");
    bus.workspace_environment_set_active(workspace_id.clone(), Some(environment.id.clone()))
        .await
        .expect("activate environment");

    let input = ApiRequestInput {
        workspace_id: workspace_id.clone(),
        name: None,
        parent_folder_id: None,
        collection_id: None,
        auth_json: None,
        method: "POST".to_string(),
        url: "https://{{HOST}}/users".to_string(),
        headers: vec![KeyValue {
            key: "X-Origin".to_string(),
            value: "{{HOST}}".to_string(),
            enabled: true,
        }],
        query: vec![],
        body: Some("{\"host\":\"{{HOST}}\"}".to_string()),
        body_kind: "json".to_string(),
        timeout_ms: None,
        pre_request_script: None,
        post_response_script: None,
        script_schema_version: 1,
        temporary_variables: vec![],
    };
    let resolved = bus
        .resolve_api_request_input(input.clone())
        .await
        .expect("resolve request with active environment");
    assert_eq!(resolved.url, "https://environment.example/users");
    assert_eq!(resolved.headers[0].value, "environment.example");
    assert_eq!(
        resolved.body.as_deref(),
        Some("{\"host\":\"environment.example\"}")
    );

    bus.workspace_environment_set_active(workspace_id, None)
        .await
        .expect("clear active environment");
    let fallback = bus
        .resolve_api_request_input(input)
        .await
        .expect("resolve with workspace variables only");
    assert_eq!(fallback.url, "https://workspace.example/users");
}

#[tokio::test]
async fn api_request_resolution_prefers_temporary_variables() {
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    bus.workspace_variables_replace(
        workspace_id.clone(),
        vec![WorkspaceVariableInput {
            id: None,
            key: "HOST".to_string(),
            value: "workspace.example".to_string(),
            is_secret: false,
            is_enabled: true,
            description: None,
            sort_order: 0,
        }],
    )
    .await
    .expect("save workspace variable");

    let mut input = api_script_test_input(workspace_id, "https://{{HOST}}/{{PATH}}".to_string());
    input.headers = vec![KeyValue {
        key: "X-{{PATH}}".to_string(),
        value: "{{HOST}}".to_string(),
        enabled: true,
    }];
    input.temporary_variables = vec![
        KeyValue {
            key: "HOST".to_string(),
            value: "temporary.example".to_string(),
            enabled: true,
        },
        KeyValue {
            key: "PATH".to_string(),
            value: "users".to_string(),
            enabled: true,
        },
    ];

    let resolved = bus
        .resolve_api_request_input(input)
        .await
        .expect("resolve request with temporary variables");
    assert_eq!(resolved.url, "https://temporary.example/users");
    assert_eq!(resolved.headers[0].key, "X-users");
    assert_eq!(resolved.headers[0].value, "temporary.example");
}
