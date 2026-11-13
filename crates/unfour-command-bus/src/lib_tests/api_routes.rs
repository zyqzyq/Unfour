use super::*;
use unfour_core::network::{ApiRoute, ApiRouteEdit, ApiRoutingInput};

fn edit(expected: ApiRoute, route: ApiRoute) -> ApiRouteEdit {
    ApiRouteEdit { expected, route }
}

#[tokio::test]
async fn api_routes_are_workspace_scoped_and_saved_atomically() {
    let bus = test_bus().await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let other = bus.create_workspace("other".into()).await.unwrap().id;
    let env = bus
        .workspace_environment_create(ws.clone(), "Private".into())
        .await
        .unwrap();
    let ssh = bus
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: ws.clone(),
            name: "Test jump".into(),
            host: "jump.invalid".into(),
            port: Some(22),
            username: "tester".into(),
            auth_kind: "password".into(),
            key_path: None,
            credential_ref: None,
            secret: Some("test-only".into()),
        })
        .await
        .unwrap();
    let routed = ApiRoute::Ssh {
        ssh_connection_id: ssh.id.clone(),
    };
    bus.set_api_environment_route(&ws, &env.id, edit(ApiRoute::Direct, routed.clone()))
        .await
        .unwrap();
    let input = api_script_test_input(ws.clone(), "http://service.invalid".into());
    let preview = ApiRoutingInput {
        request_id: None,
        route: None,
        expected_environment_id: Some(env.id.clone()),
        expected_effective: routed.clone(),
        expected_ssh_revision: Some(ssh.revision + 1),
    };
    let mut scripted = input.clone();
    scripted.pre_request_script = Some(r#"pm.environment.set("unexpected", "changed")"#.into());
    assert_eq!(
        bus.send_api_request_routed(
            "stale-jump",
            scripted,
            Some(env.id.clone()),
            None,
            Some(&preview)
        )
        .await
        .unwrap_err()
        .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
    assert!(bus
        .workspace_environments_list(ws.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.id == env.id)
        .unwrap()
        .variables
        .is_empty());
    let saved = bus.save_api_request(input.clone()).await.unwrap();
    assert_eq!(
        bus.api_routes(&ws, Some(&saved.id), Some(&env.id))
            .await
            .unwrap()
            .effective,
        routed
    );
    // A Flow pins the environment and jump definition, then rejects changes before dispatch.
    use unfour_flow_engine::{FlowExecutor, FlowPersistenceContext};
    let mut run = super::flow::request(&ws, "route-flow");
    bus.workspace_environment_set_active(ws.clone(), Some(env.id.clone()))
        .await
        .unwrap();
    let action = unfour_core::models::FlowAction {
        capability: unfour_core::models::FlowCapability::Api,
        resource_id: saved.id.clone(),
        connection_id: None,
        arguments: serde_json::json!({}),
    };
    let workspace_only = bus.prepare(&action, &run, false).await.unwrap();
    assert_eq!(workspace_only["resource"]["networkRoute"]["mode"], "direct");
    assert!(workspace_only["resource"]["routeEnvironmentId"].is_null());
    run.environment_id = Some(env.id.clone());
    let plan = bus.prepare(&action, &run, false).await.unwrap();
    assert_eq!(plan["resource"]["sshRevision"], ssh.revision);
    assert_eq!(plan["resource"]["routeEnvironmentId"], env.id);
    assert_eq!(
        bus.set_api_environment_route(&ws, &env.id, edit(ApiRoute::Direct, ApiRoute::Direct))
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
    bus.set_api_environment_route(&ws, &env.id, edit(routed.clone(), ApiRoute::Direct))
        .await
        .unwrap();
    let error = bus
        .execute(
            &action,
            &plan,
            &run,
            tokio_util::sync::CancellationToken::new(),
            &FlowPersistenceContext::default(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("FLOW_RESOURCE_CHANGED"));
    bus.set_api_environment_route(&ws, &env.id, edit(ApiRoute::Direct, routed.clone()))
        .await
        .unwrap();
    let direct = bus
        .update_api_request_routed(
            ws.clone(),
            saved.id.clone(),
            input.clone(),
            Some(saved.revision),
            Some(edit(ApiRoute::Inherit, ApiRoute::Direct)),
        )
        .await
        .unwrap();
    assert_eq!(
        direct.revision, saved.revision,
        "device routing must preserve the portable revision"
    );
    assert_eq!(
        bus.update_api_request_routed(
            ws.clone(),
            direct.id.clone(),
            input.clone(),
            Some(saved.revision),
            Some(edit(ApiRoute::Inherit, routed.clone()))
        )
        .await
        .unwrap_err()
        .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
    assert_eq!(
        bus.api_routes(&ws, Some(&direct.id), Some(&env.id))
            .await
            .unwrap()
            .effective,
        ApiRoute::Direct
    );
    let copy = bus
        .duplicate_api_request(ws.clone(), direct.id.clone())
        .await
        .unwrap();
    assert_eq!(
        bus.api_routes(&ws, Some(&copy.id), Some(&env.id))
            .await
            .unwrap()
            .request,
        ApiRoute::Direct
    );
    let bad = ApiRoute::Ssh {
        ssh_connection_id: "absent".into(),
    };
    assert_eq!(
        bus.update_api_request_routed(
            ws.clone(),
            direct.id.clone(),
            input,
            Some(direct.revision),
            Some(edit(ApiRoute::Direct, bad))
        )
        .await
        .unwrap_err()
        .code(),
        "SSH_TUNNEL_ROUTE_MISSING"
    );
    assert_eq!(
        bus.api_client
            .get_saved_request(&direct.id)
            .await
            .unwrap()
            .revision,
        direct.revision
    );
    assert!(bus
        .api_routes(&other, Some(&direct.id), None)
        .await
        .is_err());
    assert!(bus
        .set_api_environment_route(&other, &env.id, edit(routed.clone(), ApiRoute::Direct))
        .await
        .is_err());
    let foreign_env = bus
        .workspace_environment_create(other.clone(), "Foreign".into())
        .await
        .unwrap();
    assert!(bus
        .set_api_environment_route(&other, &foreign_env.id, edit(ApiRoute::Direct, routed))
        .await
        .is_err());
}

#[tokio::test]
async fn api_route_preview_change_aborts_before_running_pre_request_scripts_or_http() {
    let bus = test_bus().await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let input = api_script_test_input(ws, "http://unused.invalid".into());
    let preview = ApiRoutingInput {
        request_id: None,
        route: None,
        expected_environment_id: None,
        expected_effective: ApiRoute::Ssh {
            ssh_connection_id: "outdated".into(),
        },
        expected_ssh_revision: None,
    };
    assert_eq!(
        bus.send_api_request_routed("stale", input, None, None, Some(&preview))
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
}
