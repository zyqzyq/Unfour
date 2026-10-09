use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiTemplate {
    pub entity_id: String,
    pub field: String,
    pub pointer: String,
    pub value: String,
}

pub(super) fn apply(bundle: &mut WorkspaceBundle) -> AppResult<()> {
    let mut seen = std::collections::HashSet::new();
    for template in &bundle.api_templates {
        if !unfour_http_engine::is_bundle_variable_template(&template.value)
            || !seen.insert((&template.entity_id, &template.field, &template.pointer))
        {
            return Err(invalid());
        }
        let request = bundle
            .requests
            .iter_mut()
            .find(|r| r.id == template.entity_id)
            .ok_or_else(invalid)?;
        let mut value = match template.field.as_str() {
            "auth" => serde_json::from_str::<Value>(&request.auth_json)?,
            "headers" => serde_json::to_value(&request.headers)?,
            "query" => serde_json::to_value(&request.query)?,
            _ => return Err(invalid()),
        };
        *value
            .pointer_mut(&template.pointer)
            .filter(|v| v.is_string())
            .ok_or_else(invalid)? = json!(template.value);
        match template.field.as_str() {
            "auth" => request.auth_json = serde_json::to_string(&value)?,
            "headers" => request.headers = serde_json::from_value(value)?,
            "query" => request.query = serde_json::from_value(value)?,
            _ => unreachable!(),
        }
    }
    Ok(())
}

fn collect(id: &str, field: &str, pointer: &str, value: &Value, out: &mut Vec<ApiTemplate>) {
    match value {
        Value::String(value) if unfour_http_engine::is_bundle_variable_template(value) => {
            out.push(ApiTemplate {
                entity_id: id.into(),
                field: field.into(),
                pointer: pointer.into(),
                value: value.clone(),
            })
        }
        Value::Object(map) => {
            for (key, value) in map {
                collect(
                    id,
                    field,
                    &format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1")),
                    value,
                    out,
                );
            }
        }
        Value::Array(items) => {
            for (i, value) in items.iter().enumerate() {
                collect(id, field, &format!("{pointer}/{i}"), value, out);
            }
        }
        _ => {}
    }
}

impl CommandBus {
    pub(super) async fn collect_bundle_templates_on(
        &self,
        db: &mut sqlx::SqliteConnection,
        bundle: &mut WorkspaceBundle,
    ) -> AppResult<()> {
        for raw in self
            .api_client
            .bundle_requests_on(&mut *db, &bundle.workspace.id)
            .await?
        {
            for (field, value) in [
                ("auth", raw.auth_json),
                ("headers", raw.headers_json),
                ("query", raw.query_json),
            ] {
                collect(
                    &raw.id,
                    field,
                    "",
                    &serde_json::from_str(&value)?,
                    &mut bundle.api_templates,
                );
            }
        }
        apply(bundle)
    }
}
