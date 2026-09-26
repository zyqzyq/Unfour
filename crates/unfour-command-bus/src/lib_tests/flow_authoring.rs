use unfour_core::models::{FlowAction, FlowRunInput};
use unfour_flow_engine::FlowExecutor;

use super::*;
use crate::flow_authoring::*;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn flow_api_patch_preserves_unmodified_rows_and_legacy_replacement() {
    let saved = json!({"headers":[{"key":"Accept","value":"json","enabled":true},{"key":"X-Mode","value":"old","enabled":true}],"query":[{"key":"page","value":"1","enabled":true},{"key":"Page","value":"2","enabled":true}]});
    let mut patched = saved.clone();
    apply_api_arguments(&mut patched, &json!({"headersPatch":[{"key":"x-mode","value":"new","enabled":true}],"queryPatch":[{"key":"page","value":"","enabled":false}]})).unwrap();
    assert_eq!(patched["headers"][0], saved["headers"][0]);
    assert_eq!(patched["headers"][1]["value"], "new");
    assert_eq!(
        patched["query"],
        json!([{"key":"Page","value":"2","enabled":true}])
    );
    let mut legacy = saved.clone();
    apply_api_arguments(&mut legacy, &json!({"headers":[],"query":[]})).unwrap();
    assert_eq!(legacy, json!({"headers":[],"query":[]}));
    assert!(
        apply_api_arguments(&mut saved.clone(), &json!({"headers":[],"headersPatch":[]})).is_err()
    );
    assert!(apply_api_arguments(
        &mut saved.clone(),
        &json!({"queryPatch":[{"key":"","value":"x","enabled":true}]})
    )
    .is_err());
}

#[test]
fn flow_sql_preflight_handles_literals_comments_templates_and_dialects() {
    for sql in [
        "SELECT ';'; -- trailing",
        "SELECT ${/inputs/value};",
        "SELECT 'it''s;ok'",
    ] {
        validate_sql_argument(&json!({"sql":sql}), "sqlite").unwrap();
    }
    validate_sql_argument(&json!({"sql":"SELECT $$a;b$$;"}), "postgres").unwrap();
    validate_sql_argument(
        &json!({"sql":"CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT 1; SELECT 2; END;"}),
        "sqlite",
    )
    .unwrap();
    for args in [
        json!({}),
        json!({"sql":""}),
        json!({"sql":"-- comment"}),
        json!({"sql":"SELECT 1; SELECT 2"}),
    ] {
        assert!(validate_sql_argument(&args, "sqlite").is_err());
    }
}

#[test]
fn flow_ssh_preflight_requires_detected_inputs_and_explicit_values_win() {
    let names = vec!["VERSION".to_string()];
    let defaults = BTreeMap::from([("version".into(), "env".into())]);
    assert_eq!(
        ssh_inputs(&json!({"inputs":{}}), &names, &defaults, true).unwrap()["VERSION"],
        "env"
    );
    assert_eq!(
        ssh_inputs(
            &json!({"inputs":{"VERSION":"explicit"}}),
            &names,
            &defaults,
            true
        )
        .unwrap()["VERSION"],
        "explicit"
    );
    assert!(ssh_inputs(&json!({"inputs":{"VERSION":" "}}), &names, &defaults, true).is_err());
    assert!(ssh_inputs(&json!({}), &names, &BTreeMap::new(), true).is_err());
    assert!(ssh_inputs(
        &json!({"inputs":{"VERSION":{"$ref":"/steps/build/version"}}}),
        &names,
        &BTreeMap::new(),
        true
    )
    .is_ok());
}

#[tokio::test]
async fn flow_ssh_defaults_follow_explicit_environment_only_and_are_opt_in() {
    let bus = crate::tests::test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let variable = |key: &str, value: &str| {
        serde_json::from_value::<WorkspaceVariableInput>(json!({"key":key,"value":value})).unwrap()
    };
    bus.workspace_variables_replace(workspace.clone(), vec![variable("VERSION", "workspace")])
        .await
        .unwrap();
    let env = bus
        .workspace_environment_create(workspace.clone(), "selected".into())
        .await
        .unwrap();
    bus.workspace_environment_update(
        workspace.clone(),
        env.id.clone(),
        env.name,
        vec![variable("version", "environment")],
    )
    .await
    .unwrap();
    let mut action: FlowAction = serde_json::from_value(json!({"capability":"ssh","resourceId":"task","connectionId":"fixed","arguments":{"inputs":{},"workspaceDefaults":true}})).unwrap();
    let mut input: FlowRunInput = serde_json::from_value(json!({"workspaceId":workspace,"flowId":"flow","inputs":{},"environmentId":null,"initiator":"human","confirmEffects":true})).unwrap();
    assert_eq!(
        bus.flow_ssh_defaults(&action, &input).await.unwrap()["version"],
        "workspace"
    );
    input.environment_id = Some(env.id);
    assert_eq!(
        bus.flow_ssh_defaults(&action, &input).await.unwrap()["version"],
        "environment"
    );
    action.arguments = json!({"inputs":{}});
    assert!(bus
        .flow_ssh_defaults(&action, &input)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn flow_database_rejects_empty_and_batch_sql_at_save_and_run_preflight() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let connection = bus.save_database_connection(serde_json::from_value(json!({
        "workspaceId":workspace,"name":"Flow validation fixture","driver":"sqlite","sqlitePath":":memory:","readOnly":true
    })).unwrap()).await.unwrap();
    for arguments in [
        json!({}),
        json!({"sql":" "}),
        json!({"sql":"SELECT 1; SELECT 2"}),
    ] {
        let definition = super::flow::definition(
            &workspace,
            json!([super::flow::action(
                "db",
                "database",
                &connection.id,
                arguments
            )]),
        );
        assert!(bus.save_flow(definition.clone()).await.is_err());
        // Legacy definitions can still be read; preflight rejects unsupported SQL
        // before scheduling any action, even if it was persisted by an older UI.
        let stored = unfour_flow_engine::FlowService::new(bus.db.clone())
            .save(definition)
            .await
            .unwrap();
        let run = bus
            .run_flow(super::flow::request(&workspace, &stored.id))
            .await
            .unwrap();
        assert_eq!(
            run.status,
            unfour_core::models::FlowRunStatus::ValidationFailed
        );
        assert!(run.steps.iter().all(|step| step.attempts.is_empty()));
    }
}

#[tokio::test]
async fn flow_ssh_missing_inputs_fail_before_ssh_execution_and_connection_is_explicit() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let connection = bus.save_ssh_connection(serde_json::from_value(json!({
        "workspaceId":workspace,"name":"Flow validation fixture","host":"127.0.0.1","port":1,"username":"fixture","authKind":"password","secret":"disposable-fixture-password"
    })).unwrap()).await.unwrap();
    let detail = bus.save_ssh_task(serde_json::from_value(json!({
        "workspaceId":workspace,"name":"Flow task fixture","description":"","defaultConnectionId":connection.id,
        "steps":[{"name":"echo","stepType":"command","position":0,"enabled":true,"configJson":{"command":"echo {{VERSION}}","timeoutSeconds":5}}]
    })).unwrap()).await.unwrap();
    let mut action: FlowAction = serde_json::from_value(json!({"capability":"ssh","resourceId":detail.task.id,"connectionId":connection.id,"arguments":{"inputs":{}}})).unwrap();
    let input = super::flow::request(&workspace, "unused");
    let error = bus
        .prepare(&action, &input, false)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("FLOW_SSH_INPUT_REQUIRED"));
    action.arguments = json!({"inputs":{"VERSION":"v1"}});
    bus.prepare(&action, &input, false).await.unwrap();
    action.connection_id = None;
    assert!(bus.prepare(&action, &input, false).await.is_err());
}

#[test]
fn flow_query_patch_consumes_occurrences_without_collapsing_duplicates() {
    let pair = |key: &str, value: &str, enabled| json!({"key":key,"value":value,"enabled":enabled});
    let saved = json!({"query":[pair("tag","a",true),pair("other","x",true),pair("tag","b",true)]});
    let mut request = saved.clone();
    apply_api_arguments(&mut request, &json!({"queryPatch":[pair("tag","c",true)]})).unwrap();
    assert_eq!(
        request["query"],
        json!([
            pair("tag", "c", true),
            pair("other", "x", true),
            pair("tag", "b", true)
        ])
    );
    let mut request = saved.clone();
    apply_api_arguments(
        &mut request,
        &json!({"queryPatch":[pair("tag","",false),pair("tag","d",true),pair("tag","e",true)]}),
    )
    .unwrap();
    assert_eq!(
        request["query"],
        json!([
            pair("other", "x", true),
            pair("tag", "d", true),
            pair("tag", "e", true)
        ])
    );
    apply_api_arguments(
        &mut request,
        &json!({"query":[pair("tag","x",true),pair("tag","y",true)]}),
    )
    .unwrap();
    assert_eq!(request["query"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn flow_api_patch_validation_at_save_and_preflight() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let api = bus
        .save_api_request(api_script_test_input(
            workspace.clone(),
            "http://127.0.0.1:1".into(),
        ))
        .await
        .unwrap();
    for arguments in [
        json!({"headersPatch":[{"key":" ","value":"x","enabled":true}]}),
        json!({"queryPatch":[{"key":"","value":"x","enabled":true}]}),
        json!({"headers":[],"headersPatch":[]}),
        json!({"query":[],"queryPatch":[]}),
        json!({"queryPatch":{}}),
        json!({"queryPatch":[{"key":"a","value":"x","enabled":true,"typo":1}]}),
        json!({"headersPatch":{"$ref":"/inputs/value","extra":true}}),
        json!({"headersPatch":[{"key":"a","value":3,"enabled":true}]}),
        json!({"queryPatch":[{"key":"a","value":"b","enabled":"true"}]}),
    ] {
        let definition = super::flow::definition(
            &workspace,
            json!([super::flow::action("api", "api", &api.id, arguments)]),
        );
        assert!(bus.save_flow(definition.clone()).await.is_err());
        let stored = unfour_flow_engine::FlowService::new(bus.db.clone())
            .save(definition)
            .await
            .unwrap();
        let run = bus
            .run_flow(super::flow::request(&workspace, &stored.id))
            .await
            .unwrap();
        assert_eq!(
            run.status,
            unfour_core::models::FlowRunStatus::ValidationFailed
        );
        assert!(run.steps.iter().all(|s| s.attempts.is_empty()));
    }
}

#[tokio::test]
async fn flow_api_environment_resolves_inherited_and_overridden_fields_on_wire() {
    use std::io::{BufRead, Read, Write};
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let variable = |value: &str| {
        serde_json::from_value::<WorkspaceVariableInput>(json!({"key":"ENV_VAR","value":value}))
            .unwrap()
    };
    bus.workspace_variables_replace(workspace.clone(), vec![variable("workspace")])
        .await
        .unwrap();
    let env = bus
        .workspace_environment_create(workspace.clone(), "selected".into())
        .await
        .unwrap();
    bus.workspace_environment_update(
        workspace.clone(),
        env.id.clone(),
        env.name,
        vec![variable("selected")],
    )
    .await
    .unwrap();
    let active = bus
        .workspace_environment_create(workspace.clone(), "active-other".into())
        .await
        .unwrap();
    bus.workspace_environment_update(
        workspace.clone(),
        active.id.clone(),
        active.name,
        vec![variable("active-other")],
    )
    .await
    .unwrap();
    bus.workspace_environment_set_active(workspace.clone(), Some(active.id))
        .await
        .unwrap();
    for overrides in [false, true] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut reader = std::io::BufReader::new(socket.try_clone().unwrap());
            let mut request = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse::<usize>().unwrap();
                }
                request.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            request.push_str(std::str::from_utf8(&body).unwrap());
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
            )
            .unwrap();
            request
        });
        let mut saved = api_script_test_input(
            workspace.clone(),
            format!("{url}/inherited/{{{{ENV_VAR}}}}"),
        );
        saved.method = "POST".into();
        saved.body_kind = "raw".into();
        saved.body = Some("inherited={{ENV_VAR}}".into());
        saved.headers = serde_json::from_value(
            json!([{"key":"X-Inherited","value":"{{ENV_VAR}}","enabled":true}]),
        )
        .unwrap();
        saved.query = serde_json::from_value(
            json!([{"key":"inherited","value":"{{ENV_VAR}}","enabled":true}]),
        )
        .unwrap();
        let saved = bus.save_api_request(saved).await.unwrap();
        let arguments = if overrides {
            json!({"url":format!("{url}/override/{{{{ENV_VAR}}}}"),"body":"override={{ENV_VAR}}","headersPatch":[{"key":"X-Override","value":"{{ENV_VAR}}","enabled":true}],"queryPatch":[{"key":"override","value":"{{ENV_VAR}}","enabled":true}]})
        } else {
            json!({})
        };
        let flow = bus
            .save_flow(super::flow::definition(
                &workspace,
                json!([super::flow::action("api", "api", &saved.id, arguments)]),
            ))
            .await
            .unwrap();
        let mut input = super::flow::request(&workspace, &flow.id);
        input.environment_id = Some(env.id.clone());
        let run = bus.run_flow(input).await.unwrap();
        let result = super::flow::finished(&bus, &run).await;
        assert_eq!(
            result.status,
            unfour_core::models::FlowRunStatus::Succeeded,
            "{result:?}"
        );
        let wire = server.join().unwrap().to_lowercase();
        assert!(wire.contains("inherited=selected"), "{wire}");
        assert!(wire.contains("x-inherited: selected"));
        let mode = if overrides { "override" } else { "inherited" };
        assert!(wire.contains(&format!("/{mode}/selected")));
        assert!(wire.ends_with(&format!("{mode}=selected")));
        if overrides {
            assert!(wire.contains("x-override: selected"));
            assert!(wire.contains("override=selected"));
        }
        assert!(!wire.contains("{{"));
        assert!(!wire.contains("workspace"));
        assert!(!wire.contains("active-other"));
    }
}

#[tokio::test]
async fn flow_failure_diagnostics_redact_before_size_limits() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = super::flow::request(&workspace, "unused");
    input.inputs = json!({"private":"hidden-value"});
    input.secret_input_names = vec!["private".into()];
    let result = bus.flow_diagnostics(json!({"body":format!("{}hidden-value", "中".repeat(11000)),"headers":[{"key":"Set-Cookie","value":"private-cookie"}],"log":"version=v1 hidden-value","errorMessage":"failed hidden-value"}),&input).await.unwrap();
    let encoded = result.to_string();
    assert!(!encoded.contains("hidden-value"));
    assert!(!encoded.contains("private-cookie"));
    assert!(encoded.contains("version=v1"));
    assert_eq!(result["diagnosticsTruncated"], true);
    assert!(encoded.len() < 262144);
}

#[tokio::test]
async fn flow_http_failure_keeps_safe_bounded_diagnostics_and_stops() {
    use std::io::{Read, Write};
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let env = bus
        .workspace_environment_create(workspace.clone(), "selected".into())
        .await
        .unwrap();
    bus.workspace_environment_update(
        workspace.clone(),
        env.id.clone(),
        env.name,
        vec![serde_json::from_value(
            json!({"key":"PRIVATE","value":"environment-private","isSecret":true}),
        )
        .unwrap()],
    )
    .await
    .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut bytes = [0; 8192];
        socket.read(&mut bytes).unwrap();
        let body = format!(
            "upstream failed environment-private flow-private auth-private {}",
            "x".repeat(100000)
        );
        write!(socket,"HTTP/1.1 422 Failed\r\nX-Trace: trace-42\r\nSet-Cookie: private-cookie\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
    });
    let mut api = api_script_test_input(workspace.clone(), url);
    api.headers = serde_json::from_value(
        json!([{"key":"Authorization","value":"Bearer auth-private","enabled":true}]),
    )
    .unwrap();
    let api = bus.save_api_request(api).await.unwrap();
    let flow = bus
        .save_flow(super::flow::definition(
            &workspace,
            json!([
                super::flow::action("api","api",&api.id,json!({})),
                {"id":"later","name":"later","kind":"wait","timeoutMs":1000,"durationMs":1}
            ]),
        ))
        .await
        .unwrap();
    let mut input = super::flow::request(&workspace, &flow.id);
    input.environment_id = Some(env.id);
    input.inputs = json!({"value":"flow-private"});
    input.secret_input_names = vec!["value".into()];
    let run = bus.run_flow(input).await.unwrap();
    let result = super::flow::finished(&bus, &run).await;
    assert_eq!(result.status, unfour_core::models::FlowRunStatus::Failed);
    assert_eq!(result.error.as_deref(), Some("FLOW_HTTP_STATUS_422"));
    assert_eq!(
        result.steps[1].status,
        unfour_core::models::FlowStepRunStatus::Skipped
    );
    let output = result.steps[0].output.as_ref().unwrap();
    assert_eq!(output["status"], 422);
    assert_eq!(output["diagnosticsTruncated"], true);
    assert!(output["body"]
        .as_str()
        .unwrap()
        .starts_with("upstream failed"));
    let encoded = output.to_string();
    assert!(encoded.contains("trace-42"));
    for secret in [
        "environment-private",
        "flow-private",
        "auth-private",
        "private-cookie",
    ] {
        assert!(!encoded.contains(secret), "{secret}");
    }
    assert!(encoded.len() < 262144);
    assert_eq!(result.steps[0].attempts[0].output.as_ref(), Some(output));
    server.join().unwrap();
}

#[tokio::test]
async fn flow_ssh_secret_metadata_respects_selected_environment_and_explicit_inputs() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let variable = |key: &str, value: &str, secret| {
        serde_json::from_value::<WorkspaceVariableInput>(
            json!({"key":key,"value":value,"isSecret":secret}),
        )
        .unwrap()
    };
    bus.workspace_variables_replace(
        workspace.clone(),
        vec![
            variable("VERSION", "v1", true),
            variable("PATH", "private-path", true),
        ],
    )
    .await
    .unwrap();
    let env = bus
        .workspace_environment_create(workspace.clone(), "selected".into())
        .await
        .unwrap();
    bus.workspace_environment_update(
        workspace.clone(),
        env.id.clone(),
        env.name,
        vec![variable("VERSION", "v2", false)],
    )
    .await
    .unwrap();
    let action: FlowAction = serde_json::from_value(json!({"capability":"ssh","resourceId":"task","arguments":{"workspaceDefaults":true,"inputs":{"PATH":"public-path","ALIAS":"prefix-private-value"}}})).unwrap();
    let mut input = super::flow::request(&workspace, "unused");
    input.environment_id = Some(env.id);
    input.inputs = json!({"value":"private-value"});
    input.secret_input_names = vec!["value".into()];
    let inputs = BTreeMap::from([
        ("VERSION".into(), "v2".into()),
        ("PATH".into(), "public-path".into()),
        ("ALIAS".into(), "prefix-private-value".into()),
        ("TOKEN".into(), "sensitive-name-value".into()),
    ]);
    assert_eq!(
        bus.flow_ssh_secret_names(&action, &input, &inputs)
            .await
            .unwrap(),
        vec!["ALIAS", "TOKEN"]
    );
    let details = bus
        .flow_diagnostics(
            json!({"body":"\u{0001}".repeat(100000),"log":"\u{0001}".repeat(100000)}),
            &input,
        )
        .await
        .unwrap();
    assert!(serde_json::to_vec(&details).unwrap().len() < 262144);
    assert_eq!(details["logTruncated"], true);
}

#[test]
fn query_explicit_occurrences_round_trip() {
    let pair = |key: &str, value: &str, enabled| json!({"key":key,"value":value,"enabled":enabled});
    for (first, second) in [(true, true), (false, true), (true, false), (false, false)] {
        let patches = json!({"queryPatch":[
            {"key":"tag","value":"B","enabled":second,"occurrence":1},
            {"key":"tag","value":"A","enabled":first,"occurrence":0}
        ]});
        let wire = serde_json::to_string(&patches).unwrap();
        let mut request = json!({"query":[pair("tag","a",true),pair("tag","b",true)]});
        apply_api_arguments(&mut request, &serde_json::from_str(&wire).unwrap()).unwrap();
        let expected: Vec<_> = [(first, "A"), (second, "B")]
            .into_iter()
            .filter(|(enabled, _)| *enabled)
            .map(|(_, v)| pair("tag", v, true))
            .collect();
        assert_eq!(request["query"], json!(expected));
    }
    let mut request = json!({"query":[pair("tag","a",true),pair("tag","b",true)]});
    apply_api_arguments(
        &mut request,
        &json!({"queryPatch":[{"key":"tag","value":"B","enabled":true,"occurrence":1}]}),
    )
    .unwrap();
    assert_eq!(
        request["query"],
        json!([pair("tag", "a", true), pair("tag", "B", true)])
    );
    for occurrence in [json!(-1), json!(1.5), json!("1"), serde_json::Value::Null] {
        assert!(validate_api_arguments(&json!({"queryPatch":[{"key":"tag","value":"x","enabled":true,"occurrence":occurrence}]}),true).is_err());
    }
}

#[tokio::test]
async fn ssh_unresolved_sensitive_output_provenance() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let action: FlowAction = serde_json::from_value(json!({"capability":"ssh","resourceId":"task","arguments":{"inputs":{
        "DEPLOY":"${/steps/login/body/token}","ALIAS":{"$ref":"/steps/login/body/privateKey"},"VISIBLE":"${/steps/build/body/version}"
    }}})).unwrap();
    let inputs = BTreeMap::from([
        ("DEPLOY".into(), "hidden-one".into()),
        ("ALIAS".into(), "hidden-two".into()),
        ("VISIBLE".into(), "v1".into()),
        ("DEPLOY_TOKEN".into(), "hidden-three".into()),
        ("DB_PASSWORD".into(), "hidden-four".into()),
    ]);
    assert_eq!(
        bus.flow_ssh_secret_names(
            &action,
            &super::flow::request(&workspace, "unused"),
            &inputs
        )
        .await
        .unwrap(),
        vec!["ALIAS", "DB_PASSWORD", "DEPLOY", "DEPLOY_TOKEN"]
    );
}

#[tokio::test]
async fn flow_auth_echo_diagnostics_use_resolved_auth_values() {
    use std::io::{Read, Write};
    for mut auth in [
        json!({"type":"api-key","key":"X-Company-Key","value":"custom-private","addTo":"header"}),
        json!({"type":"api-key","key":"company","value":"custom-private","addTo":"query"}),
        json!({"type":"bearer","token":"custom-private"}),
        json!({"type":"basic","username":"user","password":"custom-private"}),
    ] {
        let bus = test_bus().await;
        let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
        // Non-secret metadata and an ordinary variable name ensure auth provenance
        // itself is what protects this resolved credential.
        bus.workspace_variables_replace(
            workspace.clone(),
            vec![serde_json::from_value(
                json!({"key":"VALUE","value":"custom-private","isSecret":false}),
            )
            .unwrap()],
        )
        .await
        .unwrap();
        let field = match auth["type"].as_str().unwrap() {
            "bearer" => "token",
            "basic" => "password",
            _ => "value",
        };
        auth[field] = json!("{{VALUE}}");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 8192];
            let n = socket.read(&mut bytes).unwrap();
            let wire = String::from_utf8_lossy(&bytes[..n]);
            let credential = wire
                .lines()
                .find_map(|line| line.strip_prefix("authorization: "))
                .unwrap_or("custom-private");
            let body = format!("echo custom-private {credential} safe-context");
            write!(socket,"HTTP/1.1 401 Failed\r\nX-Echo: custom-private {credential}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        });
        let mut api = api_script_test_input(workspace.clone(), url);
        api.auth_json = Some(auth.to_string());
        let api = bus.save_api_request(api).await.unwrap();
        let flow = bus
            .save_flow(super::flow::definition(
                &workspace,
                json!([super::flow::action("api", "api", &api.id, json!({}))]),
            ))
            .await
            .unwrap();
        let run = bus
            .run_flow(super::flow::request(&workspace, &flow.id))
            .await
            .unwrap();
        let result = super::flow::finished(&bus, &run).await;
        let output = result.steps[0].output.as_ref().unwrap().to_string();
        assert!(output.contains("safe-context"), "{output}");
        assert!(!output.contains("custom-private"), "{output}");
        assert!(!output.contains("dXNlcjpj"), "{output}");
        server.join().unwrap();
    }
}

#[tokio::test]
async fn flow_query_occurrences_survive_save_load_and_http_execution() {
    use std::io::{Read, Write};
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for (first, second) in [(true, true), (false, true), (true, false), (false, false)] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 8192];
            let n = socket.read(&mut bytes).unwrap();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
            )
            .unwrap();
            String::from_utf8_lossy(&bytes[..n])
                .lines()
                .next()
                .unwrap()
                .to_owned()
        });
        let mut api = api_script_test_input(workspace.clone(), url);
        api.query = serde_json::from_value(json!([{"key":"tag","value":"a","enabled":true},{"key":"tag","value":"b","enabled":true}])).unwrap();
        let api = bus.save_api_request(api).await.unwrap();
        let args = json!({"queryPatch":[{"key":"tag","value":"B","enabled":second,"occurrence":1},{"key":"tag","value":"A","enabled":first,"occurrence":0}]});
        let flow = bus
            .save_flow(super::flow::definition(
                &workspace,
                json!([super::flow::action("api", "api", &api.id, args)]),
            ))
            .await
            .unwrap();
        let run = bus
            .run_flow(super::flow::request(&workspace, &flow.id))
            .await
            .unwrap();
        let result = super::flow::finished(&bus, &run).await;
        assert_eq!(
            result.status,
            unfour_core::models::FlowRunStatus::Succeeded,
            "{result:?}"
        );
        let query = [(first, "tag=A"), (second, "tag=B")]
            .into_iter()
            .filter(|(enabled, _)| *enabled)
            .map(|(_, value)| value)
            .collect::<Vec<_>>()
            .join("&");
        let path = if query.is_empty() {
            "/?".to_owned()
        } else {
            format!("/?{query}")
        };
        assert_eq!(server.join().unwrap(), format!("GET {path} HTTP/1.1"));
    }
}
