use super::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

async fn service() -> TerminalHistoryService {
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
    TerminalHistoryService::new(db)
}

fn summary(workspace_id: &str, session_id: &str, connection_id: &str) -> SshSessionSummary {
    let now = Utc::now().to_rfc3339();
    SshSessionSummary {
        session_id: session_id.to_string(),
        workspace_id: workspace_id.to_string(),
        connection_id: connection_id.to_string(),
        status: "connected".to_string(),
        reconnect_attempt: 0,
        auth_kind: "password".to_string(),
        host: "localhost".to_string(),
        username: "developer".to_string(),
        cols: 120,
        rows: 32,
        created_at: now.clone(),
        updated_at: now,
    }
}

#[tokio::test]
async fn append_and_hydrate_terminal_output() {
    let service = service().await;
    let summary = summary("ws-a", "session-a", "connection-a");
    service.save_session(&summary).await.expect("save session");
    service
        .append_output(
            &summary.workspace_id,
            &summary.session_id,
            &summary.connection_id,
            "hello\r\nworld\r\n",
        )
        .await
        .expect("append output");

    let events = service
        .hydrate(&summary.workspace_id, &summary.session_id)
        .await
        .expect("hydrate output");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "hello\r\nworld\r\n");
}

#[tokio::test]
async fn retention_keeps_only_the_recent_utf8_tail() {
    let service = service().await;
    let summary = summary("ws-a", "session-a", "connection-a");
    service.save_session(&summary).await.expect("save session");
    let prefix = "x".repeat(TERMINAL_HISTORY_MAX_BYTES);
    service
        .append_output(
            &summary.workspace_id,
            &summary.session_id,
            &summary.connection_id,
            &prefix,
        )
        .await
        .expect("append prefix");
    service
        .append_output(
            &summary.workspace_id,
            &summary.session_id,
            &summary.connection_id,
            "recent-终端",
        )
        .await
        .expect("append recent output");

    let events = service
        .hydrate(&summary.workspace_id, &summary.session_id)
        .await
        .expect("hydrate output");
    assert!(events[0].data.len() <= TERMINAL_HISTORY_MAX_BYTES);
    assert!(events[0].data.ends_with("recent-终端"));
}

#[tokio::test]
async fn sessions_and_workspaces_are_isolated() {
    let service = service().await;
    for summary in [
        summary("ws-a", "session-a", "connection-a"),
        summary("ws-a", "session-b", "connection-a"),
        summary("ws-b", "session-a", "connection-b"),
    ] {
        service.save_session(&summary).await.expect("save session");
        service
            .append_output(
                &summary.workspace_id,
                &summary.session_id,
                &summary.connection_id,
                &format!("{}:{}\r\n", summary.workspace_id, summary.session_id),
            )
            .await
            .expect("append output");
    }

    let session_a = service
        .hydrate("ws-a", "session-a")
        .await
        .expect("hydrate session a");
    let session_b = service
        .hydrate("ws-a", "session-b")
        .await
        .expect("hydrate session b");
    let other_workspace = service
        .hydrate("ws-b", "session-a")
        .await
        .expect("hydrate other workspace");
    assert_eq!(session_a[0].data, "ws-a:session-a\r\n");
    assert_eq!(session_b[0].data, "ws-a:session-b\r\n");
    assert_eq!(other_workspace[0].data, "ws-b:session-a\r\n");
}

#[tokio::test]
async fn persistence_redacts_secrets_and_credential_references() {
    let service = service().await;
    let summary = summary("ws-a", "session-a", "connection-a");
    service.save_session(&summary).await.expect("save session");
    service
        .append_output(
            &summary.workspace_id,
            &summary.session_id,
            &summary.connection_id,
            "ok\r\npassword=secret\r\ncredential_ref=ssh-password-1\r\nprivate key: data\r\n",
        )
        .await
        .expect("append output");

    let events = service
        .hydrate(&summary.workspace_id, &summary.session_id)
        .await
        .expect("hydrate output");
    assert!(events[0].data.contains("ok"));
    assert!(!events[0].data.contains("secret"));
    assert!(!events[0].data.contains("ssh-password-1"));
    assert!(!events[0].data.contains("private key: data"));
}

#[tokio::test]
async fn deleting_connection_cleans_up_only_matching_history() {
    let service = service().await;
    let removed = summary("ws-a", "session-a", "connection-a");
    let retained = summary("ws-a", "session-b", "connection-c");
    service.save_session(&removed).await.expect("save removed");
    service
        .save_session(&retained)
        .await
        .expect("save retained");

    service
        .delete_connection_history("ws-a", "connection-a")
        .await
        .expect("delete history");
    assert!(service
        .hydrate("ws-a", "session-a")
        .await
        .expect("hydrate removed")
        .is_empty());
    assert_eq!(
        service
            .list_sessions("ws-a")
            .await
            .expect("list sessions")
            .len(),
        1
    );
}
