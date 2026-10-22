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
        let snapshot = self
            .workspace
            .variable_snapshot(&input.workspace_id, context.environment_id.as_deref())
            .await
            .map_err(|error| match error {
                unfour_core::AppError::NotFound(ref name) if name == "workspace environment" => {
                    invalid("SSH_TASK_ENVIRONMENT_MISSING")
                }
                other => other,
            })?;
        let variables = &snapshot.variables;
        let environment = snapshot.environment.as_ref();
        // Older contexts can still be decoded, but a referenced value without an
        // expected version must be reloaded, never silently upgraded to today's value.
        if context.input_versions.is_none()
            && context.environment_updated_at.as_deref()
                != environment.map(|env| env.updated_at.as_str())
        {
            return Err(invalid("SSH_TASK_ENVIRONMENT_CHANGED"));
        }
        for (name, source) in &context.input_sources {
            let key = name.trim().to_lowercase();
            let version = match source {
                SshTaskInputSource::Manual => continue,
                SshTaskInputSource::Workspace => variables
                    .iter()
                    .find(|v| v.is_enabled && v.key.trim().to_lowercase() == key)
                    .map(|v| (&v.id, v.revision)),
                SshTaskInputSource::Environment => environment.and_then(|env| {
                    env.variables
                        .iter()
                        .find(|v| v.is_enabled && v.key.trim().to_lowercase() == key)
                        .map(|v| (&v.id, v.revision))
                }),
            };
            let expected = context
                .input_versions
                .as_ref()
                .and_then(|versions| versions.get(name));
            if !version
                .zip(expected)
                .is_some_and(|((id, revision), expected)| {
                    *id == expected.id && revision == expected.revision
                })
            {
                return Err(invalid("SSH_TASK_INPUTS_CHANGED"));
            }
        }
        let workspace_resolver = snapshot.resolver(false);
        let resolver = snapshot.resolver(true);
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

// Defense in depth against accidental fallback/lookup mismatches within the snapshot.
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
    fn ssh_context_rejects_resolver_lookup_fallbacks() {
        assert!(checked_source_value("stale-secret".into(), "rotated-secret").is_err());
        assert!(checked_source_value("workspace-fallback".into(), "environment").is_err());
        assert_eq!(
            checked_source_value("current".into(), "current").unwrap(),
            "current"
        );
    }
}
