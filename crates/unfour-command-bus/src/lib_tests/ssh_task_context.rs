use super::*;
use unfour_core::models::{SshTaskInputSource, SshTaskInputVersion, SshTaskRunContextInput};

async fn refresh_versions(bus: &CommandBus, input: &mut SshTaskRunInput) {
    let context = input.context.as_mut().unwrap();
    let snapshot = bus
        .workspace
        .variable_snapshot(&input.workspace_id, context.environment_id.as_deref())
        .await
        .unwrap();
    context.input_versions = Some(
        context
            .input_sources
            .iter()
            .filter_map(|(name, source)| {
                let version = match source {
                    SshTaskInputSource::Manual => return None,
                    SshTaskInputSource::Workspace => snapshot
                        .variables
                        .iter()
                        .find(|v| v.key == *name)
                        .map(|v| (v.id.clone(), v.revision)),
                    SshTaskInputSource::Environment => snapshot
                        .environment
                        .as_ref()
                        .and_then(|env| env.variables.iter().find(|v| v.key == *name))
                        .map(|v| (v.id.clone(), v.revision)),
                }?;
                Some((
                    name.clone(),
                    SshTaskInputVersion {
                        id: version.0,
                        revision: version.1,
                    },
                ))
            })
            .collect(),
    );
}

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
            input_versions: None,
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
    refresh_versions(&bus, &mut input).await;
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
    assert!(error.to_string().contains("SSH_TASK_INPUTS_CHANGED"));
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
            input_versions: None,
        }),
    };
    assert!(bus
        .prepare_ssh_task_context(&mut input)
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_ENVIRONMENT_MISSING"));
}

async fn source_fixture(environment_source: bool) -> (CommandBus, SshTaskRunInput, String) {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let detail = bus
        .save_ssh_task(SshTaskSaveInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "Snapshot".into(),
            description: String::new(),
            default_connection_id: None,
            steps: vec![],
        })
        .await
        .unwrap();
    let env = bus
        .workspace_environment_create(workspace.clone(), "Test".into())
        .await
        .unwrap();
    let workspace_variable = bus
        .workspace_variable_create(workspace.clone(), variable("REGION", "old", true))
        .await
        .unwrap();
    let env_variable = bus
        .workspace_environment_variable_create(
            workspace.clone(),
            env.id.clone(),
            variable("REGION", "old", true),
        )
        .await
        .unwrap();
    let mut input = SshTaskRunInput {
        workspace_id: workspace,
        task_id: detail.task.id,
        connection_id: Some("explicit".into()),
        inputs: [("REGION".into(), String::new())].into(),
        secret_input_names: vec![],
        context: Some(SshTaskRunContextInput {
            environment_id: Some(env.id),
            environment_updated_at: Some(env.updated_at),
            task_version: "current".into(),
            input_sources: [(
                "REGION".into(),
                if environment_source {
                    SshTaskInputSource::Environment
                } else {
                    SshTaskInputSource::Workspace
                },
            )]
            .into(),
            input_versions: None,
        }),
    };
    refresh_versions(&bus, &mut input).await;
    (
        bus,
        input,
        if environment_source {
            env_variable.id
        } else {
            workspace_variable.id
        },
    )
}

#[tokio::test]
async fn ssh_context_rejects_referenced_variable_mutations_without_environment_timestamp_changes() {
    for environment_source in [false, true] {
        for mutation in [
            "edit",
            "disable",
            "delete",
            "rename",
            "recreate",
            "secret_flag",
        ] {
            let (bus, mut input, id) = source_fixture(environment_source).await;
            let workspace = input.workspace_id.clone();
            let env = input
                .context
                .as_ref()
                .unwrap()
                .environment_id
                .clone()
                .unwrap();
            let mut updated = variable("REGION", "old", true);
            match mutation {
                "edit" => updated.value = "rotated-secret".into(),
                "disable" => updated.is_enabled = false,
                "rename" => updated.key = "RENAMED".into(),
                "secret_flag" => updated.is_secret = false,
                _ => {}
            }
            if environment_source {
                if mutation == "delete" || mutation == "recreate" {
                    bus.workspace_environment_variable_delete(workspace.clone(), env.clone(), id)
                        .await
                        .unwrap();
                    if mutation == "recreate" {
                        bus.workspace_environment_variable_create(workspace, env, updated)
                            .await
                            .unwrap();
                    }
                } else {
                    bus.workspace_environment_variable_update(workspace, env, id, updated)
                        .await
                        .unwrap();
                }
                let current = bus
                    .workspace_environments_list(input.workspace_id.clone())
                    .await
                    .unwrap();
                assert_eq!(
                    Some(current[0].updated_at.clone()),
                    input.context.as_ref().unwrap().environment_updated_at
                );
            } else if mutation == "delete" || mutation == "recreate" {
                bus.workspace_variable_delete(workspace.clone(), id)
                    .await
                    .unwrap();
                if mutation == "recreate" {
                    bus.workspace_variable_create(workspace, updated)
                        .await
                        .unwrap();
                }
            } else {
                bus.workspace_variable_update(workspace, id, updated)
                    .await
                    .unwrap();
            }
            let error = bus.prepare_ssh_task_context(&mut input).await.unwrap_err();
            assert!(
                error.to_string().contains("SSH_TASK_INPUTS_CHANGED"),
                "{environment_source}/{mutation}: {error}"
            );
            assert_eq!(input.inputs["REGION"], "");
            assert!(!error.to_string().contains("rotated-secret"));
            // Rejected before reaching the SSH engine; no run/history side effects.
            assert!(bus
                .run_ssh_task(input.clone())
                .await
                .unwrap_err()
                .to_string()
                .contains("SSH_TASK_INPUTS_CHANGED"));
            assert!(bus
                .list_ssh_task_runs(input.workspace_id.clone(), input.task_id.clone())
                .await
                .unwrap()
                .is_empty());
        }
    }
}

#[tokio::test]
async fn ssh_context_checks_only_the_explicit_source_and_accepts_unrelated_changes() {
    for environment_source in [false, true] {
        let (bus, mut input, _) = source_fixture(environment_source).await;
        let workspace = input.workspace_id.clone();
        let env_id = input
            .context
            .as_ref()
            .unwrap()
            .environment_id
            .clone()
            .unwrap();
        bus.workspace_variable_create(workspace.clone(), variable("UNRELATED", "one", false))
            .await
            .unwrap();
        let variables = if environment_source {
            let mut workspace_value = variable("REGION", "changed-shadowed-workspace", true);
            workspace_value.id = Some(
                bus.workspace_variables_list(workspace.clone())
                    .await
                    .unwrap()[0]
                    .id
                    .clone(),
            );
            bus.workspace_variable_update(
                workspace.clone(),
                workspace_value.id.clone().unwrap(),
                workspace_value,
            )
            .await
            .unwrap();
            let envs = bus
                .workspace_environments_list(workspace.clone())
                .await
                .unwrap();
            let v = &envs[0].variables[0];
            let mut same = variable("REGION", "old", true);
            same.id = Some(v.id.clone());
            vec![same, variable("OTHER", "two", false)]
        } else {
            vec![
                variable("REGION", "changed-unused-environment", true),
                variable("OTHER", "two", false),
            ]
        };
        bus.workspace_environment_update(workspace.clone(), env_id, "Renamed".into(), variables)
            .await
            .unwrap();
        bus.prepare_ssh_task_context(&mut input).await.unwrap();
        assert_eq!(input.inputs["REGION"], "old");
        assert!(input.secret_input_names.contains(&"REGION".into()));
    }
}

#[tokio::test]
async fn ssh_context_legacy_manual_inputs_and_missing_sources_do_not_silently_rebind() {
    let (bus, mut input, _) = source_fixture(true).await;
    // Older request metadata decodes, but cannot authorize unknown current variable values.
    let mut legacy = serde_json::to_value(input.clone()).unwrap();
    legacy["context"]
        .as_object_mut()
        .unwrap()
        .remove("inputVersions");
    let mut legacy: SshTaskRunInput = serde_json::from_value(legacy).unwrap();
    assert!(bus
        .prepare_ssh_task_context(&mut legacy)
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_INPUTS_CHANGED"));
    legacy
        .context
        .as_mut()
        .unwrap()
        .input_sources
        .insert("REGION".into(), SshTaskInputSource::Manual);
    legacy.inputs.insert("REGION".into(), "manual".into());
    bus.prepare_ssh_task_context(&mut legacy).await.unwrap();
    assert_eq!(legacy.inputs["REGION"], "manual");
    // A missing historical source has no expected version; adding a variable later cannot fill it.
    input.context.as_mut().unwrap().input_versions = Some(Default::default());
    assert!(bus
        .prepare_ssh_task_context(&mut input)
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_INPUTS_CHANGED"));
    // Manual values remain manual when a matching source is added after preparation.
    legacy
        .context
        .as_mut()
        .unwrap()
        .input_sources
        .insert("NEW".into(), SshTaskInputSource::Manual);
    legacy.inputs.insert("NEW".into(), "typed".into());
    bus.workspace_variable_create(input.workspace_id.clone(), variable("NEW", "added", false))
        .await
        .unwrap();
    bus.prepare_ssh_task_context(&mut legacy).await.unwrap();
    assert_eq!(legacy.inputs["NEW"], "typed");
}

#[tokio::test]
async fn ssh_context_concurrent_rotation_never_executes_an_unconfirmed_value() {
    let (bus, input, id) = source_fixture(true).await;
    let reads = async {
        for _ in 0..16 {
            let mut attempt = input.clone();
            match bus.prepare_ssh_task_context(&mut attempt).await {
                Ok(()) => assert_eq!(attempt.inputs["REGION"], "old"),
                Err(error) => assert!(error.to_string().contains("SSH_TASK_INPUTS_CHANGED")),
            }
        }
    };
    let write = bus.workspace_environment_variable_update(
        input.workspace_id.clone(),
        input
            .context
            .as_ref()
            .unwrap()
            .environment_id
            .clone()
            .unwrap(),
        id,
        variable("REGION", "rotated", true),
    );
    let (_, updated) = tokio::join!(reads, write);
    updated.unwrap();
    let mut stale = input.clone();
    assert!(bus.prepare_ssh_task_context(&mut stale).await.is_err());
    let mut fresh = input;
    refresh_versions(&bus, &mut fresh).await;
    bus.prepare_ssh_task_context(&mut fresh).await.unwrap();
    assert_eq!(fresh.inputs["REGION"], "rotated");
}

#[tokio::test]
async fn ssh_context_source_switch_and_newly_available_source_require_reload() {
    let (bus, mut input, _) = source_fixture(false).await;
    // Even identical values in the other source do not authorize switching provenance.
    input
        .context
        .as_mut()
        .unwrap()
        .input_sources
        .insert("REGION".into(), SshTaskInputSource::Environment);
    assert!(bus
        .prepare_ssh_task_context(&mut input)
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_INPUTS_CHANGED"));
    refresh_versions(&bus, &mut input).await;
    bus.prepare_ssh_task_context(&mut input).await.unwrap();
    input
        .context
        .as_mut()
        .unwrap()
        .input_sources
        .insert("NEW".into(), SshTaskInputSource::Workspace);
    input.inputs.insert("NEW".into(), String::new());
    bus.workspace_variable_create(
        input.workspace_id.clone(),
        variable("NEW", "newly-added", true),
    )
    .await
    .unwrap();
    assert!(bus
        .prepare_ssh_task_context(&mut input)
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_INPUTS_CHANGED"));
    refresh_versions(&bus, &mut input).await;
    bus.prepare_ssh_task_context(&mut input).await.unwrap();
    assert_eq!(input.inputs["NEW"], "newly-added");
}
