use super::*;
use std::sync::Arc;
const PASSWORD: &str = "test-only-backup-password";

#[tokio::test]
async fn ordinary_urls_do_not_become_credentials_due_to_url_normalization() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = source(&bus).await;
    sqlx::query("UPDATE api_requests SET url=?,auth_json=?,headers_json=? WHERE workspace_id=?")
        .bind("https://example.test")
        .bind(r#"{"type":"none"}"#)
        .bind("[]")
        .bind(&workspace.id)
        .execute(bus.db.pool())
        .await
        .unwrap();
    let artifact = bus
        .workspace_bundle_export_with_options(workspace.id, options())
        .await
        .unwrap();
    let preview = bus
        .workspace_bundle_preview_with_options(&artifact.content, options())
        .await
        .unwrap();
    assert_eq!(preview.counts["credentials"], 2);
}

#[tokio::test]
async fn sharing_preserves_variable_templates_without_keychain_or_cloud_secrets() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let source = source(&bus).await;
    sqlx::query("UPDATE api_requests SET auth_json=?, headers_json=? WHERE workspace_id=?")
        .bind(r#"{"type":"bearer","token":"{{access_token}}"}"#)
        .bind(r#"[{"key":"Authorization","value":"Bearer {{access_token}}","enabled":true}]"#)
        .bind(&source.id)
        .execute(bus.db.pool())
        .await
        .unwrap();
    let artifact = bus
        .workspace_bundle_export_with_options(source.id, WorkspaceBundleOptions::default())
        .await
        .unwrap();
    assert!(!artifact.content.contains("canary"));
    let preview = bus
        .workspace_bundle_preview_with_options(&artifact.content, WorkspaceBundleOptions::default())
        .await
        .unwrap();
    assert!(!preview
        .reconfigure
        .iter()
        .any(|i| i.field == "auth" || i.field == "headers"));
    let copy = bus
        .workspace_bundle_import_with_options(
            artifact.content,
            "Templates".into(),
            WorkspaceBundleOptions::default(),
        )
        .await
        .unwrap();
    let auth: String =
        sqlx::query_scalar("SELECT auth_json FROM api_requests WHERE workspace_id=?")
            .bind(&copy.id)
            .fetch_one(bus.db.pool())
            .await
            .unwrap();
    assert!(auth.contains("{{access_token}}"));
    // The existing cloud/domain snapshot still redacts authentication fields.
    let canonical = bus.workspace_bundle_export(copy.id).await.unwrap();
    assert!(!canonical.contains("Bearer {{access_token}}"));
}

#[test]
fn prefix_mapping_honors_boundaries_and_cross_platform_separators() {
    let mut bundle = parse(&tests::fixture().to_string()).unwrap();
    bundle.local_paths = vec![LocalPath {
        entity_id: "upload".into(),
        field: "localPath".into(),
        path: r"C:\Old\dir\file.txt".into(),
    }];
    let mappings = vec![local::PathMapping {
        from: "c:/old".into(),
        to: "/Users/local".into(),
    }];
    local::map_paths(&mut bundle, &mappings).unwrap();
    assert_eq!(bundle.local_paths[0].path, "/Users/local/dir/file.txt");
    bundle.local_paths[0].path = "C:/older/file.txt".into();
    local::map_paths(&mut bundle, &mappings).unwrap();
    assert_eq!(bundle.local_paths[0].path, "C:/older/file.txt");
    bundle.local_paths[0].path = "/old/file.txt".into();
    local::map_paths(
        &mut bundle,
        &[local::PathMapping {
            from: "/".into(),
            to: "D:/".into(),
        }],
    )
    .unwrap();
    assert_eq!(bundle.local_paths[0].path, "D:/old/file.txt");
}

#[tokio::test]
async fn startup_recovery_removes_interrupted_staged_credentials() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = "interrupted-import";
    let reference = bus
        .secret_store
        .make_ref(workspace, "ssh-password", "interrupted");
    bus.workspace
        .journal_bundle_credential(workspace, &reference)
        .await
        .unwrap();
    bus.secret_store
        .rotate_credential(
            workspace.into(),
            reference.clone(),
            "recovery-canary".into(),
        )
        .await
        .unwrap();
    bus.recover_bundle_credentials().await.unwrap();
    assert!(bus
        .secret_store
        .read_secret(workspace.into(), reference)
        .await
        .is_err());
}
fn options() -> WorkspaceBundleOptions {
    WorkspaceBundleOptions {
        password: Some(PASSWORD.into()),
        include_secrets: true,
        keep_local_paths: true,
        ..Default::default()
    }
}
async fn source(bus: &CommandBus) -> Workspace {
    let workspace = bus
        .workspace_bundle_import(tests::fixture().to_string(), "Source".into())
        .await
        .unwrap();
    let credential = bus
        .secret_store
        .create_credential(
            workspace.id.clone(),
            "ssh-password".into(),
            "Backup test".into(),
            "backup-password-canary".into(),
        )
        .await
        .unwrap();
    sqlx::query(
        "UPDATE connections SET credential_ref=? WHERE workspace_id=? AND connection_type='ssh'",
    )
    .bind(credential.credential_ref)
    .bind(&workspace.id)
    .execute(bus.db.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE workspace_variables SET value=? WHERE workspace_id=? AND is_secret=1")
        .bind("backup-variable-canary")
        .bind(&workspace.id)
        .execute(bus.db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE api_requests SET auth_json=?, headers_json=?, url=? WHERE workspace_id=?")
        .bind(r#"{"type":"bearer","token":"api-canary\"quoted\\value"}"#)
        .bind(r#"[{"key":"Authorization","value":"header-canary","enabled":true}]"#)
        .bind("https://example.test?token=url-canary")
        .bind(&workspace.id)
        .execute(bus.db.pool())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE ssh_task_step SET config_json=? WHERE workspace_id=? AND step_type='upload'",
    )
    .bind(r#"{"localPath":"C:/old/file.txt","remotePath":"/tmp/file","overwrite":false}"#)
    .bind(&workspace.id)
    .execute(bus.db.pool())
    .await
    .unwrap();
    workspace
}

#[tokio::test]
async fn encrypted_round_trip_restores_local_refs_and_resolves_quoted_api_secrets() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let source = source(&bus).await;
    let artifact = bus
        .workspace_bundle_export_with_options(source.id.clone(), options())
        .await
        .unwrap();
    for canary in ["canary", "Source", "C:/old"] {
        assert!(!artifact.content.contains(canary));
    }
    let preview = bus
        .workspace_bundle_preview_with_options(&artifact.content, options())
        .await
        .unwrap();
    assert_eq!(preview.counts["credentials"], 5);
    assert!(preview
        .reconfigure
        .iter()
        .all(|i| !matches!(i.code.as_str(), "secret" | "connection" | "redacted")));
    assert!(preview
        .reconfigure
        .iter()
        .any(|i| i.status == "unchecked" && i.field == "localPath"));
    let copy = bus
        .workspace_bundle_import_with_options(artifact.content.clone(), "Copy".into(), options())
        .await
        .unwrap();
    let reference: String = sqlx::query_scalar(
        "SELECT credential_ref FROM connections WHERE workspace_id=? AND connection_type='ssh'",
    )
    .bind(&copy.id)
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert!(reference.contains(&copy.id));
    assert_eq!(
        bus.secret_store
            .read_secret(copy.id.clone(), reference.clone())
            .await
            .unwrap(),
        "backup-password-canary"
    );
    assert!(bus
        .secret_store
        .read_secret(source.id, reference)
        .await
        .is_err());
    let vars = bus.workspace.list_variables(copy.id.clone()).await.unwrap();
    assert!(vars
        .iter()
        .find(|v| v.is_secret)
        .unwrap()
        .value
        .starts_with("@unfour-secret:"));
    assert_eq!(
        bus.workspace
            .resolve_variables(&copy.id, None, "{{access_token}}")
            .await
            .unwrap(),
        "backup-variable-canary"
    );
    let mut db = bus.db.pool().acquire().await.unwrap();
    let requests = bus
        .api_client
        .bundle_requests_on(&mut db, &copy.id)
        .await
        .unwrap();
    drop(db);
    let auth = bus
        .workspace
        .resolve_json_variables(&copy.id, None, &requests[0].auth_json, &[])
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&auth).unwrap()["token"],
        "api-canary\"quoted\\value"
    );
    assert!(!requests[0].auth_json.contains("api-canary"));
    let variable = vars.iter().find(|v| v.is_secret).unwrap();
    let updated = bus
        .workspace_variable_update(
            copy.id.clone(),
            variable.id.clone(),
            WorkspaceVariableInput {
                id: Some(variable.id.clone()),
                key: variable.key.clone(),
                value: "edited-variable-canary".into(),
                is_secret: true,
                is_enabled: true,
                description: None,
                sort_order: variable.sort_order,
            },
        )
        .await
        .unwrap();
    assert!(updated.value.starts_with("@unfour-secret:"));
    assert_ne!(updated.value, variable.value);
    assert_eq!(
        bus.workspace
            .resolve_variables(&copy.id, None, "{{access_token}}")
            .await
            .unwrap(),
        "edited-variable-canary"
    );
    // Re-exporting an imported backup must dereference the new local handles too.
    let again = bus
        .workspace_bundle_export_with_options(copy.id.clone(), options())
        .await
        .unwrap();
    let second_preview = bus
        .workspace_bundle_preview_with_options(&again.content, options())
        .await
        .unwrap();
    assert_eq!(second_preview.counts["credentials"], 5);
    let share = bus
        .workspace_bundle_export_with_options(copy.id, WorkspaceBundleOptions::default())
        .await
        .unwrap();
    assert!(!share.content.contains("canary"));
    assert!(!share.content.contains("@unfour-secret:"));
    assert!(!share.content.contains("C:/old"));
    let journal: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_bundle_credential_journal WHERE state!='attached'",
    )
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(journal, 0);
}

#[tokio::test]
async fn wrong_password_invalid_content_and_commit_failure_leave_no_workspace_or_journal() {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let source = source(&bus).await;
    let artifact = bus
        .workspace_bundle_export_with_options(source.id, options())
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    let mut wrong = options();
    wrong.password = Some("wrong password".into());
    assert!(bus
        .workspace_bundle_import_with_options(artifact.content.clone(), "Wrong".into(), wrong)
        .await
        .is_err());
    bus.extensions = crate::CommandBusExtensions::new(vec![Arc::new(tests::FailCommit)]);
    assert!(bus
        .workspace_bundle_import_with_options(artifact.content, "Rollback".into(), options())
        .await
        .is_err());
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    let journal: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_bundle_credential_journal WHERE state!='attached'",
    )
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(before, after);
    assert_eq!(journal, 0);
}

#[tokio::test]
async fn paths_remap_without_filesystem_access_and_enabled_steps_are_preserved() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let source = source(&bus).await;
    let export = bus
        .workspace_bundle_export_with_options(
            source.id,
            WorkspaceBundleOptions {
                keep_local_paths: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let options: WorkspaceBundleOptions =
        serde_json::from_value(json!({"pathMappings":[{"from":"C:/old","to":"D:/new"}]})).unwrap();
    let preview = bus
        .workspace_bundle_preview_with_options(&export.content, options.clone())
        .await
        .unwrap();
    assert_eq!(preview.paths[0].path, "D:/new/file.txt");
    let copy = bus
        .workspace_bundle_import_with_options(export.content, "Mapped".into(), options)
        .await
        .unwrap();
    let (config, enabled): (String, bool) = sqlx::query_as("SELECT config_json, enabled FROM ssh_task_step WHERE workspace_id=? AND step_type='upload'")
        .bind(&copy.id).fetch_one(bus.db.pool()).await.unwrap();
    assert!(enabled);
    assert!(config.contains("D:/new/file.txt"));
    let cloud_safe = bus.workspace_bundle_export(copy.id).await.unwrap();
    assert!(!cloud_safe.contains("D:/new"));
}
