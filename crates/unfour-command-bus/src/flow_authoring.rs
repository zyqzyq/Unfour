use crate::CommandBus;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use unfour_core::{models::*, AppResult};
use unfour_flow_engine::expression::{invalid, text_value};

// References are resolved by Flow immediately before execution; validate literal
// structure at save and preflight, and validate the resolved payload again.
pub(super) fn validate_api_arguments(arguments: &Value, preflight: bool) -> AppResult<()> {
    let args = arguments
        .as_object()
        .ok_or_else(|| invalid("FLOW_ARGUMENTS_OBJECT_REQUIRED"))?;
    if args.keys().any(|key| {
        ![
            "url",
            "body",
            "headers",
            "query",
            "headersPatch",
            "queryPatch",
        ]
        .contains(&key.as_str())
    }) {
        return Err(invalid("FLOW_UNKNOWN_ARGUMENT"));
    }
    for key in ["url", "body"] {
        if let Some(value) = args.get(key) {
            if !(preflight && api_reference(value)?)
                && !value.is_string()
                && !(key == "body" && value.is_null())
            {
                return Err(invalid("FLOW_API_ARGUMENT_TYPE"));
            }
        }
    }
    for (key, patch) in [("headers", "headersPatch"), ("query", "queryPatch")] {
        if args.contains_key(key) && args.contains_key(patch) {
            return Err(invalid("FLOW_API_REPLACE_PATCH_CONFLICT"));
        }
        for field in [key, patch] {
            if let Some(value) = args.get(field) {
                if preflight && api_reference(value)? {
                    continue;
                }
                let rows = value
                    .as_array()
                    .ok_or_else(|| invalid("FLOW_API_PAIRS_REQUIRED"))?;
                for row in rows {
                    if preflight && api_reference(row)? {
                        continue;
                    }
                    let object = row
                        .as_object()
                        .ok_or_else(|| invalid("FLOW_API_PAIRS_REQUIRED"))?;
                    if field == patch
                        && object.keys().any(|key| {
                            !["key", "value", "enabled"].contains(&key.as_str())
                                && !(field == "queryPatch" && key == "occurrence")
                        })
                    {
                        return Err(invalid("FLOW_API_PAIRS_REQUIRED"));
                    }
                    if let Some(occurrence) = object.get("occurrence") {
                        if field == "queryPatch" && occurrence.as_u64().is_none() {
                            return Err(invalid("FLOW_API_PAIRS_REQUIRED"));
                        }
                    }
                    for name in ["key", "value", "enabled"] {
                        let value = object
                            .get(name)
                            .ok_or_else(|| invalid("FLOW_API_PAIRS_REQUIRED"))?;
                        if preflight && api_reference(value)? {
                            continue;
                        }
                        if (name == "enabled" && !value.is_boolean())
                            || (name != "enabled" && !value.is_string())
                        {
                            return Err(invalid("FLOW_API_PAIRS_REQUIRED"));
                        }
                    }
                    if field == patch
                        && row["key"].as_str().is_some_and(|key| key.trim().is_empty())
                    {
                        return Err(invalid("FLOW_API_PATCH_KEY_REQUIRED"));
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn apply_api_arguments(request: &mut Value, arguments: &Value) -> AppResult<()> {
    validate_api_arguments(arguments, false)?;
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
            // Query rows are ordered and may repeat. Each patch consumes one
            // original occurrence; untouched duplicates retain their position.
            let mut consumed = std::collections::HashSet::new();
            let mut removed = Vec::new();
            let original = rows.clone();
            for (patch_index, item) in patches.into_iter().enumerate() {
                if key == "query" {
                    let explicit = value[patch_index].get("occurrence").and_then(Value::as_u64);
                    let index = if let Some(occurrence) = explicit {
                        original
                            .iter()
                            .enumerate()
                            .filter(|(_, row)| row["key"].as_str() == Some(&item.key))
                            .nth(occurrence as usize)
                            .map(|(i, _)| i)
                    } else {
                        original
                            .iter()
                            .enumerate()
                            .find(|(i, row)| {
                                !consumed.contains(i) && row["key"].as_str() == Some(&item.key)
                            })
                            .map(|(i, _)| i)
                    };
                    if let Some(index) = index {
                        consumed.insert(index);
                        if item.enabled {
                            removed.retain(|removed_index| *removed_index != index);
                            rows[index] = serde_json::to_value(item)?;
                        } else {
                            removed.push(index);
                        }
                    } else if item.enabled {
                        consumed.insert(rows.len());
                        rows.push(serde_json::to_value(item)?);
                    }
                    continue;
                }
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
            removed.sort_unstable();
            removed.dedup();
            for index in removed.into_iter().rev() {
                rows.remove(index);
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
        defaults.extend(
            self.flow_variables(input)
                .await?
                .into_iter()
                .map(|(key, (value, _))| (key, value)),
        );
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

impl CommandBus {
    async fn flow_variables(
        &self,
        input: &FlowRunInput,
    ) -> AppResult<BTreeMap<String, (String, bool)>> {
        let mut values = BTreeMap::new();
        for v in self
            .workspace
            .list_variables(input.workspace_id.clone())
            .await?
        {
            if v.is_enabled && v.deleted_at.is_none() {
                values.insert(v.key.trim().to_lowercase(), (v.value, v.is_secret));
            }
        }
        if let Some(id) = &input.environment_id {
            let env = self
                .workspace
                .list_environments(input.workspace_id.clone())
                .await?
                .into_iter()
                .find(|env| &env.id == id)
                .ok_or_else(|| invalid("FLOW_RESOURCE_MISSING"))?;
            for v in env.variables {
                if v.is_enabled && v.deleted_at.is_none() {
                    values.insert(v.key.trim().to_lowercase(), (v.value, v.is_secret));
                }
            }
        }
        Ok(values)
    }

    pub(super) async fn flow_ssh_secret_names(
        &self,
        action: &FlowAction,
        input: &FlowRunInput,
        inputs: &BTreeMap<String, String>,
    ) -> AppResult<Vec<String>> {
        let variables = self.flow_variables(input).await?;
        let mut secrets = Vec::new();
        for (key, value) in input.inputs.as_object().into_iter().flatten() {
            if input.secret_input_names.contains(key) || flow_sensitive(key) {
                secret_leaves(value, &mut secrets);
            }
        }
        Ok(inputs
            .iter()
            .filter(|(name, value)| {
                flow_sensitive(name)
                    || action
                        .arguments
                        .get("inputs")
                        .is_some_and(sensitive_binding)
                    || action
                        .arguments
                        .get("inputs")
                        .and_then(|v| v.get(name.as_str()))
                        .is_some_and(sensitive_binding)
                    || (action.arguments.get("workspaceDefaults") == Some(&json!(true))
                        && action
                            .arguments
                            .get("inputs")
                            .and_then(|v| v.get(name.as_str()))
                            .is_none()
                        && variables
                            .get(&name.to_lowercase())
                            .is_some_and(|(_, secret)| *secret))
                    || secrets
                        .iter()
                        .any(|secret| !secret.is_empty() && value.contains(secret))
            })
            .map(|(name, _)| name.clone())
            .collect())
    }

    pub(super) async fn flow_diagnostics(
        &self,
        mut details: Value,
        input: &FlowRunInput,
    ) -> AppResult<Value> {
        let mut secrets = Vec::new();
        for (key, (value, secret)) in self.flow_variables(input).await? {
            if secret || flow_sensitive(&key) {
                secrets.push(value);
            }
        }
        for (key, value) in input.inputs.as_object().into_iter().flatten() {
            if input.secret_input_names.contains(key) || flow_sensitive(key) {
                secret_leaves(value, &mut secrets);
            }
        }
        // Redact before truncation so a boundary never exposes a partial secret.
        unfour_flow_engine::expression::redact(&mut details);
        scrub_diagnostics(&mut details, &secrets);
        let mut truncated = false;
        for key in ["body", "headers", "log", "errorMessage"] {
            if let Some(value) = details.get_mut(key) {
                let encoded = if let Some(text) = value.as_str() {
                    text.to_owned()
                } else {
                    serde_json::to_string(value)?
                };
                if serde_json::to_vec(value)?.len() > 32768 {
                    let mut end = encoded.len().min(32768);
                    while !encoded.is_char_boundary(end) {
                        end -= 1;
                    }
                    *value = Value::String(encoded[..end].to_owned());
                    while serde_json::to_vec(value)?.len() > 32768 {
                        end /= 2;
                        while !encoded.is_char_boundary(end) {
                            end -= 1;
                        }
                        *value = Value::String(encoded[..end].to_owned());
                    }
                    truncated = true;
                    if key == "log" {
                        details["logTruncated"] = json!(true);
                    }
                }
            }
        }
        details["diagnosticsTruncated"] = json!(truncated);
        Ok(details)
    }
}

fn secret_leaves(value: &Value, secrets: &mut Vec<String>) {
    match value {
        Value::Object(values) => values.values().for_each(|v| secret_leaves(v, secrets)),
        Value::Array(values) => values.iter().for_each(|v| secret_leaves(v, secrets)),
        Value::Null => {}
        _ => secrets.push(text_value(value)),
    }
}
pub(super) fn scrub_diagnostics(value: &mut Value, secrets: &[String]) {
    match value {
        Value::Object(values) => values
            .values_mut()
            .for_each(|v| scrub_diagnostics(v, secrets)),
        Value::Array(values) => values
            .iter_mut()
            .for_each(|v| scrub_diagnostics(v, secrets)),
        Value::String(text) => {
            for secret in secrets.iter().filter(|s| !s.is_empty()) {
                *text = text.replace(secret, "<redacted>");
            }
        }
        _ => {}
    }
}

fn flow_sensitive(key: &str) -> bool {
    unfour_core::redaction::is_sensitive_flow_name(key)
}

fn api_reference(value: &Value) -> AppResult<bool> {
    if let Some(object) = value.as_object().filter(|o| o.contains_key("$ref")) {
        if object.len() != 1
            || !object["$ref"]
                .as_str()
                .is_some_and(|s| s.starts_with("/inputs/") || s.starts_with("/steps/"))
        {
            return Err(invalid("FLOW_INVALID_REFERENCE"));
        }
        return Ok(true);
    }
    Ok(false)
}

// Examine only reference paths, not arbitrary literal input text.
fn sensitive_binding(value: &Value) -> bool {
    let sensitive_path = |path: &str| {
        path.split('/')
            .skip(if path.starts_with("/steps/") { 3 } else { 2 })
            .any(|part| flow_sensitive(&part.replace("~1", "/").replace("~0", "~")))
    };
    match value {
        Value::Object(object) => object
            .get("$ref")
            .and_then(Value::as_str)
            .is_some_and(sensitive_path),
        Value::String(text) => text
            .split("${")
            .skip(1)
            .filter_map(|part| part.split_once('}'))
            .any(|(path, _)| sensitive_path(path)),
        _ => false,
    }
}

pub(super) fn auth_secrets(request: &ApiRequestInput) -> AppResult<Vec<String>> {
    let auth: Value = serde_json::from_str(request.auth_json.as_deref().unwrap_or("{}"))?;
    let field = match auth["type"].as_str() {
        Some("bearer") => "token",
        Some("basic") => "password",
        Some("api-key") => "value",
        _ => return Ok(vec![]),
    };
    let mut secrets: Vec<String> = auth[field]
        .as_str()
        .map(str::to_owned)
        .into_iter()
        .collect();
    // Auth configuration identifies the credential slot, including custom names
    // and explicit entries that took precedence during materialization.
    let api_key = auth["type"] == "api-key";
    let key = if api_key {
        auth["key"].as_str().unwrap_or("")
    } else {
        "Authorization"
    };
    let query = api_key && auth["addTo"] == "query";
    let rows = if query {
        &request.query
    } else {
        &request.headers
    };
    for row in rows.iter().filter(|row| {
        row.enabled
            && if query {
                row.key == key
            } else {
                row.key.eq_ignore_ascii_case(key)
            }
    }) {
        secrets.push(row.value.clone());
        if !api_key {
            if let Some((_, credential)) = row.value.split_once(' ') {
                secrets.push(credential.to_owned());
            }
        }
    }
    Ok(secrets)
}
