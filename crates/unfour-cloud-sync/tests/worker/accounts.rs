//! Account isolation, generation fencing and pause preferences.

use super::support::*;

#[tokio::test]
async fn account_switch_keeps_bindings_isolated_and_pauses_old_worker_context() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    service.pause_current_account().await.unwrap();
    transport.switch_account("account-b");
    assert!(service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .is_none());
    let create_calls_before_b = transport.create_workspace_calls.load(Ordering::SeqCst);
    assert!(matches!(
        service.enable(&workspace_id).await,
        Err(SyncError::WorkspaceOwnedByAnotherAccount)
    ));
    assert_eq!(
        transport.create_workspace_calls.load(Ordering::SeqCst),
        create_calls_before_b
    );
    let bindings: Vec<(String, bool)> = sqlx::query_as(
        "SELECT account_id, sync_enabled FROM cloud_sync_workspace_bindings ORDER BY account_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(bindings, vec![("account-a".into(), false)]);
    let owner: (String, String) = sqlx::query_as(
        "SELECT account_id, cloud_workspace_id FROM cloud_sync_workspace_ownership WHERE local_workspace_id = ?1",
    )
    .bind(workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(owner.0, "account-a");
}

#[tokio::test]
async fn active_non_owner_cannot_receive_mutations_or_worker_pushes() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, mut receiver) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    sqlx::query("DELETE FROM cloud_sync_outbox")
        .execute(db.pool())
        .await
        .unwrap();
    insert_binding(&db, "account-b", &workspace_id, "cloud-b", true, "active").await;

    transport.switch_account("account-b");
    service.activate_account_context().await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let created = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "NON_OWNER", "must-stay-with-a", false),
        )
        .await
        .unwrap();

    let destinations: Vec<(String, String)> = sqlx::query_as(
        "SELECT account_id, cloud_workspace_id FROM cloud_sync_outbox WHERE entity_id = ?1",
    )
    .bind(&created.id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        destinations,
        vec![("account-a".into(), "cloud-created".into())]
    );
    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));

    let pushes_before = transport.pushes.lock().unwrap().len();
    service.sync_all().await.unwrap();
    assert_eq!(transport.pushes.lock().unwrap().len(), pushes_before);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::WorkspaceOwnedByAnotherAccount)
    ));
    assert_eq!(transport.pushes.lock().unwrap().len(), pushes_before);
}

#[tokio::test]
async fn signed_out_owner_outbox_coalesces_repeated_updates() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, mut receiver) = SyncRuntime::build(db.clone(), transport);
    service.enable(&workspace_id).await.unwrap();
    sqlx::query("DELETE FROM cloud_sync_outbox")
        .execute(db.pool())
        .await
        .unwrap();
    service.pause_current_account().await.unwrap();

    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let created = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "COALESCED", "value-0", false),
        )
        .await
        .unwrap();
    for index in 1..100 {
        bus.workspace_variable_update(
            workspace_id.clone(),
            created.id.clone(),
            variable(
                Some(created.id.clone()),
                "COALESCED",
                &format!("value-{index}"),
                false,
            ),
        )
        .await
        .unwrap();
    }

    let outbox: (i64, i64, String, String) = sqlx::query_as(
        r#"SELECT COUNT(*), MAX(content_revision), canonical_payload_json, status
           FROM cloud_sync_outbox
           WHERE account_id = 'account-a' AND local_workspace_id = ?1
             AND entity_id = ?2"#,
    )
    .bind(&workspace_id)
    .bind(&created.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let local_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM workspace_variables WHERE workspace_id = ?1 AND id = ?2",
    )
    .bind(&workspace_id)
    .bind(&created.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(outbox.0, 1);
    assert_eq!(outbox.1, local_revision);
    assert!(outbox.2.contains("value-99"));
    assert_eq!(outbox.3, "pending");
    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn stale_generation_error_cannot_overwrite_a_newer_success() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    service.enable(&workspace_id).await.unwrap();
    let old_binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();

    sqlx::query(
        "UPDATE cloud_sync_workspace_bindings SET generation = generation + 1, state = 'active', last_error = NULL WHERE account_id = ?1 AND local_workspace_id = ?2",
    )
    .bind(&old_binding.account_id)
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    service
        .repository()
        .record_error(
            &old_binding.account_id,
            &workspace_id,
            old_binding.generation.try_into().unwrap(),
            SyncError::Transport.code(),
            Utc::now(),
        )
        .await
        .unwrap();

    let state: (String, Option<String>) = sqlx::query_as(
        "SELECT state, last_error FROM cloud_sync_workspace_bindings WHERE account_id = ?1 AND local_workspace_id = ?2",
    )
    .bind(&old_binding.account_id)
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, ("active".into(), None));
}

#[tokio::test]
async fn sign_out_pause_generation_rejects_an_old_in_flight_result() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "IN_FLIGHT", "preserved", false),
    )
    .await
    .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    *transport.push_barrier.lock().unwrap() = Some(barrier.clone());
    let task = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };
    tokio::time::timeout(Duration::from_secs(5), barrier.wait())
        .await
        .expect("worker reached push before sign-out");
    service.pause_current_account().await.unwrap();
    barrier.wait().await;
    assert!(matches!(
        task.await.unwrap(),
        Err(SyncError::AccountChanged)
    ));
    let preserved: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox WHERE account_id = 'account-a'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(preserved, 1);
    let state: (bool, String) = sqlx::query_as(
        "SELECT sync_enabled, state FROM cloud_sync_workspace_bindings WHERE account_id = 'account-a'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, (false, "paused".into()));
}

#[tokio::test]
async fn legacy_pause_stays_paused_after_same_account_reauthentication() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    service.enable(&workspace_id).await.unwrap();
    service.pause_current_account().await.unwrap();

    // Reproduce a pre-pause-reason database row upgraded by the conservative
    // legacy migration: it is paused, signed out, and has no reason marker.
    sqlx::query(
        "DELETE FROM cloud_sync_account_binding_pause_reasons WHERE account_id = 'account-a' AND local_workspace_id = ?1",
    )
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE cloud_sync_workspace_bindings SET last_error = 'cloud_sync_legacy_paused_binding_ambiguous' WHERE account_id = 'account-a' AND local_workspace_id = ?1",
    )
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();

    service.activate_account_context().await.unwrap();
    let state: (bool, String, Option<String>) = sqlx::query_as(
        "SELECT sync_enabled, state, last_error FROM cloud_sync_workspace_bindings WHERE account_id = 'account-a' AND local_workspace_id = ?1",
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        state,
        (
            false,
            "paused".into(),
            Some("cloud_sync_legacy_paused_binding_ambiguous".into())
        )
    );

    service.enable(&workspace_id).await.unwrap();
    let enabled: bool = sqlx::query_scalar(
        "SELECT sync_enabled FROM cloud_sync_workspace_bindings WHERE account_id = 'account-a' AND local_workspace_id = ?1",
    )
    .bind(workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(enabled);
}

#[tokio::test]
async fn explicit_workspace_pause_survives_same_account_reauthentication() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    service.enable(&workspace_id).await.unwrap();
    service.disable(&workspace_id).await.unwrap();

    service.activate_account_context().await.unwrap();
    let state: (bool, String) = sqlx::query_as(
        "SELECT sync_enabled, state FROM cloud_sync_workspace_bindings WHERE account_id = 'account-a' AND local_workspace_id = ?1",
    )
    .bind(workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, (false, "paused".into()));
}

#[tokio::test]
async fn sign_out_mutation_keeps_owned_outbox_and_only_owner_can_resume_it() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let environment = seed
        .workspace_environment_create(workspace_id.clone(), "Test".into())
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, hook, mut receiver) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    sqlx::query("DELETE FROM cloud_sync_outbox")
        .execute(db.pool())
        .await
        .unwrap();

    service.pause_current_account().await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let variable = bus
        .workspace_environment_variable_create(
            workspace_id.clone(),
            environment.id,
            variable(None, "AFTER_SIGN_OUT", "preserved", false),
        )
        .await
        .unwrap();

    let stored: (String, String, String) = sqlx::query_as(
        r#"SELECT account_id, entity_type, operation
           FROM cloud_sync_outbox WHERE entity_id = ?1"#,
    )
    .bind(&variable.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        stored,
        (
            "account-a".into(),
            "workspaceEnvironmentVariable".into(),
            "upsert".into()
        )
    );
    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));

    let pushes_before_b = transport.pushes.lock().unwrap().len();
    transport.switch_account("account-b");
    service.activate_account_context().await.unwrap();
    service.sync_all().await.unwrap();
    assert_eq!(transport.pushes.lock().unwrap().len(), pushes_before_b);
    let owner: String =
        sqlx::query_scalar("SELECT account_id FROM cloud_sync_outbox WHERE entity_id = ?1")
            .bind(&variable.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(owner, "account-a");

    transport.switch_account("account-a");
    service.activate_account_context().await.unwrap();
    service.sync_all().await.unwrap();
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_outbox WHERE account_id = 'account-a' AND entity_id = ?1",
    )
    .bind(variable.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn global_pause_preserves_workspace_preferences_and_resumes_them() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();

    service.set_global_sync_enabled(false).await.unwrap();
    assert!(!service.global_sync_enabled().await.unwrap());
    let paused = service.status(&workspace_id).await.unwrap();
    assert!(paused.binding.as_ref().unwrap().sync_enabled);

    let calls_before = transport.changes_calls.load(Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    assert_eq!(transport.changes_calls.load(Ordering::SeqCst), calls_before);

    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());
    service.set_global_sync_enabled(true).await.unwrap();
    assert!(service.global_sync_enabled().await.unwrap());
    let resumed = service.status(&workspace_id).await.unwrap();
    assert!(resumed.binding.unwrap().sync_enabled);
    tokio::time::timeout(Duration::from_secs(5), barrier.wait())
        .await
        .expect("resume triggered a pull without a manual sync");
    assert!(transport.changes_calls.load(Ordering::SeqCst) > calls_before);
    barrier.wait().await;
}

#[tokio::test]
async fn global_pause_keeps_environment_updates_in_the_outbox() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let environment = seed
        .workspace_environment_create(workspace_id.clone(), "Test".into())
        .await
        .unwrap();
    let environment_variable = seed
        .workspace_environment_variable_create(
            workspace_id.clone(),
            environment.id.clone(),
            variable(None, "HOST", "old.example.test", false),
        )
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport);
    service.enable(&workspace_id).await.unwrap();
    sqlx::query("DELETE FROM cloud_sync_outbox")
        .execute(db.pool())
        .await
        .unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();

    service.set_global_sync_enabled(false).await.unwrap();
    bus.workspace_environment_variable_update(
        workspace_id,
        environment.id,
        environment_variable.id.clone(),
        variable(
            Some(environment_variable.id.clone()),
            "HOST",
            "new.example.test",
            false,
        ),
    )
    .await
    .unwrap();

    let outbox: (String, String, String) = sqlx::query_as(
        r#"SELECT entity_type, operation, canonical_payload_json
           FROM cloud_sync_outbox WHERE entity_id = ?1"#,
    )
    .bind(environment_variable.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(outbox.0, "workspaceEnvironmentVariable");
    assert_eq!(outbox.1, "upsert");
    assert!(outbox.2.contains("new.example.test"));
}
