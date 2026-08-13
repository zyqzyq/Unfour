use super::*;
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

async fn service() -> SshCommandHistoryService {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect in-memory sqlite");
    let db = LocalDb::from_pool(pool);
    db.migrate().await.expect("run migrations");
    let now = Utc::now().to_rfc3339();
    for workspace_id in ["ws-a", "ws-b"] {
        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?1, 0, ?2, ?2, 1, 'local')
            "#,
        )
        .bind(workspace_id)
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("insert workspace");
    }
    for (workspace_id, connection_id) in [
        ("ws-a", "connection-a"),
        ("ws-a", "connection-c"),
        ("ws-b", "connection-b"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO connections (
              id, workspace_id, connection_type, name, host, port,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, 'ssh', ?1, 'localhost', 22, ?3, ?3, 1, 'local')
            "#,
        )
        .bind(connection_id)
        .bind(workspace_id)
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("insert connection");
    }
    SshCommandHistoryService::new(db)
}

fn record_input(
    workspace_id: &str,
    connection_id: &str,
    command: &str,
    executed_at: &str,
) -> SshCommandHistoryRecordInput {
    SshCommandHistoryRecordInput {
        workspace_id: workspace_id.to_string(),
        connection_id: connection_id.to_string(),
        session_id: Some("session-a".to_string()),
        command: command.to_string(),
        cwd: None,
        exit_code: None,
        duration_ms: None,
        executed_at: executed_at.to_string(),
    }
}

fn query(workspace_id: &str, connection_id: Option<&str>) -> SshCommandHistoryQuery {
    SshCommandHistoryQuery {
        workspace_id: workspace_id.to_string(),
        connection_id: connection_id.map(str::to_string),
        search: None,
        limit: Some(20),
        include_redacted: false,
        since: None,
        until: None,
    }
}

#[tokio::test]
async fn records_and_lists_newest_commands_first() {
    let service = service().await;
    service
        .record(record_input(
            "ws-a",
            "connection-a",
            "pwd",
            "2026-08-13T00:00:00Z",
        ))
        .await
        .expect("record pwd");
    service
        .record(record_input(
            "ws-a",
            "connection-a",
            "git status",
            "2026-08-13T00:00:01Z",
        ))
        .await
        .expect("record status");

    let entries = service
        .list(query("ws-a", Some("connection-a")))
        .await
        .expect("list history");
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.command.as_str())
            .collect::<Vec<_>>(),
        vec!["git status", "pwd"]
    );
    assert_eq!(entries[0].workspace_id, "ws-a");
    assert_eq!(entries[0].connection_id, "connection-a");
}

#[tokio::test]
async fn workspace_and_connection_queries_are_isolated() {
    let service = service().await;
    for (workspace_id, connection_id, command, executed_at) in [
        ("ws-a", "connection-a", "echo a", "2026-08-13T00:00:00Z"),
        ("ws-a", "connection-c", "echo c", "2026-08-13T00:00:01Z"),
        ("ws-b", "connection-b", "echo b", "2026-08-13T00:00:02Z"),
    ] {
        service
            .record(record_input(
                workspace_id,
                connection_id,
                command,
                executed_at,
            ))
            .await
            .expect("record scoped command");
    }

    let connection = service
        .list(query("ws-a", Some("connection-a")))
        .await
        .expect("list connection history");
    assert_eq!(connection.len(), 1);
    assert_eq!(connection[0].command, "echo a");

    let workspace = service
        .list(query("ws-a", None))
        .await
        .expect("list workspace history");
    assert_eq!(workspace.len(), 2);
    assert!(workspace.iter().all(|entry| entry.workspace_id == "ws-a"));
}

#[tokio::test]
async fn consecutive_duplicate_refreshes_one_row() {
    let service = service().await;
    let first = service
        .record(record_input(
            "ws-a",
            "connection-a",
            "ls -la",
            "2026-08-13T00:00:00Z",
        ))
        .await
        .expect("record first")
        .expect("entry");
    let second = service
        .record(record_input(
            "ws-a",
            "connection-a",
            "ls -la",
            "2026-08-13T00:00:02Z",
        ))
        .await
        .expect("record duplicate")
        .expect("entry");

    let entries = service
        .list(query("ws-a", Some("connection-a")))
        .await
        .expect("list history");
    assert_eq!(entries.len(), 1);
    assert_eq!(first.id, second.id);
    assert_eq!(entries[0].executed_at, "2026-08-13T00:00:02Z");
}

#[tokio::test]
async fn sensitive_commands_are_redacted_and_excluded_by_default() {
    let service = service().await;
    let recorded = service
        .record(record_input(
            "ws-a",
            "connection-a",
            "curl -H 'Authorization: Bearer abc' https://example.test",
            "2026-08-13T00:00:00Z",
        ))
        .await
        .expect("record sensitive command")
        .expect("entry");
    assert!(recorded.redacted);
    assert_eq!(recorded.command, "<redacted>");

    assert!(service
        .list(query("ws-a", Some("connection-a")))
        .await
        .expect("list safe history")
        .is_empty());

    let mut all_query = query("ws-a", Some("connection-a"));
    all_query.include_redacted = true;
    let all = service.list(all_query).await.expect("list all history");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].command, "<redacted>");
}

#[tokio::test]
async fn search_treats_like_wildcards_as_literals() {
    let service = service().await;
    for (command, executed_at) in [
        ("printf 100%", "2026-08-13T00:00:00Z"),
        ("printf 1000", "2026-08-13T00:00:01Z"),
    ] {
        service
            .record(record_input("ws-a", "connection-a", command, executed_at))
            .await
            .expect("record searchable command");
    }
    let mut search = query("ws-a", Some("connection-a"));
    search.search = Some("100%".to_string());
    let entries = service.list(search).await.expect("search history");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].command, "printf 100%");
}

#[tokio::test]
async fn stores_at_most_two_hundred_commands_per_connection() {
    let service = service().await;
    for index in 0..205 {
        service
            .record(record_input(
                "ws-a",
                "connection-a",
                &format!("cmd-{index}"),
                &format!("2026-08-13T00:00:00.{index:03}Z"),
            ))
            .await
            .expect("record command");
    }

    let mut all = query("ws-a", Some("connection-a"));
    all.limit = Some(200);
    let entries = service.list(all).await.expect("list capped history");
    assert_eq!(entries.len(), 200);
    assert_eq!(entries[0].command, "cmd-204");
    assert!(entries.iter().all(|entry| entry.command != "cmd-0"));
}

#[tokio::test]
async fn time_range_filters_executed_at() {
    let service = service().await;
    for (command, executed_at) in [
        ("echo early", "2026-08-13T00:00:00Z"),
        ("echo mid", "2026-08-13T12:00:00Z"),
        ("echo late", "2026-08-13T23:00:00Z"),
    ] {
        service
            .record(record_input("ws-a", "connection-a", command, executed_at))
            .await
            .expect("record timed command");
    }

    let mut since = query("ws-a", Some("connection-a"));
    since.since = Some("2026-08-13T12:00:00Z".to_string());
    let after_noon = service.list(since).await.expect("list since noon");
    assert_eq!(
        after_noon
            .iter()
            .map(|entry| entry.command.as_str())
            .collect::<Vec<_>>(),
        vec!["echo late", "echo mid"]
    );

    let mut until = query("ws-a", Some("connection-a"));
    until.until = Some("2026-08-13T12:00:00Z".to_string());
    let through_noon = service.list(until).await.expect("list until noon");
    assert_eq!(
        through_noon
            .iter()
            .map(|entry| entry.command.as_str())
            .collect::<Vec<_>>(),
        vec!["echo mid", "echo early"]
    );

    let mut window = query("ws-a", Some("connection-a"));
    window.since = Some("2026-08-13T12:00:00Z".to_string());
    window.until = Some("2026-08-13T12:00:00Z".to_string());
    let exact = service.list(window).await.expect("list exact window");
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].command, "echo mid");
}
