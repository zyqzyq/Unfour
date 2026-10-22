use super::*;
use unfour_core::models::{SshTaskInputSource, SshTaskRunContextInput};

fn variable(key: &str, value: &str, secret: bool) -> WorkspaceVariableInput {
    WorkspaceVariableInput {
        id: None,
        key: key.into(),
        value: value.into(),
        is_secret: secret,
        is_enabled: true,
        description: None,
        sort_order: 0,
    }
}

#[tokio::test]
async fn ssh_context_resolves_current_values_and_taints_manual_aliases() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    bus.workspace_variables_replace(
        workspace.clone(),
        vec![
            variable("REGION", "workspace", false),
            variable("PRIVATE", "current-secret", true),
        ],
    )
    .await
    .unwrap();
    let env = bus
        .workspace_environment_create(workspace.clone(), "Test".into())
        .await
        .unwrap();
    let env = bus
        .workspace_environment_update(
            workspace.clone(),
            env.id,
            "Test".into(),
            vec![
                variable("REGION", "updated-region", false),
                variable("DEPLOY", "rotated-secret", true),
            ],
        )
        .await
        .unwrap();
    let mut input = SshTaskRunInput {
        workspace_id: workspace.clone(),
        task_id: "task".into(),
        connection_id: Some("explicit".into()),
        inputs: [
            ("REGION", "stale"),
            ("DEPLOY", "expired"),
            ("PRIVATE", "stale"),
            ("alias", "prefix-current-secret"),
            ("version", "v2"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect(),
        secret_input_names: vec![],
        context: Some(SshTaskRunContextInput {
            environment_id: Some(env.id.clone()),
            task_version: "current".into(),
            environment_updated_at: Some(env.updated_at),
            input_sources: [
                ("REGION", SshTaskInputSource::Environment),
                ("DEPLOY", SshTaskInputSource::Environment),
                ("PRIVATE", SshTaskInputSource::Workspace),
                ("alias", SshTaskInputSource::Manual),
                ("version", SshTaskInputSource::Manual),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect(),
        }),
    };
    bus.prepare_ssh_task_context(&mut input).await.unwrap();
    assert_eq!(input.inputs["DEPLOY"], "rotated-secret");
    assert_eq!(input.inputs["PRIVATE"], "current-secret");
    assert_eq!(input.inputs["REGION"], "updated-region");
    assert_eq!(input.inputs["version"], "v2");
    for name in ["DEPLOY", "PRIVATE", "alias"] {
        assert!(input.secret_input_names.contains(&name.into()));
    }
    // Changing the active environment never changes an explicit selected source.
    bus.workspace_environment_set_active(workspace.clone(), None)
        .await
        .unwrap();
    bus.prepare_ssh_task_context(&mut input).await.unwrap();
    assert_eq!(input.inputs["REGION"], "updated-region");
    // A removed environment variable cannot fall back to the same workspace key.
    let updated = bus
        .workspace_environment_update(workspace.clone(), env.id.clone(), "Test".into(), vec![])
        .await
        .unwrap();
    assert!(bus.prepare_ssh_task_context(&mut input).await.is_err());
    input.context.as_mut().unwrap().environment_updated_at = Some(updated.updated_at);
    let error = bus.prepare_ssh_task_context(&mut input).await.unwrap_err();
    assert!(error.to_string().contains("SSH_TASK_INPUT_SOURCE_MISSING"));
    bus.workspace_environment_delete(workspace.clone(), env.id)
        .await
        .unwrap();
    assert!(bus
        .prepare_ssh_task_context(&mut input)
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_ENVIRONMENT_MISSING"));
    input.context.as_mut().unwrap().environment_id = None;
    input.context.as_mut().unwrap().environment_updated_at = None;
    input.connection_id = None;
    assert!(bus
        .prepare_ssh_task_context(&mut input)
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_CONNECTION_REQUIRED"));
}

#[tokio::test]
async fn ssh_context_cannot_resolve_another_workspaces_environment() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = SshTaskRunInput {
        workspace_id: workspace,
        task_id: "task".into(),
        connection_id: Some("explicit".into()),
        inputs: Default::default(),
        secret_input_names: vec![],
        context: Some(SshTaskRunContextInput {
            environment_id: Some("foreign-or-deleted".into()),
            task_version: "current".into(),
            environment_updated_at: None,
            input_sources: Default::default(),
        }),
    };
    assert!(bus
        .prepare_ssh_task_context(&mut input)
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_ENVIRONMENT_MISSING"));
}
