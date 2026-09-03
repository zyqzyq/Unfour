use super::*;

#[tokio::test]
async fn legacy_paused_binding_stays_paused_and_requires_manual_enable() {
    let pool = test_pool().await;
    let db = unfour_local_storage::LocalDb::from_pool(pool.clone());
    db.migrate().await.expect("run core migrations");
    apply_cloud_sync_migrations_through(&pool, 20260903020000).await;

    sqlx::query(
        r#"INSERT INTO workspaces (
             id, name, is_default, last_opened_at, environment_type, mcp_policy,
             created_at, updated_at, revision
           ) VALUES ('legacy-paused-workspace', 'Legacy paused', 0, NULL, 'dev', 'auto',
                     '2026-09-03T00:00:00Z', '2026-09-03T00:00:00Z', 1)"#,
    )
    .execute(&pool)
    .await
    .expect("insert legacy workspace");
    sqlx::query(
        r#"INSERT INTO cloud_sync_workspace_bindings (
             account_id, local_workspace_id, cloud_workspace_id,
             sync_enabled, state, initial_cursor, created_at, updated_at
           ) VALUES ('legacy-account', 'legacy-paused-workspace', 'legacy-cloud',
                     0, 'paused', 0, '2026-09-03T00:00:00Z',
                     '2026-09-03T00:00:00Z')"#,
    )
    .execute(&pool)
    .await
    .expect("insert legacy paused binding");
    sqlx::query(
        "INSERT INTO cloud_sync_runtime_context (singleton, active_account_id, generation, updated_at) VALUES (1, NULL, 0, '2026-09-03T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .expect("insert signed-out runtime context");

    migrate(&pool)
        .await
        .expect("run ownership safety migrations");

    let binding: (bool, String, Option<String>) = sqlx::query_as(
        "SELECT sync_enabled, state, last_error FROM cloud_sync_workspace_bindings WHERE local_workspace_id = 'legacy-paused-workspace'",
    )
    .fetch_one(&pool)
    .await
    .expect("read conservative legacy binding");
    assert_eq!(
        binding,
        (
            false,
            "paused".into(),
            Some("cloud_sync_legacy_paused_binding_ambiguous".into())
        )
    );
    let owner: (String, String) = sqlx::query_as(
        "SELECT account_id, cloud_workspace_id FROM cloud_sync_workspace_ownership WHERE local_workspace_id = 'legacy-paused-workspace'",
    )
    .fetch_one(&pool)
    .await
    .expect("backfill unambiguous owner");
    assert_eq!(owner, ("legacy-account".into(), "legacy-cloud".into()));
    let diagnostic: (i64, String) = sqlx::query_as(
        "SELECT COUNT(*), MAX(error_code) FROM cloud_sync_diagnostics WHERE account_id = 'legacy-account' AND cloud_workspace_id = 'legacy-cloud'",
    )
    .fetch_one(&pool)
    .await
    .expect("read legacy pause diagnostic");
    assert_eq!(
        diagnostic,
        (1, "cloud_sync_legacy_paused_binding_ambiguous".into())
    );
}

#[tokio::test]
async fn account_context_pause_reason_is_not_marked_as_legacy_ambiguous() {
    let pool = test_pool().await;
    let db = unfour_local_storage::LocalDb::from_pool(pool.clone());
    db.migrate().await.expect("run core migrations");
    apply_cloud_sync_migrations_through(&pool, 20260903020000).await;

    sqlx::query(
        r#"INSERT INTO workspaces (
             id, name, is_default, last_opened_at, environment_type, mcp_policy,
             created_at, updated_at, revision
           ) VALUES ('marked-paused-workspace', 'Marked paused', 0, NULL, 'dev', 'auto',
                     '2026-09-03T00:00:00Z', '2026-09-03T00:00:00Z', 1)"#,
    )
    .execute(&pool)
    .await
    .expect("insert marked workspace");
    sqlx::query(
        r#"INSERT INTO cloud_sync_workspace_bindings (
             account_id, local_workspace_id, cloud_workspace_id,
             sync_enabled, state, initial_cursor, created_at, updated_at
           ) VALUES ('marked-account', 'marked-paused-workspace', 'marked-cloud',
                     0, 'paused', 0, '2026-09-03T00:00:00Z',
                     '2026-09-03T00:00:00Z')"#,
    )
    .execute(&pool)
    .await
    .expect("insert marked paused binding");
    sqlx::query(
        r#"INSERT INTO cloud_sync_account_binding_pause_reasons (
             account_id, local_workspace_id, previous_state, created_at, updated_at
           ) VALUES ('marked-account', 'marked-paused-workspace', 'active',
                     '2026-09-03T00:00:00Z', '2026-09-03T00:00:00Z')"#,
    )
    .execute(&pool)
    .await
    .expect("insert explicit account pause reason");

    migrate(&pool)
        .await
        .expect("run ownership safety migrations");

    let binding: (bool, String, Option<String>) = sqlx::query_as(
        "SELECT sync_enabled, state, last_error FROM cloud_sync_workspace_bindings WHERE local_workspace_id = 'marked-paused-workspace'",
    )
    .fetch_one(&pool)
    .await
    .expect("read marked paused binding");
    assert_eq!(binding, (false, "paused".into(), None));
    let diagnostic_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_diagnostics WHERE account_id = 'marked-account'",
    )
    .fetch_one(&pool)
    .await
    .expect("count marked pause diagnostics");
    assert_eq!(diagnostic_count, 0);
}

#[tokio::test]
async fn duplicate_historical_bindings_are_quarantined_without_fanout() {
    let pool = test_pool().await;
    let db = unfour_local_storage::LocalDb::from_pool(pool.clone());
    db.migrate().await.expect("run core migrations");
    apply_cloud_sync_migrations_through(&pool, 20260903020000).await;

    sqlx::query(
        r#"INSERT INTO workspaces (
             id, name, is_default, last_opened_at, environment_type, mcp_policy,
             created_at, updated_at, revision
           ) VALUES ('duplicate-workspace', 'Duplicate', 0, NULL, 'dev', 'auto',
                     '2026-09-03T00:00:00Z', '2026-09-03T00:00:00Z', 1)"#,
    )
    .execute(&pool)
    .await
    .expect("insert duplicate workspace");
    for (account_id, cloud_workspace_id) in [
        ("duplicate-account-a", "duplicate-cloud-a"),
        ("duplicate-account-b", "duplicate-cloud-b"),
    ] {
        sqlx::query(
            r#"INSERT INTO cloud_sync_workspace_bindings (
                 account_id, local_workspace_id, cloud_workspace_id,
                 sync_enabled, state, initial_cursor, created_at, updated_at
               ) VALUES (?1, 'duplicate-workspace', ?2, 1, 'active', 0,
                         '2026-09-03T00:00:00Z', '2026-09-03T00:00:00Z')"#,
        )
        .bind(account_id)
        .bind(cloud_workspace_id)
        .execute(&pool)
        .await
        .expect("insert duplicate historical binding");
    }

    migrate(&pool).await.expect("quarantine duplicate bindings");

    let owners: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_workspace_ownership WHERE local_workspace_id = 'duplicate-workspace'",
    )
    .fetch_one(&pool)
    .await
    .expect("count duplicate owners");
    assert_eq!(owners, 0);
    let bindings: Vec<(String, bool, String, Option<String>)> = sqlx::query_as(
        "SELECT account_id, sync_enabled, state, last_error FROM cloud_sync_workspace_bindings WHERE local_workspace_id = 'duplicate-workspace' ORDER BY account_id",
    )
    .fetch_all(&pool)
    .await
    .expect("read quarantined bindings");
    assert_eq!(
        bindings,
        vec![
            (
                "duplicate-account-a".into(),
                false,
                "error".into(),
                Some("cloud_sync_workspace_ownership_ambiguous".into())
            ),
            (
                "duplicate-account-b".into(),
                false,
                "error".into(),
                Some("cloud_sync_workspace_ownership_ambiguous".into())
            )
        ]
    );
    let diagnostic_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_diagnostics WHERE error_code = 'cloud_sync_workspace_ownership_ambiguous'",
    )
    .fetch_one(&pool)
    .await
    .expect("count duplicate diagnostics");
    assert_eq!(diagnostic_count, 2);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("check quarantined foreign keys")
        .is_empty());
}
