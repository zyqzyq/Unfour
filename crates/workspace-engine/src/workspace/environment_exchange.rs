use super::*;
use serde_json::{json, Value};
use unfour_core::models::{
    ApiCollectionExportArtifact, WorkspaceEnvironment, WorkspaceVariableInput,
};

fn sensitive(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace('-', "_");
    unfour_core::redaction::is_sensitive_key(&key)
        || key.contains("password")
        || key.contains("secret")
        || key.contains("passphrase")
        || key.ends_with("token")
        || key.ends_with("api_key")
}

fn decode(content: &str) -> AppResult<(String, String, Vec<WorkspaceVariableInput>)> {
    if content.len() > 10 * 1024 * 1024 {
        return Err(AppError::Validation(
            "environment import file is too large".into(),
        ));
    }
    let value: Value = serde_json::from_str(content)?;
    let (format, values) = if value["format"] == "unfour.environment" && value["version"] == 1 {
        ("unfour", &value["variables"])
    } else if value["_postman_variable_scope"] == "environment" {
        ("postman", &value["values"])
    } else {
        return Err(AppError::Validation(
            "unsupported environment format or version".into(),
        ));
    };
    let name = value["name"]
        .as_str()
        .ok_or_else(|| AppError::Validation("missing environment name".into()))?
        .to_string();
    let values = values
        .as_array()
        .ok_or_else(|| AppError::Validation("invalid environment variables".into()))?;
    if values.len() > 10_000 {
        return Err(AppError::Validation(
            "too many environment variables".into(),
        ));
    }
    let variables = values
        .iter()
        .enumerate()
        .map(|(index, v)| {
            let key = v["key"]
                .as_str()
                .ok_or_else(|| AppError::Validation("invalid variable key".into()))?
                .to_string();
            let is_secret = v["isSecret"].as_bool().unwrap_or(false)
                || v["type"] == "secret"
                || sensitive(&key);
            Ok(WorkspaceVariableInput {
                id: None,
                key,
                value: if v["redacted"] == true {
                    String::new()
                } else {
                    match &v["value"] {
                        Value::String(text) => text.clone(),
                        Value::Null => String::new(),
                        scalar @ (Value::Number(_) | Value::Bool(_)) if format == "postman" => {
                            scalar.to_string()
                        }
                        _ => {
                            return Err(AppError::Validation(
                                "environment variable value must be a string".into(),
                            ))
                        }
                    }
                },
                is_secret,
                is_enabled: v[if format == "postman" {
                    "enabled"
                } else {
                    "isEnabled"
                }]
                .as_bool()
                .unwrap_or(true),
                description: v["description"].as_str().map(Into::into),
                sort_order: index as i64,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    let name = super::variables::normalize_environment_name(name)?;
    super::variables::validate_variables(&variables)?;
    Ok((format.into(), name, variables))
}

impl WorkspaceService {
    pub async fn preview_environment_import(
        &self,
        workspace_id: String,
        content: &str,
    ) -> AppResult<Value> {
        let (format, name, variables) = decode(content)?;
        let existing = self.list_environments(workspace_id).await?;
        Ok(
            json!({"format":format,"name":name,"variables":variables.iter().map(|v| json!({"key":v.key,"isSecret":v.is_secret,"isEnabled":v.is_enabled})).collect::<Vec<_>>(),"conflict":existing.iter().any(|e| e.name.eq_ignore_ascii_case(&name)),"warnings":["environmentCopy","secretExport"]}),
        )
    }

    pub async fn import_environment_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        content: &str,
    ) -> AppResult<DomainCommandResult<WorkspaceEnvironment>> {
        let (_, base, variables) = decode(content)?;
        let existing = super::variables::list_environments_on(connection, &workspace_id).await?;
        let name = unfour_core::naming::import_copy_name(
            &base,
            existing.iter().map(|e| e.name.as_str()),
            super::variables::MAX_ENVIRONMENT_NAME_CHARS,
        )?;
        let created = self
            .create_environment_on(connection, context, workspace_id.clone(), name.clone())
            .await?;
        let updated = self
            .update_environment_on(
                connection,
                context,
                workspace_id,
                created.value.id,
                name,
                variables,
            )
            .await?;
        let mut mutations = created.mutations;
        mutations.extend(updated.mutations);
        Ok(DomainCommandResult::new(updated.value, mutations))
    }

    pub async fn export_environment(
        &self,
        workspace_id: String,
        environment_id: String,
        format: String,
    ) -> AppResult<ApiCollectionExportArtifact> {
        let environment = self
            .list_environments(workspace_id)
            .await?
            .into_iter()
            .find(|e| e.id == environment_id)
            .ok_or_else(|| AppError::NotFound("workspace environment".into()))?;
        let variables = environment.variables.iter().map(|v| {
            let redacted = v.is_secret || sensitive(&v.key);
            let value = if redacted { "" } else { &v.value };
            if format=="postman" { json!({"key":v.key,"value":value,"type":if redacted {"secret"} else {"default"},"enabled":v.is_enabled,"description":v.description,"redacted":redacted}) }
            else { json!({"key":v.key,"value":value,"isSecret":redacted,"isEnabled":v.is_enabled,"description":v.description,"redacted":redacted}) }
        }).collect::<Vec<_>>();
        let document = match format.as_str() {
            "unfour" => {
                json!({"format":"unfour.environment","version":1,"name":environment.name,"variables":variables})
            }
            "postman" => {
                json!({"_postman_variable_scope":"environment","name":environment.name,"values":variables})
            }
            _ => {
                return Err(AppError::Validation(
                    "unsupported environment export format".into(),
                ))
            }
        };
        Ok(ApiCollectionExportArtifact {
            content: serde_json::to_string_pretty(&document)?,
            media_type: "application/json".into(),
            suggested_file_name: format!("environment.{format}.json"),
        })
    }
}
