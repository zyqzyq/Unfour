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
