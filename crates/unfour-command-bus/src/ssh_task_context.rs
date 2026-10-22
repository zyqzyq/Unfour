use super::*;
use unfour_core::models::SshTaskInputSource;
use unfour_core::redaction::is_sensitive_workspace_variable;

impl CommandBus {
    /// Resolve references at execution time. Never trust client values or secret flags
    /// for workspace/environment inputs, and never persist their resolved values.
    pub(super) async fn prepare_ssh_task_context(
        &self,
        input: &mut SshTaskRunInput,
    ) -> AppResult<()> {
        let Some(context) = &mut input.context else {
            return Ok(());
        };
        let invalid = |code: &str| unfour_core::AppError::Validation(code.into());
        if input
            .connection_id
            .as_ref()
            .is_none_or(|id| id.trim().is_empty())
        {
            return Err(invalid("SSH_TASK_CONNECTION_REQUIRED"));
        }
        let workspace_resolver = self
            .workspace
            .variable_resolver(&input.workspace_id, None, &[])
            .await?;
        let resolver = self
            .workspace
            .variable_resolver(&input.workspace_id, context.environment_id.as_deref(), &[])
            .await;
        let variables = self
            .workspace
            .list_variables(input.workspace_id.clone())
            .await?;
        let environments = self
            .workspace
            .list_environments(input.workspace_id.clone())
            .await?;
        let environment = match &context.environment_id {
            Some(id) => Some(
                environments
                    .iter()
                    .find(|env| &env.id == id && env.deleted_at.is_none())
                    .ok_or_else(|| invalid("SSH_TASK_ENVIRONMENT_MISSING"))?,
            ),
            None => None,
        };
        if context.environment_updated_at.as_deref()
            != environment.map(|env| env.updated_at.as_str())
        {
            return Err(invalid("SSH_TASK_ENVIRONMENT_CHANGED"));
        }
        let resolver = resolver?;
        for (name, source) in &context.input_sources {
            let key = name.trim().to_lowercase();
            let workspace_match = variables.iter().find(|v| {
                v.is_enabled && v.deleted_at.is_none() && v.key.trim().to_lowercase() == key
            });
            let environment_match = environment.and_then(|env| {
                env.variables.iter().find(|v| {
                    v.is_enabled && v.deleted_at.is_none() && v.key.trim().to_lowercase() == key
                })
            });
            let secret = workspace_match
                .is_some_and(|v| is_sensitive_workspace_variable(&v.key, &v.value, v.is_secret))
                || environment_match.is_some_and(|v| {
                    is_sensitive_workspace_variable(&v.key, &v.value, v.is_secret)
                });
            match source {
                SshTaskInputSource::Manual => {}
                SshTaskInputSource::Workspace => {
                    let variable =
                        workspace_match.ok_or_else(|| invalid("SSH_TASK_INPUT_SOURCE_MISSING"))?;
                    input.inputs.insert(
                        name.clone(),
                        checked_source_value(
                            workspace_resolver.resolve(&format!("{{{{{}}}}}", variable.key))?,
                            &variable.value,
                        )?,
                    );
                }
                SshTaskInputSource::Environment => {
                    let variable = environment_match
                        .ok_or_else(|| invalid("SSH_TASK_INPUT_SOURCE_MISSING"))?;
                    input.inputs.insert(
                        name.clone(),
                        checked_source_value(
                            resolver.resolve(&format!("{{{{{}}}}}", variable.key))?,
                            &variable.value,
                        )?,
                    );
                }
            }
            if secret && !input.secret_input_names.contains(name) {
                input.secret_input_names.push(name.clone());
            }
        }
        // Also prevent aliases or composites of known secrets being recalled as manual text.
        for (name, value) in &input.inputs {
            if workspace_resolver
                .secret_values()
                .iter()
                .chain(resolver.secret_values())
                .any(|secret| !secret.is_empty() && value.contains(secret))
                && !input.secret_input_names.contains(name)
            {
                input.secret_input_names.push(name.clone());
            }
        }
        Ok(())
    }
}

// Resolver and metadata are separate reads. Refuse a changed source instead of
// executing a stale value or a workspace fallback after an environment edit.
fn checked_source_value(resolved: String, current: &str) -> AppResult<String> {
    if resolved != current {
        return Err(unfour_core::AppError::Validation(
            "SSH_TASK_INPUT_SOURCE_MISSING".into(),
        ));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_context_rejects_values_changed_between_resolver_and_metadata_reads() {
        assert!(checked_source_value("stale-secret".into(), "rotated-secret").is_err());
        assert!(checked_source_value("workspace-fallback".into(), "environment").is_err());
        assert_eq!(
            checked_source_value("current".into(), "current").unwrap(),
            "current"
        );
    }
}
