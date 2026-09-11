use super::super::script_parser::split_script;
use super::super::*;
use super::support::{service_with_workspace, sqlite_fixture, sqlite_input};
use unfour_core::models::DatabaseScriptInput;

fn input(workspace: &str, connection: &str, sql: &str) -> DatabaseScriptInput {
    DatabaseScriptInput {
        query: DatabaseQueryInput {
            workspace_id: workspace.into(),
            connection_id: connection.into(),
            sql: sql.into(),
            limit: Some(1),
            confirm_mutation: Some(true),
            catalog: None,
            schema: None,
            timeout_ms: None,
        },
        run_id: uuid::Uuid::new_v4().to_string(),
        cursor_offset: None,
        explain: false,
    }
}

#[test]
fn dialect_boundaries_preserve_source_and_skip_comments() {
    for driver in ["postgres", "mysql", "sqlite"] {
        let sql = "-- ; comment\r\nSELECT '中😀;it''s'; /* ; */ SELECT 2; -- end";
        let parts = split_script(sql, driver).unwrap();
        assert_eq!(parts.len(), 2, "{driver}");
        assert_eq!(parts[0].sql, "SELECT '中😀;it''s';");
        assert_eq!(parts[1].sql, "SELECT 2;");
        assert_eq!(
            parts[0].start,
            sql[..sql.find("SELECT").unwrap()].encode_utf16().count()
        );
        assert_eq!(
            split_script("-- only\n/* comments */ ;", driver)
                .unwrap()
                .len(),
            0
        );
        assert!(split_script("SELECT 'unterminated", driver).is_err());
    }
    assert_eq!(
        split_script(
            "DO $body$ BEGIN PERFORM ';'; END; $body$; SELECT $$a;b$$;",
            "postgres"
        )
        .unwrap()
        .len(),
        2
    );
    assert_eq!(
        split_script("SELECT 'a\\';b'; SELECT 2;", "mysql")
            .unwrap()
            .len(),
        2
    );
    assert_eq!(split_script("CREATE TRIGGER tr AFTER INSERT ON t BEGIN UPDATE t SET n=CASE WHEN n=1 THEN 2 ELSE 3 END; INSERT INTO t VALUES(4); END; SELECT 1;", "sqlite").unwrap().len(), 2);
    assert!(split_script(
        "DELIMITER //\nCREATE PROCEDURE p() BEGIN SELECT 1; END//",
        "mysql"
    )
    .is_err());
    assert!(split_script("SELECT 1 /*! INTO OUTFILE '/tmp/x' */", "mysql").is_err());
}

#[test]
fn safety_ignores_quoted_words_and_catches_wrapped_writes() {
    for driver in ["postgres", "mysql", "sqlite"] {
        for sql in [
            "-- hi\nSELECT 'into update delete'",
            "WITH t AS (SELECT 'delete') SELECT * FROM t",
        ] {
            assert_eq!(
                classify_query_for_driver(sql, driver).classification,
                "read",
                "{driver}: {sql}"
            );
        }
        for sql in [
            "WITH t AS (SELECT 1) UPDATE users SET n=2",
            "WITH t AS (SELECT 1) DELETE FROM users",
            "UPDATE users SET n=2 RETURNING n",
            "INSERT INTO users VALUES(1) RETURNING n",
            "SELECT * INTO copy FROM users",
        ] {
            assert!(
                classify_query_for_driver(sql, driver).requires_confirmation,
                "{driver}: {sql}"
            );
        }
    }
    assert!(classify_query_for_driver("PRAGMA writable_schema=ON", "sqlite").requires_confirmation);
}

#[tokio::test]
async fn script_keeps_temp_and_transaction_state_and_drains_returning() {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    let script = "CREATE TEMP TABLE tmp(n INT); BEGIN; INSERT INTO tmp VALUES(1),(2),(3) RETURNING n; WITH t AS (SELECT 1) UPDATE tmp SET n=n+10 RETURNING n; ROLLBACK; SELECT count(*) FROM tmp;";
    let result = service
        .execute_script(input(&workspace, &connection.id, script))
        .await
        .unwrap();
    assert!(
        result.statements.iter().all(|s| s.status == "success"),
        "{result:?}"
    );
    let insert = result.statements[2].result.as_ref().unwrap();
    assert_eq!(insert.rows.len(), 1);
    assert_eq!(insert.affected_rows, 3);
    let update = result.statements[3].result.as_ref().unwrap();
    assert_eq!(update.affected_rows, 3);
    assert_eq!(update.rows[0][0].as_deref(), Some("11"));
    assert_eq!(
        result.statements[5].result.as_ref().unwrap().rows[0][0].as_deref(),
        Some("0")
    );
    // A new run is a new session: recreating a TEMP table is valid.
    let repeat = service
        .execute_script(input(&workspace, &connection.id, script))
        .await
        .unwrap();
    assert!(repeat.statements.iter().all(|s| s.status == "success"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn first_error_preserves_success_and_repeated_create_is_not_hidden() {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    let script = "SELECT 'a;b'; CREATE TABLE repeated(n INT); INSERT INTO missing VALUES(1); INSERT INTO repeated VALUES(99);";
    let first = service
        .execute_script(input(&workspace, &connection.id, script))
        .await
        .unwrap();
    assert_eq!(
        first
            .statements
            .iter()
            .map(|s| s.status.as_str())
            .collect::<Vec<_>>(),
        ["success", "success", "failed", "skipped"]
    );
    assert_eq!(first.statements[2].index, 3);
    assert!(first.statements[2].error.as_ref().unwrap()["message"]
        .as_str()
        .unwrap()
        .contains("missing"));
    assert_eq!(
        first.statements[0].result.as_ref().unwrap().rows[0][0].as_deref(),
        Some("a;b")
    );
    let second = service
        .execute_script(input(&workspace, &connection.id, script))
        .await
        .unwrap();
    assert_eq!(
        second
            .statements
            .iter()
            .map(|s| s.status.as_str())
            .collect::<Vec<_>>(),
        ["success", "failed", "skipped", "skipped"]
    );
    assert!(second.statements[1].error.as_ref().unwrap()["message"]
        .as_str()
        .unwrap()
        .contains("already exists"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn preflight_runs_nothing_and_current_resolves_utf16_cursor() {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    let mut request = input(
        &workspace,
        &connection.id,
        "CREATE TABLE never(n INT); SELECT 1;",
    );
    request.query.confirm_mutation = Some(false);
    assert!(matches!(
        service.execute_script(request).await,
        Err(AppError::ConfirmationRequired { .. })
    ));
    let check = service
        .execute_script(input(&workspace, &connection.id, "SELECT * FROM never;"))
        .await
        .unwrap();
    assert_eq!(check.statements[0].status, "failed");
    let mut request = input(&workspace, &connection.id, "SELECT '😀'; SELECT 2;");
    request.cursor_offset = Some("SELECT '😀'; ".encode_utf16().count());
    let result = service.execute_script(request).await.unwrap();
    assert_eq!(result.statements.len(), 1);
    assert_eq!(result.statements[0].index, 2);
    assert_eq!(
        result.statements[0].result.as_ref().unwrap().rows[0][0].as_deref(),
        Some("2")
    );
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn sqlite_trigger_script_and_single_statement_share_semicolon_support() {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    let script = "CREATE TABLE source(n); CREATE TABLE audit(n); CREATE TRIGGER tr AFTER INSERT ON source BEGIN INSERT INTO audit VALUES(CASE WHEN new.n=1 THEN 2 ELSE 3 END); INSERT INTO audit VALUES(4); END; INSERT INTO source VALUES(1); SELECT count(*) FROM audit;";
    let result = service
        .execute_script(input(&workspace, &connection.id, script))
        .await
        .unwrap();
    assert!(
        result.statements.iter().all(|s| s.status == "success"),
        "{result:?}"
    );
    assert_eq!(
        result
            .statements
            .last()
            .unwrap()
            .result
            .as_ref()
            .unwrap()
            .rows[0][0]
            .as_deref(),
        Some("2")
    );
    let single = input(&workspace, &connection.id, "SELECT ';' AS text;").query;
    assert_eq!(
        service.execute_query(single).await.unwrap().rows[0][0].as_deref(),
        Some(";")
    );
    assert!(service
        .execute_query(input(&workspace, &connection.id, "SELECT 1; SELECT 2;").query)
        .await
        .is_err());
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn failed_transaction_is_discarded_and_with_delete_is_not_limited() {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    let first = service
        .execute_script(input(
            &workspace,
            &connection.id,
            "CREATE TABLE tx(n); BEGIN; INSERT INTO tx VALUES(1); SELECT * FROM missing; COMMIT;",
        ))
        .await
        .unwrap();
    assert_eq!(first.statements[3].status, "failed");
    assert_eq!(first.statements[4].status, "skipped");
    let check = service.execute_script(input(&workspace, &connection.id,
        "SELECT count(*) FROM tx; INSERT INTO tx VALUES(1),(2),(3); WITH c AS (SELECT 1) DELETE FROM tx RETURNING n; SELECT count(*) FROM tx;")).await.unwrap();
    assert!(
        check.statements.iter().all(|s| s.status == "success"),
        "{check:?}"
    );
    assert_eq!(
        check.statements[0].result.as_ref().unwrap().rows[0][0].as_deref(),
        Some("0")
    );
    assert_eq!(
        check.statements[2].result.as_ref().unwrap().affected_rows,
        3
    );
    assert_eq!(check.statements[2].result.as_ref().unwrap().rows.len(), 1);
    assert_eq!(
        check.statements[3].result.as_ref().unwrap().rows[0][0].as_deref(),
        Some("0")
    );
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn stop_is_workspace_scoped_skips_the_suffix_and_releases_the_run_id() {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    let request = input(&workspace, &connection.id,
        "WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<2000000) SELECT sum(x) FROM n; CREATE TABLE should_not_run(n);");
    let run_id = request.run_id.clone();
    let task_service = service.clone();
    let task = tokio::spawn(async move { task_service.execute_script(request).await.unwrap() });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            assert!(!service
                .stop_script("another-workspace".into(), run_id.clone())
                .unwrap());
            if service
                .stop_script(workspace.clone(), run_id.clone())
                .unwrap()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("active run registration");
    let output = task.await.unwrap();
    assert!(output.stopped);
    assert_eq!(output.statements[1].status, "skipped");
    assert!(!service
        .stop_script(workspace.clone(), run_id.clone())
        .unwrap());
    let mut next = input(&workspace, &connection.id, "SELECT 1;");
    next.run_id = run_id;
    assert_eq!(
        service.execute_script(next).await.unwrap().statements[0].status,
        "success"
    );
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn whole_batch_readonly_preflight_applies_to_all_drivers_before_connecting() {
    use super::support::{mysql_input, postgres_input};
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    for mut config in [
        sqlite_input(&workspace, &path),
        postgres_input(&workspace),
        mysql_input(&workspace, None),
    ] {
        config.read_only = true;
        let connection = service.save_connection(config).await.unwrap();
        let error = service
            .execute_script(input(
                &workspace,
                &connection.id,
                "SELECT 1; DELETE FROM t;",
            ))
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::ReadOnly(_)));
        assert!(error.to_string().contains("statement 2"));
    }
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn result_stream_never_merges_multiple_result_shapes() {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    let request = input(&workspace, &connection.id, "SELECT 1;");
    let mut conn = service
        .script_connection(&connection, &request.query)
        .await
        .unwrap();
    // Exercise the driver stream directly, simulating a procedure that returns
    // two rowsets. Normal editor scripts are split before reaching this layer.
    let (result, more) = conn
        .execute("SELECT 1 AS first; SELECT 2 AS second;", 100, read_safety())
        .await
        .unwrap();
    assert!(more);
    assert_eq!(result.columns[0].name, "first");
    assert_eq!(result.rows, vec![vec![Some("1".to_string())]]);
    let (empty, more) = conn
        .execute("SELECT 1 WHERE 0; SELECT 2;", 100, read_safety())
        .await
        .unwrap();
    assert!(empty.rows.is_empty());
    assert!(more);
    let _ = std::fs::remove_file(path);
}
