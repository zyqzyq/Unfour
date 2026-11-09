//! API organization uses the same scoped domain operations as Desktop.
use super::confirmation::ensure_confirmed_if_guarded;
use super::metadata_support::*;
use super::policy::ToolPolicyEvaluation;
use super::{RegisteredTool, ToolAnnotations, ToolCallError};
use crate::command_bus_adapter::CommandBusAdapter;
use serde_json::{json, Value};
use unfour_core::models::{ApiCollectionFolder, ApiSavedRequest};

fn folder_schema() -> Value {
    result_schema(
        json!({"id":{"type":"string"},"workspaceId":{"type":"string"},"collectionId":{"type":"string"},
        "parentFolderId":{"type":["string","null"]},"name":{"type":"string"},"sortOrder":{"type":"integer"},
        "updatedAt":{"type":"string"},"revision":{"type":"integer"}}),
    )
}
pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    let id = json!({"type":"string","minLength":1});
    let parent = json!({"type":["string","null"],"minLength":1,"description":"null selects the collection root."});
    let one = result_schema(json!({"folder":folder_schema()}));
    let many = result_schema(json!({"folders":{"type":"array","items":folder_schema()}}));
    let request = result_schema(json!({"request":result_schema(json!({
        "id":{"type":"string"},"workspaceId":{"type":"string"},"collectionId":{"type":"string"},
        "parentFolderId":{"type":["string","null"]},"name":{"type":"string"},"revision":{"type":"integer"}
    }))}));
    let mut delete = json!({"workspaceId":id,"folderId":id});
    confirmation_properties(&mut delete);
    vec![
        tool("unfour.api.list_folders", "Lists API folder metadata and parent IDs in the selected workspace, optionally restricted to a collection.", json!({"workspaceId":id,"collectionId":id}), &[], many.clone(), ToolAnnotations::local_read(), list),
        tool("unfour.api.create_folder", "Creates a folder in a saved API collection. Parent must belong to that collection; null means root.", json!({"workspaceId":id,"collectionId":id,"parentFolderId":parent,"name":id}), &["collectionId","parentFolderId","name"], one.clone(), ToolAnnotations::local_write(), create),
        tool("unfour.api.rename_folder", "Renames a folder. expectedUpdatedAt from list_folders prevents overwriting a concurrent edit.", json!({"workspaceId":id,"folderId":id,"name":id,"expectedUpdatedAt":id}), &["folderId","name","expectedUpdatedAt"], one.clone(), ToolAnnotations::local_write(), rename),
        tool("unfour.api.move_folder", "Moves a folder within its collection, including to root with parentFolderId=null. Cycles and foreign workspace/collection targets are rejected.", json!({"workspaceId":id,"folderId":id,"parentFolderId":parent}), &["folderId","parentFolderId"], one, ToolAnnotations::local_write(), move_folder),
        tool("unfour.api.delete_folder", "Recursively soft-deletes a folder, descendant folders and their saved requests. Guarded policy requires confirmation bound to the current subtree and request revisions.", delete, &["folderId"], result_schema(json!({"deleted":{"const":true},"folderId":{"type":"string"}})), ToolAnnotations::local_write_destructive(), delete_folder),
        tool("unfour.api.reorder_folders", "Reorders sibling folders in one collection and parent. Supply their IDs in the desired order; no requests are executed.", json!({"workspaceId":id,"collectionId":id,"parentFolderId":parent,"folderIds":{"type":"array","items":id,"uniqueItems":true}}), &["collectionId","parentFolderId","folderIds"], many, ToolAnnotations::local_write(), reorder),
        tool("unfour.api.move_request", "Moves a saved request to a collection/folder in the same workspace. Explicit parentFolderId=null moves it to the collection root. Preserves its body, authentication and scripts without returning them.", json!({"workspaceId":id,"requestId":id,"collectionId":id,"parentFolderId":parent}), &["requestId","collectionId","parentFolderId"], request.clone(), ToolAnnotations::local_write(), move_request),
        tool("unfour.api.duplicate_request", "Duplicates the original saved request in its current collection/folder through CommandBus, preserving authentication, scripts and untruncated content. Does not send traffic or return secrets.", json!({"workspaceId":id,"requestId":id}), &["requestId"], request, ToolAnnotations::local_write(), duplicate),
    ]
}
fn folder(f: ApiCollectionFolder) -> Value {
    json!({"id":f.id,"workspaceId":f.workspace_id,"collectionId":f.collection_id,"parentFolderId":f.parent_folder_id,
        "name":safe_label(&f.name),"sortOrder":f.sort_order,"updatedAt":f.updated_at,"revision":f.revision})
}
fn request(r: ApiSavedRequest) -> Value {
    json!({"request":{"id":r.id,"workspaceId":r.workspace_id,"collectionId":r.collection_id,"parentFolderId":r.parent_folder_id,"name":safe_label(&r.name),"revision":r.revision}})
}
fn list(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(value, &["workspaceId", "collectionId"])?;
    let folders = bus
        .list_api_folders(
            e.workspace.workspace_id.clone(),
            optional(&a, "collectionId")?,
        )
        .map_err(error)?;
    Ok(json!({"folders":folders.into_iter().map(folder).collect::<Vec<_>>()}))
}
fn create(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(
        value,
        &["workspaceId", "collectionId", "parentFolderId", "name"],
    )?;
    let f = bus
        .create_api_folder(
            e.workspace.workspace_id.clone(),
            required(&a, "collectionId")?,
            nullable_required(&a, "parentFolderId")?,
            required(&a, "name")?,
        )
        .map_err(error)?;
    Ok(json!({"folder":folder(f)}))
}
fn rename(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(
        value,
        &["workspaceId", "folderId", "name", "expectedUpdatedAt"],
    )?;
    let f = bus
        .rename_api_folder(
            e.workspace.workspace_id.clone(),
            required(&a, "folderId")?,
            required(&a, "name")?,
            required(&a, "expectedUpdatedAt")?,
        )
        .map_err(error)?;
    Ok(json!({"folder":folder(f)}))
}
fn move_folder(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(value, &["workspaceId", "folderId", "parentFolderId"])?;
    let f = bus
        .move_api_folder(
            e.workspace.workspace_id.clone(),
            required(&a, "folderId")?,
            nullable_required(&a, "parentFolderId")?,
        )
        .map_err(error)?;
    Ok(json!({"folder":folder(f)}))
}
fn reorder(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(
        value,
        &["workspaceId", "collectionId", "parentFolderId", "folderIds"],
    )?;
    let ids = a
        .get("folderIds")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("folderIds"))?
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|s| !s.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| invalid("folderIds"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len() {
        return Err(invalid("folderIds"));
    }
    let folders = bus
        .reorder_api_folders(
            e.workspace.workspace_id.clone(),
            required(&a, "collectionId")?,
            nullable_required(&a, "parentFolderId")?,
            ids,
        )
        .map_err(error)?;
    Ok(json!({"folders":folders.into_iter().map(folder).collect::<Vec<_>>()}))
}
fn delete_folder(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(
        value,
        &[
            "workspaceId",
            "folderId",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let id = required(&a, "folderId")?;
    let ws = &e.workspace.workspace_id;
    let snapshot = bus
        .api_folder_delete_snapshot(ws.clone(), id.clone())
        .map_err(error)?;
    ensure_confirmed_if_guarded(
        e,
        &a,
        "API_DELETE_FOLDER",
        "Deleting this folder recursively removes its saved requests and descendants.",
        json!({"workspaceId":ws,"folderId":id,"snapshot":snapshot}),
    )?;
    bus.delete_api_folder(ws.clone(), id.clone(), snapshot)
        .map_err(error)?;
    Ok(json!({"deleted":true,"folderId":id}))
}
fn move_request(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(
        value,
        &["workspaceId", "requestId", "collectionId", "parentFolderId"],
    )?;
    bus.move_api_request(
        e.workspace.workspace_id.clone(),
        required(&a, "requestId")?,
        required(&a, "collectionId")?,
        nullable_required(&a, "parentFolderId")?,
    )
    .map(request)
    .map_err(error)
}
fn duplicate(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(value, &["workspaceId", "requestId"])?;
    bus.duplicate_api_request(e.workspace.workspace_id.clone(), required(&a, "requestId")?)
        .map(request)
        .map_err(error)
}
