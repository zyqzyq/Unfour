use super::*;
use crate::{
    output_schema::assert_success_matches_output_schema, response::content_json, ToolRegistry,
};
use serde_json::{json, Value};
use unfour_core::models::*;

#[test]
fn folder_delete_confirmation_covers_requests_below_tombstoned_folders() {
    let storage_dir = test_storage_dir("metadata-hidden-descendants");
    let ws = initialize_storage_dir(&storage_dir);
    let adapter = LocalCommandBusAdapter::from_storage_dir(&storage_dir).unwrap();
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    adapter
        .run(
            adapter
                .bus
                .update_workspace_environment(ws.clone(), "test".into()),
        )
        .unwrap();
    let c = collection(&registry, &ws);
    let root = folder(&registry, &ws, &c, Value::Null, "Root");
    let child = folder(&registry, &ws, &c, root["id"].clone(), "Hidden");
    let request = adapter.save_api_request(serde_json::from_value(json!({
        "workspaceId":ws,"collectionId":c,"parentFolderId":child["id"],"name":"Hidden request",
        "method":"GET","url":"https://example.invalid","headers":[],"query":[],"bodyKind":"none"
    })).unwrap()).unwrap();
    // Reproduce a partially tombstoned hierarchy, as can exist during recovery.
    let db = adapter
        .run(LocalDb::connect_existing_path(
            storage_dir.join(unfour_command_bus::DEFAULT_DATABASE_FILE),
        ))
        .unwrap();
    adapter.run(sqlx::query("UPDATE api_collection_folders SET deleted_at = 'deleted', revision = revision + 1 WHERE id = ?")
        .bind(child["id"].as_str().unwrap()).execute(db.pool())).unwrap();
    let root_id = root["id"].as_str().unwrap().to_owned();
    let snapshot = adapter
        .run(
            adapter
                .bus
                .api_folder_delete_snapshot(ws.clone(), root_id.clone()),
        )
        .unwrap();
    assert!(snapshot.requests.iter().any(|(id, _)| id == &request.id));
    assert!(!snapshot
        .folders
        .iter()
        .any(|(id, _)| id == child["id"].as_str().unwrap()));
    let mut args = json!({"workspaceId":ws,"folderId":root_id});
    let first = content_json(
        &registry
            .call("unfour.api.delete_folder", args.clone())
            .unwrap(),
    );
    assert_eq!(first["error"]["code"], "CONFIRMATION_REQUIRED");
    adapter
        .run(
            sqlx::query("UPDATE api_requests SET revision = revision + 1 WHERE id = ?")
                .bind(&request.id)
                .execute(db.pool()),
        )
        .unwrap();
    args["confirm"] = json!(true);
    args["confirmation_text"] = first["confirmation_text"].clone();
    let refreshed = content_json(
        &registry
            .call("unfour.api.delete_folder", args.clone())
            .unwrap(),
    );
    assert_eq!(refreshed["error"]["code"], "CONFIRMATION_REQUIRED");
    assert!(adapter
        .run(
            adapter
                .bus
                .api_collection_folder_delete_checked(ws.clone(), root_id, Some(snapshot))
        )
        .is_err());
    args["confirmation_text"] = refreshed["confirmation_text"].clone();
    success(&registry, "unfour.api.delete_folder", args);
    assert!(!adapter
        .run(adapter.bus.list_saved_api_requests(ws))
        .unwrap()
        .iter()
        .any(|r| r.id == request.id));
    adapter.run(db.pool().close());
    adapter.shutdown();
}

#[test]
fn workspace_creation_rechecks_the_authorizing_context_in_the_transaction() {
    let (adapter, registry, ws) = setup("dev", "guarded");
    let created = success(
        &registry,
        "unfour.workspace.create",
        json!({"name":"Guarded child"}),
    );
    assert_eq!(created["workspace"]["mcpPolicy"], "guarded");
    adapter
        .run(
            adapter
                .bus
                .update_workspace_mcp_policy(ws.clone(), "read_only".into()),
        )
        .unwrap();
    assert!(adapter
        .run(adapter.bus.create_workspace_inactive_from(
            "Stale security".into(),
            ws.clone(),
            "dev".into(),
            "guarded".into()
        ))
        .is_err());
    let other = adapter
        .run(adapter.bus.create_workspace("Other".into()))
        .unwrap()
        .id;
    assert!(adapter
        .run(adapter.bus.create_workspace_inactive_from(
            "Stale selection".into(),
            ws.clone(),
            "dev".into(),
            "read_only".into()
        ))
        .is_err());
    let state = adapter.run(adapter.bus.list_workspaces()).unwrap();
    assert_eq!(state.active_workspace_id, other);
    assert!(!state.workspaces.iter().any(|w| w.name.starts_with("Stale")));
    // An explicit writable workspaceId cannot substitute for a read-only active source.
    adapter.run(adapter.bus.set_active_workspace(ws)).unwrap();
    assert_eq!(
        code(
            &registry,
            "unfour.workspace.create",
            json!({"name":"Bypass","workspaceId":other})
        ),
        "WORKSPACE_POLICY_BLOCKED"
    );
    adapter.shutdown();
}

#[test]
fn folder_delete_atomically_rejects_a_stale_request_snapshot() {
    let (adapter, registry, ws) = setup("test", "auto");
    let c = collection(&registry, &ws);
    let root = folder(&registry, &ws, &c, Value::Null, "Root");
    let id = root["id"].as_str().unwrap().to_owned();
    let snapshot = adapter
        .run(
            adapter
                .bus
                .api_folder_delete_snapshot(ws.clone(), id.clone()),
        )
        .unwrap();
    let request = adapter.save_api_request(serde_json::from_value(json!({
        "workspaceId":ws,"collectionId":c,"parentFolderId":id,"name":"Added after preview",
        "method":"GET","url":"https://example.invalid","headers":[],"query":[],"bodyKind":"none"
    })).unwrap()).unwrap();
    assert!(adapter
        .run(adapter.bus.api_collection_folder_delete_checked(
            ws.clone(),
            id.clone(),
            Some(snapshot)
        ))
        .is_err());
    assert!(adapter
        .run(adapter.bus.list_saved_api_requests(ws.clone()))
        .unwrap()
        .iter()
        .any(|r| r.id == request.id));
    assert!(adapter
        .run(adapter.bus.api_collection_folders_list(ws.clone(), None))
        .unwrap()
        .iter()
        .any(|f| f.id == id));
    adapter.shutdown();
}

fn setup(env: &str, policy: &str) -> (Arc<LocalCommandBusAdapter>, ToolRegistry, String) {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let ws = adapter
        .run(adapter.bus.create_workspace_with_options(
            "Source".into(),
            Some(env.into()),
            Some(policy.into()),
        ))
        .unwrap()
        .id;
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    (adapter, registry, ws)
}
fn success(registry: &ToolRegistry, name: &str, args: Value) -> Value {
    let schema = registry
        .definitions()
        .into_iter()
        .find(|d| d.name == name)
        .unwrap()
        .input_schema;
    crate::output_schema::assert_valid_against_schema(&schema, &args, name);
    let result = registry.call(name, args).unwrap();
    assert_success_matches_output_schema(registry, name, &result);
    assert!(!result.to_string().contains("private-canary"));
    content_json(&result)
}
fn code(registry: &ToolRegistry, name: &str, args: Value) -> Value {
    let result = registry.call(name, args).unwrap();
    assert_eq!(result["isError"], true, "{result}");
    content_json(&result)["error"]["code"].clone()
}
fn collection(registry: &ToolRegistry, ws: &str) -> String {
    success(
        registry,
        "unfour.api.create_collection",
        json!({"workspaceId":ws,"name":"APIs"}),
    )["collection"]["id"]
        .as_str()
        .unwrap()
        .into()
}
fn folder(registry: &ToolRegistry, ws: &str, collection: &str, parent: Value, name: &str) -> Value {
    success(
        registry,
        "unfour.api.create_folder",
        json!({"workspaceId":ws,"collectionId":collection,"parentFolderId":parent,"name":name}),
    )["folder"]
        .clone()
}

#[test]
fn workspace_creation_inherits_safety_without_activation_and_rename_checks_expectations() {
    let (adapter, registry, ws) = setup("test", "auto");
    let created = success(
        &registry,
        "unfour.workspace.create",
        json!({"name":"Created"}),
    )["workspace"]
        .clone();
    assert_eq!(created["environmentType"], "test");
    assert_eq!(created["mcpPolicy"], "auto");
    assert_eq!(
        adapter
            .run(adapter.bus.list_workspaces())
            .unwrap()
            .active_workspace_id,
        ws
    );
    for extra in ["workspaceId", "environmentType", "mcpPolicy", "activate"] {
        let mut args = json!({"name":"Invalid"});
        args[extra] = json!("full_access");
        assert!(registry.call("unfour.workspace.create", args).is_err());
    }
    let renamed = success(
        &registry,
        "unfour.workspace.rename",
        json!({"workspaceId":created["id"],"name":"Renamed","expectedName":"Created"}),
    );
    assert_eq!(renamed["workspace"]["name"], "Renamed");
    assert_eq!(
        code(
            &registry,
            "unfour.workspace.rename",
            json!({"workspaceId":created["id"],"name":"Stale","expectedName":"Created"})
        ),
        "ROW_CONFLICT"
    );
    assert_eq!(
        adapter
            .run(adapter.bus.list_workspaces())
            .unwrap()
            .active_workspace_id,
        ws
    );
    // Existing Desktop creation still activates.
    let desktop = adapter
        .run(adapter.bus.create_workspace("Desktop".into()))
        .unwrap();
    assert_eq!(
        adapter
            .run(adapter.bus.list_workspaces())
            .unwrap()
            .active_workspace_id,
        desktop.id
    );
    adapter.shutdown();
}

#[test]
fn metadata_tools_fail_closed_under_readonly_and_disabled_policies() {
    for (env, policy) in [("prod", "auto"), ("dev", "read_only"), ("dev", "disabled")] {
        let (adapter, registry, ws) = setup(env, policy);
        for name in [
            "unfour.workspace.create",
            "unfour.workspace.rename",
            "unfour.api.create_folder",
            "unfour.api.rename_folder",
            "unfour.api.move_folder",
            "unfour.api.reorder_folders",
            "unfour.api.delete_folder",
            "unfour.api.move_request",
            "unfour.api.duplicate_request",
            "unfour.db.save_saved_sql",
            "unfour.db.delete_saved_sql",
        ] {
            let args = if name == "unfour.workspace.create" {
                json!({"name":"Denied"})
            } else {
                json!({"workspaceId":ws})
            };
            assert_eq!(
                code(&registry, name, args),
                "WORKSPACE_POLICY_BLOCKED",
                "{name}"
            );
        }
        for name in ["unfour.api.list_folders", "unfour.db.list_saved_sql"] {
            if policy == "disabled" {
                assert_eq!(
                    code(&registry, name, json!({"workspaceId":ws})),
                    "WORKSPACE_POLICY_BLOCKED"
                );
            } else {
                success(&registry, name, json!({"workspaceId":ws}));
            }
        }
        assert_eq!(
            adapter
                .run(adapter.bus.list_workspaces())
                .unwrap()
                .workspaces
                .len(),
            2
        );
        adapter.shutdown();
    }
}

#[test]
fn api_organization_preserves_original_requests_and_enforces_scope() {
    let (adapter, registry, ws) = setup("dev", "auto");
    let c = collection(&registry, &ws);
    let a = folder(&registry, &ws, &c, Value::Null, "A");
    let b = folder(&registry, &ws, &c, Value::Null, "B");
    let child = folder(&registry, &ws, &c, a["id"].clone(), "Child");
    let reordered = success(
        &registry,
        "unfour.api.reorder_folders",
        json!({"workspaceId":ws,"collectionId":c,"parentFolderId":null,"folderIds":[b["id"],a["id"]]}),
    );
    assert_eq!(reordered["folders"][0]["id"], b["id"]);
    success(
        &registry,
        "unfour.api.rename_folder",
        json!({"workspaceId":ws,"folderId":child["id"],"name":"Nested","expectedUpdatedAt":child["updatedAt"]}),
    );
    assert_eq!(
        code(
            &registry,
            "unfour.api.move_folder",
            json!({"workspaceId":ws,"folderId":a["id"],"parentFolderId":child["id"]})
        ),
        "VALIDATION_ERROR"
    );
    success(
        &registry,
        "unfour.api.move_folder",
        json!({"workspaceId":ws,"folderId":child["id"],"parentFolderId":null}),
    );

    let original = adapter.save_api_request(serde_json::from_value(json!({
        "workspaceId":ws,"collectionId":c,"parentFolderId":a["id"],"name":"Original",
        "method":"POST","url":"https://example.invalid","headers":[],"query":[],
        "body":"private-canary body","bodyKind":"text",
        "authJson":"{\"type\":\"bearer\",\"token\":\"private-canary\"}",
        "preRequestScript":"console.log('private-canary');","postResponseScript":"pm.test('ok',()=>{});"
    })).unwrap()).unwrap();
    let copy = success(
        &registry,
        "unfour.api.duplicate_request",
        json!({"workspaceId":ws,"requestId":original.id}),
    );
    let copy_id = copy["request"]["id"].as_str().unwrap();
    assert_ne!(copy_id, original.id);
    let records = adapter
        .run(adapter.bus.list_saved_api_requests(ws.clone()))
        .unwrap();
    let duplicate = records.iter().find(|r| r.id == copy_id).unwrap();
    assert_eq!(duplicate.body, original.body);
    assert_eq!(duplicate.auth_json, original.auth_json);
    assert_eq!(duplicate.pre_request_script, original.pre_request_script);
    assert_eq!(
        duplicate.post_response_script,
        original.post_response_script
    );
    let moved = success(
        &registry,
        "unfour.api.move_request",
        json!({"workspaceId":ws,"requestId":copy_id,"collectionId":c,"parentFolderId":null}),
    );
    assert!(moved["request"]["parentFolderId"].is_null());
    assert!(registry
        .call(
            "unfour.api.move_request",
            json!({"workspaceId":ws,"requestId":copy_id,"collectionId":c})
        )
        .is_err());

    let other = adapter
        .run(adapter.bus.create_workspace("Other".into()))
        .unwrap()
        .id;
    for (tool, args) in [
        (
            "unfour.api.duplicate_request",
            json!({"workspaceId":other,"requestId":original.id}),
        ),
        (
            "unfour.api.move_request",
            json!({"workspaceId":other,"requestId":original.id,"collectionId":c,"parentFolderId":null}),
        ),
        (
            "unfour.api.rename_folder",
            json!({"workspaceId":other,"folderId":a["id"],"name":"Wrong","expectedUpdatedAt":a["updatedAt"]}),
        ),
        (
            "unfour.api.create_folder",
            json!({"workspaceId":other,"collectionId":c,"parentFolderId":null,"name":"Wrong"}),
        ),
    ] {
        assert_eq!(
            registry.call(tool, args).unwrap()["isError"],
            true,
            "{tool}"
        );
    }
    success(
        &registry,
        "unfour.api.delete_folder",
        json!({"workspaceId":ws,"folderId":a["id"]}),
    );
    let records = adapter
        .run(adapter.bus.list_saved_api_requests(ws))
        .unwrap();
    assert!(!records.iter().any(|r| r.id == original.id));
    assert!(records.iter().any(|r| r.id == copy_id));
    adapter.shutdown();
}

#[test]
fn folder_delete_confirmation_changes_when_descendants_change() {
    let (adapter, registry, ws) = setup("test", "auto");
    let c = collection(&registry, &ws);
    let root = folder(&registry, &ws, &c, Value::Null, "Root");
    let args = json!({"workspaceId":ws,"folderId":root["id"]});
    let required = content_json(
        &registry
            .call("unfour.api.delete_folder", args.clone())
            .unwrap(),
    );
    assert_eq!(required["error"]["code"], "CONFIRMATION_REQUIRED");
    folder(&registry, &ws, &c, root["id"].clone(), "Added later");
    let mut confirmed = args;
    confirmed["confirm"] = json!(true);
    confirmed["confirmation_text"] = required["confirmation_text"].clone();
    let refreshed = content_json(
        &registry
            .call("unfour.api.delete_folder", confirmed.clone())
            .unwrap(),
    );
    assert_eq!(refreshed["error"]["code"], "CONFIRMATION_REQUIRED");
    confirmed["confirmation_text"] = refreshed["confirmation_text"].clone();
    success(&registry, "unfour.api.delete_folder", confirmed);
    assert!(success(
        &registry,
        "unfour.api.list_folders",
        json!({"workspaceId":ws})
    )["folders"]
        .as_array()
        .unwrap()
        .is_empty());
    adapter.shutdown();
}

#[test]
fn saved_sql_preserves_hidden_sql_and_rejects_stale_or_foreign_mutations() {
    let (adapter, registry, ws) = setup("test", "auto");
    let s = success(
        &registry,
        "unfour.db.save_saved_sql",
        json!({"workspaceId":ws,"name":"Snippet","sql":"SELECT 'private-canary', 42"}),
    )["savedSql"]
        .clone();
    let detail = success(
        &registry,
        "unfour.db.get_saved_sql",
        json!({"workspaceId":ws,"savedSqlId":s["id"]}),
    );
    assert_eq!(detail["savedSql"]["sqlRedacted"], true);
    let updated = success(&registry,"unfour.db.save_saved_sql",json!({"workspaceId":ws,"savedSqlId":s["id"],"expectedRevision":s["revision"],"name":"Renamed"}))["savedSql"].clone();
    assert_eq!(
        adapter.run(adapter.bus.list_saved_sql(ws.clone())).unwrap()[0].sql,
        "SELECT 'private-canary', 42"
    );
    assert_eq!(
        code(
            &registry,
            "unfour.db.save_saved_sql",
            json!({"workspaceId":ws,"savedSqlId":s["id"],"expectedRevision":s["revision"],"sql":"SELECT 2"})
        ),
        "SAVED_SQL_REVISION_CONFLICT"
    );
    let args =
        json!({"workspaceId":ws,"savedSqlId":s["id"],"expectedRevision":updated["revision"]});
    let confirmation = content_json(
        &registry
            .call("unfour.db.delete_saved_sql", args.clone())
            .unwrap(),
    );
    assert_eq!(confirmation["error"]["code"], "CONFIRMATION_REQUIRED");
    let other = adapter
        .run(adapter.bus.create_workspace("Other".into()))
        .unwrap()
        .id;
    assert_eq!(
        code(
            &registry,
            "unfour.db.get_saved_sql",
            json!({"workspaceId":other,"savedSqlId":s["id"]})
        ),
        "NOT_FOUND"
    );
    assert_eq!(
        code(
            &registry,
            "unfour.db.save_saved_sql",
            json!({"workspaceId":other,"savedSqlId":s["id"],"expectedRevision":updated["revision"],"name":"Foreign"})
        ),
        "NOT_FOUND"
    );
    // Bypass the handler precheck to prove the engine's revision guard is atomic.
    let mut input = SavedSqlInput {
        id: Some(s["id"].as_str().unwrap().into()),
        workspace_id: ws.clone(),
        connection_id: None,
        catalog: None,
        schema: None,
        name: "Concurrent".into(),
        sql: "SELECT 'private-canary'".into(),
    };
    adapter
        .run(adapter.bus.save_saved_sql(input.clone()))
        .unwrap();
    input.name = "Stale".into();
    assert!(adapter
        .run(
            adapter
                .bus
                .save_saved_sql_checked(input, Some(updated["revision"].as_i64().unwrap()))
        )
        .is_err());
    assert!(adapter
        .run(adapter.bus.delete_saved_sql_checked(
            ws.clone(),
            s["id"].as_str().unwrap().into(),
            Some(updated["revision"].as_i64().unwrap())
        ))
        .is_err());
    let fresh = success(
        &registry,
        "unfour.db.get_saved_sql",
        json!({"workspaceId":ws,"savedSqlId":s["id"]}),
    )["savedSql"]
        .clone();
    let mut args =
        json!({"workspaceId":ws,"savedSqlId":s["id"],"expectedRevision":fresh["revision"]});
    let confirmation = content_json(
        &registry
            .call("unfour.db.delete_saved_sql", args.clone())
            .unwrap(),
    );
    args["confirm"] = json!(true);
    args["confirmation_text"] = confirmation["confirmation_text"].clone();
    success(&registry, "unfour.db.delete_saved_sql", args);
    assert!(success(
        &registry,
        "unfour.db.list_saved_sql",
        json!({"workspaceId":ws})
    )["savedSql"]
        .as_array()
        .unwrap()
        .is_empty());
    adapter.shutdown();
}

#[test]
fn catalog_discovery_uses_saved_connections_and_workspace_scope() {
    let (adapter, registry, ws) = setup("dev", "auto");
    let db = adapter.save_db_connection(serde_json::from_value(json!({"workspaceId":ws,"name":"SQLite","driver":"sqlite","sqlitePath":":memory:","readOnly":false})).unwrap()).unwrap();
    let result = success(
        &registry,
        "unfour.db.list_catalogs",
        json!({"workspaceId":ws,"connectionId":db.id}),
    );
    assert!(result["catalogs"].is_array());
    let other = adapter
        .run(adapter.bus.create_workspace("Other".into()))
        .unwrap()
        .id;
    assert_eq!(
        registry
            .call(
                "unfour.db.list_catalogs",
                json!({"workspaceId":other,"connectionId":db.id})
            )
            .unwrap()["isError"],
        true
    );
    adapter.shutdown();
}
