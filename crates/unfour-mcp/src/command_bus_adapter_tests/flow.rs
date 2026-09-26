use super::*;
use crate::{
    output_schema::assert_success_matches_output_schema, response::content_json,
    tools::ToolRegistry,
};
use serde_json::{json, Value};
use unfour_core::models::{FlowDefinition, FlowInitiator, FlowRunInput};

fn setup() -> (Arc<LocalCommandBusAdapter>, ToolRegistry, String) {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let workspace = adapter
        .run(adapter.bus.list_workspaces())
        .unwrap()
        .active_workspace_id;
    adapter
        .run(
            adapter
                .bus
                .update_workspace_environment(workspace.clone(), "dev".into()),
        )
        .unwrap();
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    (adapter, registry, workspace)
}
fn definition(workspace: &str) -> FlowDefinition {
    serde_json::from_value(
        json!({"id":"","workspaceId":workspace,"name":"Shared Flow","revision":0,
        "inputs":[{"name":"value","type":"string","secret":true}],
        "steps":[{"id":"wait","name":"Wait","kind":"wait","timeoutMs":60000,"durationMs":30000}]}),
    )
    .unwrap()
}
fn success(registry: &ToolRegistry, tool: &str, args: Value) -> Value {
    let name = format!("unfour.flow.{tool}");
    let schema = registry
        .definitions()
        .into_iter()
        .find(|d| d.name == name)
        .unwrap()
        .input_schema;
    crate::output_schema::assert_valid_against_schema(&schema, &args, &name);
    let result = registry.call(&name, args).unwrap();
    assert_success_matches_output_schema(registry, &name, &result);
    result["structuredContent"].clone()
}
fn confirmation(registry: &ToolRegistry, args: Value) -> String {
    let result = registry.call("unfour.flow.run", args).unwrap();
    assert_eq!(result["isError"], true);
    assert!(result.get("structuredContent").is_none());
    let payload = content_json(&result);
    assert_eq!(payload["error"]["code"], "CONFIRMATION_REQUIRED");
    payload["confirmation_text"].as_str().unwrap().into()
}

#[test]
fn flow_tools_list_exposes_valid_schemas_and_shared_crud_revision() {
    let (adapter, registry, ws) = setup();
    let server = crate::McpServer::new(adapter.clone());
    let response = server
        .handle_message(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
        .unwrap();
    let tools: Vec<_> = response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["name"].as_str().unwrap().starts_with("unfour.flow."))
        .collect();
    assert_eq!(tools.len(), 7);
    for tool in tools {
        jsonschema::validator_for(&tool["inputSchema"]).unwrap();
        jsonschema::validator_for(&tool["outputSchema"]).unwrap();
    }
    // Desktop uses the same CommandBus methods, with no MCP-specific storage.
    let desktop = adapter.run(adapter.bus.save_flow(definition(&ws))).unwrap();
    let result = success(&registry, "get", json!({"flowId":desktop.id}));
    assert_eq!(result["flow"], serde_json::to_value(&desktop).unwrap());
    assert_eq!(
        success(&registry, "list", json!({}))["flows"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let created = success(
        &registry,
        "save",
        json!({"workspaceId":ws,"definition":definition(&ws)}),
    )["flow"]
        .clone();
    let from_desktop = adapter
        .run(
            adapter
                .bus
                .get_flow(ws.clone(), created["id"].as_str().unwrap().into()),
        )
        .unwrap();
    assert_eq!(serde_json::to_value(from_desktop).unwrap(), created);
    assert_eq!(
        success(&registry, "save", json!({"definition":created}))["flow"]["revision"],
        2
    );
    let conflict = registry
        .call("unfour.flow.save", json!({"definition":created}))
        .unwrap();
    assert_eq!(
        content_json(&conflict)["error"]["code"],
        "FLOW_REVISION_CONFLICT"
    );
    let mut invalid = serde_json::to_value(definition(&ws)).unwrap();
    invalid["steps"] = json!([]);
    assert_eq!(
        registry
            .call("unfour.flow.save", json!({"definition":invalid}))
            .unwrap()["isError"],
        true
    );
}

#[test]
fn flow_confirmation_initiator_redaction_summary_detail_and_cancel_share_desktop_history() {
    let (adapter, registry, ws) = setup();
    let saved = adapter.run(adapter.bus.save_flow(definition(&ws))).unwrap();
    let mut args = json!({"flowId":saved.id,"inputs":{"value":"flow-secret-canary"}});
    let token = confirmation(&registry, args.clone());
    assert!(!token.contains("flow-secret-canary"));
    assert!(adapter.list_flow_runs(&ws, &saved.id).unwrap().is_empty());
    args["confirm"] = json!(true);
    args["confirmationText"] = json!("wrong");
    assert_eq!(confirmation(&registry, args.clone()), token);
    args["confirmationText"] = json!(token);
    let run = success(&registry, "run", args.clone())["run"].clone();
    assert_eq!(run["context"]["initiator"], "mcp");
    assert_eq!(run["context"]["confirmEffects"], true);
    assert!(!run.to_string().contains("flow-secret-canary"));
    let run_id = run["id"].as_str().unwrap();
    let detail = success(&registry, "get_run", json!({"runId":run_id}))["run"].clone();
    assert!(detail["definition"].is_object());
    assert!(detail["steps"].is_array());
    assert!(!detail.to_string().contains("flow-secret-canary"));
    let summary = success(&registry, "list_runs", json!({"flowId":saved.id}));
    assert_eq!(summary["runs"][0].as_object().unwrap().len(), 5);
    assert!(summary["runs"][0].get("definition").is_none());
    let desktop = adapter
        .run(adapter.bus.get_flow_run(ws.clone(), run_id.into()))
        .unwrap();
    assert_eq!(desktop.id, run_id);
    assert!(matches!(desktop.context.initiator, FlowInitiator::Mcp));
    success(&registry, "cancel_run", json!({"runId":run_id}));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let detail = adapter.get_flow_run(&ws, run_id).unwrap();
        if detail.status == unfour_core::models::FlowRunStatus::Cancelled {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "cancel did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    // Human runs are visible through MCP too.
    let human = adapter
        .run(adapter.bus.run_flow(FlowRunInput {
            workspace_id: ws.clone(),
            flow_id: saved.id.clone(),
            environment_id: None,
            inputs: json!({}),
            secret_input_names: vec![],
            initiator: FlowInitiator::Human,
            confirm_effects: true,
        }))
        .unwrap();
    assert_eq!(
        success(&registry, "get_run", json!({"runId":human.id}))["run"]["context"]["initiator"],
        "human"
    );
    success(&registry, "cancel_run", json!({"runId":human.id}));
    assert_eq!(
        success(&registry, "list_runs", json!({"flowId":saved.id}))["runs"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // Changed inputs and changed revisions invalidate the old confirmation.
    args["inputs"]["value"] = json!("changed");
    assert_ne!(confirmation(&registry, args.clone()), token);
    args["inputs"]["value"] = json!("flow-secret-canary");
    adapter.save_flow(saved).unwrap();
    assert_ne!(confirmation(&registry, args), token);
}

#[test]
fn flow_workspace_policy_cannot_be_bypassed_by_confirmation_or_nested_workspace() {
    let (adapter, registry, ws) = setup();
    let saved = adapter.save_flow(definition(&ws)).unwrap();
    let base = json!({"workspaceId":ws,"flowId":saved.id});
    let token = confirmation(&registry, base.clone());
    for (env, policy, allowed) in [
        ("dev", "auto", true),
        ("test", "auto", true),
        ("prod", "auto", false),
        ("dev", "read_only", false),
        ("dev", "disabled", false),
        ("prod", "full_access", true),
    ] {
        adapter
            .run(
                adapter
                    .bus
                    .update_workspace_environment(ws.clone(), env.into()),
            )
            .unwrap();
        adapter
            .run(
                adapter
                    .bus
                    .update_workspace_mcp_policy(ws.clone(), policy.into()),
            )
            .unwrap();
        if allowed {
            confirmation(&registry, base.clone());
        } else {
            let mut args = base.clone();
            args["confirm"] = json!(true);
            args["confirmationText"] = json!(token);
            let denied = registry.call("unfour.flow.run", args).unwrap();
            assert_eq!(
                content_json(&denied)["error"]["code"],
                "WORKSPACE_POLICY_BLOCKED"
            );
            let denied = registry
                .call(
                    "unfour.flow.save",
                    json!({"workspaceId":ws,"definition":saved}),
                )
                .unwrap();
            assert_eq!(
                content_json(&denied)["error"]["code"],
                "WORKSPACE_POLICY_BLOCKED"
            );
        }
    }
    assert!(adapter.list_flow_runs(&ws, &saved.id).unwrap().is_empty());
    let mut foreign = definition(&ws);
    foreign.workspace_id = "another-workspace".into();
    assert!(registry
        .call(
            "unfour.flow.save",
            json!({"workspaceId":ws,"definition":foreign})
        )
        .is_err());
    for bad in [
        json!({"flowId":saved.id,"confirmEffects":true}),
        json!({"flowId":saved.id,"initiator":"human"}),
        json!({"flowId":saved.id,"inputs":[]}),
    ] {
        assert!(registry.call("unfour.flow.run", bad).is_err());
    }
}

#[test]
fn flow_all_node_schemas_and_sensitive_inputs_keep_wire_contracts() {
    let (adapter, registry, ws) = setup();
    let connection = adapter
        .run(
            adapter.bus.save_database_connection(
                serde_json::from_value(json!({
                    "workspaceId": ws,
                    "name": "Flow validation fixture",
                    "driver": "sqlite",
                    "sqlitePath": ":memory:",
                    "readOnly": true
                }))
                .unwrap(),
            ),
        )
        .unwrap();
    let mut value = serde_json::to_value(definition(&ws)).unwrap();
    value["steps"] = json!([
        {"id":"action","name":"Action","kind":"action","timeoutMs":1000,"action":{"capability":"api","resourceId":"missing","arguments":{}}},
        {"id":"condition","name":"Condition","kind":"condition","timeoutMs":1000,"predicate":{"left":true,"op":"eq","right":true},"ifTrue":"poll","ifFalse":"$end"},
        {"id":"poll","name":"Poll","kind":"poll","timeoutMs":1000,"probe":{"capability":"database","resourceId":connection.id,"arguments":{"sql":"select 1"}},"predicate":{"left":true,"op":"eq","right":true},"intervalMs":10,"maxAttempts":2},
        {"id":"until","name":"Until","kind":"waitUntil","timeoutMs":1000,"probe":{"capability":"api","resourceId":"missing","arguments":{}},"successWhen":{"left":true,"op":"eq","right":true},"intervalMs":10},
        {"id":"wait","name":"Wait","kind":"wait","timeoutMs":1000,"durationMs":1}
    ]);
    let saved = success(&registry, "save", json!({"definition":value}))["flow"].clone();
    assert_eq!(
        success(&registry, "get", json!({"flowId":saved["id"]}))["flow"],
        saved
    );
    let mut args = json!({"flowId":saved["id"],"inputs":{
        "value":"schema-secret-canary", "manual":"manual-secret-canary",
        "headers":{"authorization":"auth-canary","cookie":"cookie-canary","proxy-authorization":"proxy-canary","x-api-key":"api-key-canary","x-auth-token":"auth-token-canary"}
    },"secretInputNames":["manual"]});
    let token = confirmation(&registry, args.clone());
    args["confirm"] = json!(true);
    args["confirmation_text"] = json!(token);
    // Missing resource fails preflight; no external request is made.
    let result = success(&registry, "run", args)["run"].clone();
    assert_eq!(result["status"], "validationFailed");
    assert!(!result.to_string().contains("canary"));
    let detail = success(&registry, "get_run", json!({"runId":result["id"]}));
    assert!(!detail.to_string().contains("canary"));
    assert_eq!(result["definition"]["inputs"][0]["secret"], true);
    let mut secret_definition = definition(&ws);
    secret_definition.inputs[0].default = Some(json!("inline-secret-canary"));
    let rejected = registry
        .call("unfour.flow.save", json!({"definition":secret_definition}))
        .unwrap();
    assert_eq!(rejected["isError"], true);
    assert!(!rejected.to_string().contains("inline-secret-canary"));
    assert_eq!(adapter.list_flows(&ws).unwrap().len(), 1);
}

#[test]
fn flow_mcp_summary_does_not_decode_run_json() {
    let storage_dir = test_storage_dir("flow-summary");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let db = runtime.block_on(async {
        let db = LocalDb::connect_path(storage_dir.join(unfour_command_bus::DEFAULT_DATABASE_FILE))
            .await
            .unwrap();
        db.migrate().await.unwrap();
        db
    });
    let adapter =
        LocalCommandBusAdapter::from_command_bus_future(CommandBus::from_db(db.clone())).unwrap();
    let ws = adapter
        .run(adapter.bus.list_workspaces())
        .unwrap()
        .active_workspace_id;
    adapter.run(async {
        sqlx::query("INSERT INTO flow_runs (id, workspace_id, flow_id, status, run_json, started_at, updated_at, finished_at) VALUES ('broken', ?, 'flow', 'succeeded', 'invalid snapshot', 'start', 'heartbeat', 'finish')")
            .bind(&ws).execute(db.pool()).await.unwrap();
    });
    adapter.run(async {
        sqlx::query("INSERT INTO flow_definitions (id, workspace_id, revision, definition_json, updated_at) VALUES ('summary-only', ?, 7, ?, 'now')")
            .bind(&ws).bind(r#"{"name":"Metadata","steps":"not valid steps","inputs":{"not":"valid inputs"}}"#)
            .execute(db.pool()).await.unwrap();
    });
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    assert_eq!(
        success(&registry, "list", json!({"workspaceId":ws}))["flows"],
        json!([{"id":"summary-only","workspaceId":ws,"name":"Metadata","revision":7}])
    );
    assert!(adapter.run(adapter.bus.list_flows(ws.clone())).is_err());
    let summaries = success(
        &registry,
        "list_runs",
        json!({"workspaceId":ws,"flowId":"flow"}),
    );
    assert_eq!(
        summaries["runs"],
        json!([{"id":"broken","flowId":"flow","status":"succeeded","startedAt":"start","finishedAt":"finish"}])
    );
    let detail = registry
        .call(
            "unfour.flow.get_run",
            json!({"workspaceId":ws,"runId":"broken"}),
        )
        .unwrap();
    assert_eq!(detail["isError"], true);
    drop(registry);
    runtime.block_on(db.pool().close());
    adapter.shutdown();
    drop(adapter);
    std::fs::remove_dir_all(storage_dir).unwrap();
}

#[test]
fn flow_old_confirmation_and_post_confirmation_revision_race_create_no_run() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use unfour_command_bus::{ReadCommand, ReadCommandResult};
    use unfour_core::models::*;

    struct RacingAdapter {
        inner: Arc<LocalCommandBusAdapter>,
        mutate_on_run: AtomicBool,
    }
    impl CommandBusAdapter for RacingAdapter {
        fn execute_read(
            &self,
            command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            self.inner.execute_read(command)
        }
        fn execute_saved_api_request(
            &self,
            id: &str,
            timeout: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            self.inner.execute_saved_api_request(id, timeout)
        }
        fn list_db_connections(
            &self,
            ws: &str,
        ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
            self.inner.list_db_connections(ws)
        }
        fn get_db_schema(
            &self,
            ws: &str,
            id: &str,
        ) -> Result<DatabaseSchema, CommandBusAdapterError> {
            self.inner.get_db_schema(ws, id)
        }
        fn execute_db_query(
            &self,
            input: DatabaseQueryInput,
        ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
            self.inner.execute_db_query(input)
        }
        fn get_flow(&self, ws: &str, id: &str) -> Result<FlowDefinition, CommandBusAdapterError> {
            self.inner.get_flow(ws, id)
        }
        fn run_flow(
            &self,
            input: FlowRunInput,
            revision: i64,
        ) -> Result<FlowRun, CommandBusAdapterError> {
            // Deterministic concurrent-writer interleaving: confirmation has passed,
            // but the service has not loaded its execution definition yet.
            if self.mutate_on_run.swap(false, Ordering::SeqCst) {
                let flow = self.inner.get_flow(&input.workspace_id, &input.flow_id)?;
                self.inner.save_flow(flow)?;
            }
            self.inner.run_flow(input, revision)
        }
    }
    let (adapter, registry, ws) = setup();
    let saved = adapter.save_flow(definition(&ws)).unwrap();
    let mut args = json!({"flowId":saved.id});
    let old = confirmation(&registry, args.clone());
    adapter.save_flow(saved.clone()).unwrap();
    args["confirm"] = json!(true);
    args["confirmationText"] = json!(old);
    assert_ne!(confirmation(&registry, args.clone()), old);
    assert!(adapter.list_flow_runs(&ws, &saved.id).unwrap().is_empty());

    let racing = ToolRegistry::with_command_bus(Arc::new(RacingAdapter {
        inner: adapter.clone(),
        mutate_on_run: AtomicBool::new(true),
    }));
    args["confirmationText"] = json!(confirmation(&racing, args.clone()));
    let rejected = racing.call("unfour.flow.run", args).unwrap();
    assert_eq!(
        content_json(&rejected)["error"]["code"],
        "FLOW_CONFIRMATION_STALE"
    );
    assert!(content_json(&rejected)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("new confirmation"));
    assert!(adapter.list_flow_runs(&ws, &saved.id).unwrap().is_empty());
}

#[test]
fn flow_save_rejects_invalid_api_patches_through_shared_command_bus() {
    let (adapter, registry, ws) = setup();
    for arguments in [
        json!({"headers":[],"headersPatch":[]}),
        json!({"queryPatch":[{"key":" ","value":"x","enabled":true}]}),
        json!({"headersPatch":{}}),
    ] {
        let mut flow = definition(&ws);
        flow.steps = serde_json::from_value(json!([{"id":"api","name":"API","kind":"action","timeoutMs":1000,"action":{"capability":"api","resourceId":"saved-request","arguments":arguments}}])).unwrap();
        assert!(adapter.run(adapter.bus.save_flow(flow.clone())).is_err());
        let result = registry
            .call(
                "unfour.flow.save",
                json!({"workspaceId":ws,"definition":flow}),
            )
            .unwrap();
        assert_eq!(result["isError"], true);
        assert_eq!(content_json(&result)["error"]["code"], "VALIDATION_ERROR");
    }
    assert!(adapter.run(adapter.bus.list_flows(ws)).unwrap().is_empty());
}
