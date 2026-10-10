use super::*;
use std::collections::{HashMap, HashSet};
pub(super) struct ImportPlan {
    pub workspace_id: String,
    pub page: ExternalApplyPage,
    pub sql: Vec<(String, SavedSqlInput)>,
    pub flows: Vec<FlowDefinition>,
    pub ids: HashMap<String, (String, &'static str)>,
    pub local_paths: Vec<LocalPath>,
    pub api_templates: Vec<templates::ApiTemplate>,
}
fn reference(
    map: &HashMap<String, (String, &'static str)>,
    id: &str,
    kind: &str,
) -> AppResult<String> {
    map.get(id)
        .filter(|(_, actual)| *actual == kind)
        .map(|(new, _)| new.clone())
        .ok_or_else(|| AppError::Validation("WORKSPACE_BUNDLE_MISSING_REFERENCE".into()))
}
pub(super) fn prepare(bundle: WorkspaceBundle, mcp_policy: Option<&str>) -> AppResult<ImportPlan> {
    let mut ids = HashMap::new();
    let mut add = |id: &str, kind| -> AppResult<()> {
        if id.trim().is_empty()
            || id.len() > 128
            || ids
                .insert(id.to_string(), (unfour_core::id::new_id(), kind))
                .is_some()
        {
            return Err(AppError::Validation("WORKSPACE_BUNDLE_DUPLICATE_ID".into()));
        }
        Ok(())
    };
    add(&bundle.workspace.id, "workspace")?;
    macro_rules! register {
        ($records:expr, $kind:expr) => {
            for r in &$records {
                add(&r.id, $kind)?;
            }
        };
    }
    register!(bundle.variables, "variable");
    register!(bundle.environments, "environment");
    register!(bundle.environment_variables, "environmentVariable");
    register!(bundle.collections, "collection");
    register!(bundle.folders, "folder");
    register!(bundle.requests, "request");
    for c in &bundle.connections {
        add(
            &c.id,
            match c.config {
                BundleConnectionConfig::Ssh { .. } if c.connection_type == "ssh" => "sshConnection",
                BundleConnectionConfig::Database { .. } if c.connection_type == "database" => {
                    "dbConnection"
                }
                _ => return Err(invalid()),
            },
        )?;
    }
    register!(bundle.ssh_tasks, "task");
    register!(bundle.ssh_steps, "taskStep");
    register!(bundle.saved_sql, "sql");
    register!(bundle.flows, "flow");
    if ids.len() > 50_000 {
        return Err(invalid());
    }
    let workspace_id = reference(&ids, &bundle.workspace.id, "workspace")?;
    let now = unfour_workspace_engine::WorkspaceService::rfc3339_now();
    let convert = |record: Value| -> AppResult<Value> {
        let mut r = record;
        let old = r["id"].as_str().ok_or_else(invalid)?;
        r["id"] = json!(ids.get(old).ok_or_else(invalid)?.0);
        r["workspaceId"] = json!(workspace_id);
        r["createdAt"] = json!(now);
        r["updatedAt"] = json!(now);
        Ok(r)
    };
    let mut workspace = convert(serde_json::to_value(&bundle.workspace)?)?;
    workspace["mcpPolicy"] = json!(mcp_policy.unwrap_or("disabled"));
    let drivers: HashMap<_, _> = bundle
        .connections
        .iter()
        .filter_map(|c| match &c.config {
            BundleConnectionConfig::Database { driver, .. } => Some((c.id.clone(), driver.clone())),
            _ => None,
        })
        .collect();
    let mut page = ExternalApplyPage {
        workspaces: vec![ExternalWorkspaceApply::Upsert(serde_json::from_value(
            workspace,
        )?)],
        ..Default::default()
    };
    macro_rules! plain {
        ($records:expr, $field:ident, $variant:ident) => {
            for r in $records {
                page.$field
                    .push($variant::Upsert(serde_json::from_value(convert(
                        serde_json::to_value(r)?,
                    )?)?));
            }
        };
    }
    plain!(
        bundle.environments,
        workspace_environments,
        ExternalWorkspaceEnvironmentApply
    );
    plain!(
        bundle.collections,
        api_collections,
        ExternalApiCollectionApply
    );
    plain!(bundle.connections, connections, ExternalConnectionApply);
    plain!(bundle.ssh_tasks, ssh_tasks, ExternalSshTaskApply);
    let variable_value = |r: &mut Value| -> AppResult<()> {
        let value: SnapshotVariableValue = serde_json::from_value(r["value"].clone())?;
        r["value"] = serde_json::to_value(match value {
            SnapshotVariableValue::Plain(value) if r["isSecret"] == false => {
                ExternalVariableValue::Set(value)
            }
            _ => ExternalVariableValue::Clear,
        })?;
        Ok(())
    };
    for r in bundle.variables {
        let mut r = convert(serde_json::to_value(r)?)?;
        variable_value(&mut r)?;
        page.workspace_variables
            .push(ExternalWorkspaceVariableApply::Upsert(
                serde_json::from_value(r)?,
            ));
    }
    for r in bundle.environment_variables {
        let mut r = convert(serde_json::to_value(r)?)?;
        variable_value(&mut r)?;
        r["environmentId"] = json!(reference(
            &ids,
            r["environmentId"].as_str().ok_or_else(invalid)?,
            "environment"
        )?);
        page.workspace_environment_variables.push(
            ExternalWorkspaceEnvironmentVariableApply::Upsert(serde_json::from_value(r)?),
        );
    }
    let folders: HashMap<_, _> = bundle.folders.iter().map(|f| (f.id.clone(), f)).collect();
    let check_parent = |parent: &Option<String>, collection: &str| -> AppResult<()> {
        let mut current = parent.as_deref();
        let mut seen = HashSet::new();
        while let Some(id) = current {
            if seen.len() >= 128 {
                return Err(AppError::Validation("WORKSPACE_BUNDLE_FOLDER_DEPTH".into()));
            }
            if !seen.insert(id) {
                return Err(AppError::Validation("WORKSPACE_BUNDLE_FOLDER_CYCLE".into()));
            }
            let folder = folders.get(id).ok_or_else(invalid)?;
            if folder.collection_id != collection {
                return Err(invalid());
            }
            current = folder.parent_folder_id.as_deref();
        }
        Ok(())
    };
    let mut ordered = Vec::new();
    let mut emitted = HashSet::new();
    for folder in &bundle.folders {
        check_parent(&folder.parent_folder_id, &folder.collection_id)?;
    }
    while ordered.len() < bundle.folders.len() {
        for folder in &bundle.folders {
            if !emitted.contains(&folder.id)
                && folder
                    .parent_folder_id
                    .as_ref()
                    .is_none_or(|p| emitted.contains(p))
            {
                emitted.insert(folder.id.clone());
                ordered.push(folder);
            }
        }
    }
    for r in ordered {
        let mut value = convert(serde_json::to_value(r)?)?;
        value["collectionId"] = json!(reference(&ids, &r.collection_id, "collection")?);
        value["parentFolderId"] = json!(r
            .parent_folder_id
            .as_ref()
            .map(|id| reference(&ids, id, "folder"))
            .transpose()?);
        page.api_folders
            .push(ExternalApiFolderApply::Upsert(serde_json::from_value(
                value,
            )?));
    }
    for r in bundle.requests {
        check_parent(&r.parent_folder_id, &r.collection_id)?;
        let mut value = convert(serde_json::to_value(&r)?)?;
        if r.body_kind == MULTIPART_BODY_KIND {
            let mut parts = parse_multipart_definition(r.body.as_deref())?;
            for part in &mut parts {
                match part {
                    ApiMultipartPart::Text { id, .. } | ApiMultipartPart::File { id, .. } => {
                        *id = unfour_core::id::new_id()
                    }
                }
            }
            value["body"] = json!(serde_json::to_string(&parts)?);
        }
        value["collectionId"] = json!(reference(&ids, &r.collection_id, "collection")?);
        value["parentFolderId"] = json!(r
            .parent_folder_id
            .as_ref()
            .map(|id| reference(&ids, id, "folder"))
            .transpose()?);
        page.api_requests
            .push(ExternalApiRequestApply::Upsert(Box::new(
                serde_json::from_value(value)?,
            )));
    }
    for r in bundle.ssh_steps {
        let mut value = convert(serde_json::to_value(&r)?)?;
        value["taskId"] = json!(reference(&ids, &r.task_id, "task")?);
        page.ssh_task_steps
            .push(ExternalSshTaskStepApply::Upsert(serde_json::from_value(
                value,
            )?));
    }
    let mut sql = Vec::new();
    for r in bundle.saved_sql {
        sql.push((
            reference(&ids, &r.id, "sql")?,
            SavedSqlInput {
                id: None,
                workspace_id: workspace_id.clone(),
                connection_id: r
                    .connection_id
                    .map(|id| reference(&ids, &id, "dbConnection"))
                    .transpose()?,
                name: r.name,
                sql: r.sql,
                catalog: r.catalog,
                schema: r.schema,
            },
        ));
    }
    let mut flows = Vec::new();
    for r in bundle.flows {
        let mut definition = FlowDefinition {
            id: reference(&ids, &r.id, "flow")?,
            workspace_id: workspace_id.clone(),
            revision: 1,
            name: r.name,
            inputs: r.inputs,
            steps: r.steps,
        };
        unfour_flow_engine::validate_definition(&definition)?;
        let step_ids: HashMap<_, _> = definition
            .steps
            .iter()
            .map(|s| (s.id.clone(), unfour_core::id::new_id()))
            .collect();
        let edge = |id: &str| -> AppResult<String> {
            if id == "$end" {
                Ok(id.into())
            } else {
                step_ids.get(id).cloned().ok_or_else(invalid)
            }
        };
        for step in &mut definition.steps {
            step.id = edge(&step.id)?;
            step.next = step.next.as_deref().map(edge).transpose()?;
            let action = match &mut step.node {
                FlowNode::Action { action } => Some(action),
                FlowNode::Poll { probe, .. } | FlowNode::WaitUntil { probe, .. } => Some(probe),
                FlowNode::Condition {
                    if_true, if_false, ..
                } => {
                    *if_true = edge(if_true)?;
                    *if_false = edge(if_false)?;
                    None
                }
                _ => None,
            };
            if let Some(action) = action {
                let (kind, conn) = match action.capability {
                    FlowCapability::Api => ("request", None),
                    FlowCapability::Ssh => ("task", Some("sshConnection")),
                    FlowCapability::Database => ("dbConnection", None),
                };
                if conn.is_none() && action.connection_id.is_some() {
                    return Err(invalid());
                }
                if conn.is_some() && action.connection_id.is_none() {
                    return Err(AppError::Validation(
                        "WORKSPACE_BUNDLE_MISSING_REFERENCE".into(),
                    ));
                }
                if action.capability == FlowCapability::Api {
                    crate::flow_authoring::validate_api_arguments(&action.arguments, true)?;
                }
                if action.capability == FlowCapability::Database {
                    crate::flow_authoring::validate_sql_argument(
                        &action.arguments,
                        drivers.get(&action.resource_id).ok_or_else(invalid)?,
                    )?;
                }
                action.resource_id = reference(&ids, &action.resource_id, kind)?;
                if let (Some(id), Some(kind)) = (&mut action.connection_id, conn) {
                    *id = reference(&ids, id, kind)?;
                }
            }
            // Expression traversal is scoped to node payloads, never arbitrary business text IDs.
            let mut value = serde_json::to_value(&step.node)?;
            rewrite_expressions(&mut value, &step_ids)?;
            step.node = serde_json::from_value(value)?;
        }
        unfour_flow_engine::validate_definition(&definition)?;
        flows.push(definition);
    }
    Ok(ImportPlan {
        workspace_id,
        page,
        sql,
        flows,
        local_paths: bundle.local_paths,
        api_templates: bundle.api_templates,
        ids,
    })
}
fn pointer(text: &str, ids: &HashMap<String, String>) -> AppResult<String> {
    let Some(rest) = text.strip_prefix("/steps/") else {
        return Ok(text.into());
    };
    let (id, suffix) = rest
        .split_once('/')
        .map(|(id, tail)| (id, format!("/{tail}")))
        .unwrap_or((rest, String::new()));
    Ok(format!(
        "/steps/{}{}",
        ids.get(id).ok_or_else(invalid)?,
        suffix
    ))
}
fn rewrite_expressions(value: &mut Value, ids: &HashMap<String, String>) -> AppResult<()> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "$ref" {
                    *value = json!(pointer(value.as_str().ok_or_else(invalid)?, ids)?);
                } else {
                    rewrite_expressions(value, ids)?;
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_expressions(item, ids)?;
            }
        }
        Value::String(text) => {
            let mut remaining = text.as_str();
            let mut rewritten = String::new();
            while let Some(start) = remaining.find("${") {
                rewritten.push_str(&remaining[..start + 2]);
                remaining = &remaining[start + 2..];
                let end = remaining.find('}').ok_or_else(invalid)?;
                rewritten.push_str(&pointer(&remaining[..end], ids)?);
                rewritten.push('}');
                remaining = &remaining[end + 1..];
            }
            rewritten.push_str(remaining);
            *text = rewritten;
        }
        _ => {}
    }
    Ok(())
}
