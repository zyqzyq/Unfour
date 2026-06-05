use crate::app_error::AppResult;
use crate::local_db::LocalDb;
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

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
}
