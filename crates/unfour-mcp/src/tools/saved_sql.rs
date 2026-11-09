use super::confirmation::ensure_confirmed_if_guarded;
use super::connection_diagnostics::safe_sql;
use super::metadata_support::*;
use super::policy::ToolPolicyEvaluation;
use super::{RegisteredTool, ToolAnnotations, ToolCallError};
use crate::command_bus_adapter::CommandBusAdapter;
use serde_json::{json, Map, Value};
use unfour_core::models::{SavedSql, SavedSqlInput};

fn summary_schema() -> Value {
    result_schema(
        json!({"id":{"type":"string"},"workspaceId":{"type":"string"},"name":{"type":"string"},
        "connectionId":{"type":["string","null"]},"catalog":{"type":["string","null"]},"schema":{"type":["string","null"]},
        "revision":{"type":"integer"},"updatedAt":{"type":"string"}}),
    )
}
pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    let id = json!({"type":"string","minLength":1});
    let nullable = json!({"type":["string","null"],"minLength":1});
    let revision = json!({"type":"integer","minimum":1});
    let mut detail = summary_schema();
    detail["properties"]["sql"] = json!({"type":"string"});
    detail["properties"]["sqlRedacted"] = json!({"type":"boolean"});
    detail["required"]
        .as_array_mut()
        .unwrap()
        .extend([json!("sql"), json!("sqlRedacted")]);
    let mut delete_properties =
        json!({"workspaceId":id,"savedSqlId":id,"expectedRevision":revision});
    confirmation_properties(&mut delete_properties);
    vec![
        tool("unfour.db.list_saved_sql", "Lists saved SQL metadata in the selected workspace without executing or returning SQL.", json!({"workspaceId":id}), &[], result_schema(json!({"savedSql":{"type":"array","items":summary_schema()}})), ToolAnnotations::local_read(), list),
        tool("unfour.db.get_saved_sql", "Reads a saved SQL diagnostic projection. Literals and sensitive constructs are masked; sqlRedacted indicates changes or truncation. Never execute or save the masked projection as original SQL.", json!({"workspaceId":id,"savedSqlId":id}), &["savedSqlId"], result_schema(json!({"savedSql":detail})), ToolAnnotations::local_read(), get),
        tool("unfour.db.save_saved_sql", "Creates or updates saved SQL metadata; never executes SQL. Creation requires name and sql. Updates require savedSqlId and expectedRevision; omitted fields preserve originals, including SQL. Null clears optional context, subject to the engine's default catalog inference. Returned metadata never contains SQL.", json!({"workspaceId":id,"savedSqlId":id,"expectedRevision":revision,"name":id,"sql":id,"connectionId":nullable,"catalog":nullable,"schema":nullable}), &[], result_schema(json!({"savedSql":summary_schema()})), ToolAnnotations::local_write(), save),
        tool("unfour.db.delete_saved_sql", "Deletes a saved SQL snippet, never database tables or rows. Requires expectedRevision; guarded policy requires content-bound confirmation. Concurrent changes are rejected atomically.", delete_properties, &["savedSqlId","expectedRevision"], result_schema(json!({"deleted":{"const":true},"savedSqlId":{"type":"string"}})), ToolAnnotations::local_write_destructive(), delete),
        tool("unfour.db.list_catalogs", "Lists catalogs for a saved database connection through CommandBus. The connection must belong to the selected workspace; accepts no ad-hoc credentials.", json!({"workspaceId":id,"connectionId":id}), &["connectionId"], result_schema(json!({"connectionId":{"type":"string"},"catalogs":{"type":"array","items":{"type":"string"}}})), ToolAnnotations::remote_read(), catalogs),
    ]
}
fn summary(s: &SavedSql) -> Value {
    json!({"id":s.id,"workspaceId":s.workspace_id,"name":safe_label(&s.name),"connectionId":s.connection_id,
        "catalog":s.catalog.as_deref().map(safe_label),"schema":s.schema.as_deref().map(safe_label),
        "revision":s.revision,"updatedAt":s.updated_at})
}
fn find(bus: &dyn CommandBusAdapter, workspace: &str, id: &str) -> Result<SavedSql, ToolCallError> {
    bus.list_saved_sql(workspace.to_owned())
        .map_err(error)?
        .into_iter()
        .find(|s| s.id == id && s.workspace_id == workspace)
        .ok_or_else(missing)
}
fn expected(a: &Map<String, Value>) -> Result<i64, ToolCallError> {
    a.get("expectedRevision")
        .and_then(Value::as_i64)
        .filter(|r| *r > 0)
        .ok_or_else(|| invalid("expectedRevision"))
}
fn check_revision(s: &SavedSql, revision: i64) -> Result<(), ToolCallError> {
    if s.revision != revision {
        return Err(ToolCallError::Execution {
            code: "SAVED_SQL_REVISION_CONFLICT",
            message: "Saved SQL changed. Reload before retrying.",
        });
    }
    Ok(())
}
fn list(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    args(value, &["workspaceId"])?;
    Ok(
        json!({"savedSql":bus.list_saved_sql(e.workspace.workspace_id.clone()).map_err(error)?.iter().map(summary).collect::<Vec<_>>()}),
    )
}
fn get(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(value, &["workspaceId", "savedSqlId"])?;
    let s = find(bus, &e.workspace.workspace_id, &required(&a, "savedSqlId")?)?;
    let sql = safe_sql(&s.sql);
    let mut detail = summary(&s);
    detail["sqlRedacted"] = json!(sql != s.sql);
    detail["sql"] = json!(sql);
    Ok(json!({"savedSql":detail}))
}
fn save(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(
        value,
        &[
            "workspaceId",
            "savedSqlId",
            "expectedRevision",
            "name",
            "sql",
            "connectionId",
            "catalog",
            "schema",
        ],
    )?;
    let id = optional(&a, "savedSqlId")?;
    if a.contains_key("savedSqlId") {
        required(&a, "savedSqlId")?;
    }
    let existing = id
        .as_deref()
        .map(|id| find(bus, &e.workspace.workspace_id, id))
        .transpose()?;
    let revision = if let Some(s) = &existing {
        let r = expected(&a)?;
        check_revision(s, r)?;
        Some(r)
    } else {
        if a.contains_key("expectedRevision") {
            return Err(invalid("expectedRevision"));
        }
        None
    };
    let field = |key: &str, current: Option<String>| {
        if a.contains_key(key) {
            optional(&a, key)
        } else {
            Ok(current)
        }
    };
    let name = if a.contains_key("name") {
        required(&a, "name")?
    } else {
        existing
            .as_ref()
            .map(|s| s.name.clone())
            .ok_or_else(|| invalid("name"))?
    };
    let sql = if a.contains_key("sql") {
        required(&a, "sql")?
    } else {
        existing
            .as_ref()
            .map(|s| s.sql.clone())
            .ok_or_else(|| invalid("sql"))?
    };
    let input = SavedSqlInput {
        id,
        workspace_id: e.workspace.workspace_id.clone(),
        name,
        sql,
        connection_id: field(
            "connectionId",
            existing.as_ref().and_then(|s| s.connection_id.clone()),
        )?,
        catalog: field("catalog", existing.as_ref().and_then(|s| s.catalog.clone()))?,
        schema: field("schema", existing.as_ref().and_then(|s| s.schema.clone()))?,
    };
    let saved = bus.save_saved_sql(input, revision).map_err(error)?;
    Ok(json!({"savedSql":summary(&saved)}))
}
fn delete(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(
        value,
        &[
            "workspaceId",
            "savedSqlId",
            "expectedRevision",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let id = required(&a, "savedSqlId")?;
    let revision = expected(&a)?;
    let saved = find(bus, &e.workspace.workspace_id, &id)?;
    check_revision(&saved, revision)?;
    ensure_confirmed_if_guarded(
        e,
        &a,
        "DELETE_SAVED_SQL",
        "Deleting saved SQL removes the local snippet.",
        json!({"workspaceId":e.workspace.workspace_id,"savedSqlId":id,"revision":revision}),
    )?;
    bus.delete_saved_sql(e.workspace.workspace_id.clone(), id.clone(), revision)
        .map_err(error)?;
    Ok(json!({"deleted":true,"savedSqlId":id}))
}
fn catalogs(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(value, &["workspaceId", "connectionId"])?;
    let id = required(&a, "connectionId")?;
    let catalogs = bus
        .list_database_catalogs(e.workspace.workspace_id.clone(), id.clone())
        .map_err(error)?;
    Ok(
        json!({"connectionId":id,"catalogs":catalogs.iter().map(|s| safe_label(s)).collect::<Vec<_>>()}),
    )
}
