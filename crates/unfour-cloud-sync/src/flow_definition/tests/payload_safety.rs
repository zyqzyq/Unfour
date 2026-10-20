use super::*;

fn action_payload(capability: &str, arguments: Value) -> Value {
    let mut value = payload();
    value["steps"] = json!([{"id":"run","name":"Run","timeoutMs":1000,"kind":"action",
        "action":{"capability":capability,"resourceId":RESOURCE_ID,"connectionId":RESOURCE_ID,"arguments":arguments}}]);
    value
}

fn roundtrip(value: Value) -> Value {
    let ExternalFlowDefinitionApply::Upsert(parsed) =
        parse_change("ws", &change("ws", FLOW_ID, Some(value.clone()), 1)).unwrap()
    else {
        panic!()
    };
    let canonical = canonical_payload(&FlowDefinitionSnapshot {
        id: FLOW_ID.into(),
        workspace_id: "ws".into(),
        revision: 1,
        definition: parsed.definition.clone(),
    })
    .unwrap();
    let ExternalFlowDefinitionApply::Upsert(replayed) =
        parse_change("ws", &change("ws", FLOW_ID, Some(canonical), 1)).unwrap()
    else {
        panic!()
    };
    assert_eq!(parsed.definition, replayed.definition);
    parsed.definition
}

fn rejected(value: Value) {
    let original = value.clone();
    assert_eq!(
        parse_change("ws", &change("ws", FLOW_ID, Some(value.clone()), 1)).unwrap_err(),
        SyncError::FlowUnsafeDefinition,
        "unsafe fixture accepted"
    );
    let mut local = value.clone();
    local["id"] = json!(FLOW_ID);
    local["workspaceId"] = json!("ws");
    local["revision"] = json!(1);
    assert_eq!(
        canonical_payload(&FlowDefinitionSnapshot {
            id: FLOW_ID.into(),
            workspace_id: "ws".into(),
            revision: 1,
            definition: local,
        })
        .unwrap_err(),
        SyncError::FlowUnsafeDefinition
    );
    assert_eq!(value, original);
}

#[tokio::test]
async fn real_sql_ssh_api_arguments_and_templates_preserve_execution_values() {
    let db = super::support::database().await;
    let bus = super::support::CommandBus::from_db(db.clone())
        .await
        .unwrap();
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for (capability, arguments) in [
        (
            "database",
            json!({"sql":"SELECT id, status FROM jobs WHERE status = 'ready'", "catalog":"app", "schema":"public", "limit":100}),
        ),
        (
            "database",
            json!({"sql":"SELECT * FROM jobs WHERE id = ${/inputs/id}", "limit":10}),
        ),
        (
            "ssh",
            json!({"inputs":{"VERSION":"v1.2", "directory":"/home/deploy/app", "file":"build-${/inputs/id}.zip", "DEPLOY_TOKEN":{"$ref":"/inputs/credential"}}, "workspaceDefaults":true}),
        ),
        (
            "api",
            json!({"url":"https://api.example.test/v1/jobs?state=ready", "body":"{\"state\":\"ready\",\"count\":2}", "headers":[{"key":"Accept","value":"application/json","enabled":true}]}),
        ),
        (
            "api",
            json!({"url":"${/inputs/base}/jobs/${/inputs/id}?page=1", "body":"{\"message\":\"build-${/inputs/id}\",\"count\":${/inputs/count},\"token\":\"${/inputs/credential}\"}", "headersPatch":[{"key":"Authorization","value":"Bearer ${/inputs/credential}","enabled":true}], "queryPatch":[{"key":"state","value":"ready","enabled":true,"occurrence":0}]}),
        ),
        (
            "api",
            json!({"body":"state=ready&label=build-${/inputs/id}&access_token=${/inputs/credential}"}),
        ),
        (
            "api",
            json!({"body":"ordinary text describing a deployment", "url":"https://example.test/token/status"}),
        ),
        (
            "api",
            json!({"url":"https://unfour:443/profile", "body":"profile: ready"}),
        ),
        (
            "api",
            json!({"url":"https://user:${/inputs/credential}@example.test/?access_token=${/inputs/credential}"}),
        ),
    ] {
        let parsed = roundtrip(action_payload(capability, arguments.clone()));
        assert_eq!(parsed["steps"][0]["action"]["arguments"], arguments);
        let context = json!({"inputs":{"id":42,"count":3,"base":"https://example.test","credential":"runtime-only"}});
        assert_eq!(
            unfour_flow_engine::expression::resolve(&arguments, &context).unwrap(),
            unfour_flow_engine::expression::resolve(
                &parsed["steps"][0]["action"]["arguments"],
                &context
            )
            .unwrap()
        );
        // Exercise the owning apply validator, not only wire roundtrip.
        let remote = parse_change(
            &workspace,
            &change(
                &workspace,
                FLOW_ID,
                Some(action_payload(capability, arguments)),
                1,
            ),
        )
        .unwrap();
        let mut tx = db.pool().begin().await.unwrap();
        unfour_flow_engine::FlowService::apply_external_definition_on(
            &mut tx,
            &CommandContext::external("test"),
            remote,
        )
        .await
        .unwrap();
        tx.rollback().await.unwrap();
    }
}

#[test]
fn string_conditions_and_nonsecret_defaults_remain_literal_data() {
    let mut value = payload();
    value["inputs"] = json!([
        {"name":"channel","type":"string","default":"stable"},
        {"name":"count","type":"number","default":2},
        {"name":"dryRun","type":"boolean","default":false},
        {"name":"config","type":"json","default":{"labels":["ready","build-${/inputs/id}"],"empty":null}},
        {"name":"literal","type":"string","default":"${this is literal default data}"}
    ]);
    value["steps"] = json!([
        {"id":"call","name":"Call","timeoutMs":1000,"kind":"action","action":{"capability":"api","resourceId":RESOURCE_ID,"arguments":{"url":"https://example.test"}}},
        {"id":"branch","name":"Branch","timeoutMs":1000,"kind":"condition","predicate":{"left":"${/steps/call/body/state}","op":"in","right":["ready","complete"]},"ifTrue":"$end","ifFalse":"$end"}
    ]);
    let parsed = roundtrip(value.clone());
    for (before, after) in value["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .zip(parsed["inputs"].as_array().unwrap())
    {
        assert_eq!(before["default"], after["default"]);
    }
    assert_eq!(
        parsed["steps"][1]["predicate"],
        value["steps"][1]["predicate"]
    );
}

#[test]
fn sensitive_fields_require_whole_runtime_values_including_real_api_pair_rows() {
    for arguments in [
        json!({"password":"fixture"}),
        json!({"password":null}),
        json!({"inputs":{"DEPLOY_TOKEN":"fixture"}}),
        json!({"headers":[{"key":"Authorization","value":"fixture","enabled":false}]}),
        json!({"headersPatch":[{"key":"X-API-Key","value":"prefix-${/inputs/credential}","enabled":true}]}),
        json!({"queryPatch":[{"key":"access_token","value":123,"enabled":true}]}),
        json!({"headers":[{"key":"${/inputs/header}","value":"fixture","enabled":true}]}),
        json!({"body":"{\"nested\":[{\"password\":\"fixture\"}]}"}),
        json!({"body":"{\"password\":{\"$ref\":\"/inputs/credential\"}}"}),
        json!({"body":"{\"token\":\"${/inputs/credential}-fixture\"}"}),
        json!({"body":r#"{"token":"\u0024{/inputs/credential}"}"#}),
        json!({"body":r#"{"token":"\u005f_unfour_runtime_value__"}"#}),
        json!({"body":"{\"${/inputs/key}\":\"fixture\"}"}),
        json!({"body":"{\"password\":\"fixture\",\"dynamic\":${/inputs/value}"}),
        json!({"body":"%70assword=fixture"}),
        json!({"body":"data=%7B%22password%22%3A%22fixture%22%7D"}),
        json!({"body":"token=%24%7B%2Finputs%2Fcredential%7D"}),
        json!({"url":"https://example.test/?%61ccess_token=fixture"}),
        json!({"url":"https://example.test/?data=%7B%22password%22%3A%22fixture%22%7D"}),
        json!({"url":"${/inputs/base}/path?token=fixture"}),
        json!({"url":"https://user:fixture@example.test"}),
        json!({"url":"https://user:%24%7B%2Finputs%2Fcredential%7D@example.test"}),
    ] {
        rejected(action_payload("api", arguments));
    }
    for reference in [
        json!({"$ref":"/inputs/credential"}),
        json!("${/inputs/credential}"),
        json!("Bearer ${/inputs/credential}"),
    ] {
        roundtrip(action_payload(
            "api",
            json!({"headersPatch":[{"key":"Authorization","value":reference,"enabled":true}]}),
        ));
    }
    roundtrip(action_payload(
        "api",
        json!({"headers":[{"key":"${/inputs/header}","value":{"$ref":"/inputs/credential"},"enabled":true}]}),
    ));
}

#[test]
fn secret_defaults_and_local_credentials_are_never_syncable() {
    for default in [
        json!(null),
        json!("fixture"),
        json!(false),
        json!("${/inputs/credential}"),
        json!({"$ref":"/inputs/credential"}),
    ] {
        let mut value = payload();
        value["inputs"][0]["default"] = default;
        rejected(value);
    }
    for default in [
        json!({"password":"fixture"}),
        json!({"password":"${/inputs/credential}"}),
        json!("{\"token\":\"${/inputs/credential}\"}"),
    ] {
        let mut value = payload();
        value["inputs"] = json!([{"name":"config","type":"json","default":default}]);
        rejected(value);
    }
    let private_header = format!("-----BEGIN {}-----", "OPENSSH PRIVATE KEY");
    for arguments in [
        json!({"credentialRef":{"$ref":"/inputs/credential"}}),
        json!({"secret_store":"${/inputs/credential}"}),
        json!({"privateKeyPath":"${/inputs/path}"}),
        json!({"data":"keychain:device-entry"}),
        json!({"data":"secret-store:device-entry"}),
        json!({"data":"unfour:workspace:ssh:record"}),
        json!({"data":"@unfour-secret:handle"}),
        json!({"data":private_header}),
        json!({"body":"{\"data\":\"@unfour-secret:handle\"}"}),
        json!({"body":"{\"credentialRef\":\"${/inputs/credential}\"}"}),
        json!({"headers":[{"key":"keychain","value":"${/inputs/credential}","enabled":true}]}),
        json!({"body":"data=%40unfour-secret%3Ahandle"}),
    ] {
        rejected(action_payload("api", arguments));
    }
}

#[test]
fn malformed_references_and_unavailable_graph_values_are_rejected() {
    for expression in [
        json!({"$ref":false}),
        json!({"$ref":"/inputs/a","extra":1}),
        json!({"$ref":"/inputs/a~2b"}),
        json!("${inputs/a}"),
        json!("prefix-${/inputs/a"),
        json!("${/inputs/a} ${/bad/root}"),
        json!("${/steps/run/body}"),
        json!("${/steps/missing/body}"),
        json!("${/probe/status}"),
    ] {
        let value = action_payload("api", json!({"body":expression}));
        assert!(parse_change("ws", &change("ws", FLOW_ID, Some(value), 1)).is_err());
    }
    // Graph validation still runs on references in otherwise safe sensitive values.
    let value = action_payload(
        "ssh",
        json!({"inputs":{"PASSWORD":{"$ref":"/steps/run/body"}}}),
    );
    assert_eq!(
        parse_change("ws", &change("ws", FLOW_ID, Some(value), 1)).unwrap_err(),
        SyncError::InvalidData
    );
}

#[test]
fn executable_payload_size_limit_applies_before_safety_and_after_normalization() {
    let value = action_payload("database", json!({"sql":"x".repeat(MAX_PAYLOAD_BYTES)}));
    assert_eq!(
        parse_change("ws", &change("ws", FLOW_ID, Some(value), 1)).unwrap_err(),
        SyncError::FlowPayloadTooLarge
    );
    let mut value = payload();
    value["inputs"][0]["description"] = json!("");
    let overhead = serde_json::to_vec(&value).unwrap().len();
    value["inputs"][0]["description"] = json!("x".repeat(MAX_PAYLOAD_BYTES - overhead));
    // Raw wire bytes fit, but the canonical model adds `next: null`.
    assert_eq!(serde_json::to_vec(&value).unwrap().len(), MAX_PAYLOAD_BYTES);
    assert_eq!(
        parse_change("ws", &change("ws", FLOW_ID, Some(value), 1)).unwrap_err(),
        SyncError::FlowPayloadTooLarge
    );
}
