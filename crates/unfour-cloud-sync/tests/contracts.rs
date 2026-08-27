use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use unfour_cloud_sync::{
    parse_snapshot_item, ApiErrorEnvelope, ChangesPage, DesktopSessionCredential,
    DesktopSessionProvider, HttpSyncTransport, PushOperation, PushRequest, PushResponse,
    SnapshotPage, SyncEntityType, SyncError, SyncOperation, SyncTransport, TransportError,
    CLOUD_SYNC_ENTITLEMENT, PAYLOAD_SCHEMA_VERSION, PROTOCOL_VERSION,
};
use unfour_core::domain::DomainEntityType;

struct FixedSession(AtomicU64);

#[test]
fn protocol_v4_connection_entity_contract_is_stable() {
    assert_eq!(PROTOCOL_VERSION, 4);
    assert_eq!(SyncEntityType::Connection.as_str(), "connection");
    assert_eq!(
        SyncEntityType::parse("connection").unwrap(),
        SyncEntityType::Connection
    );
    assert_eq!(SyncEntityType::Connection.topology_rank(), 1);
    assert_eq!(
        SyncEntityType::from(DomainEntityType::Connection),
        SyncEntityType::Connection
    );
    assert_eq!(
        DomainEntityType::from(SyncEntityType::Connection),
        DomainEntityType::Connection
    );
}

#[test]
fn cloud_sync_wire_names_and_push_fields_are_stable() {
    assert_eq!(CLOUD_SYNC_ENTITLEMENT, "cloud_sync");
    assert_eq!(PROTOCOL_VERSION, 4);
    assert_eq!(PAYLOAD_SCHEMA_VERSION, 1);

    let entity_types = [
        (SyncEntityType::Workspace, "workspace"),
        (SyncEntityType::Connection, "connection"),
        (SyncEntityType::WorkspaceVariable, "workspaceVariable"),
        (SyncEntityType::WorkspaceEnvironment, "workspaceEnvironment"),
        (
            SyncEntityType::WorkspaceEnvironmentVariable,
            "workspaceEnvironmentVariable",
        ),
        (SyncEntityType::ApiCollection, "apiCollection"),
        (SyncEntityType::ApiFolder, "apiFolder"),
        (SyncEntityType::ApiRequest, "apiRequest"),
        (SyncEntityType::SshTask, "sshTask"),
        (SyncEntityType::SshTaskStep, "sshTaskStep"),
    ];
    for (entity_type, wire_name) in entity_types {
        assert_eq!(entity_type.as_str(), wire_name);
        assert_eq!(SyncEntityType::parse(wire_name).unwrap(), entity_type);
        assert_eq!(serde_json::to_value(entity_type).unwrap(), wire_name);
    }

    let request = PushRequest {
        protocol_version: PROTOCOL_VERSION,
        operations: vec![PushOperation {
            operation_id: "operation-contract".into(),
            entity_type: SyncEntityType::WorkspaceVariable,
            entity_id: "variable-contract".into(),
            parent_entity_id: Some("workspace-contract".into()),
            operation: SyncOperation::Upsert,
            base_version: 41,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload: Some(serde_json::json!({"key": "CONTRACT"})),
        }],
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({
            "protocolVersion": 4,
            "operations": [{
                "operationId": "operation-contract",
                "entityType": "workspaceVariable",
                "entityId": "variable-contract",
                "parentEntityId": "workspace-contract",
                "operation": "upsert",
                "baseVersion": 41,
                "payloadSchemaVersion": 1,
                "payload": {"key": "CONTRACT"}
            }]
        })
    );
}

#[async_trait]
impl DesktopSessionProvider for FixedSession {
    async fn session_for_cloud_sync(&self) -> Result<DesktopSessionCredential, SyncError> {
        DesktopSessionCredential::new(
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            self.0.load(Ordering::SeqCst),
        )
    }

    fn generation(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
    fn invalidate_cloud_sync(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn api_json_fixtures_match_final_openapi_models() {
    let changes: ChangesPage = serde_json::from_str(
        r#"{
      "protocolVersion":4,"cloudWorkspaceId":"550e8400-e29b-41d4-a716-446655440010",
      "currentCursor":9223372036854775807,"nextCursor":2,"changes":[{
        "cursor":2,"operationId":"operation-2","entityType":"workspaceVariable",
        "entityId":"variable-1","parentEntityId":"workspace-1","operation":"delete",
        "serverVersion":3,"payloadSchemaVersion":1,"payload":null,
        "deletedAt":"2026-07-28T01:02:03Z"
      }]
    }"#,
    )
    .expect("OpenAPI changes fixture");
    assert_eq!(changes.current_cursor, i64::MAX);
    assert_eq!(changes.next_cursor, 2);
    assert_eq!(changes.changes[0].cursor, 2);
    assert_eq!(changes.changes[0].operation_id, "operation-2");
    assert_eq!(changes.changes[0].payload_schema_version, 1);
    assert_eq!(
        changes.changes[0].deleted_at.as_deref(),
        Some("2026-07-28T01:02:03Z")
    );

    let snapshot: SnapshotPage = serde_json::from_str(
        r#"{
      "protocolVersion":4,"cloudWorkspaceId":"550e8400-e29b-41d4-a716-446655440010",
      "atCursor":9,"currentCursor":12,"items":[{
        "entityType":"workspace","entityId":"workspace-1","parentEntityId":null,
        "serverVersion":1,"payloadSchemaVersion":1,"payload":{
          "name":"Fixture","environmentType":"dev",
          "mcpPolicy":"auto","createdAt":"2026-07-28T00:00:00Z",
          "updatedAt":"2026-07-28T00:00:00Z","deletedAt":null
        }
      },{
        "entityType":"connection","entityId":"connection-1","parentEntityId":null,
        "serverVersion":1,"payloadSchemaVersion":1,"payload":{
          "id":"connection-1","workspaceId":"workspace-1","connectionType":"ssh",
          "name":"Fixture SSH","host":"ssh.example.test","port":22,
          "config":{"kind":"ssh","username":"deploy","authMethod":"private-key"},
          "createdAt":"2026-08-21T00:00:00Z","updatedAt":"2026-08-21T00:00:00Z"
        }
      },{
        "entityType":"workspaceVariable","entityId":"variable-1","parentEntityId":"workspace-1",
        "serverVersion":1,"payloadSchemaVersion":1,"payload":{
          "key":"BASE_URL","value":"https://example.test","isSecret":false,"isEnabled":true,
          "description":null,"sortOrder":0,"createdAt":"2026-07-28T00:00:00Z",
          "updatedAt":"2026-07-28T00:00:00Z","deletedAt":null
        }
      },{
        "entityType":"workspaceEnvironment","entityId":"environment-1","parentEntityId":"workspace-1",
        "serverVersion":1,"payloadSchemaVersion":1,"payload":{
          "name":"Test","sortOrder":0,"createdAt":"2026-07-28T00:00:00Z",
          "updatedAt":"2026-07-28T00:00:00Z","deletedAt":null
        }
      },{
        "entityType":"workspaceEnvironmentVariable","entityId":"environment-variable-1","parentEntityId":"environment-1",
        "serverVersion":1,"payloadSchemaVersion":1,"payload":{
          "key":"TOKEN","isSecret":true,"isEnabled":true,"description":"device-local value omitted",
          "sortOrder":0,"createdAt":"2026-07-28T00:00:00Z",
          "updatedAt":"2026-07-28T00:00:00Z","deletedAt":null
        }
      },{
        "entityType":"apiCollection","entityId":"collection-1","parentEntityId":"workspace-1",
        "serverVersion":1,"payloadSchemaVersion":1,"payload":{
          "name":"Accounts","description":null,"createdAt":"2026-07-28T00:00:00Z",
          "updatedAt":"2026-07-28T00:00:00Z"
        }
      },{
        "entityType":"apiFolder","entityId":"folder-1","parentEntityId":"collection-1",
        "serverVersion":1,"payloadSchemaVersion":1,"payload":{
          "collectionId":"collection-1","parentFolderId":null,"name":"Root","sortOrder":0,
          "createdAt":"2026-07-28T00:00:00Z","updatedAt":"2026-07-28T00:00:00Z"
        }
      },{
        "entityType":"apiRequest","entityId":"request-1","parentEntityId":"folder-1",
        "serverVersion":1,"payloadSchemaVersion":1,"payload":{
          "collectionId":"collection-1","parentFolderId":"folder-1","name":"List accounts",
          "sortOrder":0,"authJson":"{}","method":"GET","url":"https://example.test/accounts",
          "headers":[],"query":[],"body":null,"bodyKind":"none","preRequestScript":null,
          "postResponseScript":null,"scriptSchemaVersion":1,"createdAt":"2026-07-28T00:00:00Z",
          "updatedAt":"2026-07-28T00:00:00Z"
        }
      },{
        "entityType":"sshTask","entityId":"task-1","parentEntityId":null,
        "serverVersion":1,"payloadSchemaVersion":1,"payload":{
          "name":"Deploy","description":"Deploy the service","sortOrder":0,
          "createdAt":"2026-08-17T00:00:00Z","updatedAt":"2026-08-17T00:00:00Z"
        }
      },{
        "entityType":"sshTaskStep","entityId":"step-1","parentEntityId":"task-1",
        "serverVersion":1,"payloadSchemaVersion":1,"payload":{
          "taskId":"task-1","name":"Restart","stepType":"command","position":0,
          "enabled":true,"configVersion":1,"configJson":{
            "command":"systemctl restart app","workingDirectory":"",
            "timeoutSeconds":30,"continueOnError":false
          },"createdAt":"2026-08-17T00:00:00Z","updatedAt":"2026-08-17T00:00:00Z"
        }
      }],"nextPageToken":null
    }"#,
    )
    .expect("OpenAPI snapshot fixture");
    assert_eq!(snapshot.items.len(), 10);
    for item in &snapshot.items {
        parse_snapshot_item("workspace-1", item).expect("final OpenAPI canonical payload");
        for forbidden in [
            "revision",
            "isDefault",
            "lastOpenedAt",
            "isActive",
            "secretValue",
            "credentialRef",
            "keyPath",
            "sqlitePath",
            "timeoutMs",
            "temporaryVariables",
        ] {
            assert!(
                item.payload.get(forbidden).is_none(),
                "unexpected {forbidden}"
            );
        }
    }

    let push: PushResponse = serde_json::from_str(
        r#"{
      "protocolVersion":4,"currentCursor":9,"results":[
        {"operationId":"one","serverVersion":2,"cursor":9,"status":"applied"},
        {"operationId":"two","serverVersion":2,"cursor":9,"status":"noOp"}
      ]
    }"#,
    )
    .expect("OpenAPI push fixture");
    assert_eq!(push.results.len(), 2);

    let conflict: ApiErrorEnvelope = serde_json::from_str(
        r#"{
      "error":{"code":"base_version_conflict","message":"conflict","requestId":"request-1",
      "details":{"entityType":"workspaceVariable","entityId":"variable-1",
      "parentEntityId":"workspace-1","serverVersion":4,"operation":"upsert",
      "payloadSchemaVersion":1,"payload":null}}
    }"#,
    )
    .expect("OpenAPI error fixture");
    assert_eq!(conflict.error.code, "base_version_conflict");

    assert!(
        serde_json::from_str::<ChangesPage>(
            r#"{
      "protocolVersion":4,"cloudWorkspaceId":"cloud","currentCursor":1,
      "nextCursor":"1","changes":[],"hasMore":false
    }"#
        )
        .is_err(),
        "legacy string cursor/hasMore must stay rejected"
    );
}

#[tokio::test]
async fn real_http_request_uses_desktop_session_header_and_never_bearer() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0_u8; 8192];
        let count = socket.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..count]).to_ascii_lowercase();
        assert!(request.contains("x-desktop-session: abcdefghijklmnopqrstuvwxyz0123456789_-abcde"));
        assert!(!request.contains("authorization:"));
        let body = r#"{"protocolVersion":4,"workspaces":[]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(), body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    let transport = HttpSyncTransport::new(
        &format!("http://{address}"),
        Arc::new(FixedSession(AtomicU64::new(0))),
    )
    .unwrap();
    assert!(transport.list_workspaces().await.unwrap().is_empty());
    server.await.unwrap();
    assert_eq!(PROTOCOL_VERSION, 4);
}

#[tokio::test]
async fn every_http_cloud_sync_endpoint_declares_protocol_v4() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let responses = [
            r#"{"protocolVersion":4,"workspaces":[]}"#,
            r#"{"cloudWorkspaceId":"cloud","rootEntityId":"workspace","name":null,"currentCursor":0,"createdAt":"2026-08-21T00:00:00Z","updatedAt":"2026-08-21T00:00:00Z"}"#,
            r#"{"protocolVersion":4,"currentCursor":0,"results":[]}"#,
            r#"{"protocolVersion":4,"cloudWorkspaceId":"cloud","currentCursor":0,"nextCursor":0,"changes":[]}"#,
            r#"{"protocolVersion":4,"cloudWorkspaceId":"cloud","atCursor":0,"currentCursor":0,"items":[],"nextPageToken":null}"#,
        ];
        for (index, body) in responses.into_iter().enumerate() {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 16 * 1024];
            let count = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..count]);
            match index {
                0 => assert!(request.starts_with("GET /v1/sync/workspaces?protocolVersion=4 ")),
                1 => {
                    assert!(request.starts_with("POST /v1/sync/workspaces "));
                    assert!(request.contains(r#""protocolVersion":4"#));
                }
                2 => {
                    assert!(request.starts_with("POST /v1/sync/workspaces/cloud/push "));
                    assert!(request.contains(r#""protocolVersion":4"#));
                }
                3 => assert!(request.contains(
                    "/v1/sync/workspaces/cloud/changes?protocolVersion=4&afterCursor=0&limit=200"
                )),
                4 => assert!(request.contains(
                    "/v1/sync/workspaces/cloud/snapshot?protocolVersion=4&atCursor=0&pageToken=next"
                )),
                _ => unreachable!(),
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let transport = HttpSyncTransport::new(
        &format!("http://{address}"),
        Arc::new(FixedSession(AtomicU64::new(0))),
    )
    .unwrap();
    transport.list_workspaces().await.unwrap();
    transport.create_workspace("workspace").await.unwrap();
    transport
        .push(
            "cloud",
            &PushRequest {
                protocol_version: PROTOCOL_VERSION,
                operations: Vec::new(),
            },
        )
        .await
        .unwrap();
    transport.changes("cloud", 0, 200).await.unwrap();
    transport
        .snapshot("cloud", Some(0), Some("next"))
        .await
        .unwrap();
    server.await.unwrap();
}

async fn transport_with_response(
    status: &str,
    body: &'static str,
) -> (HttpSyncTransport, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let status = status.to_string();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0_u8; 8192];
        let _ = socket.read(&mut request).await.unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    let transport = HttpSyncTransport::new(
        &format!("http://{address}"),
        Arc::new(FixedSession(AtomicU64::new(0))),
    )
    .unwrap();
    (transport, server)
}

#[tokio::test]
async fn http_error_envelope_classifies_409_protocol_and_auth_without_guessing() {
    let request = PushRequest {
        protocol_version: PROTOCOL_VERSION,
        operations: Vec::new(),
    };
    let (transport, server) = transport_with_response(
        "409 Conflict",
        r#"{"error":{"code":"base_version_conflict","message":"conflict","requestId":"r1","details":{"entityType":"workspace","entityId":"w","parentEntityId":null,"serverVersion":2,"operation":"upsert","payloadSchemaVersion":1,"payload":null}}}"#,
    )
    .await;
    assert!(matches!(
        transport.push("cloud", &request).await,
        Err(TransportError::Conflict(_))
    ));
    server.await.unwrap();

    let (transport, server) = transport_with_response(
        "400 Bad Request",
        r#"{"error":{"code":"invalid_parent_entity","message":"parent is invalid","requestId":"r-operation","details":{"operationId":"failed-operation"}}}"#,
    )
    .await;
    assert!(matches!(
        transport.push("cloud", &request).await,
        Err(TransportError::PermanentOperation { code, operation_id })
            if code == "invalid_parent_entity" && operation_id == "failed-operation"
    ));
    server.await.unwrap();

    let (transport, server) = transport_with_response(
        "400 Bad Request",
        r#"{"error":{"code":"invalid_sync_entity","message":"operation failed","requestId":"r-nested","details":{"failedOperation":{"operationId":"nested-failed-op","errorCode":"invalid_parent_entity"}}}}"#,
    )
    .await;
    assert!(matches!(
        transport.push("cloud", &request).await,
        Err(TransportError::PermanentOperation { code, operation_id })
            if code == "invalid_parent_entity" && operation_id == "nested-failed-op"
    ));
    server.await.unwrap();

    let (transport, server) = transport_with_response(
        "400 Bad Request",
        r#"{"error":{"code":"invalid_sync_entity","message":"blank operation","requestId":"r-blank","details":{"operationId":"  "}}}"#,
    )
    .await;
    assert!(matches!(
        transport.push("cloud", &request).await,
        Err(TransportError::Permanent(code)) if code == "invalid_sync_entity"
    ));
    server.await.unwrap();

    let (transport, server) = transport_with_response(
        "400 Bad Request",
        r#"{"error":{"code":"invalid_sync_entity","message":"no operation identity","requestId":"r-entity","details":{"entityId":"task-1"}}}"#,
    )
    .await;
    assert!(matches!(
        transport.push("cloud", &request).await,
        Err(TransportError::Permanent(code)) if code == "invalid_sync_entity"
    ));
    server.await.unwrap();

    let (transport, server) = transport_with_response(
        "400 Bad Request",
        r#"{"error":{"code":"protocol_version_unsupported","message":"unsupported","requestId":"r2","details":null}}"#,
    )
    .await;
    assert!(matches!(
        transport.push("cloud", &request).await,
        Err(TransportError::ProtocolIncompatible)
    ));
    server.await.unwrap();

    let (transport, server) = transport_with_response(
        "401 Unauthorized",
        r#"{"error":{"code":"unauthorized","message":"required","requestId":"r3","details":null}}"#,
    )
    .await;
    assert!(matches!(
        transport.push("cloud", &request).await,
        Err(TransportError::Unauthorized)
    ));
    server.await.unwrap();
}
