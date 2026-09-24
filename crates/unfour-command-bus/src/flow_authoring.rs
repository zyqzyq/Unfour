use crate::CommandBus;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use unfour_core::{models::*, AppResult};
use unfour_flow_engine::expression::{invalid, text_value};

pub(super) fn apply_api_arguments(request: &mut Value, arguments: &Value) -> AppResult<()> {
    let args = arguments
        .as_object()
        .ok_or_else(|| invalid("FLOW_ARGUMENTS_OBJECT_REQUIRED"))?;
    for key in ["url", "body", "headers", "query"] {
        if let Some(value) = args.get(key) {
            request[key] = value.clone();
        }
    }
    for (key, patch) in [("headers", "headersPatch"), ("query", "queryPatch")] {
        if let Some(value) = args.get(patch) {
            if args.contains_key(key) {
                return Err(invalid("FLOW_API_REPLACE_PATCH_CONFLICT"));
            }
            let patches: Vec<KeyValue> = serde_json::from_value(value.clone())?;
            let rows = request[key]
                .as_array_mut()
                .ok_or_else(|| invalid("FLOW_API_PAIRS_REQUIRED"))?;
            for item in patches {
                if item.key.trim().is_empty() {
                    return Err(invalid("FLOW_API_PATCH_KEY_REQUIRED"));
                }
                rows.retain(|row| {
                    !row["key"].as_str().is_some_and(|name| {
                        if key == "headers" {
                            name.eq_ignore_ascii_case(&item.key)
                        } else {
                            name == item.key
                        }
                    })
                });
                if item.enabled {
                    rows.push(serde_json::to_value(item)?);
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_sql_argument(arguments: &Value, driver: &str) -> AppResult<()> {
    let sql = arguments
        .get("sql")
        .ok_or_else(|| invalid("FLOW_SQL_REQUIRED"))?;
    if sql.is_object() && sql.get("$ref").and_then(Value::as_str).is_some() {
        return Ok(());
    }
    let sql = sql
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| invalid("FLOW_SQL_REQUIRED"))?;
    // Interpolation can supply identifiers or literals. Validate the surrounding
    // statement now; the owning engine validates the actual resolved SQL again.
    let mut remaining = sql;
    let mut template = String::new();
    while let Some(start) = remaining.find("${") {
        template.push_str(&remaining[..start]);
        let end = remaining[start..]
            .find('}')
            .ok_or_else(|| invalid("FLOW_INVALID_INTERPOLATION"))?
            + start;
        template.push('1');
        remaining = &remaining[end + 1..];
    }
    template.push_str(remaining);
    unfour_database_engine::DatabaseService::validate_single_statement(&template, driver)
}

impl CommandBus {
    pub(super) async fn flow_ssh_defaults(
        &self,
        action: &FlowAction,
        input: &FlowRunInput,
    ) -> AppResult<BTreeMap<String, String>> {
        let mut defaults = BTreeMap::new();
        if action.arguments.get("workspaceDefaults") != Some(&json!(true)) {
            return Ok(defaults);
        }
        for variable in self
            .workspace
            .list_variables(input.workspace_id.clone())
            .await?
        {
            if variable.is_enabled && variable.deleted_at.is_none() {
                defaults.insert(variable.key.trim().to_lowercase(), variable.value);
            }
        }
        if let Some(id) = input.environment_id.as_ref() {
            let environments = self
                .workspace
                .list_environments(input.workspace_id.clone())
                .await?;
            let env = environments
                .into_iter()
                .find(|env| &env.id == id)
                .ok_or_else(|| invalid("FLOW_RESOURCE_MISSING"))?;
            for variable in env.variables {
                if variable.is_enabled && variable.deleted_at.is_none() {
                    defaults.insert(variable.key.trim().to_lowercase(), variable.value);
                }
            }
        }
        Ok(defaults)
    }
}

pub(super) fn ssh_inputs(
    arguments: &Value,
    names: &[String],
    defaults: &BTreeMap<String, String>,
    preflight: bool,
) -> AppResult<BTreeMap<String, String>> {
    let empty = json!({});
    let values = arguments.get("inputs").unwrap_or(&empty);
    if preflight && values.get("$ref").is_some() {
        return Ok(BTreeMap::new());
    }
    let object = values
        .as_object()
        .ok_or_else(|| invalid("FLOW_INPUTS_OBJECT_REQUIRED"))?;
    let mut result: BTreeMap<String, String> = names
        .iter()
        .filter_map(|name| {
            defaults
                .get(&name.to_lowercase())
                .map(|value| (name.clone(), value.clone()))
        })
        .collect();
    result.extend(
        object
            .iter()
            .map(|(name, value)| (name.clone(), text_value(value))),
    );
    if names
        .iter()
        .any(|name| result.get(name).is_none_or(|value| value.trim().is_empty()))
    {
        return Err(invalid("FLOW_SSH_INPUT_REQUIRED"));
    }
    Ok(result)
}
