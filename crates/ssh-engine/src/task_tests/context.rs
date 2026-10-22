use super::super::*;
use super::support::*;
use unfour_core::models::{SshTaskInputSource, SshTaskRunContextInput};

async fn fixture() -> (SshService, SshTaskDetail, SshTaskRunInput) {
    let (service, workspace_id) = service().await;
    let mut task = docker_export_input(workspace_id.clone());
    task.steps.truncate(1);
    task.steps[0].config_json["command"] = serde_json::json!("echo {{version}} {{region}} {{custom}} {{alias}} {{API_TOKEN}} {{authorization}} {{cookie}} {{proxy_authorization}} {{x_api_key}} {{x_auth_token}} {{pem}} {{url}} {{header}}");
    let detail = service.save_task(task).await.unwrap();
    let mut inputs = std::collections::BTreeMap::new();
    for name in SshService::detected_task_inputs(&detail.steps).unwrap() {
        inputs.insert(name.clone(), format!("fixture-{name}"));
    }
    inputs.insert("version".into(), "v2.3.0".into());
    inputs.insert("region".into(), "current-region".into());
    inputs.insert("custom".into(), "hidden-value".into());
    inputs.insert("alias".into(), "prefix-hidden-value".into());
    // Synthetic marker only; no private key material is embedded in the fixture.
    inputs.insert(
        "pem".into(),
        ["-----BEGIN ", "PRIVATE KEY-----fixture"].concat(),
    );
    inputs.insert("url".into(), "https://user:credential@example.com".into());
    inputs.insert("header".into(), "Bearer opaque".into());
    let sources = inputs
        .keys()
        .map(|name| {
            (
                name.clone(),
                if name == "region" {
                    SshTaskInputSource::Environment
                } else {
                    SshTaskInputSource::Manual
                },
            )
        })
        .collect();
    let input = SshTaskRunInput {
        workspace_id,
        task_id: detail.task.id.clone(),
        connection_id: None,
        inputs,
        secret_input_names: vec!["custom".into()],
        context: Some(SshTaskRunContextInput {
            environment_id: Some("env".into()),
            task_version: context::task_version(&detail),
            environment_updated_at: Some("now".into()),
            input_sources: sources,
        }),
    };
    (service, detail, input)
}

#[tokio::test]
async fn recall_context_excludes_secrets_aliases_credentials_and_variable_values() {
    let (_, detail, input) = fixture().await;
    let secrets = task_secret_values(&input.inputs, &input.secret_input_names).unwrap();
    let snapshot = context::run_context(&input, &detail, &secrets)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.inputs["version"].value.as_deref(), Some("v2.3.0"));
    for (name, value) in &input.inputs {
        if name == "version" {
            continue;
        }
        assert!(snapshot.inputs[name].value.is_none(), "{name}");
        assert!(
            !serde_json::to_string(&snapshot).unwrap().contains(value),
            "{name}"
        );
        if name != "region" {
            assert!(
                !redact_task_secret_values(value, &secrets).contains(value),
                "{name}"
            );
        }
    }
    assert_eq!(
        snapshot.inputs["region"].source,
        SshTaskInputSource::Environment
    );
}

#[tokio::test]
async fn recall_rejects_changed_step_definitions_and_keeps_old_adapter_compatible() {
    let (_, mut detail, mut input) = fixture().await;
    detail.steps[0].updated_at = "changed".into();
    assert!(context::run_context(&input, &detail, &[]).is_err());
    input.context = None;
    assert_eq!(context::run_context(&input, &detail, &[]).unwrap(), None);
    input.inputs.clear();
    assert!(resolve_enabled_steps(&detail.steps, &input.inputs).is_err());
}

#[cfg(feature = "ssh-native")]
#[tokio::test]
async fn reruns_create_independent_records_and_logs_without_modifying_original() {
    let (service, detail, mut input) = fixture().await;
    let connection = connection(&service, &input.workspace_id).await;
    // A bound listener that closes accepted sockets gives deterministic local failure.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    sqlx::query("UPDATE connections SET port = ?1 WHERE id = ?2")
        .bind(listener.local_addr().unwrap().port() as i64)
        .bind(&connection.id)
        .execute(service.db.pool())
        .await
        .unwrap();
    let acceptor = tokio::spawn(async move {
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.unwrap();
            drop(stream);
        }
    });
    input.connection_id = Some(connection.id.clone());
    let first = service.run_task(input.clone()).await.unwrap();
    async fn finished(service: &SshService, run: &SshTaskRun) -> SshTaskRun {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let current = service
                    .get_task_run(&run.workspace_id, &run.id)
                    .await
                    .unwrap();
                if current.status != "running" {
                    break current;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    let first = finished(&service, &first).await;
    assert_eq!(first.status, "failed");
    let original_log = std::fs::read_to_string(&first.log_path).unwrap();
    let second = service.run_task(input.clone()).await.unwrap();
    let second = finished(&service, &second).await;
    acceptor.await.unwrap();
    assert_ne!(first.id, second.id);
    assert_ne!(first.log_path, second.log_path);
    assert_eq!(
        std::fs::read_to_string(&first.log_path).unwrap(),
        original_log
    );
    assert_eq!(
        service
            .get_task_run(&input.workspace_id, &first.id)
            .await
            .unwrap(),
        first
    );
    let runs = service
        .list_task_runs(input.workspace_id.clone(), detail.task.id)
        .await
        .unwrap();
    assert_eq!(runs.len(), 2);
    let stored: String = sqlx::query_scalar("SELECT context_json FROM ssh_task_run WHERE id = ?1")
        .bind(&second.id)
        .fetch_one(service.db.pool())
        .await
        .unwrap();
    for (name, value) in &input.inputs {
        if name != "version" {
            assert!(!stored.contains(value));
            assert!(!original_log.contains(value));
        }
    }
    sqlx::query("UPDATE connections SET deleted_at = 'deleted' WHERE id = ?1")
        .bind(&connection.id)
        .execute(service.db.pool())
        .await
        .unwrap();
    assert!(service.run_task(input).await.is_err());
    std::fs::remove_dir_all(service.task_log_dir.as_ref()).unwrap();
}
