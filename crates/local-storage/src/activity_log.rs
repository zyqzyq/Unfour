use crate::LocalDb;
use chrono::Utc;
use serde_json::Value;
use unfour_core::AppResult;
use uuid::Uuid;

/// A single persisted activity-trail row, returned verbatim from storage. The
/// `details_json` payload is the redacted summary recorded by `record`; callers
/// that expose it (e.g. the MCP layer) still apply defense-in-depth masking.
#[derive(Debug, Clone)]
pub struct ActivityEntry {
    pub id: String,
    pub workspace_id: Option<String>,
    pub action: String,
    pub target: Option<String>,
    pub details_json: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct ActivityLogService {
    db: LocalDb,
}

impl ActivityLogService {
    pub fn new(db: LocalDb) -> Self {
        Self { db }
    }

    pub async fn record(
        &self,
        workspace_id: Option<&str>,
        action: &str,
        target: Option<&str>,
        details: Value,
    ) -> AppResult<()> {
        // This is a local activity trail, not a compliance log. Callers should
        // pass only redacted summaries and avoid routine read/UI noise.
        sqlx::query(
            r#"
            INSERT INTO activity_events (id, workspace_id, action, target, details_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(workspace_id)
        .bind(action)
        .bind(target)
        .bind(serde_json::to_string(&details)?)
        .bind(Utc::now().to_rfc3339())
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// List the most recent activity events, newest first. When `workspace_id`
    /// is provided, results are scoped to that workspace; otherwise events from
    /// every workspace are returned. `limit` is the maximum number of rows; the
    /// caller is responsible for clamping it to a sane range.
    pub async fn list_recent(
        &self,
        workspace_id: Option<&str>,
        limit: i64,
    ) -> AppResult<Vec<ActivityEntry>> {
        type Row = (
            String,
            Option<String>,
            String,
            Option<String>,
            String,
            String,
        );

        let rows: Vec<Row> = match workspace_id {
            Some(ws) => {
                sqlx::query_as(
                    r#"
                SELECT id, workspace_id, action, target, details_json, created_at
                FROM activity_events
                WHERE workspace_id = ?1
                ORDER BY created_at DESC, id DESC
                LIMIT ?2
                "#,
                )
                .bind(ws)
                .bind(limit)
                .fetch_all(self.db.pool())
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"
                SELECT id, workspace_id, action, target, details_json, created_at
                FROM activity_events
                ORDER BY created_at DESC, id DESC
                LIMIT ?1
                "#,
                )
                .bind(limit)
                .fetch_all(self.db.pool())
                .await?
            }
        };

        Ok(rows
            .into_iter()
            .map(
                |(id, workspace_id, action, target, details_json, created_at)| ActivityEntry {
                    id,
                    workspace_id,
                    action,
                    target,
                    details_json,
                    created_at,
                },
            )
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn service() -> ActivityLogService {
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
        ActivityLogService::new(db)
    }

    #[tokio::test]
    async fn record_inserts_event() {
        let svc = service().await;
        svc.record(
            Some("ws-1"),
            "workspace.create",
            Some("ws-1"),
            serde_json::json!({ "name": "Test" }),
        )
        .await
        .expect("record event");

        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT workspace_id, action, target FROM activity_events ORDER BY created_at",
        )
        .fetch_all(svc.db.pool())
        .await
        .expect("fetch events");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "ws-1");
        assert_eq!(rows[0].1, "workspace.create");
        assert_eq!(rows[0].2, "ws-1");
    }

    #[tokio::test]
    async fn record_multiple_events() {
        let svc = service().await;
        svc.record(Some("ws-1"), "action.a", None, serde_json::json!({}))
            .await
            .unwrap();
        svc.record(
            None,
            "action.b",
            Some("target"),
            serde_json::json!({ "key": "val" }),
        )
        .await
        .unwrap();
        svc.record(Some("ws-2"), "action.c", None, serde_json::json!({}))
            .await
            .unwrap();

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM activity_events")
            .fetch_one(svc.db.pool())
            .await
            .expect("count events");
        assert_eq!(count.0, 3);
    }

    #[tokio::test]
    async fn record_stores_json_details() {
        let svc = service().await;
        svc.record(
            Some("ws-1"),
            "test.action",
            None,
            serde_json::json!({ "count": 42, "label": "hello" }),
        )
        .await
        .unwrap();

        let details: (String,) =
            sqlx::query_as("SELECT details_json FROM activity_events WHERE action = 'test.action'")
                .fetch_one(svc.db.pool())
                .await
                .expect("fetch details");

        let parsed: Value = serde_json::from_str(&details.0).expect("parse json");
        assert_eq!(parsed["count"], 42);
        assert_eq!(parsed["label"], "hello");
    }

    #[tokio::test]
    async fn list_recent_returns_newest_first_scoped_and_limited() {
        let svc = service().await;
        svc.record(Some("ws-1"), "first", None, serde_json::json!({}))
            .await
            .unwrap();
        svc.record(Some("ws-1"), "second", None, serde_json::json!({}))
            .await
            .unwrap();
        svc.record(Some("ws-1"), "third", None, serde_json::json!({}))
            .await
            .unwrap();
        svc.record(Some("ws-2"), "other", None, serde_json::json!({}))
            .await
            .unwrap();

        // Scoped to ws-1, newest first.
        let scoped = svc.list_recent(Some("ws-1"), 50).await.expect("list ws-1");
        assert_eq!(scoped.len(), 3);
        assert_eq!(scoped[0].action, "third");
        assert_eq!(scoped[2].action, "first");
        assert!(scoped
            .iter()
            .all(|e| e.workspace_id.as_deref() == Some("ws-1")));

        // Limit is honored.
        let limited = svc.list_recent(Some("ws-1"), 2).await.expect("limit ws-1");
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].action, "third");

        // No workspace filter returns every workspace's events.
        let all = svc.list_recent(None, 50).await.expect("list all");
        assert_eq!(all.len(), 4);
    }
}
