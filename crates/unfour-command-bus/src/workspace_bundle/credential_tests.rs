use super::*;
use crate::{CommandBusExtensions, TransactionalCommandHook};
use std::sync::{Arc, Mutex};

const PASSWORD: &str = "regression-backup-password";
fn options() -> WorkspaceBundleOptions {
    WorkspaceBundleOptions {
        password: Some(PASSWORD.into()),
        include_secrets: true,
        keep_local_paths: true,
        ..Default::default()
    }
}
fn input(variable: &WorkspaceVariable, value: &str) -> WorkspaceVariableInput {
    WorkspaceVariableInput {
        id: Some(variable.id.clone()),
        key: variable.key.clone(),
        value: value.into(),
        is_secret: false,
        is_enabled: true,
        description: None,
        sort_order: variable.sort_order,
    }
}
async fn backup(bus: &CommandBus) -> String {
    let workspace = bus
        .workspace_bundle_import(tests::fixture().to_string(), "Source".into())
        .await
        .unwrap();
    sqlx::query("UPDATE workspace_variables SET value='original-secret' WHERE workspace_id=? AND is_secret=1")
        .bind(&workspace.id).execute(bus.db.pool()).await.unwrap();
    sqlx::query("UPDATE workspace_environment_variables SET key='DB_PASSWORD',value='environment-secret',is_secret=0 WHERE workspace_id=?")
        .bind(&workspace.id).execute(bus.db.pool()).await.unwrap();
    bus.workspace_bundle_export_with_options(workspace.id, options())
        .await
        .unwrap()
        .content
}
async fn imported(bus: &CommandBus) -> Workspace {
    bus.workspace_bundle_import_with_options(backup(bus).await, "Copy".into(), options())
        .await
        .unwrap()
}
async fn secret_variable(bus: &CommandBus, workspace: &str) -> WorkspaceVariable {
    bus.workspace_variables_list(workspace.into())
        .await
        .unwrap()
        .into_iter()
        .find(|v| v.is_secret)
        .unwrap()
}
async fn read(bus: &CommandBus, workspace: &str, value: &str) -> AppResult<String> {
    bus.secret_store
        .read_secret(
            workspace.into(),
            value
                .strip_prefix("@unfour-secret:")
                .unwrap_or(value)
                .into(),
        )
        .await
}
async fn entries(bus: &CommandBus, workspace: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT credential_ref FROM workspace_bundle_credential_journal WHERE workspace_id=? ORDER BY credential_ref")
        .bind(workspace).fetch_all(bus.db.pool()).await.unwrap()
}

struct CaptureFailure(Arc<Mutex<Vec<String>>>);
impl TransactionalCommandHook for CaptureFailure {
    fn on_mutations<'a>(
        &'a self,
        db: &'a mut sqlx::SqliteConnection,
        _: &'a CommandContext,
        _: &'a [DomainMutation],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            *self.0.lock().unwrap() = sqlx::query_scalar(
                "SELECT credential_ref FROM workspace_bundle_credential_journal",
            )
            .fetch_all(db)
            .await?;
            Err(AppError::Validation("injected finalization failure".into()))
        })
    }
}

#[tokio::test]
async fn failed_import_removes_every_staged_keychain_entry() {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let content = backup(&bus).await;
    let plaintext = unfour_secret_store::backup::decrypt(&content, PASSWORD).unwrap();
    let payload: Value = serde_json::from_slice(&plaintext).unwrap();
    let expected = payload["secrets"].as_array().unwrap().len();
    assert!(expected >= 2);
    let captured = Arc::new(Mutex::new(Vec::new()));
    bus.extensions = CommandBusExtensions::new(vec![Arc::new(CaptureFailure(captured.clone()))]);
    assert!(bus
        .workspace_bundle_import_with_options(content, "Failed".into(), options())
        .await
        .is_err());
    let references = captured.lock().unwrap().clone();
    assert_eq!(references.len(), expected);
    for reference in references {
        let workspace = reference.split(':').nth(1).unwrap();
        assert!(read(&bus, workspace, &reference).await.is_err());
        assert!(entries(&bus, workspace).await.is_empty());
    }
}

#[tokio::test]
async fn variable_rotation_rolls_back_keychain_and_database_on_hook_failure() {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let workspace = imported(&bus).await;
    let old = secret_variable(&bus, &workspace.id).await;
    let before = entries(&bus, &workspace.id).await;
    let captured = Arc::new(Mutex::new(Vec::new()));
    bus.extensions = CommandBusExtensions::new(vec![Arc::new(CaptureFailure(captured.clone()))]);
    assert!(bus
        .workspace_variable_update(
            workspace.id.clone(),
            old.id.clone(),
            input(&old, "failed-replacement")
        )
        .await
        .is_err());
    assert_eq!(secret_variable(&bus, &workspace.id).await.value, old.value);
    assert_eq!(
        read(&bus, &workspace.id, &old.value).await.unwrap(),
        "original-secret"
    );
    let created: Vec<_> = captured
        .lock()
        .unwrap()
        .iter()
        .filter(|r| !before.contains(r))
        .cloned()
        .collect();
    assert_eq!(created.len(), 1);
    assert!(read(&bus, &workspace.id, &created[0]).await.is_err());
    assert_eq!(entries(&bus, &workspace.id).await, before);
}

#[tokio::test]
async fn rotation_clear_delete_and_bulk_failure_reclaim_only_unused_references() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = imported(&bus).await;
    let old = secret_variable(&bus, &workspace.id).await;
    let mut bad = input(&old, "bulk-failure");
    bad.id = Some("missing-variable".into());
    bad.key = "other".into();
    let before = entries(&bus, &workspace.id).await;
    assert!(bus
        .workspace_variables_replace(
            workspace.id.clone(),
            vec![input(&old, "failed-batch-secret"), bad]
        )
        .await
        .is_err());
    assert_eq!(entries(&bus, &workspace.id).await, before);
    assert_eq!(
        read(&bus, &workspace.id, &old.value).await.unwrap(),
        "original-secret"
    );
    let changed = bus
        .workspace_variable_update(
            workspace.id.clone(),
            old.id.clone(),
            input(&old, "rotated-secret"),
        )
        .await
        .unwrap();
    assert!(changed.is_secret); // An imported handle cannot be downgraded to plaintext.
    assert_ne!(old.value, changed.value);
    assert!(read(&bus, &workspace.id, &old.value).await.is_err());
    assert_eq!(
        read(&bus, &workspace.id, &changed.value).await.unwrap(),
        "rotated-secret"
    );
    let cleared = bus
        .workspace_variable_update(workspace.id.clone(), old.id.clone(), input(&changed, ""))
        .await
        .unwrap();
    assert_eq!(cleared.value, "");
    assert!(cleared.is_secret);
    assert!(read(&bus, &workspace.id, &changed.value).await.is_err());
    let repopulated = bus
        .workspace_variable_update(
            workspace.id.clone(),
            old.id.clone(),
            input(&cleared, "repopulated-secret"),
        )
        .await
        .unwrap();
    assert_eq!(
        read(&bus, &workspace.id, &repopulated.value).await.unwrap(),
        "repopulated-secret"
    );
    bus.workspace_variable_delete(workspace.id.clone(), old.id.clone())
        .await
        .unwrap();
    assert!(read(&bus, &workspace.id, &repopulated.value).await.is_err());
    // Replacing a collection with an empty list also reclaims attached handles.
    let environment = bus
        .workspace_environments_list(workspace.id.clone())
        .await
        .unwrap()
        .remove(0);
    let reference = environment.variables[0].value.clone();
    bus.workspace_environment_variables_replace(workspace.id.clone(), environment.id, vec![])
        .await
        .unwrap();
    assert!(read(&bus, &workspace.id, &reference).await.is_err());
}

#[tokio::test]
async fn environment_rotation_failure_metadata_and_cascade_delete_are_safe() {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let workspace = imported(&bus).await;
    let env = bus
        .workspace_environments_list(workspace.id.clone())
        .await
        .unwrap()
        .remove(0);
    let old = &env.variables[0];
    let make = |value: &str| WorkspaceVariableInput {
        id: Some(old.id.clone()),
        key: old.key.clone(),
        value: value.into(),
        is_secret: true,
        is_enabled: true,
        description: Some("metadata edit".into()),
        sort_order: 0,
    };
    let before = entries(&bus, &workspace.id).await;
    // Invalid environment metadata fails after staging the first replacement.
    assert!(bus
        .workspace_environment_update(
            workspace.id.clone(),
            env.id.clone(),
            "".into(),
            vec![make("failed-env-secret")]
        )
        .await
        .is_err());
    assert_eq!(entries(&bus, &workspace.id).await, before);
    assert_eq!(
        read(&bus, &workspace.id, &old.value).await.unwrap(),
        "environment-secret"
    );
    let updated = bus
        .workspace_environment_variable_update(
            workspace.id.clone(),
            env.id.clone(),
            old.id.clone(),
            make("new-env-secret"),
        )
        .await
        .unwrap();
    assert!(read(&bus, &workspace.id, &old.value).await.is_err());
    assert_eq!(
        read(&bus, &workspace.id, &updated.value).await.unwrap(),
        "new-env-secret"
    );
    bus.extensions = CommandBusExtensions::new(vec![Arc::new(tests::FailCommit)]);
    assert!(bus
        .workspace_environment_delete(workspace.id.clone(), env.id.clone())
        .await
        .is_err());
    assert_eq!(
        read(&bus, &workspace.id, &updated.value).await.unwrap(),
        "new-env-secret"
    );
    bus.extensions = CommandBusExtensions::default();
    bus.workspace_environment_delete(workspace.id.clone(), env.id)
        .await
        .unwrap();
    assert!(read(&bus, &workspace.id, &updated.value).await.is_err());
    let remaining = entries(&bus, &workspace.id).await;
    bus.delete_workspace(workspace.id.clone()).await.unwrap();
    for reference in remaining {
        assert!(read(&bus, &workspace.id, &reference).await.is_err());
    }
    assert!(entries(&bus, &workspace.id).await.is_empty());
}

#[tokio::test]
async fn repeated_imports_and_shared_api_references_have_independent_lifetimes() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let content = backup(&bus).await;
    let first = bus
        .workspace_bundle_import_with_options(content.clone(), "Copy".into(), options())
        .await
        .unwrap();
    let second = bus
        .workspace_bundle_import_with_options(content, "Copy".into(), options())
        .await
        .unwrap();
    let old = secret_variable(&bus, &first.id).await;
    let independent = secret_variable(&bus, &second.id).await;
    assert_ne!(old.value, independent.value);
    let request: String = sqlx::query_scalar("SELECT id FROM api_requests WHERE workspace_id=?")
        .bind(&first.id)
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    // Reuse a variable handle in a request: deleting its original variable
    // must retain it until the last runtime reference disappears.
    let token = format!("{{{{ {} }}}}", old.value);
    sqlx::query("UPDATE api_requests SET auth_json=? WHERE id=?")
        .bind(json!({"type":"bearer","token":token}).to_string())
        .bind(&request)
        .execute(bus.db.pool())
        .await
        .unwrap();
    bus.workspace_variable_delete(first.id.clone(), old.id)
        .await
        .unwrap();
    assert_eq!(
        read(&bus, &first.id, &old.value).await.unwrap(),
        "original-secret"
    );
    bus.delete_api_request(first.id.clone(), request)
        .await
        .unwrap();
    assert!(read(&bus, &first.id, &old.value).await.is_err());
    assert_eq!(
        read(&bus, &second.id, &independent.value).await.unwrap(),
        "original-secret"
    );
}

#[tokio::test]
async fn interrupted_cleanup_is_durable_and_legacy_live_handles_are_adopted() {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let workspace = imported(&bus).await;
    let old = secret_variable(&bus, &workspace.id).await;
    let store = bus.secret_store.clone();
    // Simulate a keychain provider that cannot delete this service's handles.
    bus.secret_store = unfour_secret_store::SecretStore::in_memory("unavailable-provider");
    bus.workspace_variable_delete(workspace.id.clone(), old.id)
        .await
        .unwrap();
    assert!(entries(&bus, &workspace.id)
        .await
        .iter()
        .any(|r| old.value.ends_with(r)));
    bus.secret_store = store;
    bus.recover_bundle_credentials().await.unwrap();
    assert!(read(&bus, &workspace.id, &old.value).await.is_err());
    let env = bus
        .workspace_environments_list(workspace.id.clone())
        .await
        .unwrap()
        .remove(0);
    let reference = env.variables[0].value.clone();
    sqlx::query("DELETE FROM workspace_bundle_credential_journal WHERE workspace_id=?")
        .bind(&workspace.id)
        .execute(bus.db.pool())
        .await
        .unwrap();
    bus.recover_bundle_credentials().await.unwrap();
    assert_eq!(
        read(&bus, &workspace.id, &reference).await.unwrap(),
        "environment-secret"
    );
    bus.workspace_environment_delete(workspace.id.clone(), env.id)
        .await
        .unwrap();
    assert!(read(&bus, &workspace.id, &reference).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn encrypted_backup_is_one_snapshot_during_concurrent_edits() {
    let path = std::env::temp_dir().join(format!(
        "unfour-bundle-snapshot-{}.sqlite",
        unfour_core::id::new_id()
    ));
    let db = unfour_local_storage::LocalDb::connect_path(&path)
        .await
        .unwrap();
    db.migrate().await.unwrap();
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace = bus
        .workspace_bundle_import(tests::fixture().to_string(), "Epoch 0".into())
        .await
        .unwrap();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let writer_stop = stop.clone();
    let writer_db = db.clone();
    let writer_store = bus.secret_store.clone();
    let id = workspace.id.clone();
    let (ready, started) = tokio::sync::oneshot::channel();
    let writer = tokio::spawn(async move {
        let mut ready = Some(ready);
        let mut epoch = 0;
        let mut previous_reference = None;
        while !writer_stop.load(std::sync::atomic::Ordering::SeqCst) {
            epoch += 1;
            let mut tx = writer_db
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .unwrap();
            let reference =
                writer_store.make_ref(&id, "workspace-variable", &unfour_core::id::new_id());
            writer_store
                .rotate_credential(id.clone(), reference.clone(), format!("secret-{epoch}"))
                .await
                .unwrap();
            sqlx::query("UPDATE workspaces SET name=? WHERE id=?")
                .bind(format!("Epoch {epoch}"))
                .bind(&id)
                .execute(&mut *tx)
                .await
                .unwrap();
            sqlx::query("UPDATE workspace_variables SET value=CASE WHEN is_secret=1 THEN ? ELSE ? END WHERE workspace_id=?")
                .bind(format!("@unfour-secret:{reference}")).bind(format!("https://epoch.test/{epoch}")).bind(&id).execute(&mut *tx).await.unwrap();
            sqlx::query("UPDATE api_requests SET auth_json=?,url=? WHERE workspace_id=?")
                .bind(json!({"type":"bearer","token":format!("{{{{epoch_{epoch}}}}}")}).to_string())
                .bind(format!("https://epoch.test?token=url-{epoch}"))
                .bind(&id)
                .execute(&mut *tx)
                .await
                .unwrap();
            sqlx::query("UPDATE ssh_task_step SET config_json=? WHERE workspace_id=? AND step_type='upload'")
                .bind(json!({"localPath":format!("C:/epoch/{epoch}"),"remotePath":"/tmp/file","overwrite":false}).to_string())
                .bind(&id).execute(&mut *tx).await.unwrap();
            // Capture must finish reading the old handle before a cooperative
            // writer can reclaim it, not merely retain a consistent SQL view.
            if let Some(previous) = previous_reference.replace(reference) {
                writer_store
                    .delete_credential(id.clone(), previous)
                    .await
                    .unwrap();
            }
            tx.commit().await.unwrap();
            if let Some(ready) = ready.take() {
                ready.send(()).unwrap();
            }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        epoch
    });
    started.await.unwrap();
    for _ in 0..4 {
        let artifact = bus
            .workspace_bundle_export_with_options(workspace.id.clone(), options())
            .await
            .unwrap();
        let plaintext = unfour_secret_store::backup::decrypt(&artifact.content, PASSWORD).unwrap();
        let payload: Value = serde_json::from_slice(&plaintext).unwrap();
        let epoch = payload["bundle"]["workspace"]["name"]
            .as_str()
            .unwrap()
            .strip_prefix("Epoch ")
            .unwrap();
        assert!(payload["bundle"]["variables"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["value"]["value"] == format!("https://epoch.test/{epoch}")));
        assert!(payload["bundle"]["apiTemplates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["value"] == format!("{{{{epoch_{epoch}}}}}")));
        assert!(payload["bundle"]["localPaths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["path"] == format!("C:/epoch/{epoch}")));
        assert!(payload["secrets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["kind"] == "workspace-variable" && s["value"] == format!("secret-{epoch}")));
        assert!(payload["secrets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["field"] == "url"
                && s["value"] == format!("https://epoch.test?token=url-{epoch}")));
    }
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(writer.await.unwrap() > 1);
    db.pool().close().await;
    std::fs::remove_file(path).unwrap();
}
