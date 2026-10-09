use super::*;
use crate::transaction::CommandActivity;
use unfour_core::domain::CommandContext;

impl CommandBus {
    pub async fn workspace_environment_import_preview(
        &self,
        workspace_id: String,
        content: &str,
    ) -> AppResult<serde_json::Value> {
        self.workspace
            .preview_environment_import(workspace_id, content)
            .await
    }

    pub async fn workspace_environment_export(
        &self,
        workspace_id: String,
        environment_id: String,
        format: String,
    ) -> AppResult<ApiCollectionExportArtifact> {
        self.workspace
            .export_environment(workspace_id, environment_id, format)
            .await
    }

    pub async fn workspace_environment_import(
        &self,
        workspace_id: String,
        content: String,
    ) -> AppResult<WorkspaceEnvironment> {
        let context = CommandContext::local("workspace.environment.import");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: Some(workspace_id.clone()),
                action: "workspace.environment.import",
                target: None,
                details: serde_json::json!({"contentBytes":content.len()}),
            }),
            move |connection| {
                Box::pin(async move {
                    service
                        .import_environment_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            &content,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_variables_list(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<WorkspaceVariable>> {
        self.workspace.list_variables(workspace_id).await
    }

    pub async fn workspace_variables_replace(
        &self,
        workspace_id: String,
        variables: Vec<WorkspaceVariableInput>,
    ) -> AppResult<Vec<WorkspaceVariable>> {
        let context = CommandContext::local("workspace.variables.replace");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        let variable_count = variables.len();
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: Some(activity_workspace_id.clone()),
                action: "workspace.variables.update",
                target: Some(activity_workspace_id),
                details: serde_json::json!({ "variableCount": variable_count }),
            }),
            move |connection| {
                Box::pin(async move {
                    service
                        .replace_variables_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            variables,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_variable_create(
        &self,
        workspace_id: String,
        input: WorkspaceVariableInput,
    ) -> AppResult<WorkspaceVariable> {
        let context = CommandContext::local("workspace.variable.create");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command(
            context,
            Some(entity_activity(
                &activity_workspace_id,
                "workspace.variable.create",
                None,
            )),
            move |connection| {
                Box::pin(async move {
                    service
                        .create_variable_on(connection, &executor_context, workspace_id, input)
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_variable_update(
        &self,
        workspace_id: String,
        variable_id: String,
        mut input: WorkspaceVariableInput,
    ) -> AppResult<WorkspaceVariable> {
        input.id = Some(variable_id.clone());
        let context = CommandContext::local("workspace.variable.update");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_variable_id = variable_id.clone();
        self.execute_domain_command(
            context,
            Some(entity_activity(
                &activity_workspace_id,
                "workspace.variable.update",
                Some(activity_variable_id),
            )),
            move |connection| {
                Box::pin(async move {
                    service
                        .update_variable_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            variable_id,
                            input,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_variable_delete(
        &self,
        workspace_id: String,
        variable_id: String,
    ) -> AppResult<Vec<WorkspaceVariable>> {
        let context = CommandContext::local("workspace.variable.delete");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_variable_id = variable_id.clone();
        self.execute_domain_command(
            context,
            Some(entity_activity(
                &activity_workspace_id,
                "workspace.variable.delete",
                Some(activity_variable_id),
            )),
            move |connection| {
                Box::pin(async move {
                    service
                        .delete_variable_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            variable_id,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_environments_list(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<WorkspaceEnvironment>> {
        self.workspace.list_environments(workspace_id).await
    }

    pub async fn workspace_environment_create(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<WorkspaceEnvironment> {
        let context = CommandContext::local("workspace.environment.create");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command(
            context,
            Some(entity_activity(
                &activity_workspace_id,
                "workspace.environment.create",
                None,
            )),
            move |connection| {
                Box::pin(async move {
                    service
                        .create_environment_on(connection, &executor_context, workspace_id, name)
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_environment_update(
        &self,
        workspace_id: String,
        environment_id: String,
        name: String,
        variables: Vec<WorkspaceVariableInput>,
    ) -> AppResult<WorkspaceEnvironment> {
        let context = CommandContext::local("workspace.environment.update");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_environment_id = environment_id.clone();
        let variable_count = variables.len();
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "workspace.environment.update",
                target: Some(activity_environment_id),
                details: serde_json::json!({ "variableCount": variable_count }),
            }),
            move |connection| {
                Box::pin(async move {
                    service
                        .update_environment_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            environment_id,
                            name,
                            variables,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_environment_update_metadata(
        &self,
        workspace_id: String,
        environment_id: String,
        name: String,
        sort_order: i64,
    ) -> AppResult<WorkspaceEnvironment> {
        let context = CommandContext::local("workspace.environment.metadata.update");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_environment_id = environment_id.clone();
        self.execute_domain_command(
            context,
            Some(entity_activity(
                &activity_workspace_id,
                "workspace.environment.metadata.update",
                Some(activity_environment_id),
            )),
            move |connection| {
                Box::pin(async move {
                    service
                        .update_environment_metadata_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            environment_id,
                            name,
                            sort_order,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_environments_reorder(
        &self,
        workspace_id: String,
        environment_ids: Vec<String>,
    ) -> AppResult<Vec<WorkspaceEnvironment>> {
        let context = CommandContext::local("workspace.environments.reorder");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        let count = environment_ids.len();
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: Some(activity_workspace_id.clone()),
                action: "workspace.environments.reorder",
                target: Some(activity_workspace_id),
                details: serde_json::json!({ "environmentCount": count }),
            }),
            move |connection| {
                Box::pin(async move {
                    service
                        .reorder_environments_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            environment_ids,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_environment_delete(
        &self,
        workspace_id: String,
        environment_id: String,
    ) -> AppResult<Vec<WorkspaceEnvironment>> {
        let context = CommandContext::local("workspace.environment.delete");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_environment_id = environment_id.clone();
        self.execute_domain_command(
            context,
            Some(entity_activity(
                &activity_workspace_id,
                "workspace.environment.delete",
                Some(activity_environment_id),
            )),
            move |connection| {
                Box::pin(async move {
                    service
                        .delete_environment_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            environment_id,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_environment_set_active(
        &self,
        workspace_id: String,
        environment_id: Option<String>,
    ) -> AppResult<Vec<WorkspaceEnvironment>> {
        let context = CommandContext::local("workspace.environment.activate");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        self.execute_domain_command(context, None, move |connection| {
            Box::pin(async move {
                service
                    .set_active_environment_on(
                        connection,
                        &executor_context,
                        workspace_id,
                        environment_id,
                    )
                    .await
            })
        })
        .await
    }

    pub async fn workspace_environment_variable_create(
        &self,
        workspace_id: String,
        environment_id: String,
        input: WorkspaceVariableInput,
    ) -> AppResult<WorkspaceEnvironmentVariable> {
        let context = CommandContext::local("workspace.environment_variable.create");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command(
            context,
            Some(entity_activity(
                &activity_workspace_id,
                "workspace.environment_variable.create",
                None,
            )),
            move |connection| {
                Box::pin(async move {
                    service
                        .create_environment_variable_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            environment_id,
                            input,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_environment_variable_update(
        &self,
        workspace_id: String,
        environment_id: String,
        variable_id: String,
        mut input: WorkspaceVariableInput,
    ) -> AppResult<WorkspaceEnvironmentVariable> {
        input.id = Some(variable_id.clone());
        let context = CommandContext::local("workspace.environment_variable.update");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_variable_id = variable_id.clone();
        self.execute_domain_command(
            context,
            Some(entity_activity(
                &activity_workspace_id,
                "workspace.environment_variable.update",
                Some(activity_variable_id),
            )),
            move |connection| {
                Box::pin(async move {
                    service
                        .update_environment_variable_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            environment_id,
                            variable_id,
                            input,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_environment_variables_replace(
        &self,
        workspace_id: String,
        environment_id: String,
        variables: Vec<WorkspaceVariableInput>,
    ) -> AppResult<Vec<WorkspaceEnvironmentVariable>> {
        let context = CommandContext::local("workspace.environment_variables.replace");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_environment_id = environment_id.clone();
        let count = variables.len();
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "workspace.environment_variables.replace",
                target: Some(activity_environment_id),
                details: serde_json::json!({ "variableCount": count }),
            }),
            move |connection| {
                Box::pin(async move {
                    service
                        .replace_environment_variables_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            environment_id,
                            variables,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_environment_variable_delete(
        &self,
        workspace_id: String,
        environment_id: String,
        variable_id: String,
    ) -> AppResult<Vec<WorkspaceEnvironmentVariable>> {
        let context = CommandContext::local("workspace.environment_variable.delete");
        let executor_context = context.clone();
        let service = self.workspace.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_variable_id = variable_id.clone();
        self.execute_domain_command(
            context,
            Some(entity_activity(
                &activity_workspace_id,
                "workspace.environment_variable.delete",
                Some(activity_variable_id),
            )),
            move |connection| {
                Box::pin(async move {
                    service
                        .delete_environment_variable_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            environment_id,
                            variable_id,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn workspace_variables_resolve(
        &self,
        workspace_id: String,
        active_environment_id: Option<String>,
        input: String,
    ) -> AppResult<String> {
        self.workspace
            .resolve_variables(&workspace_id, active_environment_id.as_deref(), &input)
            .await
    }

    #[cfg(test)]
    pub(crate) async fn resolve_api_request_input(
        &self,
        input: ApiRequestInput,
    ) -> AppResult<ApiRequestInput> {
        self.resolve_api_request_input_in_environment(input, None)
            .await
    }

    #[cfg(test)]
    pub(crate) async fn resolve_api_request_input_in_environment(
        &self,
        input: ApiRequestInput,
        environment_id_override: Option<&str>,
    ) -> AppResult<ApiRequestInput> {
        let environment_id = self
            .resolve_api_environment_id(&input.workspace_id, environment_id_override)
            .await?;
        self.resolve_api_request_input_for_environment(input, environment_id.as_deref())
            .await
            .map(|(input, _)| input)
    }

    pub(crate) async fn resolve_api_environment_id(
        &self,
        workspace_id: &str,
        environment_id_override: Option<&str>,
    ) -> AppResult<Option<String>> {
        match environment_id_override {
            Some(environment_id) => {
                let environment_id = environment_id.trim();
                if environment_id.is_empty() {
                    return Err(unfour_core::AppError::Validation(
                        "environment id cannot be empty".to_string(),
                    ));
                }
                self.workspace
                    .resolve_variables(workspace_id, Some(environment_id), "")
                    .await?;
                Ok(Some(environment_id.to_string()))
            }
            None => self.workspace.active_environment_id(workspace_id).await,
        }
    }

    pub(crate) async fn resolve_api_request_input_for_environment(
        &self,
        mut input: ApiRequestInput,
        environment_id: Option<&str>,
    ) -> AppResult<(ApiRequestInput, Vec<String>)> {
        let resolver = self
            .workspace
            .variable_resolver(
                &input.workspace_id,
                environment_id,
                &input.temporary_variables,
            )
            .await?;
        input.url = resolver.resolve(&input.url)?;
        input.auth_json = input
            .auth_json
            .map(|auth| resolver.resolve_json(&auth))
            .transpose()?;
        if input.body_kind == unfour_core::models::MULTIPART_BODY_KIND {
            use unfour_core::models::{parse_multipart_definition, ApiMultipartPart};
            let mut parts = parse_multipart_definition(input.body.as_deref())?;
            for part in parts.iter_mut().filter(|part| part.enabled()) {
                let (key, value) = match part {
                    ApiMultipartPart::Text { key, value, .. } => (key, Some(value)),
                    ApiMultipartPart::File { key, .. } => (key, None),
                };
                *key = resolver.resolve(key)?;
                if let Some(value) = value {
                    *value = resolver.resolve(value)?;
                }
            }
            input.body = Some(serde_json::to_string(&parts)?);
        } else {
            input.body = input.body.map(|body| resolver.resolve(&body)).transpose()?;
        }
        for rows in [&mut input.headers, &mut input.query] {
            for row in rows {
                row.key = resolver.resolve(&row.key)?;
                row.value = resolver.resolve(&row.value)?;
            }
        }
        Ok((
            self.api_client.materialize_auth(input)?,
            resolver.secret_values().to_vec(),
        ))
    }
}

fn entity_activity(
    workspace_id: &str,
    action: &'static str,
    target: Option<String>,
) -> CommandActivity {
    CommandActivity {
        workspace_id: Some(workspace_id.to_string()),
        action,
        target,
        details: serde_json::json!({}),
    }
}

pub(crate) fn legacy_api_environment(environment: WorkspaceEnvironment) -> ApiEnvironment {
    ApiEnvironment {
        id: environment.id,
        workspace_id: environment.workspace_id,
        name: environment.name,
        variables: environment
            .variables
            .into_iter()
            .map(|variable| KeyValue {
                key: variable.key,
                value: variable.value,
                enabled: variable.is_enabled,
            })
            .collect(),
        is_active: environment.is_active,
        created_at: environment.created_at,
        updated_at: environment.updated_at,
    }
}
