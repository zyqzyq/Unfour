use crate::CommandBus;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use unfour_core::{models::FlowAction, AppError, AppResult};
use unfour_flow_engine::expression::invalid;

fn text(value: &Value) -> AppResult<&[u8]> {
    value
        .as_str()
        .map(str::as_bytes)
        .ok_or_else(|| invalid("FLOW_INVALID_REDIS_CHECK"))
}

impl CommandBus {
    pub(super) async fn flow_redis_resource(
        &self,
        workspace: &str,
        resource: &str,
    ) -> AppResult<Value> {
        let mut db = self.db.pool().acquire().await?;
        let (connection, _) =
            unfour_redis_engine::connections::get_on(&mut db, workspace, resource)
                .await
                .map_err(|error| match error {
                    AppError::NotFound(_) => invalid("FLOW_RESOURCE_MISSING"),
                    other => other,
                })?;
        Ok(json!({"id": connection.id, "revision": connection.revision}))
    }

    pub(super) async fn execute_redis_check(
        &self,
        workspace: &str,
        action: &FlowAction,
        revision: i64,
        cancel: CancellationToken,
    ) -> AppResult<Value> {
        let args = &action.arguments;
        let key = text(&args["key"])?;
        let field = args.get("field").map(text).transpose()?;
        let equals = args.get("equals").map(text).transpose()?;
        // Dropping this read-only check closes the request-scoped socket.
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(invalid("FLOW_CANCELLED")),
            result = self.check_redis_key_at_revision(workspace, &action.resource_id, key, field, equals, Some(revision)) =>
                Ok(serde_json::to_value(result.map_err(|error| match error {
                    AppError::RowConflict(_) => invalid("FLOW_RESOURCE_CHANGED"),
                    other => other,
                })?)?),
        }
    }
}
