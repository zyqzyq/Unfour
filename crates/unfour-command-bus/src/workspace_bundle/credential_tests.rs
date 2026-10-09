use super::*;
use crate::CommandBusExtensions;
use std::sync::Arc;

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
async fn entries(bus: &CommandBus, workspace: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT credential_ref FROM workspace_bundle_credential_journal WHERE workspace_id=? ORDER BY credential_ref")
        .bind(workspace).fetch_all(bus.db.pool()).await.unwrap()
}

#[tokio::test]
async fn sqlite_secret_edits_clear_flags_and_rollback_keep_normal_semantics() {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let workspace = imported(&bus).await;
    let old = secret_variable(&bus, &workspace.id).await;
    assert_eq!(old.value, "original-secret");
    assert!(entries(&bus, &workspace.id).await.is_empty());
    bus.extensions = CommandBusExtensions::new(vec![Arc::new(tests::FailCommit)]);
    assert!(bus
        .workspace_variable_update(
            workspace.id.clone(),
            old.id.clone(),
            input(&old, "failed-replacement")
        )
        .await
        .is_err());
    assert_eq!(secret_variable(&bus, &workspace.id).await.value, old.value);
    bus.extensions = CommandBusExtensions::default();
    let mut change = input(&old, "replacement-secret");
    change.is_secret = true;
    let changed = bus
        .workspace_variable_update(workspace.id.clone(), old.id.clone(), change)
        .await
        .unwrap();
    assert_eq!(changed.value, "replacement-secret");
    assert!(changed.is_secret);
    let cleared = bus
        .workspace_variable_update(workspace.id.clone(), old.id.clone(), input(&changed, ""))
        .await
        .unwrap();
    assert!(cleared.value.is_empty());
    assert!(!cleared.is_secret); // Flags remain editable, as before V2.
    assert!(entries(&bus, &workspace.id).await.is_empty());
    let env = bus
        .workspace_environments_list(workspace.id.clone())
        .await
        .unwrap()
        .remove(0);
    assert_eq!(env.variables[0].value, "environment-secret");
    let mut var = input(&old, "new-env-secret");
    var.id = Some(env.variables[0].id.clone());
    var.key = env.variables[0].key.clone();
    var.is_secret = true;
    assert!(bus
        .workspace_environment_update(
            workspace.id.clone(),
            env.id.clone(),
            "".into(),
            vec![var.clone()]
        )
        .await
        .is_err());
    assert_eq!(
        bus.workspace_environments_list(workspace.id.clone())
            .await
            .unwrap()[0]
            .variables[0]
            .value,
        "environment-secret"
    );
    let updated = bus
        .workspace_environment_variable_update(
            workspace.id.clone(),
            env.id,
            var.id.clone().unwrap(),
            var,
        )
        .await
        .unwrap();
    assert_eq!(updated.value, "new-env-secret");
    assert!(updated.is_secret);
}

#[tokio::test]
async fn repeated_imports_keep_sqlite_values_and_have_independent_records() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let content = backup(&bus).await;
    let first = bus
        .workspace_bundle_import_with_options(content.clone(), "First".into(), options())
        .await
        .unwrap();
    let second = bus
        .workspace_bundle_import_with_options(content, "Second".into(), options())
        .await
        .unwrap();
    let a = secret_variable(&bus, &first.id).await;
    let b = secret_variable(&bus, &second.id).await;
    assert_ne!(a.id, b.id);
    assert_eq!(a.value, b.value);
    bus.workspace_variable_delete(first.id, a.id).await.unwrap();
    assert_eq!(
        bus.workspace
            .resolve_variables(&second.id, None, "{{access_token}}")
            .await
            .unwrap(),
        "original-secret"
    );
    assert!(entries(&bus, &second.id).await.is_empty());
}

// Spawned by the process-concurrency regression below. The child has its own
// pool and CommandBus, just like desktop startup alongside an MCP writer.
#[tokio::test]
async fn recovery_child() {
    let Ok(path) = std::env::var("UNFOUR_TEST_RECOVERY_DB") else {
        return;
    };
    let signal = std::env::var("UNFOUR_TEST_RECOVERY_SIGNAL").unwrap();
    let db = unfour_local_storage::LocalDb::connect_existing_path(path)
        .await
        .unwrap();
    std::fs::write(format!("{signal}.ready"), "ready").unwrap();
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let staged: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_bundle_credential_journal WHERE state='staged'",
    )
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(staged, 0);
    std::fs::write(format!("{signal}.done"), "done").unwrap();
    db.pool().close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn primary_recovery_waits_for_a_satellite_staging_writer_in_another_process() {
    let dir = std::env::temp_dir().join(format!(
        "unfour-credential-race-{}",
        unfour_core::id::new_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.sqlite");
    let signal = dir.join("recovery");
    let db = unfour_local_storage::LocalDb::connect_path(&path)
        .await
        .unwrap();
    db.migrate().await.unwrap();
    let store = unfour_secret_store::SecretStore::in_memory("unfour-test");
    let bus = CommandBus::from_db_with_secret_store(db.clone(), store.clone())
        .await
        .unwrap();
    let workspace = bus
        .workspace_bundle_import(tests::fixture().to_string(), "MCP writer".into())
        .await
        .unwrap();
    let guard = bus.credential_stage_guard().await.unwrap();
    let reference = bus
        .stage_connection_credential(&workspace.id, None, "ssh-password", "live-mcp-password")
        .await
        .unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "workspace_bundle::credential_tests::recovery_child",
            "--nocapture",
        ])
        .env("UNFOUR_TEST_RECOVERY_DB", &path)
        .env("UNFOUR_TEST_RECOVERY_SIGNAL", &signal)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let ready = signal.with_extension("ready");
    for _ in 0..200 {
        if ready.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(ready.exists(), "child did not start");
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert!(child.try_wait().unwrap().is_none());
    assert!(!signal.with_extension("done").exists());
    assert_eq!(entries(&bus, &workspace.id).await, vec![reference.clone()]);
    let mut tx = db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query(
        "UPDATE connections SET credential_ref=? WHERE workspace_id=? AND connection_type='ssh'",
    )
    .bind(&reference)
    .bind(&workspace.id)
    .execute(&mut *tx)
    .await
    .unwrap();
    bus.workspace
        .attach_bundle_credentials_on(&mut tx, &workspace.id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(guard);
    for _ in 0..200 {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
        panic!("recovery did not complete");
    }
    assert!(child.wait().unwrap().success());
    assert!(signal.with_extension("done").exists());
    assert_eq!(
        store
            .read_secret(workspace.id.clone(), reference.clone())
            .await
            .unwrap(),
        "live-mcp-password"
    );
    assert_eq!(entries(&bus, &workspace.id).await, vec![reference]);
    db.pool().close().await;
    std::fs::remove_dir_all(&dir).unwrap();
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
            let reference = writer_store.make_ref(&id, "ssh-password", &unfour_core::id::new_id());
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
                .bind(format!("secret-{epoch}")).bind(format!("https://epoch.test/{epoch}")).bind(&id).execute(&mut *tx).await.unwrap();
            sqlx::query("UPDATE connections SET credential_ref=? WHERE workspace_id=? AND connection_type='ssh'")
                .bind(&reference).bind(&id).execute(&mut *tx).await.unwrap();
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
