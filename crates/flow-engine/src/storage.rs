use crate::{
    expression::{invalid, redact},
    validation, FlowService,
};
use serde_json::Value;
use sqlx::Row;
use unfour_core::{models::*, AppError, AppResult};

impl FlowService {
    pub async fn list_summaries(&self, workspace: &str) -> AppResult<Vec<FlowSummary>> {
        // Project metadata without deserializing steps or inputs.
        let rows = sqlx::query("SELECT id, workspace_id, revision, json_extract(definition_json, '$.name') AS name FROM flow_definitions WHERE workspace_id = ? ORDER BY updated_at DESC")
            .bind(workspace).fetch_all(self.db.pool()).await?;
        rows.iter()
            .map(|row| {
                Ok(FlowSummary {
                    id: row.try_get("id")?,
                    workspace_id: row.try_get("workspace_id")?,
                    name: row.try_get("name")?,
                    revision: row.try_get("revision")?,
                })
            })
            .collect()
    }

    pub async fn list(&self, workspace: &str) -> AppResult<Vec<FlowDefinition>> {
        let rows: Vec<String> = sqlx::query_scalar("SELECT definition_json FROM flow_definitions WHERE workspace_id = ? ORDER BY updated_at DESC").bind(workspace).fetch_all(self.db.pool()).await?;
        rows.iter()
            .map(|row| Ok(serde_json::from_str(row)?))
            .collect()
    }
    pub async fn get(&self, workspace: &str, id: &str) -> AppResult<FlowDefinition> {
        let row: String = sqlx::query_scalar(
            "SELECT definition_json FROM flow_definitions WHERE workspace_id = ? AND id = ?",
        )
        .bind(workspace)
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?
        .ok_or_else(|| AppError::NotFound("flow".into()))?;
        Ok(serde_json::from_str(&row)?)
    }
    pub async fn save(&self, mut definition: FlowDefinition) -> AppResult<FlowDefinition> {
        validation::validate(&definition)?;
        // Definitions must use resource credential references, never inline secrets.
        let original = serde_json::to_value(&definition)?;
        let mut cleaned = original.clone();
        crate::expression::redact_definition(&mut cleaned);
        if original != cleaned {
            return Err(invalid("FLOW_INLINE_SECRET_NOT_ALLOWED"));
        }
        let now = chrono::Utc::now().to_rfc3339();
        if definition.id.is_empty() {
            definition.id = unfour_core::id::new_id();
            definition.revision = 1;
            sqlx::query("INSERT INTO flow_definitions (id, workspace_id, revision, definition_json, updated_at) VALUES (?, ?, ?, ?, ?)").bind(&definition.id).bind(&definition.workspace_id).bind(definition.revision).bind(serde_json::to_string(&definition)?).bind(now).execute(self.db.pool()).await?;
        } else {
            let previous = definition.revision;
            definition.revision += 1;
            let result = sqlx::query("UPDATE flow_definitions SET revision = ?, definition_json = ?, updated_at = ? WHERE workspace_id = ? AND id = ? AND revision = ?").bind(definition.revision).bind(serde_json::to_string(&definition)?).bind(now).bind(&definition.workspace_id).bind(&definition.id).bind(previous).execute(self.db.pool()).await?;
            if result.rows_affected() != 1 {
                return Err(invalid("FLOW_REVISION_CONFLICT"));
            }
        }
        Ok(definition)
    }
    pub async fn delete(&self, workspace: &str, id: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM flow_definitions WHERE workspace_id = ? AND id = ?")
            .bind(workspace)
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }
    pub async fn list_runs(&self, workspace: &str, flow: &str) -> AppResult<Vec<FlowRunSummary>> {
        self.recover_stale(workspace).await?;
        let rows = sqlx::query("SELECT id, flow_id, status, started_at, finished_at FROM flow_runs WHERE workspace_id = ? AND flow_id = ? ORDER BY started_at DESC LIMIT 100").bind(workspace).bind(flow).fetch_all(self.db.pool()).await?;
        rows.iter()
            .map(|row| {
                Ok(FlowRunSummary {
                    id: row.try_get("id")?,
                    flow_id: row.try_get("flow_id")?,
                    status: serde_json::from_value(Value::String(row.try_get("status")?))?,
                    started_at: row.try_get("started_at")?,
                    finished_at: row.try_get("finished_at")?,
                })
            })
            .collect()
    }
    pub async fn get_run(&self, workspace: &str, id: &str) -> AppResult<FlowRun> {
        self.recover_stale(workspace).await?;
        let row: String =
            sqlx::query_scalar("SELECT run_json FROM flow_runs WHERE workspace_id = ? AND id = ?")
                .bind(workspace)
                .bind(id)
                .fetch_optional(self.db.pool())
                .await?
                .ok_or_else(|| AppError::NotFound("flow run".into()))?;
        decode_run(&row)
    }
    pub async fn cancel(&self, workspace: &str, id: &str) -> AppResult<FlowRun> {
        sqlx::query("UPDATE flow_runs SET cancel_requested = 1 WHERE workspace_id = ? AND id = ? AND status = 'running'").bind(workspace).bind(id).execute(self.db.pool()).await?;
        self.get_run(workspace, id).await
    }
    pub(crate) async fn heartbeat(&self, run: &FlowRun) -> AppResult<bool> {
        let cancel: Option<i64> = sqlx::query_scalar("UPDATE flow_runs SET updated_at = ? WHERE workspace_id = ? AND id = ? AND status = 'running' RETURNING cancel_requested").bind(chrono::Utc::now().to_rfc3339()).bind(&run.workspace_id).bind(&run.id).fetch_optional(self.db.pool()).await?;
        Ok(cancel != Some(0))
    }
    async fn recover_stale(&self, workspace: &str) -> AppResult<()> {
        // A per-run heartbeat is safe across desktop and future satellite processes.
        let cutoff = (chrono::Utc::now() - chrono::Duration::seconds(30)).to_rfc3339();
        let finished_at = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE flow_runs SET finished_at = ?, status = 'interrupted', run_json = json_set(run_json, '$.status', 'interrupted', '$.error', 'FLOW_INTERRUPTED', '$.finishedAt', ?) WHERE workspace_id = ? AND status = 'running' AND updated_at < ?").bind(&finished_at).bind(&finished_at).bind(workspace).bind(cutoff).execute(self.db.pool()).await?;
        Ok(())
    }
    pub(crate) async fn insert_run(&self, run: &FlowRun) -> AppResult<()> {
        sqlx::query("INSERT INTO flow_runs (id, workspace_id, flow_id, status, run_json, started_at, updated_at, finished_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)").bind(&run.id).bind(&run.workspace_id).bind(&run.flow_id).bind(run.status.as_str()).bind(safe_run(run)?).bind(&run.started_at).bind(&run.started_at).bind(&run.finished_at).execute(self.db.pool()).await?;
        Ok(())
    }
    pub(crate) async fn persist_run(&self, run: &FlowRun) -> AppResult<()> {
        let result = sqlx::query("UPDATE flow_runs SET status = ?, run_json = ?, finished_at = ?, updated_at = ? WHERE workspace_id = ? AND id = ? AND status = 'running'").bind(run.status.as_str()).bind(safe_run(run)?).bind(&run.finished_at).bind(chrono::Utc::now().to_rfc3339()).bind(&run.workspace_id).bind(&run.id).execute(self.db.pool()).await?;
        if result.rows_affected() == 0 {
            return Err(invalid("FLOW_RUN_LEASE_LOST"));
        }
        Ok(())
    }
}
fn decode_run(row: &str) -> AppResult<FlowRun> {
    let mut run: FlowRun = serde_json::from_str(row)?;
    if run.status == FlowRunStatus::Interrupted {
        for step in &mut run.steps {
            if step.status == FlowStepRunStatus::Running {
                step.status = FlowStepRunStatus::Interrupted;
                step.next_check_at = None;
                step.error = Some("FLOW_INTERRUPTED".into());
            } else if step.status == FlowStepRunStatus::Pending {
                step.status = FlowStepRunStatus::Skipped;
            }
        }
    }
    Ok(run)
}
fn safe_run(run: &FlowRun) -> AppResult<String> {
    let mut value: Value = serde_json::to_value(run)?;
    // Invalid invocation payloads have no reliable field boundaries for secret
    // names. Retain only the validation error, never their raw contents.
    if !run.context.inputs.is_object() {
        value["context"]["inputs"] = Value::String("<redacted>".into());
    }
    let mut secrets = Vec::new();
    // Schema flags such as inputs[].secret are metadata, not runtime secrets.
    for key in ["context", "resources", "steps"] {
        collect_secrets(&value[key], &mut secrets);
    }
    for key in &run.context.secret_input_names {
        if let Some(secret) = run.context.inputs.get(key) {
            collect_secret_leaves(secret, &mut secrets);
            value["context"]["inputs"][key] = Value::String("<redacted>".into());
        }
    }
    let input_schema = value["definition"]["inputs"].take();
    redact(&mut value);
    value["definition"]["inputs"] = input_schema;
    scrub_values(&mut value["context"]["inputs"], &secrets);
    scrub_values(&mut value["resources"], &secrets);
    if let Some(steps) = value["steps"].as_array_mut() {
        for step in steps {
            scrub_values(&mut step["output"], &secrets);
            if let Some(attempts) = step["attempts"].as_array_mut() {
                for attempt in attempts {
                    scrub_values(&mut attempt["input"], &secrets);
                    scrub_values(&mut attempt["output"], &secrets);
                }
            }
        }
    }
    Ok(serde_json::to_string(&value)?)
}

fn collect_secrets(value: &Value, secrets: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("api-key")
                || map
                    .get("key")
                    .and_then(Value::as_str)
                    .is_some_and(crate::expression::sensitive)
            {
                if let Some(value) = map.get("value") {
                    collect_secret_leaves(value, secrets);
                }
            }
            for (key, value) in map {
                if crate::expression::sensitive(key) {
                    collect_secret_leaves(value, secrets);
                } else {
                    collect_secrets(value, secrets);
                }
            }
        }
        Value::Array(values) => values.iter().for_each(|v| collect_secrets(v, secrets)),
        Value::String(text) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                if parsed.is_object() || parsed.is_array() {
                    collect_secrets(&parsed, secrets);
                }
            }
        }
        _ => {}
    }
}
fn collect_secret_leaves(value: &Value, secrets: &mut Vec<String>) {
    match value {
        Value::Object(map) => map.values().for_each(|v| collect_secret_leaves(v, secrets)),
        Value::Array(items) => items.iter().for_each(|v| collect_secret_leaves(v, secrets)),
        Value::Null => {}
        _ => secrets.push(crate::expression::text_value(value)),
    }
}
fn scrub_values(value: &mut Value, secrets: &[String]) {
    match value {
        Value::Object(map) => {
            for value in map.values_mut() {
                scrub_values(value, secrets);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(|v| scrub_values(v, secrets)),
        Value::String(text) => {
            for secret in secrets
                .iter()
                .filter(|s| !s.is_empty() && s.as_str() != "<redacted>")
            {
                *text = text.replace(secret, "<redacted>");
            }
        }
        Value::Number(_) | Value::Bool(_) if secrets.contains(&value.to_string()) => {
            *value = Value::String("<redacted>".into());
        }
        _ => {}
    }
}
